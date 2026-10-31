use redixel::prelude::*;

/// Embedded rather than read from disk: `include_bytes!` resolves at compile
/// time, so the same code ships to desktop, web, Android and iOS.
const CRATE_PNG: &[u8] = include_bytes!("../assets/crate.png");

/// A 16×16 heart on a fully transparent background.
const HEART_PNG: &[u8] = include_bytes!("../assets/heart.png");

/// A whole multiple of the source's 16 texels, so nearest-neighbour
/// magnification lands on exact texel boundaries and the art stays square.
const HEART_SIZE: f32 = 64.0;

const HEART_GAP: f32 = 16.0;
const HEART_MARGIN: f32 = 28.0;
const HEART_COUNT: usize = 4;
const PANEL_PADDING: f32 = 16.0;

/// Distance from the camera to the centre of the cube, along `+Z`.
const CAMERA_DISTANCE: f32 = 4.0;

/// Half the cube's edge, sized against the 60° vertical field of view so the
/// whole cube stays framed as it spins, corners included.
const CUBE_HALF: f32 = 0.85;

const CUBE_CORNERS: [(f32, f32, f32); 8] = [
    (-1.0, -1.0, -1.0),
    (1.0, -1.0, -1.0),
    (1.0, 1.0, -1.0),
    (-1.0, 1.0, -1.0),
    (-1.0, -1.0, 1.0),
    (1.0, -1.0, 1.0),
    (1.0, 1.0, 1.0),
    (-1.0, 1.0, 1.0),
];

/// The cube's six faces as corner indices in `[top-left, top-right,
/// bottom-left, bottom-right]` order, so every face maps the whole texture the
/// same way up.
const CUBE_FACES: [[usize; 4]; 6] = [
    [3, 2, 0, 1],
    [6, 7, 5, 4],
    [7, 3, 4, 0],
    [2, 6, 1, 5],
    [7, 6, 3, 2],
    [0, 1, 4, 5],
];

/// Texture coordinates for a face's four corners, matching [`CUBE_FACES`]
/// order. `v = 0` is the top of the image.
const FACE_UVS: [Vec2; 4] = [
    Vec2::new(0.0, 0.0),
    Vec2::new(1.0, 0.0),
    Vec2::new(0.0, 1.0),
    Vec2::new(1.0, 1.0),
];

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Action {
    Exit,
}

#[derive(Default)]
struct Sprite {
    rotation: f32,
    crate_tex: Option<TextureId>,
    heart_tex: Option<TextureId>,
}

impl Sprite {
    /// Spins `corner` around the vertical axis and pushes it out in front of
    /// the camera.
    ///
    /// Yaw only, no tilt: the cube stays level with the camera, so it is seen
    /// head-on and the spin alone is what reveals it as a solid rather than a
    /// flat quad.
    fn place(&self, corner: (f32, f32, f32)) -> Vec3 {
        let (x, y, z): (f32, f32, f32) = (corner.0 * CUBE_HALF, corner.1 * CUBE_HALF, corner.2 * CUBE_HALF);

        let yawed_x: f32 = x * self.rotation.cos() + z * self.rotation.sin();
        let yawed_z: f32 = z * self.rotation.cos() - x * self.rotation.sin();

        Vec3::new(yawed_x, y, yawed_z + CAMERA_DISTANCE)
    }

    /// Draws the textured cube. Nothing is culled, so both triangles of a face
    /// are submitted in the order the face table lists its corners; the depth
    /// buffer decides what is actually visible.
    fn draw_cube(&self, ctx: &mut dyn GameContext<Action>, texture: TextureId) {
        let corners: [Vec3; 8] = std::array::from_fn(|i: usize| self.place(CUBE_CORNERS[i]));

        for face in CUBE_FACES.iter() {
            let [tl, tr, bl, br]: [usize; 4] = *face;

            ctx.draw_triangle_3d_textured(
                [corners[tl], corners[tr], corners[bl]],
                [FACE_UVS[0], FACE_UVS[1], FACE_UVS[2]],
                texture,
            );

            ctx.draw_triangle_3d_textured(
                [corners[tr], corners[br], corners[bl]],
                [FACE_UVS[1], FACE_UVS[3], FACE_UVS[2]],
                texture,
            );
        }
    }

    /// Draws a row of 2D hearts over an opaque panel: the image untouched,
    /// colour-modulated, then faded to 60% and 25% through the tint's alpha.
    ///
    /// Tint is a multiply, so from a red source every colour lands in the same
    /// family of darker reds — alpha is the axis that separates the row.
    ///
    /// Each shows the panel through its transparent border rather than a red
    /// box, which is the alpha channel being honoured. 2D neither tests nor
    /// writes depth, so painter order alone puts these over the cube.
    fn draw_hearts(&self, ctx: &mut dyn GameContext<Action>, texture: TextureId) {
        let panel_width: f32 =
            HEART_COUNT as f32 * HEART_SIZE + (HEART_COUNT - 1) as f32 * HEART_GAP + 2.0 * PANEL_PADDING;

        ctx.draw_rect(
            Vec2::new(HEART_MARGIN - PANEL_PADDING, HEART_MARGIN - PANEL_PADDING),
            Vec2::new(panel_width, HEART_SIZE + 2.0 * PANEL_PADDING),
            Color::rgb(0.16, 0.17, 0.23),
        );

        for slot in 0..HEART_COUNT {
            let position: Vec2 = Vec2::new(HEART_MARGIN + slot as f32 * (HEART_SIZE + HEART_GAP), HEART_MARGIN);
            let size: Vec2 = Vec2::splat(HEART_SIZE);

            match slot {
                1 => ctx.draw_sprite_tinted(position, size, texture, Color::rgb(0.45, 0.75, 1.0)),
                2 => ctx.draw_sprite_tinted(position, size, texture, Color::WHITE.with_alpha(0.6)),
                3 => ctx.draw_sprite_tinted(position, size, texture, Color::WHITE.with_alpha(0.25)),
                _ => ctx.draw_sprite(position, size, texture),
            }
        }
    }
}

impl Game for Sprite {
    type Action = Action;

    fn on_start(&mut self, ctx: &mut dyn GameContext<Self::Action>) {
        ctx.input_mut().bind(Action::Exit, KeyCode::Escape.into());
        log::info!("sprite::on_start");

        self.crate_tex = Some(ctx.load_texture(CRATE_PNG));
        self.heart_tex = Some(ctx.load_texture(HEART_PNG));
    }

    fn on_update(&mut self, ctx: &mut dyn GameContext<Self::Action>) {
        if ctx.input().just_pressed(Action::Exit) {
            ctx.exit();
        }

        self.rotation += (ctx.delta_time() as f32) * 0.8;
        self.rotation %= std::f32::consts::TAU;
    }

    fn on_render(&mut self, ctx: &mut dyn GameContext<Self::Action>) {
        ctx.clear_color(Color::rgb(0.06, 0.07, 0.10));

        if let Some(texture) = self.crate_tex {
            self.draw_cube(ctx, texture);
        }

        if let Some(texture) = self.heart_tex {
            self.draw_hearts(ctx, texture);
        }
    }
}

redixel::entry_point!(Sprite::default());

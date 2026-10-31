use redixel::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Action {
    Exit,
}

const CAMERA_DISTANCE: f32 = 3.0;

struct Triangle3d {
    rotation: f32,
}

impl Default for Triangle3d {
    fn default() -> Self {
        Self { rotation: 0.0 }
    }
}

impl Triangle3d {
    fn rotate(x: f32, y: f32, z: f32, angle: f32) -> Vec3 {
        let rx: f32 = x * angle.cos() - z * angle.sin();
        let rz: f32 = x * angle.sin() + z * angle.cos();

        let tilt: f32 = 0.4;
        let ry: f32 = y * tilt.cos() - rz * tilt.sin();
        let rz_final: f32 = y * tilt.sin() + rz * tilt.cos();

        Vec3::new(rx, -ry, rz_final)
    }
}

impl Game for Triangle3d {
    type Action = Action;

    fn on_start(&mut self, ctx: &mut dyn GameContext<Self::Action>) {
        ctx.input_mut().bind(Action::Exit, KeyCode::Escape.into());
        log::info!("triangle_3d::on_start");
    }

    fn on_update(&mut self, ctx: &mut dyn GameContext<Self::Action>) {
        if ctx.input().just_pressed(Action::Exit) {
            ctx.exit();
        }

        self.rotation += (ctx.delta_time() as f32) * 1.5;
        self.rotation %= std::f32::consts::TAU;
    }

    fn on_render(&mut self, ctx: &mut dyn GameContext<Self::Action>) {
        ctx.clear_color(Color::rgb(0.1, 0.1, 0.12));

        let vertices: [(f32, f32, f32); 4] = [(0.0, -0.8, 0.0), (-0.8, 0.6, 0.5), (0.8, 0.6, 0.5), (0.0, 0.6, -0.8)];

        let faces: [(usize, usize, usize, Color); 4] = [
            (0, 1, 2, Color::rgb(0.8, 0.2, 0.2)),
            (0, 2, 3, Color::rgb(0.2, 0.8, 0.2)),
            (0, 3, 1, Color::rgb(0.2, 0.2, 0.8)),
            (1, 3, 2, Color::rgb(0.8, 0.8, 0.1)),
        ];

        let rotated: Vec<Vec3> = vertices
            .iter()
            .map(|v: &(f32, f32, f32)| {
                Self::rotate(v.0, v.1, v.2, self.rotation) + Vec3::new(0.0, 0.0, CAMERA_DISTANCE)
            })
            .collect();

        for (i1, i2, i3, color) in faces.iter() {
            ctx.draw_triangle_3d(rotated[*i1], rotated[*i2], rotated[*i3], *color);
        }
    }
}

redixel::entry_point!(Triangle3d::default());

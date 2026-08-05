use std::f32::consts::{FRAC_PI_2, TAU};

use redixel::prelude::*;

/// View-space depth of the sun. The camera is fixed at the origin looking down
/// `+Z`, so every body in the scene is placed relative to this point.
const SUN_Z: f32 = 8.0;

const SUN_RADIUS: f32 = 1.1;

const STAR_COUNT: usize = 160;

const NEBULA_CLUSTERS: usize = 3;

const NEBULA_BLOBS_PER_CLUSTER: usize = 14;

const ASTEROID_COUNT: usize = 110;

/// A body orbiting the sun. Its position is recomputed on the CPU every frame
/// from the engine's elapsed time; only the glow shimmer runs on the GPU.
struct Planet {
    orbit_radius: f32,
    speed: f32,
    phase: f32,
    tilt: f32,
    size: f32,
    color: Color,
    has_moon: bool,
    has_ring: bool,
}

const PLANETS: [Planet; 3] = [
    Planet {
        orbit_radius: 2.2,
        speed: 0.9,
        phase: 0.0,
        tilt: 0.35,
        size: 0.28,
        color: Color::rgb(0.25, 0.5, 1.0),
        has_moon: false,
        has_ring: false,
    },
    Planet {
        orbit_radius: 3.1,
        speed: 0.55,
        phase: 2.4,
        tilt: 0.5,
        size: 0.32,
        color: Color::rgb(0.9, 0.42, 0.24),
        has_moon: true,
        has_ring: false,
    },
    Planet {
        orbit_radius: 4.5,
        speed: 0.34,
        phase: 4.4,
        tilt: 0.7,
        size: 0.4,
        color: Color::rgb(0.88, 0.74, 0.52),
        has_moon: false,
        has_ring: true,
    },
];

/// A background star: a crisp core with a soft halo, the brightest few also
/// flaring a four-point sparkle.
struct Star {
    position: Vec3,
    size: f32,
    color: Color,
    sparkles: bool,
}

/// One soft puff of a nebula cloud, faked as a radial alpha gradient.
struct NebulaBlob {
    center: Vec3,
    radius: f32,
    color: Color,
    core_alpha: f32,
}

/// A belt rock: a squashed low-poly sphere on its own slightly scattered orbit.
struct Asteroid {
    orbit_radius: f32,
    phase: f32,
    speed: f32,
    height: f32,
    scale: Vec3,
    spin_speed: f32,
    color: Color,
}

/// Deterministic pseudo-random value in `[0, 1)`, so the scene is the same on
/// every run without a random-number dependency.
fn hash01(n: f32) -> f32 {
    let x: f32 = (n * 12.9898).sin() * 43758.547;
    x - x.floor()
}

fn scale_color(color: Color, factor: f32) -> Color {
    Color::rgba(color.r * factor, color.g * factor, color.b * factor, color.a)
}

fn with_alpha(color: Color, alpha: f32) -> Color {
    Color::rgba(color.r, color.g, color.b, alpha)
}

fn rotate_y(v: Vec3, angle: f32) -> Vec3 {
    let (sin, cos): (f32, f32) = angle.sin_cos();
    Vec3::new(v.x * cos - v.z * sin, v.y, v.x * sin + v.z * cos)
}

/// Unit-sphere triangles: an icosahedron subdivided `subdivisions` times, each
/// new vertex pushed back onto the sphere. Every vertex doubles as its own
/// exact surface normal, which is what per-vertex lighting needs.
fn icosphere(subdivisions: u32) -> Vec<[Vec3; 3]> {
    let phi: f32 = (1.0 + 5.0_f32.sqrt()) / 2.0;

    let corners: [Vec3; 12] = [
        Vec3::new(-1.0, phi, 0.0),
        Vec3::new(1.0, phi, 0.0),
        Vec3::new(-1.0, -phi, 0.0),
        Vec3::new(1.0, -phi, 0.0),
        Vec3::new(0.0, -1.0, phi),
        Vec3::new(0.0, 1.0, phi),
        Vec3::new(0.0, -1.0, -phi),
        Vec3::new(0.0, 1.0, -phi),
        Vec3::new(phi, 0.0, -1.0),
        Vec3::new(phi, 0.0, 1.0),
        Vec3::new(-phi, 0.0, -1.0),
        Vec3::new(-phi, 0.0, 1.0),
    ];

    const FACES: [[usize; 3]; 20] = [
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];

    let mut triangles: Vec<[Vec3; 3]> = FACES
        .iter()
        .map(|f: &[usize; 3]| {
            [
                corners[f[0]].normalise(),
                corners[f[1]].normalise(),
                corners[f[2]].normalise(),
            ]
        })
        .collect();

    for _ in 0..subdivisions {
        let mut next: Vec<[Vec3; 3]> = Vec::with_capacity(triangles.len() * 4);
        for tri in &triangles {
            let [a, b, c]: [Vec3; 3] = *tri;
            let ab: Vec3 = ((a + b) * 0.5).normalise();
            let bc: Vec3 = ((b + c) * 0.5).normalise();
            let ca: Vec3 = ((c + a) * 0.5).normalise();
            next.push([a, ab, ca]);
            next.push([b, bc, ab]);
            next.push([c, ca, bc]);
            next.push([ab, bc, ca]);
        }
        triangles = next;
    }

    triangles
}

fn make_star(i: usize) -> Star {
    let n: f32 = i as f32;
    let z: f32 = 14.0 + hash01(n + 0.83) * 10.0;
    let position: Vec3 = Vec3::new((hash01(n + 0.13) - 0.5) * 30.0, (hash01(n + 0.47) - 0.5) * 18.0, z);

    let temperature: f32 = hash01(n + 0.29);
    let base: Color = if temperature < 0.33 {
        Color::rgb(0.7, 0.78, 1.0)
    } else if temperature < 0.72 {
        Color::rgb(0.92, 0.92, 0.97)
    } else {
        Color::rgb(1.0, 0.88, 0.7)
    };

    let brilliance: f32 = hash01(n + 0.91).powi(3);
    let brightness: f32 = 0.45 + 0.55 * hash01(n + 0.61);
    let size: f32 = (0.022 + 0.06 * brilliance) * (z / SUN_Z);

    Star {
        position,
        size,
        color: scale_color(base, brightness),
        sparkles: brilliance > 0.5,
    }
}

fn make_nebula_blobs() -> Vec<NebulaBlob> {
    let palette: [Color; NEBULA_CLUSTERS] = [
        Color::rgb(0.5, 0.22, 0.85),
        Color::rgb(0.14, 0.5, 0.8),
        Color::rgb(0.8, 0.28, 0.55),
    ];

    let mut blobs: Vec<NebulaBlob> = Vec::with_capacity(NEBULA_CLUSTERS * NEBULA_BLOBS_PER_CLUSTER);

    for (cluster, base) in palette.iter().enumerate() {
        let seed: f32 = cluster as f32 * 91.7 + 500.0;
        let center: Vec3 = Vec3::new(
            (hash01(seed + 0.37) - 0.5) * 42.0,
            (hash01(seed + 0.53) - 0.5) * 24.0,
            30.0 + hash01(seed + 0.19) * 10.0,
        );

        for b in 0..NEBULA_BLOBS_PER_CLUSTER {
            let m: f32 = seed + b as f32 * 7.77;
            let offset: Vec3 = Vec3::new(
                (hash01(m + 0.11) + hash01(m + 0.41) - 1.0) * 7.0,
                (hash01(m + 0.31) + hash01(m + 0.71) - 1.0) * 4.0,
                (hash01(m + 0.61) - 0.5) * 4.0,
            );
            let warmth: f32 = hash01(m + 0.23) * 0.35;
            blobs.push(NebulaBlob {
                center: center + offset,
                radius: 1.8 + hash01(m + 0.51) * 3.2,
                color: base.lerp(Color::WHITE, warmth),
                core_alpha: 0.03 + hash01(m + 0.87) * 0.05,
            });
        }
    }

    blobs.sort_by(|a: &NebulaBlob, b: &NebulaBlob| b.center.z.total_cmp(&a.center.z));
    blobs
}

fn make_asteroid(i: usize) -> Asteroid {
    let n: f32 = i as f32 + 900.0;
    let grit: f32 = 0.4 + hash01(n + 0.55) * 0.35;
    Asteroid {
        orbit_radius: 3.65 + (hash01(n + 0.17) - 0.5) * 0.5,
        phase: hash01(n + 0.31) * TAU,
        speed: 0.4 + hash01(n + 0.43) * 0.12,
        height: (hash01(n + 0.67) - 0.5) * 0.3,
        scale: Vec3::new(
            0.6 + hash01(n + 0.71) * 0.8,
            0.6 + hash01(n + 0.79) * 0.8,
            0.6 + hash01(n + 0.83) * 0.8,
        ) * (0.028 + hash01(n + 0.91).powi(2) * 0.05),
        spin_speed: 1.0 + hash01(n + 0.23) * 3.0,
        color: Color::rgb(grit, grit * 0.92, grit * 0.8),
    }
}

/// Draws a camera-facing diamond, the four points offset in the view plane.
fn draw_diamond(ctx: &mut dyn GameContext<()>, center: Vec3, size: f32, color: Color) {
    let up: Vec3 = center + Vec3::new(0.0, size, 0.0);
    let down: Vec3 = center + Vec3::new(0.0, -size, 0.0);
    let left: Vec3 = center + Vec3::new(-size, 0.0, 0.0);
    let right: Vec3 = center + Vec3::new(size, 0.0, 0.0);
    ctx.draw_triangle_3d(up, left, right, color);
    ctx.draw_triangle_3d(down, right, left, color);
}

/// Draws a soft round glow: a camera-facing fan whose rim carries zero alpha,
/// so the rasteriser interpolates a smooth radial falloff.
fn draw_soft_blob(ctx: &mut dyn GameContext<()>, center: Vec3, radius: f32, color: Color, core_alpha: f32) {
    const WEDGES: usize = 14;

    let core: Color = with_alpha(color, core_alpha);
    let rim: Color = with_alpha(color, 0.0);

    for i in 0..WEDGES {
        let a0: f32 = i as f32 / WEDGES as f32 * TAU;
        let a1: f32 = (i + 1) as f32 / WEDGES as f32 * TAU;
        let (s0, c0): (f32, f32) = a0.sin_cos();
        let (s1, c1): (f32, f32) = a1.sin_cos();
        let p0: Vec3 = center + Vec3::new(c0, s0, 0.0) * radius;
        let p1: Vec3 = center + Vec3::new(c1, s1, 0.0) * radius;
        ctx.draw_triangle_3d_shaded([center, p0, p1], [core, rim, rim]);
    }
}

/// Draws a mesh sphere lit per vertex: exact radial normals, Lambert diffuse
/// from the sun, a touch of specular sheen towards the camera, and colours
/// interpolated across every face so no facet edges survive.
fn draw_lit_sphere(
    ctx: &mut dyn GameContext<()>,
    mesh: &[[Vec3; 3]],
    center: Vec3,
    scale: Vec3,
    spin: f32,
    color: Color,
    light_pos: Vec3,
) {
    for tri in mesh {
        let mut points: [Vec3; 3] = [Vec3::ZERO; 3];
        let mut colors: [Color; 3] = [Color::BLACK; 3];

        for (k, unit) in tri.iter().enumerate() {
            let normal: Vec3 = rotate_y(*unit, spin);
            let world: Vec3 = center + Vec3::new(normal.x * scale.x, normal.y * scale.y, normal.z * scale.z);
            let to_light: Vec3 = (light_pos - world).normalise();
            let to_camera: Vec3 = (world * -1.0).normalise();

            let lambert: f32 = normal.dot(to_light).max(0.0);
            let halfway: Vec3 = (to_light + to_camera).normalise();
            let specular: f32 = normal.dot(halfway).max(0.0).powi(24) * 0.4;

            let lit: Color = scale_color(color, 0.1 + 0.9 * lambert);
            colors[k] = Color::rgb(
                (lit.r + specular).min(1.0),
                (lit.g + specular).min(1.0),
                (lit.b + specular).min(1.0),
            );
            points[k] = world;
        }

        ctx.draw_triangle_3d_shaded(points, colors);
    }
}

/// Draws the sun with smooth limb darkening: vertices aimed at the camera burn
/// white-hot and cool towards deep orange at the rim, interpolated per pixel.
fn draw_sun(ctx: &mut dyn GameContext<()>, mesh: &[[Vec3; 3]], center: Vec3, spin: f32) {
    const CORE: Color = Color::rgb(1.0, 0.93, 0.62);
    const LIMB: Color = Color::rgb(1.0, 0.38, 0.03);

    for tri in mesh {
        let mut points: [Vec3; 3] = [Vec3::ZERO; 3];
        let mut colors: [Color; 3] = [Color::BLACK; 3];

        for (k, unit) in tri.iter().enumerate() {
            let normal: Vec3 = rotate_y(*unit, spin);
            let world: Vec3 = center + normal * SUN_RADIUS;
            let facing: f32 = normal.dot((world * -1.0).normalise()).max(0.0);
            points[k] = world;
            colors[k] = LIMB.lerp(CORE, facing.powf(0.65));
        }

        ctx.draw_triangle_3d_shaded(points, colors);
    }
}

/// Draws the sun's glow: layered soft halos fading out with distance. Every
/// vertex sits at a different position, so the glow shader makes the whole
/// thing waver like live fire. Drawn after everything else so the blending
/// composites over the finished scene.
fn draw_corona(ctx: &mut dyn GameContext<()>, center: Vec3) {
    const HOT: Color = Color::rgb(1.0, 0.62, 0.16);
    const WARM: Color = Color::rgb(1.0, 0.45, 0.1);

    draw_soft_blob(ctx, center, 2.1, HOT, 0.5);
    draw_soft_blob(ctx, center, 3.4, WARM, 0.18);
    draw_soft_blob(ctx, center, 5.6, WARM, 0.06);
}

/// A point on a planet's (tilted, circular) orbit around `sun`.
fn orbit_point(sun: Vec3, planet: &Planet, angle: f32, radius: f32) -> Vec3 {
    sun + Vec3::new(radius * angle.cos(), planet.tilt * angle.sin(), radius * angle.sin())
}

/// Draws the faint guide line tracing a planet's orbit, soft across its width:
/// full alpha along the centreline fading to nothing at both edges.
fn draw_orbit_ring(ctx: &mut dyn GameContext<()>, sun: Vec3, planet: &Planet) {
    const SEGMENTS: usize = 96;
    const HALF_WIDTH: f32 = 0.035;
    const LINE: Color = Color::rgb(0.55, 0.65, 0.9);

    let core: Color = with_alpha(LINE, 0.16);
    let edge: Color = with_alpha(LINE, 0.0);

    for i in 0..SEGMENTS {
        let a0: f32 = i as f32 / SEGMENTS as f32 * TAU;
        let a1: f32 = (i + 1) as f32 / SEGMENTS as f32 * TAU;

        let c0: Vec3 = orbit_point(sun, planet, a0, planet.orbit_radius);
        let c1: Vec3 = orbit_point(sun, planet, a1, planet.orbit_radius);
        let i0: Vec3 = orbit_point(sun, planet, a0, planet.orbit_radius - HALF_WIDTH);
        let i1: Vec3 = orbit_point(sun, planet, a1, planet.orbit_radius - HALF_WIDTH);
        let o0: Vec3 = orbit_point(sun, planet, a0, planet.orbit_radius + HALF_WIDTH);
        let o1: Vec3 = orbit_point(sun, planet, a1, planet.orbit_radius + HALF_WIDTH);

        ctx.draw_triangle_3d_shaded([i0, c0, c1], [edge, core, core]);
        ctx.draw_triangle_3d_shaded([i0, c1, i1], [edge, core, edge]);
        ctx.draw_triangle_3d_shaded([c0, o0, o1], [core, edge, edge]);
        ctx.draw_triangle_3d_shaded([c0, o1, c1], [core, edge, core]);
    }
}

/// Draws a double ring around a planet, tilted out of the orbital plane,
/// sunlit, and fading softly at its inner and outer rims.
fn draw_planet_ring(ctx: &mut dyn GameContext<()>, center: Vec3, size: f32, light_pos: Vec3) {
    const SEGMENTS: usize = 56;
    const TILT: f32 = 0.5;
    const BANDS: [(f32, f32, f32, Color); 2] = [
        (1.45, 1.9, 0.85, Color::rgb(0.78, 0.66, 0.46)),
        (2.0, 2.35, 0.6, Color::rgb(0.62, 0.52, 0.38)),
    ];

    let u: Vec3 = Vec3::new(1.0, 0.0, 0.0);
    let v: Vec3 = Vec3::new(0.0, TILT.sin(), TILT.cos());
    let normal: Vec3 = u.cross(v).normalise();

    for (inner, outer, opacity, color) in BANDS {
        let mid: f32 = (inner + outer) * 0.5;
        for i in 0..SEGMENTS {
            let a0: f32 = i as f32 / SEGMENTS as f32 * TAU;
            let a1: f32 = (i + 1) as f32 / SEGMENTS as f32 * TAU;
            let (s0, c0): (f32, f32) = a0.sin_cos();
            let (s1, c1): (f32, f32) = a1.sin_cos();

            let d0: Vec3 = u * c0 + v * s0;
            let d1: Vec3 = u * c1 + v * s1;
            let i0: Vec3 = center + d0 * (inner * size);
            let i1: Vec3 = center + d1 * (inner * size);
            let m0: Vec3 = center + d0 * (mid * size);
            let m1: Vec3 = center + d1 * (mid * size);
            let o0: Vec3 = center + d0 * (outer * size);
            let o1: Vec3 = center + d1 * (outer * size);

            let to_light: Vec3 = (light_pos - m0).normalise();
            let lambert: f32 = normal.dot(to_light).abs();
            let lit: Color = scale_color(color, 0.3 + 0.7 * lambert);
            let core: Color = with_alpha(lit, opacity);
            let edge: Color = with_alpha(lit, 0.0);

            ctx.draw_triangle_3d_shaded([i0, m0, m1], [edge, core, core]);
            ctx.draw_triangle_3d_shaded([i0, m1, i1], [edge, core, edge]);
            ctx.draw_triangle_3d_shaded([m0, o0, o1], [core, edge, edge]);
            ctx.draw_triangle_3d_shaded([m0, o1, m1], [core, edge, core]);
        }
    }
}

struct SolarSystem {
    sphere: Vec<[Vec3; 3]>,
    rock: Vec<[Vec3; 3]>,
    stars: Vec<Star>,
    nebula: Vec<NebulaBlob>,
    asteroids: Vec<Asteroid>,
}

impl Default for SolarSystem {
    fn default() -> Self {
        Self {
            sphere: icosphere(3),
            rock: icosphere(0),
            stars: (0..STAR_COUNT).map(make_star).collect(),
            nebula: make_nebula_blobs(),
            asteroids: (0..ASTEROID_COUNT).map(make_asteroid).collect(),
        }
    }
}

impl Game for SolarSystem {
    type Action = ();

    fn on_start(&mut self, _ctx: &mut dyn GameContext<Self::Action>) {
        log::info!("solar_system::on_start");
    }

    fn on_update(&mut self, _ctx: &mut dyn GameContext<Self::Action>) {}

    /// Draw order matters only for the translucent pieces: opaque geometry
    /// resolves in any order through the depth buffer, but blended geometry
    /// composites against whatever the framebuffer already holds, so the
    /// nebula goes first (behind everything) and the corona and guide rings go
    /// last (over everything).
    fn on_render(&mut self, ctx: &mut dyn GameContext<Self::Action>) {
        let time: f32 = ctx.elapsed_time() as f32;
        let sun_center: Vec3 = Vec3::new(0.0, 0.0, SUN_Z);

        ctx.clear_color(Color::rgb(0.004, 0.005, 0.016));
        ctx.set_glow(0.45);

        for blob in &self.nebula {
            draw_soft_blob(ctx, blob.center, blob.radius, blob.color, blob.core_alpha);
        }

        for star in &self.stars {
            draw_soft_blob(ctx, star.position, star.size * 3.5, star.color, 0.2);
            draw_diamond(ctx, star.position, star.size, star.color);

            if star.sparkles {
                let spike: Color = with_alpha(star.color, 0.14);
                let tip: Color = with_alpha(star.color, 0.0);
                for arm in 0..4 {
                    let angle: f32 = arm as f32 * FRAC_PI_2;
                    let (sin, cos): (f32, f32) = angle.sin_cos();
                    let dir: Vec3 = Vec3::new(cos, sin, 0.0);
                    let side: Vec3 = Vec3::new(-sin, cos, 0.0) * (star.size * 0.35);
                    ctx.draw_triangle_3d_shaded(
                        [
                            star.position + side,
                            star.position - side,
                            star.position + dir * (star.size * 7.0),
                        ],
                        [spike, spike, tip],
                    );
                }
            }
        }

        draw_sun(ctx, &self.sphere, sun_center, time * 0.25);

        for asteroid in &self.asteroids {
            let angle: f32 = time * asteroid.speed + asteroid.phase;
            let center: Vec3 = sun_center
                + Vec3::new(
                    asteroid.orbit_radius * angle.cos(),
                    asteroid.height + 0.45 * angle.sin() * 0.5,
                    asteroid.orbit_radius * angle.sin(),
                );
            draw_lit_sphere(
                ctx,
                &self.rock,
                center,
                asteroid.scale,
                time * asteroid.spin_speed,
                asteroid.color,
                sun_center,
            );
        }

        for planet in &PLANETS {
            let angle: f32 = time * planet.speed + planet.phase;
            let center: Vec3 = orbit_point(sun_center, planet, angle, planet.orbit_radius);
            draw_lit_sphere(
                ctx,
                &self.sphere,
                center,
                Vec3::splat(planet.size),
                time * planet.speed * 3.0,
                planet.color,
                sun_center,
            );

            if planet.has_moon {
                let moon_angle: f32 = time * 3.1;
                let moon_center: Vec3 =
                    center + Vec3::new(0.65 * moon_angle.cos(), 0.2 * moon_angle.sin(), 0.65 * moon_angle.sin());
                draw_lit_sphere(
                    ctx,
                    &self.sphere,
                    moon_center,
                    Vec3::splat(0.1),
                    time,
                    Color::rgb(0.72, 0.72, 0.75),
                    sun_center,
                );
            }

            if planet.has_ring {
                draw_planet_ring(ctx, center, planet.size, sun_center);
            }
        }

        for planet in &PLANETS {
            draw_orbit_ring(ctx, sun_center, planet);
        }

        draw_corona(ctx, sun_center);
    }
}

redixel::entry_point!(SolarSystem::default());

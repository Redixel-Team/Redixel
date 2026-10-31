use std::f32::consts::TAU;

use redixel::prelude::{Color, GameContext, InputAction, Vec2};

/// Tunable parameters for a one-shot particle burst.
pub struct ParticleProps {
    pub pos:   Vec2,
    pub color: (u8, u8, u8),
    pub count: usize,
    pub speed: f32,
    pub seed:  f32,
    pub life:  f32,
    pub size:  f32,
}

/// A single short-lived spark drifting outward from its spawn point.
struct Particle {
    pos:      Vec2,
    vel:      Vec2,
    life:     f32,
    max_life: f32,
    r:        u8,
    g:        u8,
    b:        u8,
    size:     f32,
}

/// A fading ghost square left behind by a dashing agent.
struct Afterimage {
    pos:      Vec2,
    size:     f32,
    life:     f32,
    max_life: f32,
    r:        u8,
    g:        u8,
    b:        u8,
}

/// A self-contained cosmetic system — particles, dash after-images, and screen
/// shake — with no knowledge of game logic. The game drives it.
#[derive(Default)]
pub struct Effects {
    particles:   Vec<Particle>,
    afterimages: Vec<Afterimage>,
    shake:       f32,
}

impl Effects {
    /// Creates an empty cosmetic system.
    pub fn new() -> Self {
        Self::default()
    }

    /// Emits `props.count` particles in a deterministic pseudo-random spray.
    pub fn spawn_burst(&mut self, props: ParticleProps) {
        let mut p: usize = 0;
        while p < props.count {
            let pseudo_angle: f32 = (props.seed * 123.45 + p as f32 * 13.0).sin() * TAU;
            let pseudo_speed: f32 = props.speed * (0.5 + (props.seed * 50.0 + p as f32 * 7.0).cos().abs() * 0.5);
            let vel: Vec2 = Vec2::new(pseudo_angle.cos(), pseudo_angle.sin()) * pseudo_speed;
            self.particles.push(Particle {
                pos: props.pos,
                vel,
                life: props.life,
                max_life: props.life,
                r: props.color.0,
                g: props.color.1,
                b: props.color.2,
                size: props.size,
            });
            p += 1;
        }
    }

    /// Adds a fading after-image square at `pos` (a dash trail segment).
    pub fn spawn_afterimage(&mut self, pos: Vec2, size: f32, color: (u8, u8, u8)) {
        self.afterimages.push(Afterimage {
            pos,
            size,
            life: 0.4,
            max_life: 0.4,
            r: color.0,
            g: color.1,
            b: color.2,
        });
    }

    /// Raises the current screen-shake magnitude to at least `amount`.
    pub fn add_shake(&mut self, amount: f32) {
        self.shake = self.shake.max(amount);
    }

    /// Advances particles and after-images and decays screen shake by `dt`.
    pub fn update(&mut self, dt: f32) {
        self.shake = (self.shake - dt * 25.0).max(0.0);

        let mut a_idx: usize = 0;
        while a_idx < self.afterimages.len() {
            self.afterimages[a_idx].life -= dt;
            if self.afterimages[a_idx].life <= 0.0 {
                self.afterimages.remove(a_idx);
            } else {
                a_idx += 1;
            }
        }

        let mut p_idx: usize = 0;
        while p_idx < self.particles.len() {
            let p_vel: Vec2 = self.particles[p_idx].vel;
            self.particles[p_idx].vel *= 1.0 - (dt * 3.0);
            self.particles[p_idx].pos += p_vel * dt;
            self.particles[p_idx].life -= dt;
            self.particles[p_idx].size *= 1.0 - (dt * 2.0);
            if self.particles[p_idx].life <= 0.0 {
                self.particles.remove(p_idx);
            } else {
                p_idx += 1;
            }
        }
    }

    /// The current frame's shake displacement in screen units, oscillating with `time`.
    pub fn shake_offset(&self, time: f32) -> Vec2 {
        if self.shake > 0.0 {
            Vec2::new((time * 60.0).sin() * self.shake, (time * 73.0).cos() * self.shake)
        } else {
            Vec2::ZERO
        }
    }

    /// Draws after-images then particles, offsetting every position by `shake_offset`.
    pub fn draw<A: InputAction>(&self, ctx: &mut dyn GameContext<A>, shake_offset: Vec2) {
        let a_len: usize = self.afterimages.len();
        let mut i: usize = 0;
        while i < a_len {
            let img: &Afterimage = &self.afterimages[i];
            let alpha: f32 = (img.life / img.max_life).clamp(0.0, 1.0);
            let alpha_curve: f32 = alpha * alpha;
            let color: Color = Color::from_rgba8(img.r, img.g, img.b, (alpha_curve * 100.0) as u8);
            ctx.draw_rect(img.pos + shake_offset, Vec2::splat(img.size), color);
            i += 1;
        }

        let p_len: usize = self.particles.len();
        let mut j: usize = 0;
        while j < p_len {
            let p: &Particle = &self.particles[j];
            let alpha: f32 = (p.life / p.max_life).clamp(0.0, 1.0);
            let color: Color = Color::from_rgba8(p.r, p.g, p.b, (alpha * 255.0) as u8);
            ctx.draw_rect(p.pos + shake_offset, Vec2::splat(p.size), color);
            j += 1;
        }
    }
}

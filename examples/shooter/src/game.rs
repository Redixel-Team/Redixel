use redixel::prelude::{Color, Game, GameContext, KeyCode, MouseButton, SoundId, Vec2};

use crate::{
    common::{
        BASE_COOLDOWN, BOT_SPEED, BULLET_SIZE, BULLET_SPEED, ENTITY_SIZE, PLAYER_SPEED, POWERUP_DURATION, POWERUP_SIZE,
        RAPID_FIRE_COOLDOWN, WeaponType, check_collision, get_weapon_color, rotate_vec,
    },
    effects::{Effects, ParticleProps},
};

/// A short descending blip, played on every shot the player fires.
const SHOOT_SFX: &[u8] = include_bytes!("../assets/audio/shoot.wav");

/// A short impact thud, played whenever a bullet lands on an agent.
const HIT_SFX: &[u8] = include_bytes!("../assets/audio/hit.wav");

/// A four-second synthwave loop — pulsing bass, arpeggio and a soft kick over
/// Am-F-C-G — that repeats seamlessly as background music.
const MUSIC: &[u8] = include_bytes!("../assets/audio/music.wav");

/// The player's input action set, bound to keyboard and mouse in `on_start`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Action {
    Up,
    Down,
    Left,
    Right,
    Shoot,
    Dash,
    Exit,
}

/// One in-flight projectile, owned by whichever agent fired it.
struct Bullet {
    pos: Vec2,
    vel: Vec2,
    owner_id: usize,
    weapon: WeaponType,
    life: f32,
    destroyed: bool,
    size: f32,
}

/// The single roaming weapon pickup.
struct PowerUp {
    pos: Vec2,
    active: bool,
    weapon: WeaponType,
}

/// Either the player or a bot — both are simulated identically except for how
/// their aim direction is chosen (see [`Shooter::step_agents`]).
struct Agent {
    id: usize,
    pos: Vec2,
    vel: Vec2,
    health: i32,
    shoot_cooldown: f32,
    rapid_fire_timer: f32,
    dash_cooldown: f32,
    is_player: bool,
    color: Color,
    weapon: WeaponType,
}

impl Agent {
    /// Spawns a fresh agent for `id` at `(x, y)` with the default pistol loadout.
    fn new(id: usize, x: f32, y: f32, is_player: bool) -> Self {
        let agent_color: Color = if is_player {
            Color::from_rgba8(122, 202, 255, 255)
        } else {
            Color::from_rgba8(255, 122, 122, 255)
        };

        Self {
            id,
            pos: Vec2::new(x, y),
            vel: Vec2::ZERO,
            health: 100,
            shoot_cooldown: 0.0,
            rapid_fire_timer: 0.0,
            dash_cooldown: 0.0,
            is_player,
            color: agent_color,
            weapon: WeaponType::Pistol,
        }
    }

    /// Resets the agent to full health and relocates it across the arena.
    fn respawn(&mut self, w: f32, h: f32) {
        self.health = 100;
        self.rapid_fire_timer = 0.0;
        self.dash_cooldown = 0.0;
        self.weapon = WeaponType::Pistol;
        self.vel = Vec2::ZERO;
        self.pos.x = (self.pos.x + 400.0) % (w - ENTITY_SIZE);
        self.pos.y = (self.pos.y + 300.0) % (h - ENTITY_SIZE);
    }

    /// The current shot cooldown, shortened while rapid-fire is active.
    fn current_cooldown_max(&self) -> f32 {
        if self.rapid_fire_timer > 0.0 {
            RAPID_FIRE_COOLDOWN
        } else {
            BASE_COOLDOWN
        }
    }
}

/// The whole game: simulates every agent/bullet and renders them directly —
/// there is no client/server split in single-player.
pub struct Shooter {
    initialized: bool,
    agents: Vec<Agent>,
    bullets: Vec<Bullet>,
    powerup: PowerUp,
    fx: Effects,
    time_since_start: f32,
    shoot_sfx: Option<SoundId>,
    hit_sfx: Option<SoundId>,
}

impl Shooter {
    /// Creates a fresh match: one player plus six bots.
    pub fn new() -> Self {
        let mut agents: Vec<Agent> = Vec::new();
        agents.push(Agent::new(0, 0.0, 0.0, true));

        let mut i: usize = 1;
        while i <= 6 {
            agents.push(Agent::new(i, i as f32 * 200.0, i as f32 * 100.0, false));
            i += 1;
        }

        Self {
            agents,
            initialized: false,
            bullets: Vec::new(),
            fx: Effects::new(),
            time_since_start: 0.0,
            powerup: PowerUp {
                active: true,
                weapon: WeaponType::Shotgun,
                pos: Vec2::new(0.0, 100.0),
            },
            shoot_sfx: None,
            hit_sfx: None,
        }
    }

    /// Spawns the bullets for one shot and returns the recoil impulse magnitude.
    fn spawn_bullets(
        bullets: &mut Vec<Bullet>,
        owner_id: usize,
        weapon: WeaponType,
        center: Vec2,
        dir_vec: Vec2,
        time_since_start: f32,
    ) -> f32 {
        match weapon {
            WeaponType::Pistol => {
                bullets.push(Bullet {
                    pos: center,
                    vel: dir_vec * BULLET_SPEED,
                    owner_id,
                    weapon: WeaponType::Pistol,
                    life: 3.0,
                    destroyed: false,
                    size: BULLET_SIZE,
                });

                100.0
            }
            WeaponType::Shotgun => {
                let angles: [f32; 5] = [-0.3, -0.15, 0.0, 0.15, 0.3];

                for angle in angles {
                    bullets.push(Bullet {
                        pos: center,
                        vel: rotate_vec(dir_vec, angle) * BULLET_SPEED,
                        owner_id,
                        weapon: WeaponType::Shotgun,
                        life: 0.6,
                        destroyed: false,
                        size: BULLET_SIZE,
                    });
                }

                400.0
            }
            WeaponType::Flamethrower => {
                let offsets: [f32; 6] = [-0.4, -0.2, -0.05, 0.05, 0.2, 0.4];
                let speeds: [f32; 6] = [0.85, 1.0, 1.1, 0.95, 1.05, 0.9];
                let sizes: [f32; 6] = [12.0, 18.0, 24.0, 20.0, 16.0, 14.0];

                for i in 0..6 {
                    let wobble: f32 = (time_since_start * 25.0 + (i as f32)).sin() * 0.15;
                    let angle: f32 = offsets[i] + wobble;

                    bullets.push(Bullet {
                        pos: center - Vec2::splat(sizes[i] / 2.0),
                        vel: rotate_vec(dir_vec, angle) * (BULLET_SPEED * speeds[i]),
                        owner_id,
                        weapon: WeaponType::Flamethrower,
                        life: 0.9,
                        destroyed: false,
                        size: sizes[i],
                    });
                }

                25.0
            }
            WeaponType::Homing => {
                bullets.push(Bullet {
                    pos: center,
                    vel: dir_vec * (BULLET_SPEED * 0.6),
                    owner_id,
                    weapon: WeaponType::Homing,
                    life: 4.0,
                    destroyed: false,
                    size: BULLET_SIZE,
                });

                150.0
            }
        }
    }

    /// One-time setup on the first frame: centers the player and the initial
    /// powerup now that the real surface size is known.
    fn ensure_initialized(&mut self, w: f32, h: f32) {
        if self.initialized {
            return;
        }

        let screen_center: Vec2 = Vec2::new(w, h) / 2.0;
        if let Some(player) = self.agents.iter_mut().find(|a: &&mut Agent| a.is_player) {
            player.pos = screen_center - Vec2::splat(ENTITY_SIZE / 2.0);
        }

        self.powerup.pos = Vec2::new((w - POWERUP_SIZE) / 2.0, 100.0);
        self.initialized = true;
    }

    /// Respawns the roaming powerup on a fixed cadence, cycling its weapon.
    fn update_powerup(&mut self, dt: f32, w: f32, h: f32) {
        if self.powerup.active || self.time_since_start % 10.0 >= dt {
            return;
        }

        self.powerup.active = true;
        self.powerup.pos = Vec2::new((self.time_since_start * 100.0) % w, (self.time_since_start * 50.0) % h);

        let weapon_cycle: usize = ((self.time_since_start / 10.0) as usize) % 4;
        self.powerup.weapon = match weapon_cycle {
            0 => WeaponType::Shotgun,
            1 => WeaponType::Flamethrower,
            2 => WeaponType::Homing,
            _ => WeaponType::Pistol,
        };

        self.fx.spawn_burst(ParticleProps {
            pos: self.powerup.pos + Vec2::splat(POWERUP_SIZE / 2.0),
            color: (255, 255, 255),
            count: 20,
            speed: 150.0,
            seed: self.time_since_start,
            life: 0.5,
            size: 4.0,
        });
    }

    /// Applies player input / bot AI, integrates motion, fires weapons, and
    /// resolves wall bounces and powerup pickups for every agent.
    fn step_agents(&mut self, ctx: &mut dyn GameContext<Action>, dt: f32, w: f32, h: f32) {
        let mut player_pos: Vec2 = Vec2::ZERO;
        for agent in self.agents.iter() {
            if agent.is_player {
                player_pos = agent.pos;
            }
        }

        let agents_len: usize = self.agents.len();
        for i in 0..agents_len {
            let agent: &mut Agent = &mut self.agents[i];

            agent.shoot_cooldown -= dt;
            agent.dash_cooldown -= dt;

            if agent.rapid_fire_timer > 0.0 {
                agent.rapid_fire_timer -= dt;
                if agent.rapid_fire_timer <= 0.0 {
                    agent.weapon = WeaponType::Pistol;
                }
            }

            if agent.dash_cooldown > 0.8 && (self.time_since_start * 20.0).fract() < (dt * 20.0) {
                let (r, g, b): (u8, u8, u8) = if agent.rapid_fire_timer > 0.0 {
                    (255, 229, 122)
                } else if agent.is_player {
                    (122, 202, 255)
                } else {
                    (255, 122, 122)
                };

                self.fx.spawn_afterimage(agent.pos, ENTITY_SIZE, (r, g, b));
            }

            if agent.is_player {
                let mut dir: Vec2 = Vec2::ZERO;

                if ctx.input().held(Action::Left) {
                    dir -= Vec2::X;
                }
                if ctx.input().held(Action::Right) {
                    dir += Vec2::X;
                }
                if ctx.input().held(Action::Up) {
                    dir -= Vec2::Y;
                }
                if ctx.input().held(Action::Down) {
                    dir += Vec2::Y;
                }

                if dir.length_sq() > 0.0 {
                    dir = dir.normalise();
                }

                let target_vel: Vec2 = dir * PLAYER_SPEED;
                agent.vel = agent.vel.lerp(target_vel, dt * 15.0);

                if ctx.input().held(Action::Dash) && agent.dash_cooldown <= 0.0 && dir.length_sq() > 0.0 {
                    agent.vel += dir * 1500.0;
                    agent.dash_cooldown = 1.2;
                    self.fx.add_shake(5.0);

                    let particle_color: (u8, u8, u8) = if agent.rapid_fire_timer > 0.0 {
                        (255, 229, 122)
                    } else {
                        (122, 202, 255)
                    };

                    self.fx.spawn_burst(ParticleProps {
                        pos: agent.pos + Vec2::splat(ENTITY_SIZE / 2.0),
                        color: particle_color,
                        count: 15,
                        speed: 250.0,
                        seed: self.time_since_start,
                        life: 0.4,
                        size: 6.0,
                    });
                }

                if ctx.input().held(Action::Shoot)
                    && agent.shoot_cooldown <= 0.0
                    && let Some(mouse_pos) = ctx.input().mouse_position()
                {
                    let center: Vec2 = agent.pos + Vec2::splat(ENTITY_SIZE / 2.0);
                    let dir_raw: Vec2 = mouse_pos - center;
                    let mag: f32 = dir_raw.length();

                    if mag > 0.0 {
                        agent.shoot_cooldown = agent.current_cooldown_max();
                        let dir_vec: Vec2 = dir_raw / mag;

                        let recoil: f32 = Self::spawn_bullets(
                            &mut self.bullets,
                            agent.id,
                            agent.weapon,
                            center,
                            dir_vec,
                            self.time_since_start,
                        );

                        agent.vel -= dir_vec * recoil;
                        self.fx.add_shake(recoil * 0.015);

                        if let Some(sound) = self.shoot_sfx {
                            let jittered_pitch: f32 = 0.92 + (self.time_since_start * 17.0).fract() * 0.24;
                            ctx.play_sound_with(sound, 0.5, jittered_pitch);
                        }
                    }
                }
            } else {
                let center: Vec2 = agent.pos + Vec2::splat(ENTITY_SIZE / 2.0);
                let target_center: Vec2 = player_pos + Vec2::splat(ENTITY_SIZE / 2.0);
                let dir_raw: Vec2 = target_center - center;
                let mag: f32 = dir_raw.length();

                let mut dir_vec: Vec2 = Vec2::ZERO;

                if mag > 0.0 {
                    let dir_norm: Vec2 = dir_raw / mag;
                    let min_dist: f32 = match agent.weapon {
                        WeaponType::Flamethrower => 60.0,
                        WeaponType::Shotgun => 120.0,
                        WeaponType::Homing => 250.0,
                        WeaponType::Pistol => 180.0,
                    };

                    if mag > min_dist {
                        dir_vec = dir_norm;
                    } else if mag < min_dist - 30.0 {
                        dir_vec = -dir_norm;
                    } else {
                        dir_vec = Vec2::new(-dir_norm.y, dir_norm.x);
                    }

                    if agent.shoot_cooldown <= 0.0 {
                        agent.shoot_cooldown = agent.current_cooldown_max();

                        let recoil: f32 = Self::spawn_bullets(
                            &mut self.bullets,
                            agent.id,
                            agent.weapon,
                            center,
                            dir_norm,
                            self.time_since_start,
                        );

                        agent.vel -= dir_norm * recoil;
                    }
                }

                let target_vel: Vec2 = dir_vec * BOT_SPEED;
                agent.vel = agent.vel.lerp(target_vel, dt * 8.0);
            }

            agent.pos += agent.vel * dt;

            if agent.pos.x < 0.0 {
                agent.pos.x = 0.0;
                agent.vel.x *= -0.5;
            }
            if agent.pos.x > w - ENTITY_SIZE {
                agent.pos.x = w - ENTITY_SIZE;
                agent.vel.x *= -0.5;
            }
            if agent.pos.y < 0.0 {
                agent.pos.y = 0.0;
                agent.vel.y *= -0.5;
            }
            if agent.pos.y > h - ENTITY_SIZE {
                agent.pos.y = h - ENTITY_SIZE;
                agent.vel.y *= -0.5;
            }

            if self.powerup.active && check_collision(agent.pos, ENTITY_SIZE, self.powerup.pos, POWERUP_SIZE) {
                agent.weapon = self.powerup.weapon;
                agent.rapid_fire_timer = POWERUP_DURATION;
                self.powerup.active = false;

                let (r, g, b): (u8, u8, u8) = get_weapon_color(agent.weapon);
                self.fx.spawn_burst(ParticleProps {
                    pos: agent.pos + Vec2::splat(ENTITY_SIZE / 2.0),
                    color: (r, g, b),
                    count: 30,
                    speed: 350.0,
                    seed: self.time_since_start,
                    life: 0.6,
                    size: 8.0,
                });
            }
        }
    }

    /// Separates overlapping agents with a soft symmetric push, then re-clamps
    /// them inside the arena bounds.
    fn resolve_agent_overlap(&mut self, w: f32, h: f32) {
        let agents_len: usize = self.agents.len();
        let mut pushes: Vec<Vec2> = vec![Vec2::ZERO; agents_len];

        for i in 0..agents_len {
            for j in (i + 1)..agents_len {
                let p_i: Vec2 = self.agents[i].pos;
                let p_j: Vec2 = self.agents[j].pos;
                let dir_raw: Vec2 = p_i - p_j;
                let dist_sq: f32 = dir_raw.length_sq();
                let min_dist: f32 = ENTITY_SIZE;

                if dist_sq < min_dist * min_dist && dist_sq > 0.001 {
                    let dist: f32 = dist_sq.sqrt();
                    let overlap: f32 = min_dist - dist;
                    let push_dir: Vec2 = dir_raw / dist;
                    let push_vec: Vec2 = push_dir * (overlap * 0.5);

                    pushes[i] += push_vec;
                    pushes[j] -= push_vec;
                }
            }
        }

        for (agent, &push) in self.agents.iter_mut().zip(pushes.iter()) {
            agent.pos += push;
            agent.pos.x = agent.pos.x.clamp(0.0, (w - ENTITY_SIZE).max(0.0));
            agent.pos.y = agent.pos.y.clamp(0.0, (h - ENTITY_SIZE).max(0.0));
        }
    }

    /// Steers homing bullets, advances all bullets, spawns their trail sparks,
    /// and expires the ones that ran out of life or left the arena.
    fn step_bullets(&mut self, dt: f32, w: f32, h: f32) {
        let agents_len: usize = self.agents.len();
        let bullets_len: usize = self.bullets.len();

        for i in 0..bullets_len {
            let b: &mut Bullet = &mut self.bullets[i];

            if b.weapon == WeaponType::Homing && !b.destroyed {
                let mut closest_dist: f32 = f32::MAX;
                let mut target_pos: Vec2 = Vec2::ZERO;
                let mut found: bool = false;

                for j in 0..agents_len {
                    let agent: &Agent = &self.agents[j];

                    if agent.id != b.owner_id {
                        let dist_sq: f32 = (agent.pos - b.pos).length_sq();

                        if dist_sq < closest_dist {
                            closest_dist = dist_sq;
                            target_pos = agent.pos;
                            found = true;
                        }
                    }
                }

                if found && closest_dist < 60000.0 {
                    let target_dir: Vec2 = (target_pos - b.pos).normalise();
                    b.vel = (b.vel + target_dir * 25.0).normalise() * (BULLET_SPEED * 0.6);
                }
            }

            b.pos += b.vel * dt;
            b.life -= dt;

            let (r, g, b_c): (u8, u8, u8) = get_weapon_color(b.weapon);

            if self.time_since_start % 0.05 < dt {
                self.fx.spawn_burst(ParticleProps {
                    pos: b.pos + Vec2::splat(b.size / 2.0),
                    color: (r, g, b_c),
                    count: 1,
                    speed: 20.0,
                    seed: self.time_since_start + b.pos.x,
                    life: 0.2,
                    size: b.size * 0.8,
                });
            }

            if b.life <= 0.0 || b.pos.x < -b.size || b.pos.x > w || b.pos.y < -b.size || b.pos.y > h {
                b.destroyed = true;
            }
        }
    }

    /// Resolves bullet-vs-agent and bullet-vs-bullet collisions, applying
    /// damage, knockback, respawns, and the matching particle bursts.
    fn resolve_collisions(&mut self, ctx: &mut dyn GameContext<Action>, w: f32, h: f32) {
        let bullets_len: usize = self.bullets.len();

        for i in 0..bullets_len {
            if self.bullets[i].destroyed {
                continue;
            }

            let pos_i: Vec2 = self.bullets[i].pos;
            let size_i: f32 = self.bullets[i].size;
            let owner_i: usize = self.bullets[i].owner_id;

            for agent in self.agents.iter_mut() {
                if agent.id != owner_i && check_collision(pos_i, size_i, agent.pos, ENTITY_SIZE) {
                    agent.health -= 25;
                    agent.vel += self.bullets[i].vel.normalise() * 300.0;

                    self.fx.spawn_burst(ParticleProps {
                        pos: agent.pos + Vec2::splat(ENTITY_SIZE / 2.0),
                        color: (255, 122, 122),
                        count: 12,
                        speed: 250.0,
                        seed: self.time_since_start,
                        life: 0.4,
                        size: 6.0,
                    });

                    if let Some(sound) = self.hit_sfx {
                        ctx.play_sound(sound);
                    }

                    if agent.health <= 0 {
                        self.fx.spawn_burst(ParticleProps {
                            pos: agent.pos + Vec2::splat(ENTITY_SIZE / 2.0),
                            color: (255, 96, 96),
                            count: 40,
                            speed: 400.0,
                            seed: self.time_since_start,
                            life: 0.8,
                            size: 10.0,
                        });

                        agent.respawn(w, h);
                    }

                    if agent.is_player {
                        self.fx.add_shake(12.0);
                    }

                    self.bullets[i].destroyed = true;
                    break;
                }
            }
        }

        for i in 0..bullets_len {
            if self.bullets[i].destroyed {
                continue;
            }

            let pos_i: Vec2 = self.bullets[i].pos;
            let size_i: f32 = self.bullets[i].size;
            let owner_i: usize = self.bullets[i].owner_id;

            for j in (i + 1)..bullets_len {
                if self.bullets[j].destroyed {
                    continue;
                }

                if owner_i != self.bullets[j].owner_id {
                    let pos_j: Vec2 = self.bullets[j].pos;
                    let size_j: f32 = self.bullets[j].size;

                    if check_collision(pos_i, size_i, pos_j, size_j) {
                        self.bullets[i].destroyed = true;
                        self.bullets[j].destroyed = true;

                        self.fx.spawn_burst(ParticleProps {
                            pos: pos_i + Vec2::splat(size_i / 2.0),
                            color: (255, 255, 168),
                            count: 6,
                            speed: 150.0,
                            seed: self.time_since_start,
                            life: 0.3,
                            size: 4.0,
                        });

                        break;
                    }
                }
            }
        }

        self.bullets.retain(|b: &Bullet| !b.destroyed);
    }

    /// Draws the parallax background grid, offset by the player's position.
    fn draw_grid(&self, ctx: &mut dyn GameContext<Action>, w: f32, h: f32, shake_offset: Vec2) {
        let mut player_pos: Vec2 = Vec2::ZERO;
        for agent in self.agents.iter() {
            if agent.is_player {
                player_pos = agent.pos;
            }
        }

        let grid_color: Color = Color::rgb(0.13, 0.13, 0.16);
        let offset_x: f32 = (player_pos.x * -0.05) % 60.0;
        let offset_y: f32 = (player_pos.y * -0.05) % 60.0;

        let mut x: f32 = offset_x;
        if x > 0.0 {
            x -= 60.0;
        }
        while x < w {
            ctx.draw_rect(Vec2::new(x, 0.0) + shake_offset, Vec2::new(1.0, h), grid_color);
            x += 60.0;
        }

        let mut y: f32 = offset_y;
        if y > 0.0 {
            y -= 60.0;
        }
        while y < h {
            ctx.draw_rect(Vec2::new(0.0, y) + shake_offset, Vec2::new(w, 1.0), grid_color);
            y += 60.0;
        }
    }

    /// Draws the pulsing powerup pickup, if one is currently active.
    fn draw_powerup(&self, ctx: &mut dyn GameContext<Action>, shake_offset: Vec2) {
        if !self.powerup.active {
            return;
        }

        let pulse: f32 = (self.time_since_start * 5.0).sin().abs();
        let p_size: f32 = POWERUP_SIZE + (pulse * 6.0);
        let p_offset: Vec2 = Vec2::splat((p_size - POWERUP_SIZE) / 2.0);
        let p_draw_pos: Vec2 = self.powerup.pos - p_offset;

        let (r, g, b): (u8, u8, u8) = get_weapon_color(self.powerup.weapon);
        let c: Color = Color::from_rgba8(r, g, b, (150.0 + pulse * 100.0) as u8);

        ctx.draw_rect(
            p_draw_pos + Vec2::splat(4.0) + shake_offset,
            Vec2::splat(p_size),
            Color::from_rgba8(0, 0, 0, 150),
        );
        ctx.draw_rect(p_draw_pos + shake_offset, Vec2::splat(p_size), c);
    }

    /// Draws every agent: colored body, gold rapid-fire tint, core pulse, hp bar.
    fn draw_agents(&self, ctx: &mut dyn GameContext<Action>, shake_offset: Vec2) {
        let agents_len: usize = self.agents.len();
        for i in 0..agents_len {
            let agent: &Agent = &self.agents[i];

            let color: Color = if agent.rapid_fire_timer > 0.0 {
                Color::from_rgba8(255, 229, 122, 255)
            } else {
                agent.color
            };

            ctx.draw_rect(
                agent.pos + Vec2::splat(4.0) + shake_offset,
                Vec2::splat(ENTITY_SIZE),
                Color::from_rgba8(0, 0, 0, 150),
            );
            ctx.draw_rect(agent.pos + shake_offset, Vec2::splat(ENTITY_SIZE), color);

            let inner_size: f32 = ENTITY_SIZE * 0.4;
            let inner_offset: Vec2 = Vec2::splat((ENTITY_SIZE - inner_size) / 2.0);
            let pulse_core: f32 = (self.time_since_start * 3.0 + agent.id as f32).sin().abs() * 0.5 + 0.5;

            ctx.draw_rect(
                agent.pos + inner_offset + shake_offset,
                Vec2::splat(inner_size),
                Color::from_rgba8(255, 255, 255, (pulse_core * 200.0) as u8),
            );

            let hp_percent: f32 = (agent.health.max(0) as f32) / 100.0;
            let hp_pos: Vec2 = agent.pos - (Vec2::Y * 10.0);

            ctx.draw_rect(hp_pos + shake_offset, Vec2::new(ENTITY_SIZE, 4.0), Color::rgb(0.7, 0.1, 0.1));
            ctx.draw_rect(
                hp_pos + shake_offset,
                Vec2::new(ENTITY_SIZE * hp_percent, 4.0),
                Color::rgb(0.1, 0.8, 0.2),
            );
        }
    }

    /// Draws every bullet with its weapon color and a soft glow.
    fn draw_bullets(&self, ctx: &mut dyn GameContext<Action>, shake_offset: Vec2) {
        let bullets_len: usize = self.bullets.len();
        for i in 0..bullets_len {
            let b: &Bullet = &self.bullets[i];
            let (r, g, b_c): (u8, u8, u8) = get_weapon_color(b.weapon);

            let b_color: Color = Color::from_rgba8(r, g, b_c, 255);
            let glow_color: Color = Color::from_rgba8(r, g, b_c, 50);

            ctx.draw_rect(
                b.pos + Vec2::splat(3.0) + shake_offset,
                Vec2::splat(b.size),
                Color::from_rgba8(0, 0, 0, 100),
            );
            ctx.draw_rect(b.pos - Vec2::splat(4.0) + shake_offset, Vec2::splat(b.size + 8.0), glow_color);
            ctx.draw_rect(b.pos + shake_offset, Vec2::splat(b.size), b_color);
        }
    }

    /// Draws the mouse-aim crosshair, if the cursor is inside the window.
    fn draw_crosshair(&self, ctx: &mut dyn GameContext<Action>, shake_offset: Vec2) {
        if let Some(mouse_pos) = ctx.input().mouse_position() {
            let crosshair_pos: Vec2 = mouse_pos + shake_offset * 0.5;
            ctx.draw_rect(crosshair_pos - Vec2::new(2.0, 12.0), Vec2::new(4.0, 24.0), Color::WHITE);
            ctx.draw_rect(crosshair_pos - Vec2::new(12.0, 2.0), Vec2::new(24.0, 4.0), Color::WHITE);
        }
    }
}

impl Default for Shooter {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for Shooter {
    type Action = Action;

    fn on_start(&mut self, ctx: &mut dyn GameContext<Self::Action>) {
        ctx.input_mut().bind(Action::Up, KeyCode::KeyW.into());
        ctx.input_mut().bind(Action::Down, KeyCode::KeyS.into());
        ctx.input_mut().bind(Action::Left, KeyCode::KeyA.into());
        ctx.input_mut().bind(Action::Right, KeyCode::KeyD.into());
        ctx.input_mut().bind(Action::Exit, KeyCode::Escape.into());
        ctx.input_mut().bind(Action::Shoot, MouseButton::Left.into());
        ctx.input_mut().bind(Action::Dash, KeyCode::Space.into());

        self.shoot_sfx = Some(ctx.load_sound(SHOOT_SFX));
        self.hit_sfx = Some(ctx.load_sound(HIT_SFX));

        let music: SoundId = ctx.load_sound(MUSIC);
        ctx.play_music(music);
    }

    fn on_update(&mut self, ctx: &mut dyn GameContext<Self::Action>) {
        let dt: f32 = ctx.delta_time() as f32;
        let w: f32 = ctx.surface_width() as f32;
        let h: f32 = ctx.surface_height() as f32;

        self.ensure_initialized(w, h);
        self.time_since_start += dt;
        self.fx.update(dt);

        if ctx.input().held(Action::Exit) {
            ctx.exit();
        }

        self.update_powerup(dt, w, h);
        self.step_agents(ctx, dt, w, h);
        self.resolve_agent_overlap(w, h);
        self.step_bullets(dt, w, h);
        self.resolve_collisions(ctx, w, h);
    }

    fn on_render(&mut self, ctx: &mut dyn GameContext<Self::Action>) {
        let w: f32 = ctx.surface_width() as f32;
        let h: f32 = ctx.surface_height() as f32;
        let shake_offset: Vec2 = self.fx.shake_offset(self.time_since_start);

        ctx.clear_color(Color::rgb(0.08, 0.08, 0.11));

        self.draw_grid(ctx, w, h, shake_offset);
        self.fx.draw(ctx, shake_offset);
        self.draw_powerup(ctx, shake_offset);
        self.draw_agents(ctx, shake_offset);
        self.draw_bullets(ctx, shake_offset);
        self.draw_crosshair(ctx, shake_offset);
    }
}

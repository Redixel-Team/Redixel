use std::collections::{HashMap, VecDeque};

use redixel::prelude::{
    ClientId, Color, Game, GameContext, KeyCode, MouseButton, NetworkChannel, NetworkEvent, SERVER_ID, Vec2,
};

use crate::{
    effects::{Effects, ParticleProps},
    proto::{
        ARENA_H, ARENA_W, AgentState, BASE_COOLDOWN, ENTITY_SIZE, Effect, EffectBatch, EffectKind, POWERUP_SIZE,
        PlayerInput, RAPID_FIRE_COOLDOWN, Snapshot, V2, move_direction, player_color, recoil_for, send_encoded,
        seq_newer, step_movement, weapon_color,
    },
};

/// How often a dashing agent leaves an after-image behind, in seconds.
const AFTERIMAGE_INTERVAL: f32 = 0.05;

/// How often each bullet emits a trail particle, in seconds.
const TRAIL_INTERVAL: f32 = 0.05;

/// A predicted-vs-authoritative jump further apart than this is a respawn
/// (which relocates by hundreds of pixels), not ordinary correction (under
/// ~30 pixels even mid-dash) — see [`Client::reconcile`]. Restarting the
/// prediction on the spot beats sliding it across the arena.
const SNAP_DISTANCE: f32 = 150.0;

/// Predicted-vs-authoritative disagreement below this many pixels is left
/// alone: float drift and one-tick input timing slips are invisible, and
/// correcting them would only jitter the body. Kept a little loose (rather
/// than pixel-tight) so ordinary jitter in when an input's tick gets applied
/// server-side doesn't constantly re-trigger the decay in
/// [`RECONCILE_RATE`] and leave `render_error` never quite settling at zero.
const RECONCILE_TOLERANCE: f32 = 4.0;

/// Exponential decay rate of the visual reconciliation offset, in 1/s. At 20
/// the offset halves roughly every 35 ms — fast enough that a correction
/// resolves within a couple of rendered frames instead of reading as
/// lingering mush, while still gliding rather than popping.
const RECONCILE_RATE: f32 = 20.0;

/// Below this length the visual reconciliation offset snaps straight to zero
/// instead of decaying forever.
const RENDER_ERROR_EPSILON: f32 = 0.5;

/// Upper bound on unconfirmed prediction records (two seconds at 60 Hz), so
/// a server that stops confirming cannot grow the history without bound.
const PREDICTION_HISTORY: usize = 120;

/// How far behind the freshest snapshot, in server ticks, the remote-entity
/// render clock is held. Remote agents are drawn interpolated at this delay,
/// so ordinary arrival jitter and a lost snapshot or two land *inside* the
/// buffer and never surface as the freeze-then-teleport of drawing the newest
/// snapshot raw. The cost is this many ticks of visual latency on **other**
/// players — never the local one, which is predicted, not interpolated — and
/// it is invisible to the authoritative server, which resolves hits on its own
/// timeline regardless.
const INTERP_DELAY_TICKS: f64 = 2.0;

/// Rate (per second) the render clock is eased back toward its ideal
/// [`INTERP_DELAY_TICKS`] offset, to soak up client/server clock drift without
/// a visible jump in remote playback speed.
const RENDER_CLOCK_CATCHUP: f64 = 1.0;

/// If the render clock falls more than this many ticks behind the freshest
/// snapshot (a lag spike delivering a burst of snapshots at once), it resyncs
/// on the spot instead of speed-crawling across the whole backlog.
const MAX_INTERP_LAG_TICKS: f64 = 8.0;

/// Recent snapshots kept for remote interpolation, oldest at the front. Half a
/// second at 60 Hz — far more than [`INTERP_DELAY_TICKS`] needs, so a burst of
/// late arrivals still finds a bracketing pair.
const SNAPSHOT_BUFFER: usize = 32;

/// The client's input action set, bound to keyboard and mouse in `on_start`.
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

/// The render transform from fixed arena space to the (possibly differently
/// sized) window: a uniform scale plus a centering letterbox offset.
fn viewport(w: f32, h: f32) -> (f32, Vec2) {
    let scale: f32 = (w / ARENA_W).min(h / ARENA_H);
    let offset: Vec2 = Vec2::new((w - ARENA_W * scale) * 0.5, (h - ARENA_H * scale) * 0.5);
    (scale, offset)
}

/// Maps a server [`Effect`] to a particle burst, choosing density and spread by
/// kind while honoring the server-baked color.
fn effect_props(effect: &Effect, seed: f32) -> ParticleProps {
    let (count, speed, life, size): (usize, f32, f32, f32) = match effect.kind {
        EffectKind::Hit => (12, 250.0, 0.4, 6.0),
        EffectKind::Death => (40, 400.0, 0.8, 10.0),
        EffectKind::Powerup => (30, 350.0, 0.6, 8.0),
        EffectKind::PowerupSpawn => (20, 150.0, 0.5, 4.0),
        EffectKind::BulletImpact => (6, 150.0, 0.3, 4.0),
    };
    ParticleProps {
        pos: effect.pos.vec(),
        color: effect.color,
        count,
        speed,
        seed,
        life,
        size,
    }
}

/// Where the local player was predicted to be right after the input with
/// this `seq` was applied, kept until a snapshot confirms (or supersedes) it.
struct PredictedTick {
    seq: u32,
    pos: Vec2,
}

/// Client-side prediction of the local player's **movement — and nothing
/// else**. Each sampled input is applied to `pos`/`vel` immediately through
/// the same [`step_movement`] the server will run for it one network flight
/// later, so walking responds on the very tick instead of after a round trip.
///
/// Deliberately **not** predicted: shooting, dash impulses, knockback, and
/// overlap pushes — their effects arrive only via the authoritative snapshot
/// and show up here as reconciliation. Predicting the shot too would make
/// bullets visibly spawn away from the body whenever the predicted and
/// authoritative positions disagree; movement-only confines any momentary
/// error to the body itself, where a few pixels pass unnoticed. Do not
/// extend this to the shot.
///
/// `render_error` is the visual remainder of past corrections: on a
/// divergence the state jumps to the authoritative base at once, while the
/// drawn body absorbs the jump through this offset decaying to zero (see
/// [`Client::decay_render_error`]), so a correction glides instead of
/// popping.
struct Prediction {
    pos: Vec2,
    vel: Vec2,
    render_error: Vec2,
    history: VecDeque<PredictedTick>,
}

impl Prediction {
    /// A fresh prediction standing exactly on the authoritative position.
    fn spawned_at(pos: Vec2) -> Self {
        Self {
            pos,
            vel: Vec2::ZERO,
            render_error: Vec2::ZERO,
            history: VecDeque::new(),
        }
    }

    /// The predicted position with the visual reconciliation offset applied,
    /// but **no** sub-tick extrapolation — tick-aligned. This is what
    /// `send_input`/`own_center` aim and shoot from, since input sampling
    /// itself runs at fixed-tick cadence: extrapolating it would make the
    /// aim point (and the body it's measured from) disagree frame to frame
    /// for no reason, since both update together at the same cadence anyway.
    fn render_pos(&self) -> Vec2 {
        self.pos + self.render_error
    }

    /// [`render_pos`](Self::render_pos) extrapolated `alpha` (`[0, 1)`, see
    /// [`GameContext::fixed_alpha`](redixel::prelude::GameContext::fixed_alpha))
    /// of the way into the next fixed step, at the current predicted
    /// velocity. `on_render` draws this instead of the tick-aligned
    /// position: display refresh rarely lines up with the tickrate, so
    /// without this the local player's own body would visibly hold still for
    /// however many rendered frames land between two ticks — exactly the
    /// kind of stutter that reads as "input lag" even though the state
    /// itself already responded on the correct tick.
    fn extrapolated_render_pos(&self, alpha: f32, tick_duration: f32) -> Vec2 {
        self.render_pos() + self.vel * (alpha * tick_duration)
    }
}

/// A thin client: it samples input, predicts only the local player's
/// movement (see [`Prediction`]), and renders remote agents interpolated
/// between buffered server snapshots (see
/// [`interpolated_world`](Self::interpolated_world)), holding only local
/// cosmetic state (particles, shake, timers) beyond that.
pub struct Client {
    latest: Option<Snapshot>,
    snapshots: VecDeque<Snapshot>,
    render_tick: Option<f64>,
    last_tick: Option<u64>,
    prediction: Option<Prediction>,
    next_input_seq: u32,
    fx: Effects,
    time: f32,
    local: Option<ClientId>,
    prev_dashing: HashMap<ClientId, bool>,
    prev_health: HashMap<ClientId, i32>,
    local_shoot_cooldown: f32,
    afterimage_clock: f32,
    trail_clock: f32,
}

impl Client {
    /// Creates a fresh, unconnected client.
    pub fn new() -> Self {
        Self {
            latest: None,
            snapshots: VecDeque::new(),
            render_tick: None,
            last_tick: None,
            prediction: None,
            next_input_seq: 1,
            fx: Effects::new(),
            time: 0.0,
            local: None,
            prev_dashing: HashMap::new(),
            prev_health: HashMap::new(),
            local_shoot_cooldown: 0.0,
            afterimage_clock: 0.0,
            trail_clock: 0.0,
        }
    }

    /// Returns a copy of the local player's agent state from the latest snapshot.
    fn local_agent(&self) -> Option<AgentState> {
        let id: ClientId = self.local?;
        self.latest
            .as_ref()?
            .agents
            .iter()
            .find(|a: &&AgentState| a.owner == id)
            .copied()
    }

    /// Drains inbound events, learning the local id and dispatching each message
    /// by the channel it arrived on: continuous world state unreliably, one-shot
    /// cosmetics reliably.
    fn receive(&mut self, ctx: &mut dyn GameContext<Action>) {
        if self.local.is_none()
            && let Some(id) = ctx.network().local_client()
        {
            self.local = Some(id);
            log::info!("Connected as client {id}.");
        }

        while let Some(event) = ctx.network().poll() {
            match event {
                NetworkEvent::Connected(..) => {}
                NetworkEvent::Disconnected(..) => {
                    log::warn!("Disconnected from server.");
                }
                NetworkEvent::Message(.., NetworkChannel::UnreliableSequenced, payload) => {
                    if let Ok(snapshot) = postcard::from_bytes::<Snapshot>(payload) {
                        self.apply_snapshot(snapshot);
                    }
                }
                NetworkEvent::Message(.., NetworkChannel::ReliableOrdered, payload) => {
                    if let Ok(batch) = postcard::from_bytes::<EffectBatch>(payload) {
                        self.play_effects(&batch.effects);
                    }
                }
            }
        }
    }

    /// Spawns a particle burst for each one-shot cosmetic the server sent.
    fn play_effects(&mut self, effects: &[Effect]) {
        for effect in effects {
            let props: ParticleProps = effect_props(effect, self.time);
            self.fx.spawn_burst(props);
        }
    }

    /// Triggers cosmetics from a snapshot (dash trails/bursts, hit shake, baked
    /// effects), then stores it as the world to render.
    ///
    /// Snapshots ride an unreliable channel, so one can arrive out of order or
    /// be replayed. Applying a stale one would snap the world backwards, so
    /// anything not strictly newer than the last applied tick is dropped.
    fn apply_snapshot(&mut self, snapshot: Snapshot) {
        if let Some(last) = self.last_tick
            && snapshot.tick <= last
        {
            return;
        }
        self.last_tick = Some(snapshot.tick);

        self.reconcile(&snapshot);

        for agent in snapshot.agents.iter() {
            let color: (u8, u8, u8) = if agent.rapid_fire {
                (255, 229, 122)
            } else {
                player_color(agent.owner)
            };
            let center: Vec2 = agent.pos.vec() + Vec2::splat(ENTITY_SIZE / 2.0);
            let was_dashing: bool = self.prev_dashing.get(&agent.owner).copied().unwrap_or(false);

            if !was_dashing && agent.dashing {
                self.fx.spawn_burst(ParticleProps {
                    pos: center,
                    color,
                    count: 15,
                    speed: 250.0,
                    seed: self.time,
                    life: 0.4,
                    size: 6.0,
                });
                if Some(agent.owner) == self.local {
                    self.fx.add_shake(5.0);
                }
            }
            self.prev_dashing.insert(agent.owner, agent.dashing);

            let prev_hp: i32 = self.prev_health.get(&agent.owner).copied().unwrap_or(agent.health);
            if agent.health < prev_hp && Some(agent.owner) == self.local {
                self.fx.add_shake(12.0);
            }
            self.prev_health.insert(agent.owner, agent.health);
        }

        self.snapshots.push_back(snapshot);
        if self.snapshots.len() > SNAPSHOT_BUFFER {
            self.snapshots.pop_front();
        }

        self.latest = self.snapshots.back().cloned();
    }

    /// Applies a just-sent input to the local prediction immediately — the
    /// same [`step_movement`] the server will run for it one network flight
    /// later — and records the resulting position under `seq` so
    /// [`reconcile`](Self::reconcile) can compare it against the server's
    /// echo. The impulse argument is always zero here: dash and recoil stay
    /// server-side by design (see [`Prediction`]).
    ///
    /// A no-op until the first snapshot carrying the local agent installs the
    /// prediction, since before that there is no authoritative position to
    /// predict from.
    fn predict_movement(&mut self, seq: u32, input: &PlayerInput, dt: f32) {
        let Some(pred): Option<&mut Prediction> = self.prediction.as_mut() else {
            return;
        };

        let dir: Vec2 = move_direction(input.move_dir);
        step_movement(&mut pred.pos, &mut pred.vel, dir, Vec2::ZERO, dt);

        pred.history.push_back(PredictedTick { seq, pos: pred.pos });
        if pred.history.len() > PREDICTION_HISTORY {
            pred.history.pop_front();
        }
    }

    /// Checks the local player's prediction against the authoritative
    /// position in `snapshot`, matched through the input seq the server
    /// echoes in [`AgentState::input_seq`]. Installs the prediction on the
    /// first snapshot that carries the local agent.
    ///
    /// Agreement within [`RECONCILE_TOLERANCE`] just prunes the history. A
    /// divergence (an unpredicted impulse, a lost input) shifts the whole
    /// predicted state — current position and every pending history entry —
    /// onto the authoritative base, while `render_error` absorbs the shift so
    /// the drawn body glides instead of popping. A jump beyond
    /// [`SNAP_DISTANCE`] is a respawn and restarts the prediction on the
    /// spot: sliding across the arena would be worse than the cut.
    fn reconcile(&mut self, snapshot: &Snapshot) {
        let Some(local): Option<ClientId> = self.local else {
            return;
        };

        let Some(agent): Option<&AgentState> = snapshot.agents.iter().find(|a: &&AgentState| a.owner == local) else {
            return;
        };

        let auth: Vec2 = agent.pos.vec();
        let Some(pred): Option<&mut Prediction> = self.prediction.as_mut() else {
            self.prediction = Some(Prediction::spawned_at(auth));
            return;
        };

        while pred
            .history
            .front()
            .is_some_and(|h: &PredictedTick| seq_newer(agent.input_seq, h.seq))
        {
            pred.history.pop_front();
        }

        if pred
            .history
            .front()
            .is_none_or(|h: &PredictedTick| h.seq != agent.input_seq)
        {
            return;
        }

        let predicted: Vec2 = pred.history.pop_front().expect("front matched above").pos;

        let err: Vec2 = auth - predicted;
        if err.length_sq() <= RECONCILE_TOLERANCE * RECONCILE_TOLERANCE {
            return;
        }

        if err.length_sq() > SNAP_DISTANCE * SNAP_DISTANCE {
            *pred = Prediction::spawned_at(auth);
            return;
        }

        pred.pos += err;
        pred.render_error -= err;
        for entry in pred.history.iter_mut() {
            entry.pos += err;
        }
    }

    /// Bleeds the visual reconciliation offset toward zero so a corrected
    /// body reaches its true position within a few frames, snapping the tail
    /// end below [`RENDER_ERROR_EPSILON`].
    fn decay_render_error(&mut self, dt: f32) {
        let Some(pred): Option<&mut Prediction> = self.prediction.as_mut() else {
            return;
        };

        pred.render_error *= (-RECONCILE_RATE * dt).exp();
        if pred.render_error.length_sq() < RENDER_ERROR_EPSILON * RENDER_ERROR_EPSILON {
            pred.render_error = Vec2::ZERO;
        }
    }

    /// Advances the remote-entity render clock by one frame of real time,
    /// expressed in server ticks (`dt / tick_duration`), keeping it about
    /// [`INTERP_DELAY_TICKS`] behind the freshest buffered snapshot.
    ///
    /// The clock is eased toward that ideal offset ([`RENDER_CLOCK_CATCHUP`])
    /// so client/server clock drift never shows as a speed pop, resynced
    /// outright if it falls a whole [`MAX_INTERP_LAG_TICKS`] behind, and — this
    /// is the load-bearing part — clamped to never pass the newest snapshot, so
    /// [`interpolated_world`](Self::interpolated_world) only ever interpolates
    /// between two snapshots it already holds and never extrapolates a remote
    /// into the unknown.
    fn advance_render_clock(&mut self, dt: f64, tick_duration: f64) {
        let (Some(newest), Some(oldest)): (Option<u64>, Option<u64>) = (
            self.snapshots.back().map(|s: &Snapshot| s.tick),
            self.snapshots.front().map(|s: &Snapshot| s.tick),
        ) else {
            return;
        };

        if tick_duration <= 0.0 {
            return;
        }

        let newest: f64 = newest as f64;
        let oldest: f64 = oldest as f64;
        let target: f64 = newest - INTERP_DELAY_TICKS;

        let Some(rt): Option<f64> = self.render_tick else {
            self.render_tick = Some(target.max(oldest));
            return;
        };

        let mut next: f64 = rt + dt / tick_duration;
        next += (target - next) * RENDER_CLOCK_CATCHUP * dt;
        if next < newest - MAX_INTERP_LAG_TICKS {
            next = target;
        }

        self.render_tick = Some(next.clamp(oldest, newest));
    }

    /// The world to draw this frame: remote agents interpolated between the two
    /// buffered snapshots that straddle the render clock (matched by
    /// [`AgentState::owner`], a stable id — never list position), with bullets
    /// and every other field taken from the newer of the pair.
    ///
    /// The local player is not smoothed here — [`on_render`](Self::on_render)
    /// overwrites its entry with the predicted/extrapolated position afterward.
    /// Bullets are deliberately left un-interpolated: they are fast, transient,
    /// carry no stable id to match across snapshots, and read as motion rather
    /// than jitter. `None` until the render clock has a first snapshot to sit on.
    fn interpolated_world(&self) -> Option<Snapshot> {
        let rt: f64 = self.render_tick?;

        let mut lo: &Snapshot = self.snapshots.front()?;
        let mut hi: &Snapshot = self.snapshots.back()?;
        for snap in self.snapshots.iter() {
            if (snap.tick as f64) <= rt {
                lo = snap;
            }

            if (snap.tick as f64) >= rt {
                hi = snap;
                break;
            }
        }

        let span: f64 = (hi.tick as f64) - (lo.tick as f64);
        let alpha: f32 = if span > 0.0 {
            ((rt - lo.tick as f64) / span) as f32
        } else {
            0.0
        };

        let agents: Vec<AgentState> = hi
            .agents
            .iter()
            .map(|target: &AgentState| -> AgentState {
                let pos: Vec2 = match lo.agents.iter().find(|a: &&AgentState| a.owner == target.owner) {
                    Some(from) => from.pos.vec().lerp(target.pos.vec(), alpha),
                    None => target.pos.vec(),
                };
                AgentState {
                    pos: V2::of(pos),
                    ..*target
                }
            })
            .collect();

        Some(Snapshot {
            tick: hi.tick,
            agents,
            bullets: hi.bullets.clone(),
            powerup: hi.powerup,
        })
    }

    /// The center of the local player's body **as drawn this frame**: the
    /// predicted render position once prediction is running, else the latest
    /// snapshot. Aim is computed from here, so the shot direction always
    /// agrees with the body the player actually sees.
    fn own_center(&self) -> Option<Vec2> {
        if let Some(pred) = self.prediction.as_ref() {
            return Some(pred.render_pos() + Vec2::splat(ENTITY_SIZE / 2.0));
        }

        let local: ClientId = self.local?;
        let snapshot: &Snapshot = self.latest.as_ref()?;
        snapshot
            .agents
            .iter()
            .find(|a: &&AgentState| a.owner == local)
            .map(|a: &AgentState| -> Vec2 { a.pos.vec() + Vec2::splat(ENTITY_SIZE / 2.0) })
    }

    /// The unit aim direction from the local player to the cursor in arena space.
    fn aim_direction(&self, ctx: &dyn GameContext<Action>, scale: f32, offset: Vec2) -> Vec2 {
        let mouse: Vec2 = match ctx.input().mouse_position() {
            Some(p) => p,
            None => return Vec2::ZERO,
        };

        let center: Vec2 = match self.own_center() {
            Some(c) => c,
            None => return Vec2::ZERO,
        };

        if scale <= 0.0 {
            return Vec2::ZERO;
        }

        let raw: Vec2 = (mouse - offset) / scale - center;
        if raw.length_sq() > 0.0 {
            raw.normalise()
        } else {
            Vec2::ZERO
        }
    }

    /// Samples movement/aim/shoot/dash and sends one `PlayerInput` to the server.
    fn send_input(&mut self, ctx: &mut dyn GameContext<Action>) {
        if !ctx.network().is_connected() {
            return;
        }

        let (scale, offset): (f32, Vec2) = viewport(ctx.surface_width() as f32, ctx.surface_height() as f32);

        let mut move_dir: Vec2 = Vec2::ZERO;
        if ctx.input().held(Action::Left) {
            move_dir -= Vec2::X;
        }

        if ctx.input().held(Action::Right) {
            move_dir += Vec2::X;
        }

        if ctx.input().held(Action::Up) {
            move_dir -= Vec2::Y;
        }

        if ctx.input().held(Action::Down) {
            move_dir += Vec2::Y;
        }

        let seq: u32 = self.next_input_seq;
        self.next_input_seq = self.next_input_seq.wrapping_add(1);

        let shooting: bool = ctx.input().held(Action::Shoot);
        let input: PlayerInput = PlayerInput {
            seq,
            move_dir: V2::of(move_dir),
            aim: V2::of(self.aim_direction(ctx, scale, offset)),
            shoot: shooting,
            dash: ctx.input().held(Action::Dash),
        };

        if shooting
            && self.local_shoot_cooldown <= 0.0
            && let Some(agent) = self.local_agent()
        {
            let cooldown: f32 = if agent.rapid_fire {
                RAPID_FIRE_COOLDOWN
            } else {
                BASE_COOLDOWN
            };

            self.local_shoot_cooldown = cooldown;
            self.fx.add_shake(recoil_for(agent.weapon) * 0.015);
        }

        send_encoded(
            ctx.network(),
            NetworkChannel::UnreliableSequenced,
            Some(SERVER_ID),
            &input,
            "input",
        );

        self.predict_movement(seq, &input, ctx.fixed_delta() as f32);
    }

    /// Draws the arena grid in arena space through `to_screen`, with a fractional
    /// `parallax` offset that makes the grid drift as the local player moves.
    fn draw_grid(
        &self,
        ctx: &mut dyn GameContext<Action>,
        to_screen: impl Fn(Vec2) -> Vec2,
        scale: f32,
        parallax: Vec2,
    ) {
        let grid_color: Color = Color::rgb(0.13, 0.13, 0.16);

        let mut gx: f32 = parallax.x.rem_euclid(60.0);
        while gx <= ARENA_W {
            ctx.draw_rect(to_screen(Vec2::new(gx, 0.0)), Vec2::new(scale, ARENA_H * scale), grid_color);
            gx += 60.0;
        }

        let mut gy: f32 = parallax.y.rem_euclid(60.0);
        while gy <= ARENA_H {
            ctx.draw_rect(to_screen(Vec2::new(0.0, gy)), Vec2::new(ARENA_W * scale, scale), grid_color);
            gy += 60.0;
        }
    }

    /// Draws every agent: colored body, gold rapid-fire tint, core pulse, hp bar.
    fn draw_agents(
        &self,
        ctx: &mut dyn GameContext<Action>,
        snapshot: &Snapshot,
        to_screen: impl Fn(Vec2) -> Vec2,
        scale: f32,
    ) {
        for agent in snapshot.agents.iter() {
            let (r, g, b): (u8, u8, u8) = player_color(agent.owner);

            let body: Color = if agent.rapid_fire {
                Color::from_rgba8(255, 229, 122, 255)
            } else {
                Color::from_rgba8(r, g, b, 255)
            };

            let pos: Vec2 = agent.pos.vec();

            ctx.draw_rect(
                to_screen(pos + Vec2::splat(4.0)),
                Vec2::splat(ENTITY_SIZE * scale),
                Color::from_rgba8(0, 0, 0, 150),
            );

            ctx.draw_rect(to_screen(pos), Vec2::splat(ENTITY_SIZE * scale), body);

            let inner_size: f32 = ENTITY_SIZE * 0.4;
            let inner_offset: Vec2 = Vec2::splat((ENTITY_SIZE - inner_size) / 2.0);
            let pulse_core: f32 = (self.time * 3.0 + agent.owner as f32).sin().abs() * 0.5 + 0.5;
            ctx.draw_rect(
                to_screen(pos + inner_offset),
                Vec2::splat(inner_size * scale),
                Color::from_rgba8(255, 255, 255, (pulse_core * 200.0) as u8),
            );

            let hp_percent: f32 = (agent.health.max(0) as f32) / 100.0;
            let hp_pos: Vec2 = pos - Vec2::new(0.0, 10.0);
            ctx.draw_rect(
                to_screen(hp_pos),
                Vec2::new(ENTITY_SIZE * scale, 4.0 * scale),
                Color::rgb(0.7, 0.1, 0.1),
            );

            ctx.draw_rect(
                to_screen(hp_pos),
                Vec2::new(ENTITY_SIZE * hp_percent * scale, 4.0 * scale),
                Color::rgb(0.1, 0.8, 0.2),
            );
        }
    }

    /// Draws every bullet with its weapon color and a soft glow.
    fn draw_bullets(
        &self,
        ctx: &mut dyn GameContext<Action>,
        snapshot: &Snapshot,
        to_screen: impl Fn(Vec2) -> Vec2,
        scale: f32,
    ) {
        for bullet in snapshot.bullets.iter() {
            let (r, g, b): (u8, u8, u8) = weapon_color(bullet.weapon);
            let core: Color = Color::from_rgba8(r, g, b, 255);
            let glow: Color = Color::from_rgba8(r, g, b, 50);
            let pos: Vec2 = bullet.pos.vec();

            ctx.draw_rect(
                to_screen(pos + Vec2::splat(3.0)),
                Vec2::splat(bullet.size * scale),
                Color::from_rgba8(0, 0, 0, 100),
            );

            ctx.draw_rect(
                to_screen(pos - Vec2::splat(4.0)),
                Vec2::splat((bullet.size + 8.0) * scale),
                glow,
            );

            ctx.draw_rect(to_screen(pos), Vec2::splat(bullet.size * scale), core);
        }
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for Client {
    type Action = Action;

    fn on_start(&mut self, ctx: &mut dyn GameContext<Action>) {
        ctx.input_mut().bind(Action::Up, KeyCode::KeyW.into());
        ctx.input_mut().bind(Action::Down, KeyCode::KeyS.into());
        ctx.input_mut().bind(Action::Left, KeyCode::KeyA.into());
        ctx.input_mut().bind(Action::Right, KeyCode::KeyD.into());
        ctx.input_mut().bind(Action::Dash, KeyCode::Space.into());
        ctx.input_mut().bind(Action::Exit, KeyCode::Escape.into());
        ctx.input_mut().bind(Action::Shoot, MouseButton::Left.into());
        log::info!("Client started. Connecting to server...");
    }

    fn on_fixed_update(&mut self, ctx: &mut dyn GameContext<Action>) {
        let dt: f32 = ctx.fixed_delta() as f32;
        self.local_shoot_cooldown -= dt;
        self.afterimage_clock -= dt;

        self.receive(ctx);

        if self.afterimage_clock <= 0.0 {
            self.afterimage_clock = AFTERIMAGE_INTERVAL;

            if let Some(world) = self.interpolated_world() {
                for agent in world.agents.iter() {
                    if agent.dashing {
                        let color: (u8, u8, u8) = if agent.rapid_fire {
                            (255, 229, 122)
                        } else {
                            player_color(agent.owner)
                        };
                        self.fx.spawn_afterimage(agent.pos.vec(), ENTITY_SIZE, color);
                    }
                }
            }
        }

        self.send_input(ctx);
    }

    fn on_update(&mut self, ctx: &mut dyn GameContext<Action>) {
        let dt: f32 = ctx.delta_time() as f32;
        self.time = (self.time + dt) % 3600.0;
        self.trail_clock -= dt;
        self.decay_render_error(dt);
        self.advance_render_clock(ctx.delta_time(), ctx.fixed_delta());
        self.fx.update(dt);

        if self.trail_clock <= 0.0 {
            self.trail_clock = TRAIL_INTERVAL;

            if let Some(world) = self.interpolated_world() {
                for bullet in world.bullets.iter() {
                    let (r, g, b_c): (u8, u8, u8) = weapon_color(bullet.weapon);
                    self.fx.spawn_burst(ParticleProps {
                        pos: bullet.pos.vec() + Vec2::splat(bullet.size / 2.0),
                        color: (r, g, b_c),
                        count: 1,
                        speed: 20.0,
                        seed: self.time + bullet.pos.x,
                        life: 0.2,
                        size: bullet.size * 0.8,
                    });
                }
            }
        }

        if ctx.input().held(Action::Exit) {
            ctx.exit();
        }
    }

    fn on_render(&mut self, ctx: &mut dyn GameContext<Action>) {
        let w: f32 = ctx.surface_width() as f32;
        let h: f32 = ctx.surface_height() as f32;
        let (scale, offset): (f32, Vec2) = viewport(w, h);
        let shake: Vec2 = self.fx.shake_offset(self.time);
        let to_screen = move |p: Vec2| -> Vec2 { offset + (p + shake) * scale };

        let tick_duration: f32 = ctx.fixed_delta() as f32;
        let alpha: f32 = ctx.fixed_alpha() as f32;
        let mut world: Option<Snapshot> = self.interpolated_world();

        if let Some(world) = world.as_mut()
            && let Some(pred) = self.prediction.as_ref()
            && let Some(id) = self.local
            && let Some(agent) = world.agents.iter_mut().find(|a: &&mut AgentState| a.owner == id)
        {
            agent.pos = V2::of(pred.extrapolated_render_pos(alpha, tick_duration));
        }

        let parallax: Vec2 = self
            .local
            .and_then(|id: ClientId| -> Option<Vec2> {
                world
                    .as_ref()?
                    .agents
                    .iter()
                    .find(|a: &&AgentState| a.owner == id)
                    .map(|a: &AgentState| -> Vec2 { a.pos.vec() })
            })
            .map(|p: Vec2| -> Vec2 { Vec2::new(p.x * -0.05, p.y * -0.05) })
            .unwrap_or(Vec2::ZERO);

        ctx.clear_color(Color::rgb(0.08, 0.08, 0.11));
        ctx.draw_rect(
            to_screen(Vec2::ZERO),
            Vec2::new(ARENA_W, ARENA_H) * scale,
            Color::rgb(0.10, 0.10, 0.13),
        );

        self.draw_grid(ctx, to_screen, scale, parallax);

        let snapshot: &Snapshot = match world.as_ref() {
            Some(s) => s,
            None => {
                let pulse: f32 = (self.time * 3.0).sin().abs() * 0.5 + 0.5;
                let size: f32 = ENTITY_SIZE * 2.0;
                let center: Vec2 = Vec2::new(ARENA_W / 2.0, ARENA_H / 2.0) - Vec2::splat(size / 2.0);

                ctx.draw_rect(
                    to_screen(center),
                    Vec2::splat(size * scale),
                    Color::from_rgba8(152, 152, 182, (pulse * 200.0) as u8),
                );

                return;
            }
        };

        self.fx.draw(ctx, to_screen, scale);

        if let Some(powerup) = snapshot.powerup {
            let pulse: f32 = (self.time * 5.0).sin().abs();
            let p_size: f32 = POWERUP_SIZE + pulse * 6.0;
            let p_draw_pos: Vec2 = powerup.pos.vec() - Vec2::splat((p_size - POWERUP_SIZE) / 2.0);
            let (r, g, b): (u8, u8, u8) = weapon_color(powerup.weapon);
            let color: Color = Color::from_rgba8(r, g, b, (150.0 + pulse * 100.0) as u8);

            ctx.draw_rect(
                to_screen(p_draw_pos + Vec2::splat(4.0)),
                Vec2::splat(p_size * scale),
                Color::from_rgba8(0, 0, 0, 150),
            );

            ctx.draw_rect(to_screen(p_draw_pos), Vec2::splat(p_size * scale), color);
        }

        self.draw_agents(ctx, snapshot, to_screen, scale);
        self.draw_bullets(ctx, snapshot, to_screen, scale);

        if let Some(mouse) = ctx.input().mouse_position() {
            let cross: Vec2 = mouse + shake * (scale * 0.5);
            ctx.draw_rect(cross - Vec2::new(2.0, 12.0), Vec2::new(4.0, 24.0), Color::WHITE);
            ctx.draw_rect(cross - Vec2::new(12.0, 2.0), Vec2::new(24.0, 4.0), Color::WHITE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::proto::{BulletState, Weapon};

    const TICK: f32 = 1.0 / 60.0;

    fn snap(tick: u64, agent_x: f32, bullets: Vec<BulletState>) -> Snapshot {
        Snapshot {
            tick,
            agents: vec![AgentState {
                owner: 1,
                pos: V2 { x: agent_x, y: 0.0 },
                input_seq: 0,
                health: 100,
                weapon: Weapon::Pistol,
                rapid_fire: false,
                dashing: false,
            }],
            bullets,
            powerup: None,
        }
    }

    fn agent_at(owner: ClientId, x: f32, y: f32) -> AgentState {
        AgentState {
            owner,
            pos: V2 { x, y },
            input_seq: 0,
            health: 100,
            weapon: Weapon::Pistol,
            rapid_fire: false,
            dashing: false,
        }
    }

    fn remote_snap(tick: u64, agents: Vec<AgentState>) -> Snapshot {
        Snapshot {
            tick,
            agents,
            bullets: Vec::new(),
            powerup: None,
        }
    }

    fn buffered(snaps: Vec<Snapshot>, render_tick: f64) -> Client {
        let mut client: Client = Client::new();
        for snap in snaps {
            client.snapshots.push_back(snap);
        }
        client.render_tick = Some(render_tick);
        client
    }

    #[test]
    fn interpolated_world_is_none_before_the_first_snapshot() {
        assert_eq!(Client::new().interpolated_world(), None);
    }

    #[test]
    fn interpolated_world_lerps_a_remote_agent_halfway_by_owner() {
        let client: Client = buffered(
            vec![
                remote_snap(10, vec![agent_at(2, 0.0, 0.0)]),
                remote_snap(11, vec![agent_at(2, 100.0, 40.0)]),
            ],
            10.5,
        );

        let world: Snapshot = client.interpolated_world().expect("a bracketing pair exists");
        assert_eq!(world.agents[0].pos.x, 50.0, "halfway between the two ticks");
        assert_eq!(world.agents[0].pos.y, 20.0);
    }

    #[test]
    fn interpolated_world_matches_agents_by_owner_not_list_order() {
        let client: Client = buffered(
            vec![
                remote_snap(10, vec![agent_at(1, 0.0, 0.0), agent_at(2, 0.0, 0.0)]),
                remote_snap(11, vec![agent_at(2, 100.0, 0.0), agent_at(1, 200.0, 0.0)]),
            ],
            10.5,
        );

        let world: Snapshot = client.interpolated_world().expect("a bracketing pair exists");
        let owner1: &AgentState = world
            .agents
            .iter()
            .find(|a: &&AgentState| a.owner == 1)
            .expect("owner 1");

        let owner2: &AgentState = world
            .agents
            .iter()
            .find(|a: &&AgentState| a.owner == 2)
            .expect("owner 2");

        assert_eq!(owner1.pos.x, 100.0, "owner 1 lerps its own 0->200, not against owner 2's slot");
        assert_eq!(owner2.pos.x, 50.0, "owner 2 lerps its own 0->100");
    }

    #[test]
    fn interpolated_world_bridges_a_dropped_snapshot() {
        let client: Client = buffered(
            vec![
                remote_snap(10, vec![agent_at(2, 0.0, 0.0)]),
                remote_snap(12, vec![agent_at(2, 200.0, 0.0)]),
            ],
            11.0,
        );

        let world: Snapshot = client.interpolated_world().expect("a bracketing pair exists");
        assert_eq!(
            world.agents[0].pos.x, 100.0,
            "a lost snapshot interpolates across the gap instead of freezing then jumping"
        );
    }

    #[test]
    fn interpolated_world_holds_at_the_newest_and_never_extrapolates() {
        let client: Client = buffered(
            vec![
                remote_snap(10, vec![agent_at(2, 0.0, 0.0)]),
                remote_snap(11, vec![agent_at(2, 100.0, 0.0)]),
            ],
            50.0,
        );

        let world: Snapshot = client
            .interpolated_world()
            .expect("the newest snapshot is always drawable");

        assert_eq!(
            world.agents[0].pos.x, 100.0,
            "a starved clock holds on the newest, never invents a future position"
        );
    }

    #[test]
    fn advance_render_clock_trails_the_newest_without_passing_it() {
        let snaps: Vec<Snapshot> = (10..=20u64)
            .map(|t: u64| remote_snap(t, vec![agent_at(2, t as f32, 0.0)]))
            .collect();

        let mut client: Client = Client::new();
        for snap in snaps {
            client.snapshots.push_back(snap);
        }

        client.advance_render_clock(TICK as f64, TICK as f64);
        let seeded: f64 = client.render_tick.expect("the first call seeds the clock");
        assert!(seeded <= 20.0, "the clock starts at or behind the newest tick, got {seeded}");
        assert!(
            seeded >= 20.0 - INTERP_DELAY_TICKS - 1e-3,
            "and no further back than the interpolation delay, got {seeded}"
        );

        client.advance_render_clock(10.0, TICK as f64);
        let after: f64 = client.render_tick.expect("the clock advanced");
        assert!(
            after <= 20.0,
            "a huge frame must not let the clock pass the newest snapshot, got {after}"
        );
    }

    fn right_input(seq: u32) -> PlayerInput {
        PlayerInput {
            seq,
            move_dir: V2 { x: 1.0, y: 0.0 },
            aim: V2 { x: 0.0, y: 0.0 },
            shoot: false,
            dash: false,
        }
    }

    fn predicting_client(pos: Vec2) -> Client {
        let mut client: Client = Client::new();
        client.local = Some(1);
        client.prediction = Some(Prediction::spawned_at(pos));
        client
    }

    fn confirming(input_seq: u32, pos: Vec2) -> Snapshot {
        let mut snapshot: Snapshot = snap(1, 0.0, Vec::new());
        snapshot.agents[0].pos = V2::of(pos);
        snapshot.agents[0].input_seq = input_seq;
        snapshot
    }

    #[test]
    fn prediction_advances_immediately_without_a_server_reply() {
        let start: Vec2 = Vec2::new(100.0, 100.0);
        let mut client: Client = predicting_client(start);

        let mut expected_pos: Vec2 = start;
        let mut expected_vel: Vec2 = Vec2::ZERO;
        for seq in 1..=5u32 {
            client.predict_movement(seq, &right_input(seq), TICK);
            let dir: Vec2 = move_direction(right_input(seq).move_dir);
            step_movement(&mut expected_pos, &mut expected_vel, dir, Vec2::ZERO, TICK);
        }

        let pred: &Prediction = client.prediction.as_ref().expect("prediction installed");
        assert!(
            pred.pos.x > start.x,
            "moving right must show on the very tick, not after a round trip"
        );
        assert_eq!(pred.pos.x, expected_pos.x, "prediction must run the shared integrator verbatim");
        assert_eq!(pred.pos.y, expected_pos.y);
        assert_eq!(pred.history.len(), 5);
        assert_eq!(pred.history.front().expect("has entries").seq, 1);
    }

    #[test]
    fn reconcile_installs_the_prediction_from_the_first_snapshot() {
        let mut client: Client = Client::new();
        client.local = Some(1);

        client.reconcile(&confirming(0, Vec2::new(40.0, 25.0)));

        let pred: &Prediction = client
            .prediction
            .as_ref()
            .expect("first snapshot installs the prediction");

        assert_eq!(pred.pos.x, 40.0);
        assert_eq!(pred.pos.y, 25.0);
    }

    #[test]
    fn reconcile_leaves_an_exact_prediction_untouched_and_prunes_history() {
        let mut client: Client = predicting_client(Vec2::new(100.0, 100.0));
        for seq in 1..=3u32 {
            client.predict_movement(seq, &right_input(seq), TICK);
        }

        let (confirmed_pos, pos_before): (Vec2, Vec2) = {
            let pred: &Prediction = client.prediction.as_ref().expect("prediction installed");
            (pred.history[1].pos, pred.pos)
        };

        client.reconcile(&confirming(2, confirmed_pos));

        let pred: &Prediction = client.prediction.as_ref().expect("prediction installed");
        assert_eq!(pred.pos.x, pos_before.x, "an exact match must not move the prediction");
        assert_eq!(pred.render_error.x, 0.0);
        assert_eq!(pred.render_error.y, 0.0);
        assert_eq!(pred.history.len(), 1, "everything up to the confirmed seq must be pruned");
        assert_eq!(pred.history[0].seq, 3);
    }

    #[test]
    fn reconcile_absorbs_a_divergence_without_a_visual_pop() {
        let mut client: Client = predicting_client(Vec2::new(100.0, 100.0));
        for seq in 1..=3u32 {
            client.predict_movement(seq, &right_input(seq), TICK);
        }

        let (predicted_at_1, pending_at_3, drawn_before): (Vec2, Vec2, Vec2) = {
            let pred: &Prediction = client.prediction.as_ref().expect("prediction installed");
            (pred.history[0].pos, pred.history[2].pos, pred.render_pos())
        };

        let auth: Vec2 = predicted_at_1 + Vec2::new(30.0, 0.0);
        client.reconcile(&confirming(1, auth));

        let pred: &Prediction = client.prediction.as_ref().expect("prediction installed");
        assert!(
            (pred.pos.x - (drawn_before.x + 30.0)).abs() < 1e-3,
            "the state must adopt the authoritative base at once"
        );
        assert!(
            (pred.render_pos().x - drawn_before.x).abs() < 1e-3,
            "the drawn body must not pop to the raw server value"
        );
        assert!(
            (pred.history[1].pos.x - (pending_at_3.x + 30.0)).abs() < 1e-3,
            "pending history must shift with the base or the same error re-reports every snapshot"
        );
    }

    #[test]
    fn render_error_decays_to_zero_over_a_few_frames() {
        let mut client: Client = predicting_client(Vec2::new(100.0, 100.0));
        client.prediction.as_mut().expect("prediction installed").render_error = Vec2::new(30.0, 0.0);

        client.decay_render_error(TICK);
        let after_one: f32 = client.prediction.as_ref().expect("prediction installed").render_error.x;
        assert!(
            after_one > 0.0 && after_one < 30.0,
            "one frame must shrink the offset, got {after_one}"
        );

        for _ in 0..120 {
            client.decay_render_error(TICK);
        }
        let settled: Vec2 = client.prediction.as_ref().expect("prediction installed").render_error;
        assert_eq!(settled.x, 0.0, "the offset must snap to exactly zero, not decay forever");
        assert_eq!(settled.y, 0.0);
    }

    #[test]
    fn reconcile_snaps_a_respawn_instead_of_sliding() {
        let mut client: Client = predicting_client(Vec2::new(100.0, 100.0));
        for seq in 1..=2u32 {
            client.predict_movement(seq, &right_input(seq), TICK);
        }

        let predicted_at_1: Vec2 = client.prediction.as_ref().expect("prediction installed").history[0].pos;
        let auth: Vec2 = predicted_at_1 + Vec2::new(SNAP_DISTANCE * 4.0, 0.0);
        client.reconcile(&confirming(1, auth));

        let pred: &Prediction = client.prediction.as_ref().expect("prediction installed");
        assert_eq!(pred.pos.x, auth.x, "a respawn must restart the prediction on the spot");
        assert_eq!(pred.render_error.x, 0.0, "no visual offset may slide the body across the arena");
        assert!(pred.history.is_empty(), "stale pre-respawn history must be discarded");
        assert_eq!(pred.vel.x, 0.0, "a respawned agent starts at rest on the server too");
    }

    #[test]
    fn reconcile_skips_a_confirmation_it_has_no_record_of() {
        let mut client: Client = predicting_client(Vec2::new(100.0, 100.0));
        client.predict_movement(8, &right_input(8), TICK);
        let pos_before: Vec2 = client.prediction.as_ref().expect("prediction installed").pos;

        client.reconcile(&confirming(5, Vec2::new(900.0, 0.0)));

        let pred: &Prediction = client.prediction.as_ref().expect("prediction installed");
        assert_eq!(
            pred.pos.x, pos_before.x,
            "a seq older than the history must not correct anything"
        );
        assert_eq!(pred.history.len(), 1, "the pending entry must survive");
    }

    #[test]
    fn prediction_history_is_capped() {
        let mut client: Client = predicting_client(Vec2::new(100.0, 100.0));
        for seq in 1..=(PREDICTION_HISTORY as u32 + 20) {
            client.predict_movement(seq, &right_input(seq), TICK);
        }

        let pred: &Prediction = client.prediction.as_ref().expect("prediction installed");
        assert_eq!(pred.history.len(), PREDICTION_HISTORY);
        assert_eq!(
            pred.history.front().expect("full history").seq,
            21,
            "the oldest entries must be the ones dropped"
        );
    }

    #[test]
    fn extrapolated_render_pos_matches_tick_aligned_at_alpha_zero() {
        let mut pred: Prediction = Prediction::spawned_at(Vec2::new(100.0, 100.0));
        pred.vel = Vec2::new(600.0, 0.0);

        assert_eq!(pred.extrapolated_render_pos(0.0, TICK), pred.render_pos());
    }

    #[test]
    fn extrapolated_render_pos_advances_by_velocity_and_alpha() {
        let mut pred: Prediction = Prediction::spawned_at(Vec2::new(100.0, 100.0));
        pred.vel = Vec2::new(600.0, 0.0);

        let half: Vec2 = pred.extrapolated_render_pos(0.5, TICK);
        assert!(
            (half.x - (pred.pos.x + 600.0 * 0.5 * TICK)).abs() < 1e-4,
            "halfway into the tick must advance by half a tick's worth of velocity, got {}",
            half.x
        );

        let almost_full: Vec2 = pred.extrapolated_render_pos(0.999, TICK);
        assert!(
            almost_full.x > half.x,
            "extrapolation must keep advancing smoothly as alpha approaches the next tick"
        );
    }

    #[test]
    fn extrapolated_render_pos_stacks_on_top_of_the_reconciliation_offset() {
        let mut pred: Prediction = Prediction::spawned_at(Vec2::new(100.0, 100.0));
        pred.vel = Vec2::new(600.0, 0.0);
        pred.render_error = Vec2::new(-10.0, 0.0);

        let extrapolated: Vec2 = pred.extrapolated_render_pos(0.5, TICK);
        let expected: f32 = pred.pos.x + pred.render_error.x + 600.0 * 0.5 * TICK;
        assert!(
            (extrapolated.x - expected).abs() < 1e-4,
            "extrapolation must add on top of the visual reconciliation offset, not replace it"
        );
    }

    #[test]
    fn extrapolated_render_pos_stays_put_when_idle() {
        let pred: Prediction = Prediction::spawned_at(Vec2::new(100.0, 100.0));
        assert_eq!(
            pred.extrapolated_render_pos(0.8, TICK),
            pred.render_pos(),
            "zero velocity, zero extrapolation"
        );
    }

    #[test]
    fn own_center_uses_the_tick_aligned_position_not_the_extrapolated_one() {
        let mut client: Client = predicting_client(Vec2::new(100.0, 100.0));
        client.prediction.as_mut().expect("prediction installed").vel = Vec2::new(600.0, 0.0);

        let expected: Vec2 =
            client.prediction.as_ref().expect("prediction installed").render_pos() + Vec2::splat(ENTITY_SIZE / 2.0);
        assert_eq!(
            client.own_center(),
            Some(expected),
            "aim must be measured from the tick-aligned body, matching what send_input predicted the shot from"
        );
    }
}

use super::*;

include!("../../assets/goblin_saqueador/frames.rs");

#[derive(Clone, Copy)]
pub(super) struct RunnerSprites {
    idle: TextureId,
    run: [TextureId; RUN_PNG.len()],
    attack: [TextureId; ATTACK_PNG.len()],
}

impl RunnerSprites {
    pub(super) fn load(ctx: &mut dyn GameContext<Action>) -> Self {
        Self {
            idle: ctx.load_texture_filtered(IDLE_PNG, TextureFilter::Linear),
            run: RUN_PNG.map(|png| ctx.load_texture_filtered(png, TextureFilter::Linear)),
            attack: ATTACK_PNG.map(|png| ctx.load_texture_filtered(png, TextureFilter::Linear)),
        }
    }

    fn canvas_size() -> f32 {
        UnitKind::Runner.stats().size.y * 1.4
    }

    fn cycle_seconds() -> f32 {
        let ground_speed = GROUND_SPEED_PIXELS * Self::canvas_size() / FRAME_PIXELS;
        RUN_CYCLE_SECONDS * ground_speed / UnitKind::Runner.stats().speed
    }

    fn run_frame(time: f32) -> usize {
        let cycle = Self::cycle_seconds();
        ((time.rem_euclid(cycle) / cycle * RUN_PNG.len() as f32) as usize).min(RUN_PNG.len() - 1)
    }

    fn attack_frame(remaining: f32) -> usize {
        let progress = (1.0 - remaining / ATTACK_SECONDS).clamp(0.0, 1.0);
        (progress * (ATTACK_PNG.len() - 1) as f32 + 0.00001) as usize
    }

    fn texture(&self, unit: &Unit) -> TextureId {
        if unit.attack_animation > 0.0 {
            self.attack[Self::attack_frame(unit.attack_animation)]
        } else if unit.moving {
            self.run[Self::run_frame(unit.animation_time)]
        } else {
            self.idle
        }
    }

    fn rect(foot: Vec2, canvas: f32, faction: Faction) -> (Vec2, Vec2) {
        // A negative width mirrors UVs around the same ground anchor.
        let size = Vec2::new(canvas * faction.direction(), canvas);
        let position = foot - Vec2::new(size.x * ANCHOR_UV[0], size.y * ANCHOR_UV[1]);
        (position, size)
    }

    fn draw_frame(
        &self,
        ctx: &mut dyn GameContext<Action>,
        foot: Vec2,
        canvas: f32,
        faction: Faction,
        texture: TextureId,
        hit: bool,
    ) {
        let (position, size) = Self::rect(foot, canvas, faction);
        let tint = if hit {
            Color::rgb(1.0, 0.42, 0.28)
        } else if faction == Faction::Enemy {
            Color::rgb(1.0, 0.68, 0.52)
        } else {
            Color::WHITE
        };
        ctx.draw_sprite_tinted(position, size, texture, tint);
    }

    fn draw_unit(&self, ctx: &mut dyn GameContext<Action>, foot: Vec2, scale: f32, unit: &Unit) {
        UnitesWar::ellipse(
            ctx,
            foot - Vec2::new(0.0, scale),
            Vec2::new(12.0, 2.8) * scale,
            Color::from_rgba8(18, 25, 16, 105),
        );
        // The torso movement is baked into Attack; keep its feet on the ground.
        self.draw_frame(
            ctx,
            foot,
            Self::canvas_size() * scale,
            unit.faction,
            self.texture(unit),
            unit.hit_flash > 0.0,
        );
    }
}

impl UnitesWar {
    pub(super) fn draw_blender_runner(
        &self,
        ctx: &mut dyn GameContext<Action>,
        foot: Vec2,
        scale: f32,
        unit: &Unit,
    ) -> bool {
        if unit.kind == UnitKind::Runner
            && let Some(sprites) = &self.runner_sprites
        {
            sprites.draw_unit(ctx, foot, scale, unit);
            true
        } else {
            false
        }
    }

    pub(super) fn draw_runner_portrait(&self, ctx: &mut dyn GameContext<Action>, foot: Vec2, canvas: f32) -> bool {
        if let Some(sprites) = &self.runner_sprites {
            sprites.draw_frame(ctx, foot, canvas, Faction::Player, sprites.idle, false);
            true
        } else {
            false
        }
    }

    pub(super) fn start_runner_test(&mut self) {
        self.start_battle();
        self.player_coins = 300.0;
        self.clan.camera = 180.0;
        for (faction, x) in [
            (Faction::Player, 290.0),
            (Faction::Player, 350.0),
            (Faction::Enemy, 1040.0),
            (Faction::Enemy, 1100.0),
        ] {
            self.units
                .push(Unit::new(UnitKind::Runner, faction, Vec2::new(x, 0.0), 0));
        }
        self.align_forest_units();
        self.clan.notice = "TESTE DO SAQUEADOR: F6 REINICIA | ESPACO PAUSA | 1 RECRUTA".into();
        self.clan.notice_time = 10.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sprites() -> RunnerSprites {
        RunnerSprites {
            idle: TextureId::new(0),
            run: std::array::from_fn(|i| TextureId::new(i as u32 + 1)),
            attack: std::array::from_fn(|i| TextureId::new(i as u32 + 100)),
        }
    }

    #[test]
    fn attack_takes_priority_over_running_and_returns_to_the_current_state() {
        let sprites = sprites();
        let mut unit = Unit::new(UnitKind::Runner, Faction::Player, Vec2::ZERO, 0);
        unit.animation_time = RunnerSprites::cycle_seconds() * 0.51;
        assert_eq!(sprites.texture(&unit), sprites.idle);
        unit.moving = true;
        assert_eq!(sprites.texture(&unit), sprites.run[12]);
        unit.attack_animation = RUNNER_ATTACK_ANIMATION;
        assert_eq!(sprites.texture(&unit), sprites.attack[0]);
        unit.attack_animation = RUNNER_ATTACK_ANIMATION * 0.5;
        assert_eq!(sprites.texture(&unit), sprites.attack[4]);
        unit.attack_animation = 0.0;
        assert_eq!(sprites.texture(&unit), sprites.run[12]);
        unit.moving = false;
        assert_eq!(sprites.texture(&unit), sprites.idle);
    }

    #[test]
    fn knife_attack_advances_once_and_clamps_at_both_ends() {
        assert_eq!(RunnerSprites::attack_frame(ATTACK_SECONDS * 2.0), 0);
        assert_eq!(RunnerSprites::attack_frame(ATTACK_SECONDS), 0);
        assert_eq!(RunnerSprites::attack_frame(ATTACK_SECONDS * 0.51), 4);
        assert_eq!(RunnerSprites::attack_frame(ATTACK_SECONDS * (1.0 - ATTACK_HIT_FRACTION)), 4);
        assert_eq!(RunnerSprites::attack_frame(0.0), ATTACK_PNG.len() - 1);
        assert_eq!(RunnerSprites::attack_frame(-ATTACK_SECONDS), ATTACK_PNG.len() - 1);
        let frames: Vec<_> = (0..=100)
            .map(|step| RunnerSprites::attack_frame(ATTACK_SECONDS * (1.0 - step as f32 / 100.0)))
            .collect();
        assert!(frames.windows(2).all(|pair| pair[0] <= pair[1]));
    }

    #[test]
    fn loop_matches_the_distance_travelled_without_holding_the_last_frame() {
        let cycle = RunnerSprites::cycle_seconds();
        assert_eq!(RunnerSprites::run_frame(0.0), 0);
        assert_eq!(RunnerSprites::run_frame(cycle * 0.999), 23);
        assert_eq!(RunnerSprites::run_frame(cycle), 0);
        let travel = UnitKind::Runner.stats().speed * cycle;
        let stride = GROUND_SPEED_PIXELS * RUN_CYCLE_SECONDS * RunnerSprites::canvas_size() / FRAME_PIXELS;
        assert!((travel - stride).abs() < 0.0001);
    }

    #[test]
    fn mirrored_sprites_keep_the_ground_anchor_at_any_view_scale() {
        let foot = Vec2::new(413.0, 275.0);
        for canvas in [53.2, 106.4] {
            for faction in [Faction::Player, Faction::Enemy] {
                let (pos, size) = RunnerSprites::rect(foot, canvas, faction);
                let anchor = pos + Vec2::new(size.x * ANCHOR_UV[0], size.y * ANCHOR_UV[1]);
                assert!((anchor.x - foot.x).abs() < 0.0001);
                assert!((anchor.y - foot.y).abs() < 0.0001);
                assert_eq!(size.x.signum(), faction.direction());
            }
        }
    }

    #[test]
    fn restart_and_the_test_battle_keep_loaded_gpu_handles() {
        let mut game = UnitesWar::new();
        game.runner_sprites = Some(sprites());
        game.start_battle();
        assert_eq!(game.runner_sprites.unwrap().run, sprites().run);
        game.start_runner_test();
        assert_eq!(game.runner_sprites.unwrap().idle, sprites().idle);
        assert_eq!(game.runner_sprites.unwrap().attack, sprites().attack);
        assert_eq!(game.screen, ScreenState::Battle);
        assert_eq!(game.units.len(), 4);
        assert_eq!(game.units.iter().filter(|u| u.faction == Faction::Player).count(), 2);
        game.update_battle(0.05, WORLD_WIDTH, WORLD_HEIGHT);
        assert!(game.units.iter().all(|u| u.moving && u.animation_time > 0.0));
    }

    #[test]
    fn embedded_frames_are_consistent_rgba_pngs() {
        for bytes in std::iter::once(IDLE_PNG).chain(RUN_PNG).chain(ATTACK_PNG) {
            assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
            assert_eq!(u32::from_be_bytes(bytes[16..20].try_into().unwrap()), FRAME_PIXELS as u32);
            assert_eq!(u32::from_be_bytes(bytes[20..24].try_into().unwrap()), FRAME_PIXELS as u32);
            assert_eq!(bytes[25], 6, "The exported character must retain alpha");
        }
        assert_ne!(RUN_PNG[0], RUN_PNG[12]);
        assert_ne!(ATTACK_PNG[0], ATTACK_PNG[4]);
    }
}

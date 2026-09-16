use super::*;

struct SpriteData {
    idle: &'static [&'static [u8]],
    run: &'static [&'static [u8]],
    attack: &'static [&'static [u8]],
    attack_times: &'static [f32],
    anchor: [f32; 2],
    canvas_height_ratio: f32,
    stride_height_ratio: f32,
    run_seconds: f32,
    attack_seconds: f32,
    hit_fraction: f32,
    idle_seconds: f32,
    pixels: u32,
}

mod archer {
    use super::SpriteData;
    include!("../../assets/goblin_arqueiro/frames.rs");
}
mod guard {
    use super::SpriteData;
    include!("../../assets/goblin_guardiao/frames.rs");
}
mod ogre {
    use super::SpriteData;
    include!("../../assets/ogro/frames.rs");
}
mod hero {
    use super::SpriteData;
    include!("../../assets/arqueiro_heroi/frames.rs");
}

pub(super) const HERO_ATTACK_SECONDS: f32 = hero::DATA.attack_seconds;
pub(super) const HERO_HIT_FRACTION: f32 = hero::DATA.hit_fraction;

fn unit_data(kind: UnitKind) -> Option<&'static SpriteData> {
    match kind {
        UnitKind::Runner => None,
        UnitKind::Guard => Some(&guard::DATA),
        UnitKind::Archer => Some(&archer::DATA),
        UnitKind::Brute => Some(&ogre::DATA),
    }
}

pub(super) fn attack_seconds(kind: UnitKind) -> f32 {
    unit_data(kind).map_or(RUNNER_ATTACK_ANIMATION, |data| data.attack_seconds)
}

pub(super) fn hit_fraction(kind: UnitKind) -> f32 {
    unit_data(kind).map_or(goblin_sprites::ATTACK_HIT_FRACTION, |data| data.hit_fraction)
}

#[derive(Clone)]
struct CharacterSprites {
    data: &'static SpriteData,
    idle: Vec<TextureId>,
    run: Vec<TextureId>,
    attack: Vec<TextureId>,
}

impl CharacterSprites {
    fn load(ctx: &mut dyn GameContext<Action>, data: &'static SpriteData) -> Self {
        let mut load = |frames: &[&[u8]]| {
            frames
                .iter()
                .map(|png| ctx.load_texture_filtered(png, TextureFilter::Linear))
                .collect()
        };
        debug_assert!(data.pixels <= 2048 && data.run_seconds > 0.0);
        Self {
            data,
            idle: load(data.idle),
            run: load(data.run),
            attack: load(data.attack),
        }
    }

    fn loop_frame(time: f32, duration: f32, count: usize) -> usize {
        ((time.rem_euclid(duration) / duration * count as f32) as usize).min(count - 1)
    }

    fn attack_frame(&self, remaining: f32, duration: f32) -> usize {
        let progress = (1.0 - remaining / duration).clamp(0.0, 1.0);
        self.data
            .attack_times
            .partition_point(|&t| t <= progress + 0.00001)
            .saturating_sub(1)
    }

    fn texture(
        &self,
        time: f32,
        moving: bool,
        remaining: f32,
        attack_duration: f32,
        speed: f32,
        height: f32,
    ) -> TextureId {
        if remaining > 0.0 {
            self.attack[self.attack_frame(remaining, attack_duration)]
        } else if moving {
            let cycle = self.data.stride_height_ratio * height / speed;
            self.run[Self::loop_frame(time, cycle, self.run.len())]
        } else {
            self.idle[Self::loop_frame(time, self.data.idle_seconds, self.idle.len())]
        }
    }

    fn rect(&self, foot: Vec2, height: f32, direction: f32) -> (Vec2, Vec2) {
        let canvas = height * self.data.canvas_height_ratio;
        let size = Vec2::new(canvas * direction, canvas);
        (
            foot - Vec2::new(size.x * self.data.anchor[0], size.y * self.data.anchor[1]),
            size,
        )
    }

    fn draw(
        &self,
        ctx: &mut dyn GameContext<Action>,
        foot: Vec2,
        height: f32,
        direction: f32,
        texture: TextureId,
        tint: Color,
    ) {
        let (pos, size) = self.rect(foot, height, direction);
        ctx.draw_sprite_tinted(pos, size, texture, tint);
    }
}

#[derive(Clone)]
pub(super) struct ArmySprites {
    archer: CharacterSprites,
    guard: CharacterSprites,
    ogre: CharacterSprites,
    hero: CharacterSprites,
}

impl ArmySprites {
    pub(super) fn load(ctx: &mut dyn GameContext<Action>) -> Self {
        Self {
            archer: CharacterSprites::load(ctx, &archer::DATA),
            guard: CharacterSprites::load(ctx, &guard::DATA),
            ogre: CharacterSprites::load(ctx, &ogre::DATA),
            hero: CharacterSprites::load(ctx, &hero::DATA),
        }
    }

    fn unit(&self, kind: UnitKind) -> Option<&CharacterSprites> {
        match kind {
            UnitKind::Runner => None,
            UnitKind::Guard => Some(&self.guard),
            UnitKind::Archer => Some(&self.archer),
            UnitKind::Brute => Some(&self.ogre),
        }
    }
}

impl UnitesWar {
    pub(super) fn draw_blender_unit(
        &self,
        ctx: &mut dyn GameContext<Action>,
        foot: Vec2,
        scale: f32,
        unit: &Unit,
    ) -> bool {
        if unit.kind == UnitKind::Runner {
            return self.draw_blender_runner(ctx, foot, scale, unit);
        }
        let Some(sprites) = self.army_sprites.as_ref().and_then(|s| s.unit(unit.kind)) else {
            return false;
        };
        let stats = unit.kind.stats();
        Self::ellipse(
            ctx,
            foot - Vec2::new(0.0, scale),
            Vec2::new(stats.size.x * 0.45, 3.0) * scale,
            Color::from_rgba8(18, 25, 16, 105),
        );
        let texture = sprites.texture(
            unit.animation_time,
            unit.moving,
            unit.attack_animation,
            unit.attack_duration,
            stats.speed,
            stats.size.y,
        );
        let tint = if unit.hit_flash > 0.0 {
            Color::rgb(1.0, 0.42, 0.28)
        } else if unit.faction == Faction::Enemy {
            Color::rgb(1.0, 0.68, 0.52)
        } else {
            Color::WHITE
        };
        sprites.draw(ctx, foot, stats.size.y * scale, unit.faction.direction(), texture, tint);
        true
    }

    pub(super) fn draw_blender_hero(&self, ctx: &mut dyn GameContext<Action>, foot: Vec2, scale: f32) -> bool {
        let Some(army) = &self.army_sprites else { return false };
        let texture = army.hero.texture(
            self.hero.animation_time,
            self.hero.moving,
            self.hero.attack_animation,
            HERO_ATTACK_ANIMATION,
            HERO_MOVE_SPEED,
            HERO_SIZE.y,
        );
        let direction = if self.hero.moving && self.hero.attack_animation <= 0.0 {
            self.hero.move_direction
        } else {
            1.0
        };
        let tint = if self.hero.hit_flash > 0.0 {
            Color::rgb(1.0, 0.42, 0.28)
        } else {
            Color::WHITE
        };
        army.hero.draw(ctx, foot, HERO_SIZE.y * scale, direction, texture, tint);
        true
    }

    pub(super) fn draw_unit_portrait(
        &self,
        ctx: &mut dyn GameContext<Action>,
        foot: Vec2,
        height: f32,
        kind: UnitKind,
        is_hero: bool,
    ) -> bool {
        if kind == UnitKind::Runner && !is_hero {
            return self.draw_runner_portrait(ctx, foot, height * 1.4);
        }
        let Some(army) = &self.army_sprites else { return false };
        let sprites = if is_hero { Some(&army.hero) } else { army.unit(kind) };
        let Some(sprites) = sprites else { return false };
        sprites.draw(ctx, foot, height, 1.0, sprites.idle[0], Color::WHITE);
        true
    }

    pub(super) fn start_army_test(&mut self) {
        self.start_battle();
        self.player_coins = 1000.0;
        self.clan.experience[0] = 400.0;
        self.clan.camera = 240.0;
        self.enemy_think_timer = 30.0;
        for (faction, front) in [(Faction::Player, 560.0), (Faction::Enemy, 810.0)] {
            for (i, kind) in [UnitKind::Runner, UnitKind::Guard, UnitKind::Brute, UnitKind::Archer]
                .into_iter()
                .enumerate()
            {
                self.units.push(Unit::new(
                    kind,
                    faction,
                    Vec2::new(front - faction.direction() * i as f32 * 70.0, 0.0),
                    0,
                ));
            }
        }
        self.summon_hero(WORLD_WIDTH, WORLD_HEIGHT);
        self.hero.pos.x = 280.0;
        self.align_forest_units();
        self.clan.notice = "TESTE DAS TROPAS: F8 REINICIA | ESPACO PAUSA | C ABRE COMANDOS DO HEROI".into();
        self.clan.notice_time = 12.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sprites(data: &'static SpriteData) -> CharacterSprites {
        let handles = |count, offset| (0..count).map(|i| TextureId::new(i as u32 + offset)).collect();
        CharacterSprites {
            data,
            idle: handles(data.idle.len(), 0),
            run: handles(data.run.len(), 100),
            attack: handles(data.attack.len(), 200),
        }
    }

    #[test]
    fn embedded_models_have_alpha_and_distinct_motion_frames() {
        for data in [&archer::DATA, &guard::DATA, &ogre::DATA, &hero::DATA] {
            for png in data.idle.iter().chain(data.run).chain(data.attack) {
                assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
                assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), data.pixels);
                assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), data.pixels);
                assert_eq!(png[25], 6, "Sprites must remain transparent");
            }
            assert_ne!(data.run[0], data.run[3]);
            assert_ne!(data.attack[0], data.attack[data.attack.len() / 2]);
            assert!(data.attack_times.windows(2).all(|p| p[0] < p[1]));
            assert_eq!(data.attack_times.last(), Some(&1.0));
            assert!(
                data.attack_times
                    .iter()
                    .any(|t| (t - data.hit_fraction).abs() < 0.00001)
            );
        }
    }

    #[test]
    fn attacks_override_running_and_show_the_release_frame_at_the_damage_event() {
        for data in [&archer::DATA, &guard::DATA, &ogre::DATA, &hero::DATA] {
            let sprites = sprites(data);
            let duration = 0.6;
            let before = sprites.attack_frame(duration * (1.0 - data.hit_fraction) + 0.001, duration);
            let release = sprites.attack_frame(duration * (1.0 - data.hit_fraction), duration);
            assert!(before < release);
            assert!((data.attack_times[release] - data.hit_fraction).abs() < 0.00001);
            assert_eq!(sprites.texture(0.1, true, duration, duration, 40.0, 45.0), sprites.attack[0]);
            assert!(
                sprites
                    .run
                    .contains(&sprites.texture(0.1, true, 0.0, duration, 40.0, 45.0))
            );
            assert!(
                sprites
                    .idle
                    .contains(&sprites.texture(0.1, false, 0.0, duration, 40.0, 45.0))
            );
            assert_eq!(sprites.attack_frame(-1.0, duration), sprites.attack.len() - 1);
        }
    }

    #[test]
    fn both_factions_and_retreating_heroes_keep_their_feet_at_the_ground_anchor() {
        for data in [&archer::DATA, &guard::DATA, &ogre::DATA, &hero::DATA] {
            let sprites = sprites(data);
            for height in [30.0, 49.0, 122.0] {
                for direction in [-1.0, 1.0] {
                    let foot = Vec2::new(510.0, 320.0);
                    let (pos, size) = sprites.rect(foot, height, direction);
                    let anchor = pos + Vec2::new(size.x * data.anchor[0], size.y * data.anchor[1]);
                    assert!((anchor - foot).length() < 0.0001);
                    assert_eq!(size.x.signum(), direction);
                }
            }
        }
    }

    #[test]
    fn test_battle_loads_every_unit_and_preserves_texture_handles_on_restart() {
        let mut game = UnitesWar::new();
        game.army_sprites = Some(ArmySprites {
            archer: sprites(&archer::DATA),
            guard: sprites(&guard::DATA),
            ogre: sprites(&ogre::DATA),
            hero: sprites(&hero::DATA),
        });
        game.start_army_test();
        assert!(game.hero.alive());
        for faction in [Faction::Player, Faction::Enemy] {
            for kind in UnitKind::ALL {
                assert!(game.units.iter().any(|u| u.kind == kind && u.faction == faction));
            }
        }
        let handles = game.army_sprites.as_ref().unwrap().hero.attack.clone();
        game.start_battle();
        assert_eq!(game.army_sprites.as_ref().unwrap().hero.attack, handles);
        assert!(!game.hero.attack_pending);
    }
}

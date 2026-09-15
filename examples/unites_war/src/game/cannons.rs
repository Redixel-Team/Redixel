use super::*;

include!("../../assets/cannon/frames.rs");
const BALL_SPEED: f32 = 330.0;
pub(super) const BLAST_RADIUS: f32 = 55.0;
const MUZZLE_DISTANCE: f32 = 46.4;
const MULTISHOT_SPACING: f32 = 32.0;

#[derive(Clone, Copy)]
pub(super) struct CannonSprites {
    barrels: [TextureId; CANNON_PNG.len()],
    ball: TextureId,
}

impl CannonSprites {
    pub(super) fn load(ctx: &mut dyn GameContext<Action>) -> Self {
        Self {
            barrels: CANNON_PNG.map(|png| ctx.load_texture_filtered(png, TextureFilter::Linear)),
            ball: ctx.load_texture_filtered(include_bytes!("../../assets/cannon/ball.png"), TextureFilter::Linear),
        }
    }
}

pub(super) struct Cannonball {
    pub(super) pos: Vec2,
    destination: Vec2,
    faction: Faction,
    damage: f32,
    radius: f32,
}

impl UnitesWar {
    #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
    pub(super) fn start_cannon_test(&mut self) {
        self.start_battle();
        self.player_coins = 1000.0;
        self.enemy_think_timer = 60.0;
        for faction in [Faction::Player, Faction::Enemy] {
            let x = Self::castle_x(faction, WORLD_WIDTH);
            for distance in [180.0, 210.0, 240.0] {
                self.units.push(Unit::new(
                    UnitKind::Guard,
                    faction.opposite(),
                    Vec2::new(x + faction.direction() * distance, 0.0),
                    0,
                ));
            }
        }
        self.align_forest_units();
        self.clan.notice = "TESTE DOS CANHOES: ESPACO PAUSA | SETAS MOVEM A CAMERA".into();
        self.clan.notice_time = 10.0;
    }

    fn cannon_pivot(faction: Faction, width: f32) -> Vec2 {
        let x = Self::castle_x(faction, width);
        Vec2::new(x + faction.direction() * 8.0, Self::ground_at(x) - 88.0)
    }

    pub(super) fn update_castle_attack(&mut self, faction: Faction, dt: f32, width: f32) {
        let x = Self::castle_x(faction, width);
        let mut targets: Vec<_> = self
            .units
            .iter()
            .filter(|u| u.faction != faction && u.alive() && (u.pos.x - x).abs() <= CASTLE_ATTACK_RANGE)
            .map(|u| u.pos)
            .collect();
        if faction == Faction::Enemy && self.hero.alive() && (self.hero.pos.x - x).abs() <= CASTLE_ATTACK_RANGE {
            targets.push(self.hero.pos);
        }
        targets.sort_by(|a, b| (a.x - x).abs().total_cmp(&(b.x - x).abs()));
        let pivot = Self::cannon_pivot(faction, width);
        let castle = match faction {
            Faction::Player => &mut self.player_castle,
            Faction::Enemy => &mut self.enemy_castle,
        };
        castle.attack_cooldown = (castle.attack_cooldown - dt).max(0.0);
        if castle.health <= 0.0 {
            return;
        }
        let Some(&destination) = targets.first() else {
            return;
        };
        let delta = destination - pivot;
        // Local angle is shared by the mirrored enemy barrel and its shot.
        castle.cannon_angle = delta.y.atan2(delta.x * faction.direction());
        if castle.attack_cooldown > 0.0 {
            return;
        }
        let direction = delta.normalise();
        let muzzle = pivot + direction * MUZZLE_DISTANCE.min(delta.length());
        let shot_count = 1 + castle.upgrades[TowerUpgrade::Multishot.index()] as usize;
        for shot in 0..shot_count {
            // Fan out around the aimed position, even with only one enemy in range.
            // Follow the terrain so the spread lands along the battlefield.
            let offset = (shot as f32 - (shot_count - 1) as f32 * 0.5) * MULTISHOT_SPACING;
            let impact_x = (destination.x + offset * faction.direction()).clamp(0.0, width);
            let impact = Vec2::new(
                impact_x,
                destination.y + (Self::ground_at(impact_x) - Self::ground_at(destination.x)),
            );
            self.cannonballs.push(Cannonball {
                pos: muzzle,
                destination: impact,
                faction,
                damage: 13.0,
                radius: BLAST_RADIUS + 15.0 * castle.upgrades[TowerUpgrade::BlastArea.index()] as f32,
            });
        }
        castle.attack_cooldown = 1.25 / (1.0 + 0.2 * castle.upgrades[TowerUpgrade::AttackSpeed.index()] as f32);
    }

    pub(super) fn update_cannonballs(&mut self, dt: f32, active_count: usize) {
        // New shots do not consume time that elapsed before they were fired.
        let active: Vec<_> = self.cannonballs.drain(..active_count).collect();
        for mut ball in active {
            let delta = ball.destination - ball.pos;
            let travel = BALL_SPEED * dt.max(0.0);
            if delta.length() > travel {
                ball.pos += delta.normalise() * travel;
                self.cannonballs.push(ball);
                continue;
            }
            // Fixed impact point: targets can leave or enter the blast during flight.
            for index in 0..self.units.len() {
                let unit = &self.units[index];
                if unit.faction != ball.faction && unit.alive() && (unit.pos - ball.destination).length() <= ball.radius
                {
                    self.apply_damage(index, ball.damage, 0.18);
                }
            }
            if ball.faction == Faction::Enemy
                && self.hero.alive()
                && (self.hero.pos - ball.destination).length() <= ball.radius
            {
                self.apply_hero_damage(ball.damage, 0.18);
            }
            self.effects.push(AttackEffect {
                start: ball.destination,
                end: ball.destination,
                life: 0.38,
                max_life: 0.38,
                color: Color::from_rgba8(244, 162, 66, 255),
                kind: AttackEffectKind::CannonImpact { radius: ball.radius },
            });
        }
    }

    pub(super) fn draw_cannon(
        &self,
        ctx: &mut dyn GameContext<Action>,
        foot: Vec2,
        scale: f32,
        castle: &Castle,
        faction: Faction,
    ) {
        let pivot = foot + Vec2::new(faction.direction() * 8.0, -88.0) * scale;
        Self::ellipse(ctx, pivot, Vec2::new(12.0, 14.0) * scale, Color::from_rgba8(38, 39, 33, 255));
        if let Some(sprites) = self.cannon_sprites {
            let index = ((castle.cannon_angle.to_degrees() + 10.0) / 5.0)
                .round()
                .clamp(0.0, 20.0) as usize;
            let size = Vec2::new(128.0 * faction.direction(), 128.0) * scale;
            ctx.draw_sprite(pivot - size * 0.5, size, sprites.barrels[index]);
        }
    }

    pub(super) fn draw_cannonballs(&self, ctx: &mut dyn GameContext<Action>, scale: f32) {
        for ball in &self.cannonballs {
            let center = Vec2::new(ball.pos.x - self.clan.camera, ball.pos.y) * scale;
            if let Some(sprites) = self.cannon_sprites {
                ctx.draw_sprite(center - Vec2::splat(6.25 * scale), Vec2::splat(12.5 * scale), sprites.ball);
            } else {
                Self::ellipse(ctx, center, Vec2::splat(5.0 * scale), Color::from_rgba8(37, 40, 41, 255));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add(game: &mut UnitesWar, faction: Faction, x: f32) {
        game.units.push(Unit::new(
            UnitKind::Guard,
            faction,
            Vec2::new(x, UnitesWar::ground_at(x) - 20.0),
            0,
        ));
    }

    #[test]
    fn cannon_delays_damage_until_impact_and_splashes_only_nearby_enemies_once() {
        let mut game = UnitesWar::new();
        add(&mut game, Faction::Enemy, 220.0);
        add(&mut game, Faction::Enemy, 250.0);
        add(&mut game, Faction::Enemy, 290.0);
        add(&mut game, Faction::Player, 230.0);
        let health: Vec<_> = game.units.iter().map(|u| u.health).collect();
        game.update_castle_attack(Faction::Player, 0.0, WORLD_WIDTH);
        assert_eq!(game.cannonballs.len(), 1);
        for (unit, before) in game.units.iter().zip(&health) {
            assert_eq!(unit.health, *before);
        }
        let ball = &game.cannonballs[0];
        let flight = (ball.destination - ball.pos).length() / BALL_SPEED;
        game.update_cannonballs(flight * 0.5, 1);
        assert_eq!(game.units[0].health, health[0]);
        game.update_cannonballs(flight * 0.5 + 0.001, 1);
        assert!(game.cannonballs.is_empty());
        assert_eq!(game.units[0].health, health[0] - 13.0);
        assert_eq!(game.units[1].health, health[1] - 13.0);
        assert_eq!(game.units[2].health, health[2]);
        assert_eq!(game.units[3].health, health[3]);
        game.update_cannonballs(1.0, 0);
        assert_eq!(game.units[0].health, health[0] - 13.0);
    }

    #[test]
    fn cannon_aim_tracks_during_reload_and_muzzle_points_at_target() {
        for faction in [Faction::Player, Faction::Enemy] {
            let mut game = UnitesWar::new();
            let x = UnitesWar::castle_x(faction, WORLD_WIDTH) + faction.direction() * 170.0;
            add(&mut game, faction.opposite(), x);
            game.update_castle_attack(faction, 0.0, WORLD_WIDTH);
            let pivot = UnitesWar::cannon_pivot(faction, WORLD_WIDTH);
            let ball = &game.cannonballs[0];
            assert!(((ball.pos - pivot).normalise() - (ball.destination - pivot).normalise()).length() < 0.0001);
            let old_angle = match faction {
                Faction::Player => game.player_castle.cannon_angle,
                Faction::Enemy => game.enemy_castle.cannon_angle,
            };
            game.units[0].pos.x += faction.direction() * 40.0;
            game.update_castle_attack(faction, 0.01, WORLD_WIDTH);
            let angle = match faction {
                Faction::Player => game.player_castle.cannon_angle,
                Faction::Enemy => game.enemy_castle.cannon_angle,
            };
            assert!((old_angle - angle).abs() > 0.01);
            assert_eq!(game.cannonballs.len(), 1);
        }
    }

    #[test]
    fn removed_target_does_not_redirect_shot_and_units_can_dodge_or_enter_blast() {
        let mut game = UnitesWar::new();
        add(&mut game, Faction::Enemy, 220.0);
        add(&mut game, Faction::Enemy, 500.0);
        game.update_castle_attack(Faction::Player, 0.0, WORLD_WIDTH);
        let destination = game.cannonballs[0].destination;
        game.units.remove(0);
        let before = game.units[0].health;
        game.update_cannonballs(2.0, 1);
        assert_eq!(game.units[0].health, before);
        game.player_castle.attack_cooldown = 0.0;
        game.units[0].pos = destination;
        game.update_castle_attack(Faction::Player, 0.0, WORLD_WIDTH);
        game.units[0].pos.x = 500.0;
        add(&mut game, Faction::Enemy, destination.x + 10.0);
        let newcomer = game.units[1].health;
        game.update_cannonballs(2.0, 1);
        assert_eq!(game.units[0].health, before);
        assert_eq!(game.units[1].health, newcomer - 13.0);
    }

    #[test]
    fn upgraded_enemy_cannon_hits_hero_and_troops_but_not_allies() {
        let mut game = UnitesWar::new();
        game.enemy_castle.upgrades[TowerUpgrade::BlastArea.index()] = 2;
        let x = UnitesWar::castle_x(Faction::Enemy, WORLD_WIDTH) - 170.0;
        game.hero.summoned = true;
        game.hero.pos = Vec2::new(x, UnitesWar::ground_at(x) - 20.0);
        add(&mut game, Faction::Player, x - 20.0);
        add(&mut game, Faction::Enemy, x);
        let troop = game.units[0].health;
        let ally = game.units[1].health;
        game.update_castle_attack(Faction::Enemy, 0.0, WORLD_WIDTH);
        assert_eq!(game.cannonballs[0].destination, game.hero.pos);
        assert_eq!(game.hero.health, HERO_MAX_HEALTH);
        game.update_cannonballs(2.0, 1);
        assert_eq!(game.hero.health, HERO_MAX_HEALTH - 13.0);
        assert_eq!(game.units[0].health, troop - 13.0);
        assert_eq!(game.units[1].health, ally);
    }

    #[test]
    fn new_shots_wait_until_next_step_and_castles_respect_range_and_health() {
        let mut game = UnitesWar::new();
        game.start_battle();
        game.enemy_think_timer = 60.0;
        add(&mut game, Faction::Enemy, 220.0);
        let health = game.units[0].health;
        game.update_battle(0.5, WORLD_WIDTH, WORLD_HEIGHT);
        assert_eq!(game.cannonballs.len(), 1);
        assert_eq!(game.units[0].health, health);
        game.update_battle(0.5, WORLD_WIDTH, WORLD_HEIGHT);
        assert_eq!(game.units[0].health, health - 13.0);
        game.reset();
        add(&mut game, Faction::Enemy, 500.0);
        game.update_castle_attack(Faction::Player, 1.0, WORLD_WIDTH);
        assert!(game.cannonballs.is_empty());
        game.units[0].pos.x = 220.0;
        game.player_castle.health = 0.0;
        game.update_castle_attack(Faction::Player, 1.0, WORLD_WIDTH);
        assert!(game.cannonballs.is_empty());
    }

    #[test]
    fn multishot_fires_full_spread_at_one_enemy_at_every_level() {
        for faction in [Faction::Player, Faction::Enemy] {
            for level in 0..=3 {
                let mut game = UnitesWar::new();
                game.player_coins = 5000.0;
                game.enemy_coins = 5000.0;
                for _ in 0..level {
                    assert!(game.buy_tower_upgrade(faction, TowerUpgrade::Multishot));
                }
                let x = UnitesWar::castle_x(faction, WORLD_WIDTH) + faction.direction() * 180.0;
                add(&mut game, faction.opposite(), x);
                let health = game.units[0].health;
                game.update_castle_attack(faction, 0.0, WORLD_WIDTH);
                let count = level + 1;
                assert_eq!(game.cannonballs.len(), count);
                assert_eq!(game.units[0].health, health);
                let muzzle = game.cannonballs[0].pos;
                for ball in &game.cannonballs {
                    assert_eq!(ball.pos, muzzle);
                    assert!((ball.destination.y - (UnitesWar::ground_at(ball.destination.x) - 20.0)).abs() < 0.001);
                }
                for pair in game.cannonballs.windows(2) {
                    let a = (pair[0].destination - muzzle).normalise();
                    let b = (pair[1].destination - muzzle).normalise();
                    assert!((a - b).length() > 0.01, "shots must travel in different directions");
                }
                let center = game.cannonballs.iter().map(|ball| ball.destination.x).sum::<f32>() / count as f32;
                assert!((center - x).abs() < 0.001);
                game.update_castle_attack(faction, 0.01, WORLD_WIDTH);
                assert_eq!(game.cannonballs.len(), count, "the whole volley shares one cooldown");
                game.update_cannonballs(0.01, count);
                assert_eq!(game.units[0].health, health);
                game.update_cannonballs(2.0, count);
                assert!(game.cannonballs.is_empty());
                assert_eq!(game.effects.len(), count);
                assert!(game.units[0].health < health);
            }
        }
    }

    #[test]
    fn multishot_spread_does_not_depend_on_extra_enemies_and_speed_changes_reload() {
        for faction in [Faction::Player, Faction::Enemy] {
            let mut game = UnitesWar::new();
            game.player_coins = 5000.0;
            game.enemy_coins = 5000.0;
            assert!(game.buy_tower_upgrade(faction, TowerUpgrade::Multishot));
            assert!(game.buy_tower_upgrade(faction, TowerUpgrade::AttackSpeed));
            let x = UnitesWar::castle_x(faction, WORLD_WIDTH);
            add(&mut game, faction.opposite(), x + faction.direction() * 150.0);
            game.update_castle_attack(faction, 0.0, WORLD_WIDTH);
            assert_eq!(game.cannonballs.len(), 2);
            assert_ne!(game.cannonballs[0].destination, game.cannonballs[1].destination);
            let destinations: Vec<_> = game.cannonballs.iter().map(|ball| ball.destination).collect();
            let reload = match faction {
                Faction::Player => game.player_castle.attack_cooldown,
                Faction::Enemy => game.enemy_castle.attack_cooldown,
            };
            assert!((reload - 1.25 / 1.2).abs() < 0.001);
            for distance in [180.0, 210.0] {
                add(&mut game, faction.opposite(), x + faction.direction() * distance);
            }
            game.cannonballs.clear();
            game.update_castle_attack(faction, reload, WORLD_WIDTH);
            assert_eq!(
                game.cannonballs.iter().map(|ball| ball.destination).collect::<Vec<_>>(),
                destinations
            );
            assert!(game.units.iter().all(|u| u.health == u.max_health));
        }
    }
    #[test]
    fn area_upgrade_affects_new_shots_and_matches_the_impact_effect() {
        let mut game = UnitesWar::new();
        add(&mut game, Faction::Enemy, 220.0);
        add(&mut game, Faction::Enemy, 280.0);
        let health = game.units[1].health;
        game.update_castle_attack(Faction::Player, 0.0, WORLD_WIDTH);
        assert!(game.buy_tower_upgrade(Faction::Player, TowerUpgrade::BlastArea));
        game.update_cannonballs(2.0, 1);
        assert_eq!(game.units[1].health, health);
        game.update_castle_attack(Faction::Player, 2.0, WORLD_WIDTH);
        assert_eq!(game.cannonballs[0].radius, 70.0);
        game.update_cannonballs(2.0, 1);
        assert_eq!(game.units[1].health, health - 13.0);
        assert_eq!(
            game.effects.last().unwrap().kind,
            AttackEffectKind::CannonImpact { radius: 70.0 }
        );
    }

    #[test]
    fn pause_freezes_flight_and_reset_clears_projectiles() {
        let mut game = UnitesWar::new();
        game.start_battle();
        add(&mut game, Faction::Enemy, 220.0);
        game.update_castle_attack(Faction::Player, 0.0, WORLD_WIDTH);
        let pos = game.cannonballs[0].pos;
        game.clan.paused = true;
        game.update_battle(1.0, WORLD_WIDTH, WORLD_HEIGHT);
        assert_eq!(game.cannonballs[0].pos, pos);
        game.reset();
        assert!(game.cannonballs.is_empty());
    }
}

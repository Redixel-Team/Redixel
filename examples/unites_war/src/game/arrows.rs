use super::*;

const ARROW_SPEED: f32 = 620.0;
const ARROW_MAX_LIFE: f32 = 2.0;

enum ArrowTarget {
    Unit(u64),
    Hero,
    Castle(Faction),
}

struct ArrowHit {
    target: ArrowTarget,
    damage: f32,
}

pub(super) struct Arrow {
    pub(super) pos: Vec2,
    pub(super) direction: Vec2,
    pub(super) color: Color,
    last_destination: Vec2,
    hits: VecDeque<ArrowHit>,
    life: f32,
}

impl UnitesWar {
    pub(super) fn launch_arrow(&mut self, start: Vec2, targets: Vec<(AttackTarget, f32)>, color: Color, width: f32) {
        // Vector indices change when casualties are collected. Assign stable
        // identities before an arrow retains any target across updates.
        for unit in &mut self.units {
            if unit.id == 0 {
                self.next_unit_id += 1;
                unit.id = self.next_unit_id;
            }
        }
        let hits: VecDeque<_> = targets
            .into_iter()
            .map(|(target, damage)| ArrowHit {
                target: match target {
                    AttackTarget::Unit(index) => ArrowTarget::Unit(self.units[index].id),
                    AttackTarget::Hero => ArrowTarget::Hero,
                    AttackTarget::Castle(faction) => ArrowTarget::Castle(faction),
                },
                damage,
            })
            .collect();
        let Some(destination) = hits
            .front()
            .and_then(|hit| self.arrow_destination(&hit.target, start.y, width))
        else {
            return;
        };
        let delta = destination - start;
        self.arrows.push(Arrow {
            pos: start,
            direction: if delta.length() > 0.0001 {
                delta.normalise()
            } else {
                Vec2::new(1.0, 0.0)
            },
            color,
            last_destination: destination,
            hits,
            life: ARROW_MAX_LIFE,
        });
    }

    fn arrow_destination(&self, target: &ArrowTarget, y: f32, width: f32) -> Option<Vec2> {
        match *target {
            ArrowTarget::Unit(id) => self
                .units
                .iter()
                .find(|unit| unit.id == id && unit.alive())
                .map(|unit| unit.pos),
            ArrowTarget::Hero => self.hero.alive().then_some(self.hero.pos),
            ArrowTarget::Castle(faction) => {
                let castle = match faction {
                    Faction::Player => &self.player_castle,
                    Faction::Enemy => &self.enemy_castle,
                };
                (castle.health > 0.0).then_some(Vec2::new(Self::castle_x(faction, width), y))
            }
        }
    }

    pub(super) fn update_arrows(&mut self, dt: f32, width: f32, active_count: usize) {
        // Only arrows already in flight at the start of this simulation step
        // advance. A newly released arrow must not consume its windup's dt.
        let active: Vec<_> = self.arrows.drain(..active_count).collect();
        for mut arrow in active {
            let mut time = dt.max(0.0).min(arrow.life);
            arrow.life -= time;
            let mut finished = false;
            while time > 0.0 {
                let destination = loop {
                    let Some(hit) = arrow.hits.front() else {
                        break arrow.last_destination;
                    };
                    if let Some(point) = self.arrow_destination(&hit.target, arrow.pos.y, width) {
                        arrow.last_destination = point;
                        break point;
                    }
                    // A dead target cannot receive damage, nor can a new unit
                    // occupying its former vector slot inherit the hit.
                    arrow.hits.pop_front();
                };
                let delta = destination - arrow.pos;
                let distance = delta.length();
                if distance > 0.0001 {
                    arrow.direction = delta / distance;
                }
                if distance > ARROW_SPEED * time {
                    arrow.pos += arrow.direction * ARROW_SPEED * time;
                    break;
                }
                arrow.pos = destination;
                time = (time - distance / ARROW_SPEED).max(0.0);
                if let Some(hit) = arrow.hits.pop_front() {
                    match hit.target {
                        ArrowTarget::Unit(id) => {
                            if let Some(index) = self.units.iter().position(|unit| unit.id == id && unit.alive()) {
                                self.apply_damage(index, hit.damage, 0.14);
                            }
                        }
                        ArrowTarget::Hero => {
                            self.apply_hero_damage(hit.damage, 0.14);
                        }
                        ArrowTarget::Castle(Faction::Player) => self.player_castle.health -= hit.damage,
                        ArrowTarget::Castle(Faction::Enemy) => self.enemy_castle.health -= hit.damage,
                    }
                    self.effects.push(AttackEffect {
                        start: destination,
                        end: destination,
                        life: 0.12,
                        max_life: 0.12,
                        color: arrow.color,
                        kind: AttackEffectKind::ArrowImpact,
                    });
                }
                if arrow.hits.is_empty() {
                    finished = true;
                    break;
                }
            }
            if !finished && arrow.life > 0.0 {
                self.arrows.push(arrow);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target_game() -> UnitesWar {
        let mut game = UnitesWar::new();
        game.start_battle();
        game.units
            .push(Unit::new(UnitKind::Guard, Faction::Enemy, Vec2::new(100.0, 0.0), 0));
        game
    }

    fn fly(game: &mut UnitesWar, dt: f32) {
        game.update_arrows(dt, WORLD_WIDTH, game.arrows.len());
    }

    #[test]
    fn arrow_damage_and_flash_wait_for_the_rendered_tip_to_arrive() {
        let mut game = target_game();
        let health = game.units[0].health;
        game.launch_arrow(Vec2::ZERO, vec![(AttackTarget::Unit(0), 20.0)], Color::WHITE, WORLD_WIDTH);
        assert_eq!(game.units[0].health, health);
        fly(&mut game, 0.1);
        assert_eq!(game.units[0].health, health);
        assert_eq!(game.units[0].hit_flash, 0.0);
        assert!((game.arrows[0].pos.x - 62.0).abs() < 0.001);
        fly(&mut game, 0.07);
        assert_eq!(game.units[0].health, health - 20.0);
        assert!(game.units[0].hit_flash > 0.0);
        assert!(game.arrows.is_empty());
        assert_eq!(game.effects[0].end, game.units[0].pos);
        fly(&mut game, 1.0);
        assert_eq!(game.units[0].health, health - 20.0);
    }

    #[test]
    fn moving_target_is_not_hit_at_its_old_position() {
        let mut game = target_game();
        let health = game.units[0].health;
        game.launch_arrow(Vec2::ZERO, vec![(AttackTarget::Unit(0), 20.0)], Color::WHITE, WORLD_WIDTH);
        fly(&mut game, 0.1);
        game.units[0].pos = Vec2::new(200.0, 30.0);
        fly(&mut game, 0.1);
        assert_eq!(game.units[0].health, health);
        assert!(game.arrows[0].pos.y > 0.0);
        fly(&mut game, 0.2);
        assert_eq!(game.units[0].health, health - 20.0);
        assert_eq!(game.effects[0].end, game.units[0].pos);
    }

    #[test]
    fn casualties_do_not_redirect_an_arrow_to_an_ally_or_replacement() {
        let mut game = target_game();
        game.units
            .push(Unit::new(UnitKind::Guard, Faction::Player, Vec2::new(100.0, 0.0), 0));
        game.launch_arrow(Vec2::ZERO, vec![(AttackTarget::Unit(0), 20.0)], Color::WHITE, WORLD_WIDTH);
        game.units[0].health = 0.0;
        game.collect_defeated_units();
        let ally_health = game.units[0].health;
        game.units
            .push(Unit::new(UnitKind::Guard, Faction::Enemy, Vec2::new(100.0, 0.0), 0));
        let replacement_health = game.units[1].health;
        fly(&mut game, 1.0);
        assert_eq!(game.units[0].health, ally_health);
        assert_eq!(game.units[1].health, replacement_health);
        assert!(game.arrows.is_empty());
    }

    #[test]
    fn piercing_damage_is_applied_sequentially_in_flight() {
        let mut game = target_game();
        game.units
            .push(Unit::new(UnitKind::Guard, Faction::Enemy, Vec2::new(200.0, 0.0), 0));
        let health = game.units[0].health;
        game.launch_arrow(
            Vec2::ZERO,
            vec![(AttackTarget::Unit(0), 20.0), (AttackTarget::Unit(1), 18.0)],
            Color::WHITE,
            WORLD_WIDTH,
        );
        fly(&mut game, 0.2);
        assert_eq!(game.units[0].health, health - 20.0);
        assert_eq!(game.units[1].health, health);
        fly(&mut game, 0.2);
        assert_eq!(game.units[0].health, health - 20.0);
        assert_eq!(game.units[1].health, health - 18.0);
        assert!(game.arrows.is_empty());
    }

    #[test]
    fn enemy_arrows_wait_before_hitting_the_hero() {
        let mut game = UnitesWar::new();
        game.start_battle();
        game.hero.summoned = true;
        game.hero.pos = Vec2::new(400.0, 0.0);
        game.units
            .push(Unit::new(UnitKind::Archer, Faction::Enemy, Vec2::new(500.0, 0.0), 0));
        game.units[0].attack_cooldown = 0.0;
        game.update_units(0.0, WORLD_WIDTH);
        let windup = game.units[0].attack_duration * character_sprites::hit_fraction(UnitKind::Archer);
        game.update_units(windup, WORLD_WIDTH);
        assert_eq!(game.hero.health, HERO_MAX_HEALTH);
        assert_eq!(game.arrows.len(), 1);
        fly(&mut game, 0.05);
        assert_eq!(game.hero.health, HERO_MAX_HEALTH);
        fly(&mut game, 0.2);
        assert!(game.hero.health < HERO_MAX_HEALTH);
    }

    #[test]
    fn newly_released_arrows_do_not_advance_by_the_windup_timestep() {
        let mut game = UnitesWar::new();
        game.start_battle();
        game.enemy_think_timer = 100.0;
        game.units
            .push(Unit::new(UnitKind::Archer, Faction::Player, Vec2::new(500.0, 300.0), 0));
        game.units
            .push(Unit::new(UnitKind::Guard, Faction::Enemy, Vec2::new(620.0, 300.0), 0));
        game.units[0].attack_cooldown = 0.0;
        game.units[1].attack_cooldown = 100.0;
        let health = game.units[1].health;
        game.update_battle(0.0, WORLD_WIDTH, WORLD_HEIGHT);
        let windup = game.units[0].attack_duration * character_sprites::hit_fraction(UnitKind::Archer);
        game.update_battle(windup, WORLD_WIDTH, WORLD_HEIGHT);
        assert_eq!(game.units[1].health, health);
        assert_eq!(game.arrows.len(), 1);
        assert_eq!(game.arrows[0].pos, game.units[0].pos + Vec2::new(10.0, -7.0));
        game.clan.paused = true;
        let position = game.arrows[0].pos;
        game.update_battle(1.0, WORLD_WIDTH, WORLD_HEIGHT);
        assert_eq!(game.arrows[0].pos, position);
        assert_eq!(game.units[1].health, health);
        game.clan.paused = false;
        game.update_battle(0.5, WORLD_WIDTH, WORLD_HEIGHT);
        assert_eq!(game.units[1].health, health - UnitKind::Archer.stats().damage);
    }

    #[test]
    fn tower_damage_waits_for_arrival_even_with_a_large_timestep() {
        for faction in [Faction::Player, Faction::Enemy] {
            let mut game = UnitesWar::new();
            game.start_battle();
            let x = UnitesWar::castle_x(faction, WORLD_WIDTH);
            let health = game.player_castle.health;
            game.launch_arrow(
                Vec2::new(x - faction.direction() * 100.0, 200.0),
                vec![(AttackTarget::Castle(faction), 30.0)],
                Color::WHITE,
                WORLD_WIDTH,
            );
            fly(&mut game, 0.1);
            assert_eq!(game.player_castle.health, health);
            assert_eq!(game.enemy_castle.health, health);
            fly(&mut game, 3.0);
            let castle = if faction == Faction::Player {
                &game.player_castle
            } else {
                &game.enemy_castle
            };
            assert_eq!(castle.health, health - 30.0);
            assert!(game.arrows.is_empty());
        }
    }
}

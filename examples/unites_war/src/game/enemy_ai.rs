use super::*;

struct Battlefield {
    own: [f32; 4],
    opponent: [f32; 4],
    own_power: f32,
    opponent_power: f32,
    home_pressure: f32,
    home_defense: f32,
    nearest_threat: f32,
    hero_present: bool,
}

impl Battlefield {
    fn emergency(&self) -> bool {
        self.nearest_threat < 220.0 || self.home_pressure > self.home_defense * 0.85 + 40.0
    }
}

impl Difficulty {
    fn reaction_seconds(self) -> f32 {
        match self {
            Self::Scout => 2.2,
            Self::Warrior => 1.05,
            Self::Warlord => 0.5,
        }
    }

    fn investment_delay(self) -> f32 {
        match self {
            Self::Scout => 90.0,
            Self::Warrior => 55.0,
            Self::Warlord => 30.0,
        }
    }
}

impl UnitesWar {
    fn enemy_battlefield(&self, width: f32) -> Battlefield {
        let home = Self::castle_x(Faction::Enemy, width);
        let mut field = Battlefield {
            own: [0.0; 4],
            opponent: [0.0; 4],
            own_power: 0.0,
            opponent_power: 0.0,
            home_pressure: 0.0,
            home_defense: 0.0,
            nearest_threat: f32::INFINITY,
            hero_present: self.hero.alive(),
        };
        for unit in self.units.iter().filter(|unit| unit.alive()) {
            let power = unit.health.sqrt() * unit.kind.stats().damage / unit.kind.stats().cooldown;
            let distance = (home - unit.pos.x).abs();
            if unit.faction == Faction::Enemy {
                field.own[unit.kind as usize] += 1.0;
                field.own_power += power;
                if distance < 450.0 {
                    field.home_defense += power;
                }
            } else {
                field.opponent[unit.kind as usize] += 1.0;
                field.opponent_power += power;
                field.nearest_threat = field.nearest_threat.min(distance);
                if distance < 450.0 {
                    field.home_pressure += power;
                }
            }
        }
        if self.hero.alive() {
            let power = self.hero.health.sqrt() * HERO_DAMAGE / HERO_ATTACK_COOLDOWN;
            let distance = (home - self.hero.pos.x).abs();
            field.opponent_power += power;
            field.nearest_threat = field.nearest_threat.min(distance);
            if distance < 500.0 {
                field.home_pressure += power;
            }
        }
        field
    }

    fn enemy_recruit_choice(&self, field: &Battlefield) -> Option<UnitKind> {
        let advanced = self.clan.difficulty != Difficulty::Scout;
        let [runners, guards, archers, brutes] = field.opponent;
        let ranged_pressure = archers + if field.hero_present { 2.0 } else { 0.0 };
        let mut weights = if advanced {
            [2.0, 3.5, 3.0, 1.5]
        } else {
            [4.0, 2.5, 1.5, 0.8]
        };
        if advanced {
            weights[0] += ranged_pressure * 0.7;
            weights[1] += ranged_pressure * 1.3 + runners * 0.7;
            weights[2] += (guards + brutes) * 1.8;
            weights[3] += guards * 0.8 + runners * 0.4;
            if field.own[1] + field.own[3] <= field.own[2] {
                weights[1] += 5.0;
                weights[2] *= 0.35;
            }
        }
        let available = self
            .population_cap(Faction::Enemy)
            .saturating_sub(self.population(Faction::Enemy));
        UnitKind::ALL
            .into_iter()
            .filter(|kind| self.unit_unlocked(*kind, Faction::Enemy) && kind.population() <= available)
            .filter(|kind| !field.emergency() || kind.stats().cost <= self.enemy_coins)
            .max_by(|a, b| {
                let score = |kind: UnitKind| weights[kind as usize] / (field.own[kind as usize] + 1.0);
                score(*a).total_cmp(&score(*b))
            })
    }

    fn enemy_investment(&mut self, field: &Battlefield) -> bool {
        let difficulty = self.clan.difficulty;
        if field.emergency()
            || self.enemy_investment_timer > 0.0
            || self.clan.elapsed < difficulty.investment_delay()
            || field.own.iter().sum::<f32>() < 4.0
            || field.own_power < field.opponent_power * 0.75
        {
            return false;
        }

        let repair = self.enemy_castle.health < self.enemy_castle.max_health * 0.65;
        let tower = if repair {
            Some(TowerUpgrade::Repair)
        } else if difficulty == Difficulty::Scout {
            Some(TowerUpgrade::Health)
        } else {
            None
        };
        let tower = tower.filter(|upgrade| self.tower_level(Faction::Enemy, *upgrade) < 3);
        let order = if difficulty == Difficulty::Warrior || field.hero_present || field.opponent[2] > field.opponent[0]
        {
            [Research::Armor, Research::Weapons]
        } else {
            [Research::Weapons, Research::Armor]
        };
        let research = order.into_iter().find(|r| self.research_level(Faction::Enemy, *r) < 3);
        let tower = tower.or_else(|| {
            research
                .is_none()
                .then(|| {
                    TowerUpgrade::ALL
                        .into_iter()
                        .find(|u| self.tower_level(Faction::Enemy, *u) < 3)
                })
                .flatten()
        });
        let cost = if let Some(upgrade) = tower {
            self.tower_upgrade_cost(Faction::Enemy, upgrade)
        } else if let Some(research) = research {
            self.faction_research_cost(Faction::Enemy, research)
        } else {
            return false;
        };
        // Save for the chosen improvement, retaining enough gold for an
        // emergency recruit. Reassess safety on every subsequent decision.
        if self.enemy_coins >= cost + UnitKind::Runner.stats().cost {
            let bought = if let Some(upgrade) = tower {
                self.buy_tower_upgrade(Faction::Enemy, upgrade)
            } else {
                self.buy_faction_research(Faction::Enemy, research.unwrap())
            };
            if bought {
                self.enemy_investment_timer = difficulty.investment_delay();
            }
        }
        true
    }

    pub(super) fn update_enemy_ai(&mut self, dt: f32, width: f32, height: f32) {
        if self.state != BattleState::Playing || self.clan.paused {
            return;
        }
        self.enemy_investment_timer = (self.enemy_investment_timer - dt).max(0.0);
        self.enemy_think_timer -= dt;
        if self.enemy_think_timer > 0.0 {
            return;
        }
        let jitter = (self.next_random() % 21) as f32 / 100.0;
        self.enemy_think_timer = self.clan.difficulty.reaction_seconds() + jitter;
        let field = self.enemy_battlefield(width);
        if self.clan.difficulty != Difficulty::Scout
            && field.emergency()
            && self.enemy_castle.health < self.enemy_castle.max_health * 0.3
            && field.home_defense > 0.0
            && self.buy_tower_upgrade(Faction::Enemy, TowerUpgrade::Health)
        {
            return;
        }
        if self.enemy_investment(&field) {
            return;
        }
        if let Some(kind) = self.enemy_recruit_choice(&field) {
            // Do not squander savings on a runner just because the planned
            // guard/archer/ogre is not affordable yet.
            self.recruit(Faction::Enemy, kind, width, height);
        }
    }

    pub(super) fn enemy_archer_waits_for_cover(&self, index: usize) -> bool {
        let unit = &self.units[index];
        unit.faction == Faction::Enemy
            && unit.kind == UnitKind::Archer
            && self.clan.difficulty != Difficulty::Scout
            && self.units.iter().any(|other| {
                other.alive()
                    && other.faction == Faction::Enemy
                    && matches!(other.kind, UnitKind::Guard | UnitKind::Brute)
                    && (0.0..90.0).contains(&(other.pos.x - unit.pos.x))
            })
    }

    pub(super) fn enemy_archer_focus(&self, index: usize) -> Option<(AttackTarget, f32, f32)> {
        let archer = &self.units[index];
        let mut targets: Vec<_> = self
            .units
            .iter()
            .enumerate()
            .filter(|(_, unit)| unit.faction == Faction::Player && unit.alive())
            .filter_map(|(i, unit)| {
                let distance = (unit.pos.x - archer.pos.x).abs();
                let size = unit.kind.stats().size.x;
                (distance <= archer.kind.stats().range + size * 0.5).then_some((
                    unit.health + distance * 0.1,
                    AttackTarget::Unit(i),
                    distance,
                    size,
                ))
            })
            .collect();
        let distance = (self.hero.pos.x - archer.pos.x).abs();
        if self.hero.alive() && distance <= archer.kind.stats().range + HERO_SIZE.x * 0.5 {
            targets.push((
                self.hero.health * 0.7 + distance * 0.1,
                AttackTarget::Hero,
                distance,
                HERO_SIZE.x,
            ));
        }
        targets
            .into_iter()
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, target, distance, size)| (target, distance, size))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(difficulty: Difficulty) -> UnitesWar {
        let mut game = UnitesWar::new();
        game.clan.difficulty = difficulty;
        game.start_battle();
        game.clan.experience[1] = 1000.0;
        game.enemy_think_timer = 0.0;
        game
    }

    fn add(game: &mut UnitesWar, kind: UnitKind, faction: Faction, x: f32) {
        game.units.push(Unit::new(kind, faction, Vec2::new(x, 300.0), 0));
    }

    fn decide(game: &mut UnitesWar) {
        game.enemy_think_timer = 0.0;
        game.update_enemy_ai(0.0, WORLD_WIDTH, WORLD_HEIGHT);
    }

    #[test]
    fn saves_for_a_counter_instead_of_spamming_cheap_units() {
        let mut game = game(Difficulty::Warrior);
        add(&mut game, UnitKind::Guard, Faction::Enemy, 1800.0);
        for x in [1100.0, 1200.0, 1300.0] {
            add(&mut game, UnitKind::Brute, Faction::Player, x);
        }
        game.enemy_coins = 30.0;
        decide(&mut game);
        assert_eq!(game.enemy_coins, 30.0);
        assert_eq!(game.units.len(), 4);
        game.enemy_coins = 65.0;
        decide(&mut game);
        assert_eq!(game.units.last().unwrap().kind, UnitKind::Archer);
        assert_eq!(game.enemy_coins, 0.0);
    }

    #[test]
    fn a_siege_cancels_saving_and_buys_an_affordable_defender() {
        let mut game = game(Difficulty::Warlord);
        add(&mut game, UnitKind::Guard, Faction::Enemy, 1500.0);
        let home = UnitesWar::castle_x(Faction::Enemy, WORLD_WIDTH);
        add(&mut game, UnitKind::Brute, Faction::Player, home - 100.0);
        game.enemy_coins = 30.0;
        game.clan.elapsed = 100.0;
        decide(&mut game);
        assert_eq!(game.units.last().unwrap().kind, UnitKind::Runner);
        assert_eq!(game.units.last().unwrap().faction, Faction::Enemy);
        assert_eq!(game.enemy_coins, 0.0);
        assert_eq!(game.clan.enemy_research, [0; 2]);
    }

    #[test]
    fn compositions_change_with_the_opposing_army() {
        let mut counters = Vec::new();
        for opponent in [UnitKind::Brute, UnitKind::Archer] {
            let mut game = game(Difficulty::Warrior);
            for x in [1700.0, 1750.0] {
                add(&mut game, UnitKind::Guard, Faction::Enemy, x);
            }
            for i in 0..5 {
                add(&mut game, opponent, Faction::Player, 1000.0 + i as f32 * 40.0);
            }
            game.enemy_coins = 100.0;
            decide(&mut game);
            counters.push(game.units.last().unwrap().kind);
        }
        assert_eq!(counters, vec![UnitKind::Archer, UnitKind::Runner]);
    }

    #[test]
    fn hard_difficulty_invests_earlier_but_spends_real_gold() {
        for difficulty in [Difficulty::Warrior, Difficulty::Warlord] {
            let mut game = game(difficulty);
            for i in 0..4 {
                add(&mut game, UnitKind::Guard, Faction::Enemy, 1700.0 + i as f32 * 40.0);
            }
            game.clan.elapsed = 31.0;
            game.enemy_coins = 130.0;
            decide(&mut game);
            if difficulty == Difficulty::Warlord {
                assert_eq!(game.clan.enemy_research[Research::Weapons.index()], 1);
                assert_eq!(game.enemy_coins, 30.0);
                assert_eq!(game.units.len(), 4);
            } else {
                assert_eq!(game.clan.enemy_research, [0; 2]);
                assert_eq!(game.units.len(), 5);
            }
        }
    }

    #[test]
    fn upgrades_repair_a_critical_tower_with_defenders_present() {
        let mut game = game(Difficulty::Warlord);
        let home = UnitesWar::castle_x(Faction::Enemy, WORLD_WIDTH);
        add(&mut game, UnitKind::Guard, Faction::Enemy, home - 30.0);
        add(&mut game, UnitKind::Brute, Faction::Player, home - 100.0);
        game.enemy_castle.health = 50.0;
        game.enemy_coins = 130.0;
        decide(&mut game);
        assert_eq!(game.enemy_castle.upgrades[TowerUpgrade::Health.index()], 1);
        assert_eq!(game.enemy_castle.health, 250.0);
        assert_eq!(game.enemy_coins, 10.0);
    }

    #[test]
    fn enemy_research_preserves_costs_caps_and_faction_ownership() {
        let mut game = game(Difficulty::Warlord);
        add(&mut game, UnitKind::Guard, Faction::Enemy, 1000.0);
        add(&mut game, UnitKind::Guard, Faction::Player, 700.0);
        game.units[0].health *= 0.5;
        let enemy_health = game.units[0].health;
        let player_health = game.units[1].health;
        let player_coins = game.player_coins;
        game.enemy_coins = 100.0;
        assert!(game.buy_faction_research(Faction::Enemy, Research::Armor));
        assert!((game.units[0].health - enemy_health * 1.2).abs() < 0.001);
        assert_eq!(game.units[1].health, player_health);
        assert_eq!(game.player_coins, player_coins);
        assert_eq!(game.clan.research, [0; 2]);
        assert!(!game.buy_faction_research(Faction::Enemy, Research::Weapons));
        game.enemy_coins = 1000.0;
        assert!(game.buy_faction_research(Faction::Enemy, Research::Armor));
        assert!(game.buy_faction_research(Faction::Enemy, Research::Armor));
        assert_eq!(game.enemy_coins, 470.0);
        assert!(!game.buy_faction_research(Faction::Enemy, Research::Armor));
        assert!(game.spawn_unit(Faction::Enemy, UnitKind::Guard, WORLD_WIDTH, WORLD_HEIGHT));
        assert!((game.units.last().unwrap().max_health - 155.0 * 1.6).abs() < 0.001);
    }

    #[test]
    fn hard_archers_focus_vulnerable_targets_in_range() {
        for difficulty in [Difficulty::Warrior, Difficulty::Warlord] {
            let mut game = game(difficulty);
            add(&mut game, UnitKind::Archer, Faction::Enemy, 600.0);
            add(&mut game, UnitKind::Guard, Faction::Player, 500.0);
            add(&mut game, UnitKind::Archer, Faction::Player, 470.0);
            game.units[0].attack_cooldown = 0.0;
            game.units[1].attack_cooldown = 100.0;
            game.units[2].attack_cooldown = 100.0;
            game.units[2].health = 10.0;
            game.update_units(0.0, WORLD_WIDTH);
            let windup = game.units[0].attack_duration * character_sprites::hit_fraction(UnitKind::Archer);
            game.update_units(windup, WORLD_WIDTH);
            game.update_arrows(0.5, WORLD_WIDTH, game.arrows.len());
            if difficulty == Difficulty::Warlord {
                assert!(!game.units[2].alive());
                assert_eq!(game.units[1].health, 155.0);
            } else {
                assert_eq!(game.units[2].health, 10.0);
                assert!(game.units[1].health < 155.0);
            }
        }
    }

    #[test]
    fn archers_wait_for_cover_without_blocking_the_front_line() {
        let mut game = game(Difficulty::Warrior);
        add(&mut game, UnitKind::Archer, Faction::Enemy, 1000.0);
        add(&mut game, UnitKind::Guard, Faction::Enemy, 1030.0);
        game.update_units(0.1, WORLD_WIDTH);
        assert_eq!(game.units[0].pos.x, 1000.0);
        assert!(game.units[1].pos.x < 1030.0);
    }

    #[test]
    fn ai_respects_unlocks_population_and_pause_on_every_difficulty() {
        for difficulty in Difficulty::ALL {
            let mut game = game(difficulty);
            game.clan.experience[1] = 0.0;
            game.enemy_coins = 10000.0;
            for _ in 0..50 {
                decide(&mut game);
            }
            assert!(game.population(Faction::Enemy) <= game.population_cap(Faction::Enemy));
            assert!(
                game.units
                    .iter()
                    .all(|unit| matches!(unit.kind, UnitKind::Runner | UnitKind::Guard))
            );
            let coins = game.enemy_coins;
            game.clan.paused = true;
            game.update_enemy_ai(100.0, WORLD_WIDTH, WORLD_HEIGHT);
            assert_eq!(game.enemy_coins, coins);
        }
    }

    #[test]
    fn enemy_research_affects_combat_without_increasing_income() {
        let mut game = game(Difficulty::Warrior);
        game.enemy_coins = 200.0;
        assert!(game.buy_faction_research(Faction::Enemy, Research::Weapons));
        assert!(game.buy_faction_research(Faction::Enemy, Research::Armor));
        game.update_economy(10.0);
        assert_eq!(game.enemy_coins, 60.0);
        add(&mut game, UnitKind::Runner, Faction::Enemy, 600.0);
        add(&mut game, UnitKind::Guard, Faction::Player, 575.0);
        game.units[0].attack_cooldown = 0.0;
        game.units[1].attack_cooldown = 100.0;
        game.update_units(0.0, WORLD_WIDTH);
        let windup = game.units[0].attack_duration * character_sprites::hit_fraction(UnitKind::Runner);
        game.update_units(windup, WORLD_WIDTH);
        assert_eq!(game.units[1].health, 155.0 - 12.0);
    }

    #[test]
    fn hard_ai_prioritizes_weapons_when_its_front_line_reaches_the_castle() {
        let mut game = game(Difficulty::Warlord);
        for x in [130.0, 170.0, 210.0, 250.0] {
            add(&mut game, UnitKind::Guard, Faction::Enemy, x);
        }
        game.clan.elapsed = 61.0;
        game.enemy_coins = 130.0;
        decide(&mut game);
        assert_eq!(game.clan.enemy_research[Research::Weapons.index()], 1);
        assert_eq!(game.clan.enemy_research[Research::Armor.index()], 0);
        assert_eq!(game.enemy_coins, 30.0);
    }

    #[test]
    fn harder_difficulties_put_pressure_on_an_undefended_base_sooner() {
        let mut totals = [0.0; 3];
        for (i, difficulty) in Difficulty::ALL.into_iter().enumerate() {
            for seed in [7, 71, 701] {
                let mut game = UnitesWar::new();
                game.clan.difficulty = difficulty;
                game.start_battle();
                game.rng = seed;
                for _ in 0..18000 {
                    game.update_battle(0.05, WORLD_WIDTH, WORLD_HEIGHT);
                    game.align_forest_units();
                    assert!(game.enemy_coins >= 0.0);
                    assert!(game.population(Faction::Enemy) <= game.population_cap(Faction::Enemy));
                    if game.state != BattleState::Playing {
                        break;
                    }
                }
                assert_eq!(game.state, BattleState::Defeat, "{difficulty:?}, seed {seed}");
                totals[i] += game.clan.elapsed;
            }
        }
        println!("Average undefended defeat times: {:?}", totals.map(|total| total / 3.0));
        assert!(totals[2] < totals[1] && totals[1] < totals[0], "{totals:?}");
    }
}

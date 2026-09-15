use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TowerUpgrade {
    Multishot,
    AttackSpeed,
    BlastArea,
    Repair,
    Health,
}
impl TowerUpgrade {
    pub(super) const ALL: [Self; 5] = [
        Self::Multishot,
        Self::AttackSpeed,
        Self::BlastArea,
        Self::Repair,
        Self::Health,
    ];
    pub(super) fn index(self) -> usize {
        self as usize
    }
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Multishot => "TIRO MULTIPLO",
            Self::AttackSpeed => "VEL. DE ATAQUE",
            Self::BlastArea => "AREA DE DANO",
            Self::Repair => "REPARO",
            Self::Health => "VIDA MAXIMA",
        }
    }
    pub(super) fn description(self) -> &'static str {
        match self {
            Self::Multishot => "+1 BOLA POR SALVA EM DISPERSAO, ATE 4.",
            Self::AttackSpeed => "+20% DE VELOCIDADE DE ATAQUE POR NIVEL.",
            Self::BlastArea => "+15 DE RAIO DA EXPLOSAO POR NIVEL.",
            Self::Repair => "RECUPERA 3 DE VIDA POR SEGUNDO POR NIVEL.",
            Self::Health => "+200 DE VIDA MAXIMA POR NIVEL.",
        }
    }
}
impl UnitesWar {
    pub(super) fn tower_level(&self, faction: Faction, upgrade: TowerUpgrade) -> u8 {
        match faction {
            Faction::Player => self.player_castle.upgrades[upgrade.index()],
            Faction::Enemy => self.enemy_castle.upgrades[upgrade.index()],
        }
    }
    pub(super) fn tower_upgrade_cost(&self, faction: Faction, upgrade: TowerUpgrade) -> f32 {
        120.0 + 100.0 * self.tower_level(faction, upgrade) as f32
    }
    pub(super) fn buy_tower_upgrade(&mut self, faction: Faction, upgrade: TowerUpgrade) -> bool {
        if self.state != BattleState::Playing || self.clan.paused {
            return false;
        }
        let cost = self.tower_upgrade_cost(faction, upgrade);
        let (castle, coins) = match faction {
            Faction::Player => (&mut self.player_castle, &mut self.player_coins),
            Faction::Enemy => (&mut self.enemy_castle, &mut self.enemy_coins),
        };
        if castle.health <= 0.0 || castle.upgrades[upgrade.index()] >= 3 || *coins < cost {
            return false;
        }
        *coins -= cost;
        castle.upgrades[upgrade.index()] += 1;
        if upgrade == TowerUpgrade::Health {
            castle.max_health += 200.0;
            castle.health += 200.0;
        }
        if faction == Faction::Player {
            self.announce(format!("{} MELHORADA", upgrade.name()));
        }
        true
    }
    pub(super) fn update_tower_repairs(&mut self, dt: f32) {
        for castle in [&mut self.player_castle, &mut self.enemy_castle] {
            if castle.health > 0.0 {
                castle.health = (castle.health
                    + 3.0 * castle.upgrades[TowerUpgrade::Repair.index()] as f32 * dt.max(0.0))
                .min(castle.max_health);
            }
        }
    }
    pub(super) fn draw_tower_icon(ctx: &mut dyn GameContext<Action>, center: Vec2, s: f32) {
        let stone = Color::from_rgba8(167, 163, 137, 255);
        ctx.draw_rect(center + Vec2::new(-9.0, -8.0) * s, Vec2::new(18.0, 23.0) * s, stone);
        for x in [-10.0, -2.0, 6.0] {
            ctx.draw_rect(center + Vec2::new(x, -15.0) * s, Vec2::new(5.0, 10.0) * s, stone);
        }
        ctx.draw_rect(
            center + Vec2::new(-3.0, 5.0) * s,
            Vec2::new(6.0, 10.0) * s,
            Color::from_rgba8(39, 39, 32, 255),
        );
        ctx.draw_rect(
            center + Vec2::new(-2.0, -4.0) * s,
            Vec2::new(4.0, 5.0) * s,
            Color::from_rgba8(39, 39, 32, 255),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upgrades_have_independent_levels_costs_caps_and_factions() {
        let mut game = UnitesWar::new();
        game.start_battle();
        game.player_coins = 5000.0;
        for upgrade in TowerUpgrade::ALL {
            for level in 0..3 {
                let before = game.player_coins;
                assert!(game.buy_tower_upgrade(Faction::Player, upgrade));
                assert_eq!(before - game.player_coins, 120.0 + 100.0 * level as f32);
                assert_eq!(game.tower_level(Faction::Player, upgrade), level + 1);
                assert_eq!(game.tower_level(Faction::Enemy, upgrade), 0);
            }
            let before = game.player_coins;
            assert!(!game.buy_tower_upgrade(Faction::Player, upgrade));
            assert_eq!(game.player_coins, before);
        }
        assert_eq!(game.player_castle.max_health, CASTLE_MAX_HEALTH + 600.0);
        assert_eq!(game.clan.research, [0; 2]);
        assert_eq!(game.population_cap(Faction::Player), 20);
        game.reset();
        assert_eq!(game.player_castle.upgrades, [0; 5]);
        assert_eq!(game.player_castle.max_health, CASTLE_MAX_HEALTH);
    }
    #[test]
    fn repair_is_gradual_capped_paused_and_cannot_revive_destroyed_castles() {
        let mut game = UnitesWar::new();
        game.start_battle();
        game.player_castle.health = 500.0;
        assert!(game.buy_tower_upgrade(Faction::Player, TowerUpgrade::Repair));
        assert_eq!(game.player_castle.health, 500.0);
        game.update_tower_repairs(2.0);
        assert_eq!(game.player_castle.health, 506.0);
        game.clan.paused = true;
        game.update_battle(30.0, WORLD_WIDTH, WORLD_HEIGHT);
        assert_eq!(game.player_castle.health, 506.0);
        game.update_tower_repairs(1000.0);
        assert_eq!(game.player_castle.health, game.player_castle.max_health);
        game.player_castle.health = 0.0;
        game.update_tower_repairs(1000.0);
        assert_eq!(game.player_castle.health, 0.0);
    }
    #[test]
    fn unaffordable_paused_or_destroyed_towers_cannot_buy_upgrades() {
        let mut game = UnitesWar::new();
        game.player_coins = 119.0;
        assert!(!game.buy_tower_upgrade(Faction::Player, TowerUpgrade::Health));
        game.player_coins = 1000.0;
        game.clan.paused = true;
        assert!(!game.buy_tower_upgrade(Faction::Player, TowerUpgrade::Health));
        game.clan.paused = false;
        game.player_castle.health = 0.0;
        assert!(!game.buy_tower_upgrade(Faction::Player, TowerUpgrade::Health));
        assert_eq!(game.player_coins, 1000.0);
    }
}

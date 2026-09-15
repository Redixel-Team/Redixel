use super::*;

pub(super) const WORLD_WIDTH: f32 = 2400.0;
pub(super) const WORLD_HEIGHT: f32 = 540.0;
pub(super) const HERO_UNLOCK_XP: f32 = 250.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Difficulty {
    Scout,
    #[default]
    Warrior,
    Warlord,
}

impl Difficulty {
    pub(super) const ALL: [Self; 3] = [Self::Scout, Self::Warrior, Self::Warlord];
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Scout => "BATEDOR",
            Self::Warrior => "GUERREIRO",
            Self::Warlord => "SENHOR DA GUERRA",
        }
    }
    pub(super) fn income(self) -> f32 {
        match self {
            Self::Scout => 0.72,
            Self::Warrior => 1.0,
            Self::Warlord => 1.3,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum CommandTab {
    #[default]
    Army,
    Research,
    Magic,
    Hero,
    Tower,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Research {
    Weapons,
    Armor,
}

impl Research {
    pub(super) const ALL: [Self; 2] = [Self::Weapons, Self::Armor];
    pub(super) fn index(self) -> usize {
        self as usize
    }
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Weapons => "ARMAS",
            Self::Armor => "ARMADURA",
        }
    }
    pub(super) fn description(self) -> &'static str {
        match self {
            Self::Weapons => "+20% DE DANO POR NIVEL",
            Self::Armor => "+20% DE VIDA POR NIVEL",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Spell {
    Lightning,
    Meteor,
}

pub(super) struct ClanWar {
    pub(super) experience: [f32; 2],
    pub(super) reputation: u32,
    pub(super) research: [u8; 2],
    pub(super) enemy_research: [u8; 2],
    pub(super) difficulty: Difficulty,
    pub(super) camera: f32,
    pub(super) elapsed: f32,
    pub(super) tab: CommandTab,
    pub(super) panel_open: bool,
    pub(super) selected_spell: Option<Spell>,
    pub(super) meteor_cooldown: f32,
    pub(super) fury_cooldown: f32,
    pub(super) fury_time: f32,
    pub(super) hero_respawn: f32,
    pub(super) paused: bool,
    pub(super) notice: String,
    pub(super) notice_time: f32,
}

impl Default for ClanWar {
    fn default() -> Self {
        Self {
            experience: [0.0; 2],
            reputation: 0,
            research: [0; 2],
            enemy_research: [0; 2],
            difficulty: Difficulty::default(),
            camera: 0.0,
            elapsed: 0.0,
            tab: CommandTab::Army,
            panel_open: false,
            selected_spell: None,
            meteor_cooldown: 0.0,
            fury_cooldown: 0.0,
            fury_time: 0.0,
            hero_respawn: 0.0,
            paused: false,
            notice: "RECRUTE GOBLINS. DESTRUA A TORRE RIVAL.".into(),
            notice_time: 8.0,
        }
    }
}

impl UnitKind {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Runner => "SAQUEADOR",
            Self::Guard => "GUARDIAO",
            Self::Archer => "ARQUEIRO",
            Self::Brute => "OGRO",
        }
    }
    pub(super) fn unlock_xp(self) -> f32 {
        match self {
            Self::Runner | Self::Guard => 0.0,
            Self::Archer => 120.0,
            Self::Brute => 360.0,
        }
    }
    pub(super) fn population(self) -> usize {
        match self {
            Self::Brute => 3,
            Self::Guard => 2,
            _ => 1,
        }
    }
}

impl UnitesWar {
    pub(super) fn faction_index(faction: Faction) -> usize {
        if faction == Faction::Player { 0 } else { 1 }
    }

    pub(super) fn population(&self, faction: Faction) -> usize {
        self.units
            .iter()
            .filter(|unit| unit.faction == faction && unit.alive())
            .map(|unit| unit.kind.population())
            .sum::<usize>()
            + if faction == Faction::Player {
                self.player_recruit_queue.iter().map(|kind| kind.population()).sum()
            } else {
                0
            }
    }

    pub(super) fn population_cap(&self, _faction: Faction) -> usize {
        20
    }

    pub(super) fn unit_unlocked(&self, kind: UnitKind, faction: Faction) -> bool {
        self.clan.experience[Self::faction_index(faction)] >= kind.unlock_xp()
    }

    pub(super) fn announce(&mut self, text: impl Into<String>) {
        self.clan.notice = text.into();
        self.clan.notice_time = 3.5;
    }

    pub(super) fn research_level(&self, faction: Faction, research: Research) -> u8 {
        match faction {
            Faction::Player => self.clan.research[research.index()],
            Faction::Enemy => self.clan.enemy_research[research.index()],
        }
    }

    pub(super) fn faction_research_cost(&self, faction: Faction, research: Research) -> f32 {
        100.0 + self.research_level(faction, research) as f32 * 110.0
    }

    pub(super) fn research_cost(&self, research: Research) -> f32 {
        self.faction_research_cost(Faction::Player, research)
    }

    pub(super) fn buy_research(&mut self, research: Research) -> bool {
        self.buy_faction_research(Faction::Player, research)
    }

    pub(super) fn buy_faction_research(&mut self, faction: Faction, research: Research) -> bool {
        let level = self.research_level(faction, research);
        let cost = self.faction_research_cost(faction, research);
        let coins = match faction {
            Faction::Player => &mut self.player_coins,
            Faction::Enemy => &mut self.enemy_coins,
        };
        if self.state != BattleState::Playing || level >= 3 || *coins < cost {
            return false;
        }
        *coins -= cost;
        match faction {
            Faction::Player => self.clan.research[research.index()] += 1,
            Faction::Enemy => self.clan.enemy_research[research.index()] += 1,
        }
        if research == Research::Armor {
            let ratio = (1.0 + (level + 1) as f32 * 0.2) / (1.0 + level as f32 * 0.2);
            for unit in self
                .units
                .iter_mut()
                .filter(|unit| unit.faction == faction && unit.alive())
            {
                unit.max_health *= ratio;
                unit.health *= ratio;
            }
        }
        if faction == Faction::Player {
            self.announce(format!("{} MELHORADA", research.name()));
        }
        true
    }

    pub(super) fn ground_at(x: f32) -> f32 {
        // A valley between the two strongholds; independent of window resolution.
        337.0 - 45.0 * (x / WORLD_WIDTH * std::f32::consts::TAU).cos()
    }

    pub(super) fn align_forest_units(&mut self) {
        for unit in &mut self.units {
            unit.pos.y = Self::ground_at(unit.pos.x) - unit.kind.stats().size.y * 0.5;
        }
        self.hero.pos.y = Self::ground_at(self.hero.pos.x) - HERO_SIZE.y * 0.5;
    }

    pub(super) fn view_scale(width: f32, height: f32) -> f32 {
        (width / HUD_BASE_WIDTH).min(height / HUD_BASE_HEIGHT).max(0.1)
    }

    pub(super) fn screen_to_world(&self, point: Vec2, width: f32, height: f32) -> Vec2 {
        point / Self::view_scale(width, height) + Vec2::new(self.clan.camera, 0.0)
    }

    pub(super) fn move_camera(&mut self, delta: f32, width: f32, height: f32) {
        let visible = width / Self::view_scale(width, height);
        self.clan.camera = (self.clan.camera + delta).clamp(0.0, (WORLD_WIDTH - visible).max(0.0));
    }

    pub(super) fn select_spell(&mut self, spell: Spell) {
        let (cooldown, xp) = match spell {
            Spell::Lightning => (self.spell_cooldown, 0.0),
            Spell::Meteor => (self.clan.meteor_cooldown, 500.0),
        };
        if self.clan.experience[0] < xp {
            self.announce(format!("REQUER {} XP", xp as u32));
            return;
        }
        if cooldown > 0.0 {
            self.announce("MAGIA RECARREGANDO");
            return;
        }
        self.clan.selected_spell = if self.clan.selected_spell == Some(spell) {
            None
        } else {
            Some(spell)
        };
    }

    pub(super) fn cast_selected_spell(&mut self, target: Vec2) -> bool {
        if self.state != BattleState::Playing || !(0.0..=WORLD_WIDTH).contains(&target.x) {
            return false;
        }
        let target = Vec2::new(target.x, Self::ground_at(target.x) - 24.0);
        let success = match self.clan.selected_spell {
            Some(Spell::Lightning) => self.cast_spell(target, WORLD_WIDTH, WORLD_HEIGHT),
            Some(Spell::Meteor) if self.clan.meteor_cooldown <= 0.0 && self.clan.experience[0] >= 500.0 => {
                let targets: Vec<usize> = self
                    .units
                    .iter()
                    .enumerate()
                    .filter(|(_, u)| u.alive() && u.faction == Faction::Enemy && (u.pos.x - target.x).abs() < 160.0)
                    .map(|(i, _)| i)
                    .collect();
                for index in targets {
                    self.apply_damage(index, 170.0, 0.3);
                }
                if (Self::castle_x(Faction::Enemy, WORLD_WIDTH) - target.x).abs() < 160.0 {
                    self.enemy_castle.health -= 100.0;
                }
                self.effects.push(AttackEffect {
                    start: target + Vec2::new(-180.0, -370.0),
                    end: target,
                    life: 0.65,
                    max_life: 0.65,
                    color: Color::from_rgba8(255, 158, 53, 255),
                    kind: AttackEffectKind::Meteor,
                });
                self.clan.meteor_cooldown = 32.0;
                true
            }
            _ => false,
        };
        if success {
            self.clan.selected_spell = None;
        }
        success
    }

    pub(super) fn cast_fury(&mut self) -> bool {
        if !self.hero.alive() || self.clan.fury_cooldown > 0.0 {
            return false;
        }
        self.clan.fury_time = 8.0;
        self.clan.fury_cooldown = 28.0;
        self.announce("GRITO DE GUERRA! DANO DO CHEFE DOBRADO");
        true
    }

    pub(super) fn update_clan(&mut self, dt: f32) {
        self.clan.elapsed += dt;
        self.clan.notice_time = (self.clan.notice_time - dt).max(0.0);
        self.clan.meteor_cooldown = (self.clan.meteor_cooldown - dt).max(0.0);
        self.clan.fury_time = (self.clan.fury_time - dt).max(0.0);
        self.clan.fury_cooldown = (self.clan.fury_cooldown - dt).max(0.0);
        if self.hero.summoned && !self.hero.alive() {
            self.clan.hero_respawn += dt;
            if self.clan.hero_respawn >= HERO_RETURN_COOLDOWN {
                self.hero = Hero::new();
                self.clan.hero_respawn = 0.0;
                self.announce("O CHEFE SE RECUPEROU. CONVOQUE COM 5.");
            }
        }
    }
}

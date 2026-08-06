use std::collections::VecDeque;

use redixel::prelude::*;

mod passives;
mod render;

use passives::*;

const STARTING_COINS: f32 = 90.0;
const PASSIVE_COIN_RATE: f32 = 4.0;
const ENEMY_PASSIVE_COIN_RATE: f32 = 4.4;
const MAX_COINS: f32 = 9_999.0;
const COIN_METER_RANGE: f32 = 300.0;
const CASTLE_MAX_HEALTH: f32 = 1_000.0;
const CASTLE_WIDTH: f32 = 92.0;
const CASTLE_ATTACK_RANGE: f32 = 235.0;
const SPELL_COOLDOWN: f32 = 15.0;
const SPELL_RADIUS: f32 = 135.0;
const SPELL_DAMAGE: f32 = 85.0;
const TOOLBAR_HEIGHT: f32 = 104.0;
const BUTTON_SIZE: f32 = 66.0;
const BUTTON_GAP: f32 = 12.0;
const MAX_UNITS: usize = 80;
const PLAYER_RECRUIT_INTERVAL: f32 = 0.85;
const MENU_BUTTON_WIDTH: f32 = 250.0;
const MENU_BUTTON_HEIGHT: f32 = 58.0;
const STAGE_COUNT: usize = 6;
const HERO_MAX_HEALTH: f32 = 115.0;
const HERO_DAMAGE: f32 = 48.0;
const HERO_TOWER_DAMAGE_MULTIPLIER: f32 = 1.8;
const HERO_RANGE: f32 = 520.0;
const HERO_MOVE_SPEED: f32 = 112.0;
const HERO_ATTACK_COOLDOWN: f32 = 0.92;
const HERO_MAX_PIERCED_TARGETS: usize = 4;
const HERO_SIZE: Vec2 = Vec2::new(31.0, 49.0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Action {
    RecruitRunner,
    RecruitGuard,
    RecruitArcher,
    RecruitBrute,
    CastSpell,
    Upgrade,
    ToggleSkills,
    Confirm,
    Click,
    Restart,
    Exit,
    HeroAdvance,
    HeroRetreat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Faction {
    Player,
    Enemy,
}

impl Faction {
    fn direction(self) -> f32 {
        match self {
            Self::Player => 1.0,
            Self::Enemy => -1.0,
        }
    }

    fn opposite(self) -> Self {
        match self {
            Self::Player => Self::Enemy,
            Self::Enemy => Self::Player,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnitKind {
    Runner,
    Guard,
    Archer,
    Brute,
}

impl UnitKind {
    const ALL: [Self; 4] = [Self::Runner, Self::Guard, Self::Archer, Self::Brute];

    fn stats(self) -> UnitStats {
        match self {
            Self::Runner => UnitStats {
                cost: 30.0,
                bounty: 12.0,
                health: 72.0,
                damage: 10.0,
                speed: 72.0,
                range: 27.0,
                cooldown: 0.62,
                size: Vec2::new(25.0, 38.0),
            },
            Self::Guard => UnitStats {
                cost: 50.0,
                bounty: 20.0,
                health: 155.0,
                damage: 21.0,
                speed: 43.0,
                range: 31.0,
                cooldown: 0.92,
                size: Vec2::new(31.0, 48.0),
            },
            Self::Archer => UnitStats {
                cost: 65.0,
                bounty: 26.0,
                health: 92.0,
                damage: 17.0,
                speed: 38.0,
                range: 155.0,
                cooldown: 1.15,
                size: Vec2::new(27.0, 43.0),
            },
            Self::Brute => UnitStats {
                cost: 100.0,
                bounty: 42.0,
                health: 310.0,
                damage: 39.0,
                speed: 27.0,
                range: 39.0,
                cooldown: 1.42,
                size: Vec2::new(45.0, 61.0),
            },
        }
    }

    fn color(self, faction: Faction) -> Color {
        match (self, faction) {
            (Self::Runner, Faction::Player) => Color::from_rgba8(117, 214, 88, 255),
            (Self::Guard, Faction::Player) => Color::from_rgba8(66, 160, 91, 255),
            (Self::Archer, Faction::Player) => Color::from_rgba8(106, 194, 164, 255),
            (Self::Brute, Faction::Player) => Color::from_rgba8(64, 118, 61, 255),
            (Self::Runner, Faction::Enemy) => Color::from_rgba8(235, 119, 77, 255),
            (Self::Guard, Faction::Enemy) => Color::from_rgba8(194, 72, 58, 255),
            (Self::Archer, Faction::Enemy) => Color::from_rgba8(220, 135, 75, 255),
            (Self::Brute, Faction::Enemy) => Color::from_rgba8(137, 55, 52, 255),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct UnitStats {
    cost: f32,
    bounty: f32,
    health: f32,
    damage: f32,
    speed: f32,
    range: f32,
    cooldown: f32,
    size: Vec2,
}

struct Unit {
    kind: UnitKind,
    faction: Faction,
    pos: Vec2,
    health: f32,
    max_health: f32,
    attack_cooldown: f32,
    hit_flash: f32,
    momentum_stacks: u8,
    momentum_timer: f32,
    attacks_until_piercing: u8,
}

impl Unit {
    fn new(kind: UnitKind, faction: Faction, pos: Vec2, level: u8) -> Self {
        let stats: UnitStats = kind.stats();
        let max_health: f32 = stats.health * level_multiplier(level);
        Self {
            kind,
            faction,
            pos,
            health: max_health,
            max_health,
            attack_cooldown: 0.15,
            hit_flash: 0.0,
            momentum_stacks: 0,
            momentum_timer: 0.0,
            attacks_until_piercing: 0,
        }
    }

    fn alive(&self) -> bool {
        self.health > 0.0
    }
}

#[derive(Debug, Clone, Copy)]
struct Castle {
    health: f32,
    max_health: f32,
    level: u8,
    attack_cooldown: f32,
}

#[derive(Debug, Clone, Copy)]
struct Hero {
    pos: Vec2,
    health: f32,
    max_health: f32,
    attack_cooldown: f32,
    hit_flash: f32,
    aim_style: u8,
}

impl Hero {
    fn new() -> Self {
        Self {
            pos: Vec2::new(158.0, 400.0),
            health: HERO_MAX_HEALTH,
            max_health: HERO_MAX_HEALTH,
            attack_cooldown: 0.0,
            hit_flash: 0.0,
            aim_style: 0,
        }
    }

    fn alive(&self) -> bool {
        self.health > 0.0
    }
}

impl Castle {
    fn new() -> Self {
        Self {
            health: CASTLE_MAX_HEALTH,
            max_health: CASTLE_MAX_HEALTH,
            level: 0,
            attack_cooldown: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BattleState {
    Playing,
    Victory,
    Defeat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScreenState {
    Menu,
    StageSelect,
    Battle,
}

struct AttackEffect {
    start: Vec2,
    end: Vec2,
    life: f32,
    max_life: f32,
    color: Color,
    heavy: bool,
}

#[derive(Clone, Copy)]
enum AttackTarget {
    Unit(usize),
    Hero,
    Castle(Faction),
}

struct AttackIntent {
    attacker: usize,
    target: AttackTarget,
}

pub(crate) struct UnitesWar {
    units: Vec<Unit>,
    player_castle: Castle,
    enemy_castle: Castle,
    hero: Hero,
    player_recruit_queue: VecDeque<UnitKind>,
    player_recruit_timer: f32,
    player_coins: f32,
    enemy_coins: f32,
    spell_cooldown: f32,
    enemy_think_timer: f32,
    battle_time: f32,
    effects: Vec<AttackEffect>,
    player_passives: PassiveUnlocks,
    skills_panel_open: bool,
    screen: ScreenState,
    state: BattleState,
    rng: u32,
}

impl UnitesWar {
    pub(crate) fn new() -> Self {
        Self {
            units: Vec::new(),
            player_castle: Castle::new(),
            enemy_castle: Castle::new(),
            hero: Hero::new(),
            player_recruit_queue: VecDeque::new(),
            player_recruit_timer: 0.0,
            player_coins: STARTING_COINS,
            enemy_coins: STARTING_COINS,
            spell_cooldown: 0.0,
            enemy_think_timer: 1.2,
            battle_time: 0.0,
            effects: Vec::new(),
            player_passives: PassiveUnlocks::default(),
            skills_panel_open: false,
            screen: ScreenState::Menu,
            state: BattleState::Playing,
            rng: 0xC1A4_5EED,
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
        self.screen = ScreenState::Battle;
    }

    fn start_battle(&mut self) {
        self.reset();
    }

    fn open_stage_select(&mut self) {
        self.screen = ScreenState::StageSelect;
    }

    fn ground_y(height: f32) -> f32 {
        (height - TOOLBAR_HEIGHT - 42.0).max(260.0)
    }

    fn castle_x(faction: Faction, width: f32) -> f32 {
        match faction {
            Faction::Player => 70.0,
            Faction::Enemy => width - 70.0,
        }
    }

    fn spawn_x(faction: Faction, width: f32) -> f32 {
        match faction {
            Faction::Player => 132.0,
            Faction::Enemy => width - 132.0,
        }
    }

    fn unit_y(kind: UnitKind, height: f32) -> f32 {
        Self::ground_y(height) - kind.stats().size.y * 0.5
    }

    fn next_random(&mut self) -> u32 {
        self.rng = self.rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.rng
    }

    fn spawn_unit(&mut self, faction: Faction, kind: UnitKind, width: f32, height: f32) -> bool {
        if self.state != BattleState::Playing || self.units.len() >= MAX_UNITS {
            return false;
        }

        let level: u8 = match faction {
            Faction::Player => self.player_castle.level,
            Faction::Enemy => self.enemy_castle.level,
        };
        self.units.push(Unit::new(
            kind,
            faction,
            Vec2::new(Self::spawn_x(faction, width), Self::unit_y(kind, height)),
            level,
        ));
        true
    }

    fn recruit(&mut self, faction: Faction, kind: UnitKind, width: f32, height: f32) -> bool {
        if self.state != BattleState::Playing || self.units.len() >= MAX_UNITS {
            return false;
        }

        let cost: f32 = kind.stats().cost;
        let coins: &mut f32 = match faction {
            Faction::Player => &mut self.player_coins,
            Faction::Enemy => &mut self.enemy_coins,
        };

        if *coins < cost {
            return false;
        }

        *coins -= cost;
        self.spawn_unit(faction, kind, width, height)
    }

    fn enqueue_player_recruit(&mut self, kind: UnitKind) -> bool {
        if self.state != BattleState::Playing
            || self.units.len() + self.player_recruit_queue.len() >= MAX_UNITS
            || self.player_coins < kind.stats().cost
        {
            return false;
        }

        self.player_coins -= kind.stats().cost;
        let queue_was_empty: bool = self.player_recruit_queue.is_empty();
        self.player_recruit_queue.push_back(kind);
        if queue_was_empty {
            self.player_recruit_timer = PLAYER_RECRUIT_INTERVAL;
        }
        true
    }

    fn update_player_recruit_queue(&mut self, dt: f32, width: f32, height: f32) {
        if self.player_recruit_queue.is_empty() {
            self.player_recruit_timer = 0.0;
            return;
        }

        self.player_recruit_timer = (self.player_recruit_timer - dt).max(0.0);
        if self.player_recruit_timer > 0.0 || self.units.len() >= MAX_UNITS {
            return;
        }

        let Some(kind) = self.player_recruit_queue.pop_front() else {
            return;
        };
        if self.spawn_unit(Faction::Player, kind, width, height) {
            self.player_recruit_timer = if self.player_recruit_queue.is_empty() {
                0.0
            } else {
                PLAYER_RECRUIT_INTERVAL
            };
        } else {
            self.player_recruit_queue.push_front(kind);
        }
    }

    fn upgrade_cost(level: u8) -> f32 {
        130.0 + level as f32 * 90.0
    }

    fn upgrade(&mut self, faction: Faction) -> bool {
        let (castle, coins): (&mut Castle, &mut f32) = match faction {
            Faction::Player => (&mut self.player_castle, &mut self.player_coins),
            Faction::Enemy => (&mut self.enemy_castle, &mut self.enemy_coins),
        };

        if castle.level >= 3 {
            return false;
        }

        let cost: f32 = Self::upgrade_cost(castle.level);
        if *coins < cost {
            return false;
        }

        *coins -= cost;
        castle.level += 1;
        castle.max_health += 125.0;
        castle.health = (castle.health + 180.0).min(castle.max_health);
        true
    }

    fn cast_spell(&mut self, target: Vec2, width: f32, height: f32) -> bool {
        if self.state != BattleState::Playing || self.spell_cooldown > 0.0 {
            return false;
        }

        if target.y >= height - TOOLBAR_HEIGHT || target.x < 0.0 || target.x > width {
            return false;
        }

        self.spell_cooldown = SPELL_COOLDOWN;
        let targets: Vec<usize> = self
            .units
            .iter()
            .enumerate()
            .filter(|(_, unit)| {
                unit.faction == Faction::Enemy && unit.alive() && (unit.pos - target).length() <= SPELL_RADIUS
            })
            .map(|(index, _)| index)
            .collect();
        let damage: f32 = SPELL_DAMAGE * level_multiplier(self.player_castle.level);
        for index in targets {
            self.apply_damage(index, damage, 0.18);
        }

        for offset in [-24.0_f32, 0.0, 22.0] {
            self.effects.push(AttackEffect {
                start: Vec2::new(target.x + offset * 0.3, 0.0),
                end: Vec2::new(target.x + offset, target.y),
                life: 0.38,
                max_life: 0.38,
                color: Color::from_rgba8(235, 238, 109, 255),
                heavy: true,
            });
        }
        true
    }

    fn button_rect(index: usize, width: f32, height: f32) -> (Vec2, Vec2) {
        let count: f32 = 5.0;
        let total: f32 = count * BUTTON_SIZE + (count - 1.0) * BUTTON_GAP;
        let x: f32 = (width - total) * 0.5 + index as f32 * (BUTTON_SIZE + BUTTON_GAP);
        let y: f32 = height - TOOLBAR_HEIGHT + (TOOLBAR_HEIGHT - BUTTON_SIZE) * 0.5;
        (Vec2::new(x, y), Vec2::splat(BUTTON_SIZE))
    }

    fn menu_button_rect(index: usize, width: f32, height: f32) -> (Vec2, Vec2) {
        let x: f32 = (width - MENU_BUTTON_WIDTH) * 0.5;
        let y: f32 = height * 0.54 + index as f32 * (MENU_BUTTON_HEIGHT + 18.0);
        (Vec2::new(x, y), Vec2::new(MENU_BUTTON_WIDTH, MENU_BUTTON_HEIGHT))
    }

    fn stage_select_panel_rect(width: f32, height: f32) -> (Vec2, Vec2) {
        let size: Vec2 = Vec2::new((width - 36.0).min(1060.0), (height - 34.0).min(690.0));
        (Vec2::new((width - size.x) * 0.5, (height - size.y) * 0.46), size)
    }

    fn stage_card_rect(index: usize, width: f32, height: f32) -> (Vec2, Vec2) {
        let (panel_pos, panel_size): (Vec2, Vec2) = Self::stage_select_panel_rect(width, height);
        let columns: usize = if panel_size.x >= 710.0 { 3 } else { 2 };
        let rows: usize = STAGE_COUNT.div_ceil(columns);
        let gap: f32 = 16.0;
        let grid_pos: Vec2 = panel_pos + Vec2::new(24.0, 105.0);
        let grid_size: Vec2 = Vec2::new(panel_size.x - 48.0, panel_size.y - 181.0);
        let card_size: Vec2 = Vec2::new(
            (grid_size.x - gap * (columns - 1) as f32) / columns as f32,
            (grid_size.y - gap * (rows - 1) as f32) / rows as f32,
        );
        let column: f32 = (index % columns) as f32;
        let row: f32 = (index / columns) as f32;
        (
            grid_pos + Vec2::new(column * (card_size.x + gap), row * (card_size.y + gap)),
            card_size,
        )
    }

    fn stage_back_button_rect(width: f32, height: f32) -> (Vec2, Vec2) {
        let (panel_pos, panel_size): (Vec2, Vec2) = Self::stage_select_panel_rect(width, height);
        let size: Vec2 = Vec2::new(190.0_f32.min(panel_size.x - 48.0), 43.0);
        (
            Vec2::new(panel_pos.x + (panel_size.x - size.x) * 0.5, panel_pos.y + panel_size.y - 58.0),
            size,
        )
    }

    fn skills_button_rect(width: f32) -> (Vec2, Vec2) {
        let size: Vec2 = Vec2::new(180.0, 39.0);
        (Vec2::new((width - size.x) * 0.5, 16.0), size)
    }

    fn skills_panel_rect(width: f32, height: f32) -> (Vec2, Vec2) {
        let size: Vec2 = Vec2::new((width * 0.68).clamp(430.0, 820.0), (height * 0.7).clamp(390.0, 570.0));
        (Vec2::new((width - size.x) * 0.5, (height - size.y) * 0.45), size)
    }

    fn skills_close_button_rect(width: f32, height: f32) -> (Vec2, Vec2) {
        let (panel_pos, panel_size): (Vec2, Vec2) = Self::skills_panel_rect(width, height);
        let size: Vec2 = Vec2::new(170.0, 44.0);
        (
            Vec2::new(
                panel_pos.x + (panel_size.x - size.x) * 0.5,
                panel_pos.y + panel_size.y - size.y - 23.0,
            ),
            size,
        )
    }

    fn passive_slot_rect(index: usize, width: f32, height: f32) -> (Vec2, Vec2) {
        let (panel_pos, panel_size): (Vec2, Vec2) = Self::skills_panel_rect(width, height);
        let (close_pos, _): (Vec2, Vec2) = Self::skills_close_button_rect(width, height);
        let gap: f32 = 14.0;
        let grid_pos: Vec2 = panel_pos + Vec2::new(28.0, 96.0);
        let grid_size: Vec2 = Vec2::new(panel_size.x - 56.0, close_pos.y - 20.0 - grid_pos.y);
        let slot_size: Vec2 = Vec2::new((grid_size.x - gap) * 0.5, (grid_size.y - gap) * 0.5);
        let column: f32 = (index % 2) as f32;
        let row: f32 = (index / 2) as f32;
        (
            grid_pos + Vec2::new(column * (slot_size.x + gap), row * (slot_size.y + gap)),
            slot_size,
        )
    }

    fn toggle_skills_panel(&mut self) {
        if self.screen == ScreenState::Battle && self.state == BattleState::Playing {
            self.skills_panel_open = !self.skills_panel_open;
        }
    }

    fn point_in_rect(point: Vec2, pos: Vec2, size: Vec2) -> bool {
        point.x >= pos.x && point.x <= pos.x + size.x && point.y >= pos.y && point.y <= pos.y + size.y
    }

    fn handle_click(&mut self, mouse: Vec2, width: f32, height: f32) {
        if self.state != BattleState::Playing {
            self.reset();
            return;
        }

        for (index, kind) in UnitKind::ALL.into_iter().enumerate() {
            let (pos, size): (Vec2, Vec2) = Self::button_rect(index, width, height);
            if Self::point_in_rect(mouse, pos, size) {
                self.enqueue_player_recruit(kind);
                return;
            }
        }

        let (pos, size): (Vec2, Vec2) = Self::button_rect(4, width, height);
        if Self::point_in_rect(mouse, pos, size) {
            self.upgrade(Faction::Player);
            return;
        }

        self.cast_spell(mouse, width, height);
    }

    fn update_hero_movement(&mut self, direction: f32, dt: f32, width: f32, height: f32) {
        self.hero.pos.y = Self::ground_y(height) - HERO_SIZE.y * 0.5;
        if !self.hero.alive() {
            return;
        }

        let min_x: f32 = Self::castle_x(Faction::Player, width) + CASTLE_WIDTH * 0.62;
        let max_x: f32 = Self::castle_x(Faction::Enemy, width) - CASTLE_WIDTH * 0.72;
        self.hero.pos.x =
            (self.hero.pos.x + direction.clamp(-1.0, 1.0) * HERO_MOVE_SPEED * dt).clamp(min_x, max_x.max(min_x));
    }

    fn hero_targets(&self) -> Vec<usize> {
        let mut targets: Vec<(usize, f32)> = self
            .units
            .iter()
            .enumerate()
            .filter(|(_, unit)| unit.faction == Faction::Enemy && unit.alive())
            .filter_map(|(index, unit)| {
                let distance: f32 = unit.pos.x - self.hero.pos.x;
                (0.0..=HERO_RANGE).contains(&distance).then_some((index, distance))
            })
            .collect();
        targets.sort_by(|a, b| a.1.total_cmp(&b.1));
        targets
            .into_iter()
            .take(HERO_MAX_PIERCED_TARGETS)
            .map(|(index, _)| index)
            .collect()
    }

    fn hero_aim_target(&self, width: f32) -> Vec2 {
        self.hero_targets()
            .first()
            .map(|index| self.units[*index].pos)
            .unwrap_or_else(|| {
                let castle_x: f32 = Self::castle_x(Faction::Enemy, width);
                let target_x: f32 = castle_x.min(self.hero.pos.x + HERO_RANGE);
                let target_y: f32 = if target_x == castle_x {
                    self.hero.pos.y - 55.0
                } else {
                    self.hero.pos.y
                };
                Vec2::new(target_x, target_y)
            })
    }

    fn fire_hero(&mut self, width: f32) -> bool {
        if self.state != BattleState::Playing || !self.hero.alive() || self.hero.attack_cooldown > 0.0 {
            return false;
        }

        let targets: Vec<usize> = self.hero_targets();
        let enemy_castle_x: f32 = Self::castle_x(Faction::Enemy, width);
        let castle_in_range: bool = enemy_castle_x >= self.hero.pos.x && enemy_castle_x - self.hero.pos.x <= HERO_RANGE;
        if targets.is_empty() && !castle_in_range {
            return false;
        }

        let mut end: Vec2 = Vec2::new((self.hero.pos.x + HERO_RANGE).min(width), self.hero.pos.y);
        for (pierce_index, target_index) in targets.into_iter().enumerate() {
            let target_pos: Vec2 = self.units[target_index].pos;
            let damage_falloff: f32 = 1.0 - pierce_index as f32 * 0.08;
            self.apply_damage(target_index, HERO_DAMAGE * damage_falloff, 0.14);
            end = target_pos;
        }

        if castle_in_range {
            self.enemy_castle.health -= HERO_DAMAGE * HERO_TOWER_DAMAGE_MULTIPLIER;
            end = Vec2::new(enemy_castle_x, self.hero.pos.y - 35.0);
        }

        let arrow_color: Color = match self.hero.aim_style {
            0 => Color::from_rgba8(205, 133, 255, 255),
            1 => Color::from_rgba8(108, 224, 210, 255),
            _ => Color::from_rgba8(248, 194, 78, 255),
        };
        self.effects.push(AttackEffect {
            start: self.hero.pos + Vec2::new(15.0, -7.0),
            end,
            life: 0.32,
            max_life: 0.32,
            color: arrow_color,
            heavy: false,
        });
        self.hero.attack_cooldown = HERO_ATTACK_COOLDOWN;
        self.hero.aim_style = (self.hero.aim_style + 1) % 3;
        true
    }

    fn apply_hero_damage(&mut self, damage: f32, flash_time: f32) -> f32 {
        if !self.hero.alive() {
            return 0.0;
        }
        let applied: f32 = damage.min(self.hero.health);
        self.hero.health = (self.hero.health - damage).max(0.0);
        self.hero.hit_flash = self.hero.hit_flash.max(flash_time);
        applied
    }

    fn update_enemy_ai(&mut self, dt: f32, width: f32, height: f32) {
        self.enemy_think_timer -= dt;
        if self.enemy_think_timer > 0.0 {
            return;
        }

        let roll: u32 = self.next_random();
        self.enemy_think_timer = 0.72 + (roll % 90) as f32 / 100.0;

        if self.enemy_castle.level < 3
            && self.enemy_coins >= Self::upgrade_cost(self.enemy_castle.level)
            && (roll >> 8).is_multiple_of(6)
        {
            self.upgrade(Faction::Enemy);
            return;
        }

        let kind: UnitKind = match (roll >> 12) % 10 {
            0..=3 => UnitKind::Runner,
            4..=6 => UnitKind::Guard,
            7..=8 => UnitKind::Archer,
            _ => UnitKind::Brute,
        };
        self.recruit(Faction::Enemy, kind, width, height);
    }

    fn update_units(&mut self, dt: f32, width: f32) {
        self.update_passive_states(dt);
        for unit in &mut self.units {
            unit.attack_cooldown = (unit.attack_cooldown - dt).max(0.0);
            unit.hit_flash = (unit.hit_flash - dt).max(0.0);
        }

        let snapshot: Vec<(Faction, Vec2, Vec2, bool)> = self
            .units
            .iter()
            .map(|unit: &Unit| (unit.faction, unit.pos, unit.kind.stats().size, unit.alive()))
            .collect();
        let mut attacks: Vec<AttackIntent> = Vec::new();

        for index in 0..self.units.len() {
            if !self.units[index].alive() {
                continue;
            }

            let faction: Faction = self.units[index].faction;
            let pos: Vec2 = self.units[index].pos;
            let stats: UnitStats = self.units[index].kind.stats();
            let mut nearest: Option<(AttackTarget, f32, f32)> = snapshot
                .iter()
                .enumerate()
                .filter(|(_, (other_faction, _, _, alive))| *alive && *other_faction == faction.opposite())
                .map(|(other_index, (_, other_pos, other_size, _))| {
                    (AttackTarget::Unit(other_index), (other_pos.x - pos.x).abs(), other_size.x)
                })
                .min_by(|a, b| a.1.total_cmp(&b.1));

            if faction == Faction::Enemy && self.hero.alive() {
                let hero_distance: f32 = (self.hero.pos.x - pos.x).abs();
                if nearest.is_none_or(|(_, distance, _)| hero_distance < distance) {
                    nearest = Some((AttackTarget::Hero, hero_distance, HERO_SIZE.x));
                }
            }

            if let Some((target, distance, target_width)) = nearest
                && distance <= stats.range + target_width * 0.5
            {
                if self.units[index].attack_cooldown <= 0.0 {
                    attacks.push(AttackIntent {
                        attacker: index,
                        target,
                    });
                }
                continue;
            }

            let enemy_castle_x: f32 = Self::castle_x(faction.opposite(), width);
            if (enemy_castle_x - pos.x).abs() <= stats.range + CASTLE_WIDTH * 0.5 {
                if self.units[index].attack_cooldown <= 0.0 {
                    attacks.push(AttackIntent {
                        attacker: index,
                        target: AttackTarget::Castle(faction.opposite()),
                    });
                }
                continue;
            }

            let direction: f32 = faction.direction();
            let blocked: bool =
                snapshot
                    .iter()
                    .enumerate()
                    .any(|(other_index, (other_faction, other_pos, other_size, alive))| {
                        if other_index == index || !*alive || *other_faction != faction {
                            return false;
                        }
                        let ahead: f32 = (other_pos.x - pos.x) * direction;
                        ahead > 0.0 && ahead < (stats.size.x + other_size.x) * 0.58
                    });

            if !blocked {
                self.units[index].pos.x += direction * stats.speed * dt;
            }
        }

        for intent in attacks {
            if intent.attacker >= self.units.len() || !self.units[intent.attacker].alive() {
                continue;
            }

            let attacker_pos: Vec2 = self.units[intent.attacker].pos;
            let kind: UnitKind = self.units[intent.attacker].kind;
            let faction: Faction = self.units[intent.attacker].faction;
            let stats: UnitStats = kind.stats();
            let level: u8 = match faction {
                Faction::Player => self.player_castle.level,
                Faction::Enemy => self.enemy_castle.level,
            };
            let modifiers: AttackModifiers = self.prepare_attack(
                intent.attacker,
                stats.damage * level_multiplier(level),
                stats.cooldown,
                matches!(intent.target, AttackTarget::Unit(_) | AttackTarget::Hero),
            );
            self.units[intent.attacker].attack_cooldown = modifiers.cooldown;

            let end: Vec2 = match intent.target {
                AttackTarget::Unit(target_index) => {
                    if target_index >= self.units.len()
                        || !self.units[target_index].alive()
                        || self.units[target_index].faction == faction
                    {
                        continue;
                    }
                    let end: Vec2 = self.units[target_index].pos;
                    self.apply_damage(target_index, modifiers.damage, 0.11);

                    if let Some(piercing_damage) = modifiers.piercing_damage
                        && let Some(pierced_index) = self.piercing_target(intent.attacker, target_index)
                    {
                        let pierced_end: Vec2 = self.units[pierced_index].pos;
                        self.apply_damage(pierced_index, piercing_damage, 0.11);
                        self.effects.push(AttackEffect {
                            start: end,
                            end: pierced_end,
                            life: 0.2,
                            max_life: 0.2,
                            color: Color::from_rgba8(135, 238, 199, 255),
                            heavy: false,
                        });
                    }
                    end
                }
                AttackTarget::Hero => {
                    if faction != Faction::Enemy || !self.hero.alive() {
                        continue;
                    }
                    let end: Vec2 = self.hero.pos;
                    self.apply_hero_damage(modifiers.damage, 0.11);
                    end
                }
                AttackTarget::Castle(target_faction) => {
                    let castle: &mut Castle = match target_faction {
                        Faction::Player => &mut self.player_castle,
                        Faction::Enemy => &mut self.enemy_castle,
                    };
                    castle.health -= modifiers.damage;
                    Vec2::new(Self::castle_x(target_faction, width), attacker_pos.y)
                }
            };

            self.effects.push(AttackEffect {
                start: attacker_pos,
                end,
                life: if kind == UnitKind::Archer { 0.24 } else { 0.1 },
                max_life: if kind == UnitKind::Archer { 0.24 } else { 0.1 },
                color: if kind == UnitKind::Archer {
                    Color::from_rgba8(247, 218, 122, 255)
                } else {
                    Color::WHITE
                },
                heavy: kind == UnitKind::Brute,
            });
        }
    }

    fn collect_defeated_units(&mut self) -> (f32, f32) {
        let mut player_reward: f32 = 0.0;
        let mut enemy_reward: f32 = 0.0;

        for unit in self.units.iter().filter(|unit: &&Unit| !unit.alive()) {
            match unit.faction {
                Faction::Enemy => player_reward += unit.kind.stats().bounty,
                Faction::Player => enemy_reward += unit.kind.stats().bounty,
            }
        }

        self.player_coins = (self.player_coins + player_reward).min(MAX_COINS);
        self.enemy_coins = (self.enemy_coins + enemy_reward).min(MAX_COINS);
        self.units.retain(Unit::alive);
        (player_reward, enemy_reward)
    }

    fn update_economy(&mut self, dt: f32) {
        self.player_coins = (self.player_coins + PASSIVE_COIN_RATE * dt).min(MAX_COINS);
        self.enemy_coins = (self.enemy_coins + ENEMY_PASSIVE_COIN_RATE * dt).min(MAX_COINS);
    }

    fn update_castle_attack(&mut self, faction: Faction, dt: f32, width: f32) {
        {
            let castle: &mut Castle = match faction {
                Faction::Player => &mut self.player_castle,
                Faction::Enemy => &mut self.enemy_castle,
            };
            castle.attack_cooldown = (castle.attack_cooldown - dt).max(0.0);
            if castle.attack_cooldown > 0.0 {
                return;
            }
        }

        let castle_x: f32 = Self::castle_x(faction, width);
        let mut target: Option<(AttackTarget, Vec2, f32)> = self
            .units
            .iter()
            .enumerate()
            .filter(|(_, unit)| unit.faction == faction.opposite() && unit.alive())
            .filter(|(_, unit)| (unit.pos.x - castle_x).abs() <= CASTLE_ATTACK_RANGE)
            .map(|(index, unit)| (AttackTarget::Unit(index), unit.pos, (unit.pos.x - castle_x).abs()))
            .min_by(|a, b| a.2.total_cmp(&b.2));

        if faction == Faction::Enemy && self.hero.alive() {
            let hero_distance: f32 = (self.hero.pos.x - castle_x).abs();
            if hero_distance <= CASTLE_ATTACK_RANGE
                && target.is_none_or(|(_, _, target_distance)| hero_distance < target_distance)
            {
                target = Some((AttackTarget::Hero, self.hero.pos, hero_distance));
            }
        }

        if let Some((target, end, _)) = target {
            let castle_level: u8 = match faction {
                Faction::Player => self.player_castle.level,
                Faction::Enemy => self.enemy_castle.level,
            };
            let damage: f32 = 13.0 + castle_level as f32 * 6.0;
            match target {
                AttackTarget::Unit(index) => {
                    self.apply_damage(index, damage, 0.1);
                }
                AttackTarget::Hero => {
                    self.apply_hero_damage(damage, 0.1);
                }
                AttackTarget::Castle(_) => {}
            }
            let castle: &mut Castle = match faction {
                Faction::Player => &mut self.player_castle,
                Faction::Enemy => &mut self.enemy_castle,
            };
            castle.attack_cooldown = (1.25 - castle_level as f32 * 0.12).max(0.72);
            self.effects.push(AttackEffect {
                start: Vec2::new(castle_x, end.y - 75.0),
                end,
                life: 0.28,
                max_life: 0.28,
                color: Color::from_rgba8(255, 226, 123, 255),
                heavy: false,
            });
        }
    }

    fn update_effects(&mut self, dt: f32) {
        for effect in &mut self.effects {
            effect.life -= dt;
        }
        self.effects.retain(|effect: &AttackEffect| effect.life > 0.0);
    }

    fn update_battle(&mut self, dt: f32, width: f32, height: f32) {
        self.battle_time += dt;
        self.update_economy(dt);
        self.spell_cooldown = (self.spell_cooldown - dt).max(0.0);
        self.hero.attack_cooldown = (self.hero.attack_cooldown - dt).max(0.0);
        self.hero.hit_flash = (self.hero.hit_flash - dt).max(0.0);

        self.update_player_recruit_queue(dt, width, height);
        self.update_enemy_ai(dt, width, height);
        self.update_units(dt, width);
        self.update_castle_attack(Faction::Player, dt, width);
        self.update_castle_attack(Faction::Enemy, dt, width);
        self.collect_defeated_units();
        self.update_effects(dt);

        if self.enemy_castle.health <= 0.0 {
            self.enemy_castle.health = 0.0;
            self.state = BattleState::Victory;
        } else if self.player_castle.health <= 0.0 {
            self.player_castle.health = 0.0;
            self.state = BattleState::Defeat;
        }
    }
}

fn level_multiplier(level: u8) -> f32 {
    1.0 + level as f32 * 0.13
}

impl Game for UnitesWar {
    type Action = Action;

    fn on_start(&mut self, ctx: &mut dyn GameContext<Self::Action>) {
        ctx.input_mut().bind(Action::RecruitRunner, KeyCode::Digit1.into());
        ctx.input_mut().bind(Action::RecruitGuard, KeyCode::Digit2.into());
        ctx.input_mut().bind(Action::RecruitArcher, KeyCode::Digit3.into());
        ctx.input_mut().bind(Action::RecruitBrute, KeyCode::Digit4.into());
        ctx.input_mut().bind(Action::CastSpell, KeyCode::KeyQ.into());
        ctx.input_mut().bind(Action::Upgrade, KeyCode::KeyU.into());
        ctx.input_mut().bind(Action::ToggleSkills, KeyCode::KeyH.into());
        ctx.input_mut().bind(Action::Confirm, KeyCode::Enter.into());
        ctx.input_mut().bind(Action::Confirm, KeyCode::NumpadEnter.into());
        ctx.input_mut().bind(Action::Restart, KeyCode::KeyR.into());
        ctx.input_mut().bind(Action::Exit, KeyCode::Escape.into());
        ctx.input_mut().bind(Action::Click, MouseButton::Left.into());
        ctx.input_mut().bind(Action::HeroAdvance, KeyCode::KeyD.into());
        ctx.input_mut().bind(Action::HeroAdvance, KeyCode::ArrowRight.into());
        ctx.input_mut().bind(Action::HeroRetreat, KeyCode::KeyA.into());
        ctx.input_mut().bind(Action::HeroRetreat, KeyCode::ArrowLeft.into());
        log::info!("Unites War: A/D hero | auto hero attack | 1-4 recruit | Q lightning | U upgrade | H skills");
    }

    fn on_update(&mut self, ctx: &mut dyn GameContext<Self::Action>) {
        let width: f32 = ctx.surface_width() as f32;
        let height: f32 = ctx.surface_height() as f32;
        let dt: f32 = (ctx.delta_time() as f32).min(0.05);

        if self.screen == ScreenState::Menu {
            if ctx.input().just_pressed(Action::Exit) {
                ctx.exit();
                return;
            }
            if ctx.input().just_pressed(Action::Confirm) {
                self.open_stage_select();
                return;
            }
            if ctx.input().just_pressed(Action::Click)
                && let Some(mouse) = ctx.input().mouse_position()
            {
                let (play_pos, play_size): (Vec2, Vec2) = Self::menu_button_rect(0, width, height);
                let (exit_pos, exit_size): (Vec2, Vec2) = Self::menu_button_rect(1, width, height);
                if Self::point_in_rect(mouse, play_pos, play_size) {
                    self.open_stage_select();
                } else if Self::point_in_rect(mouse, exit_pos, exit_size) {
                    ctx.exit();
                }
            }
            return;
        }

        if self.screen == ScreenState::StageSelect {
            if ctx.input().just_pressed(Action::Exit) {
                self.screen = ScreenState::Menu;
                return;
            }
            if ctx.input().just_pressed(Action::Confirm) {
                self.start_battle();
                return;
            }
            if ctx.input().just_pressed(Action::Click)
                && let Some(mouse) = ctx.input().mouse_position()
            {
                let (first_stage_pos, first_stage_size): (Vec2, Vec2) = Self::stage_card_rect(0, width, height);
                let (back_pos, back_size): (Vec2, Vec2) = Self::stage_back_button_rect(width, height);
                if Self::point_in_rect(mouse, first_stage_pos, first_stage_size) {
                    self.start_battle();
                } else if Self::point_in_rect(mouse, back_pos, back_size) {
                    self.screen = ScreenState::Menu;
                }
            }
            return;
        }

        if self.skills_panel_open {
            if ctx.input().just_pressed(Action::ToggleSkills) || ctx.input().just_pressed(Action::Exit) {
                self.skills_panel_open = false;
                return;
            }
            if ctx.input().just_pressed(Action::Click)
                && let Some(mouse) = ctx.input().mouse_position()
            {
                let (close_pos, close_size): (Vec2, Vec2) = Self::skills_close_button_rect(width, height);
                if Self::point_in_rect(mouse, close_pos, close_size) {
                    self.skills_panel_open = false;
                    return;
                }

                for (index, passive) in PassiveKind::ALL.into_iter().enumerate() {
                    let (slot_pos, slot_size): (Vec2, Vec2) = Self::passive_slot_rect(index, width, height);
                    if Self::point_in_rect(mouse, slot_pos, slot_size) {
                        self.buy_passive(passive);
                        return;
                    }
                }
            }
            return;
        }

        if self.state == BattleState::Playing && ctx.input().just_pressed(Action::ToggleSkills) {
            self.toggle_skills_panel();
            return;
        }

        if ctx.input().just_pressed(Action::Exit) {
            ctx.exit();
            return;
        }

        if ctx.input().just_pressed(Action::Restart) {
            self.reset();
            return;
        }

        if self.state == BattleState::Playing {
            let mut hero_direction: f32 = 0.0;
            if ctx.input().held(Action::HeroAdvance) {
                hero_direction += 1.0;
            }
            if ctx.input().held(Action::HeroRetreat) {
                hero_direction -= 1.0;
            }
            self.update_hero_movement(hero_direction, dt, width, height);
            self.fire_hero(width);

            if ctx.input().just_pressed(Action::RecruitRunner) {
                self.enqueue_player_recruit(UnitKind::Runner);
            }
            if ctx.input().just_pressed(Action::RecruitGuard) {
                self.enqueue_player_recruit(UnitKind::Guard);
            }
            if ctx.input().just_pressed(Action::RecruitArcher) {
                self.enqueue_player_recruit(UnitKind::Archer);
            }
            if ctx.input().just_pressed(Action::RecruitBrute) {
                self.enqueue_player_recruit(UnitKind::Brute);
            }
            if ctx.input().just_pressed(Action::Upgrade) {
                self.upgrade(Faction::Player);
            }
            if ctx.input().just_pressed(Action::CastSpell) {
                let target: Vec2 = ctx
                    .input()
                    .mouse_position()
                    .unwrap_or(Vec2::new(width * 0.65, Self::ground_y(height) - 30.0));
                self.cast_spell(target, width, height);
            }
            if ctx.input().just_pressed(Action::Click)
                && let Some(mouse) = ctx.input().mouse_position()
            {
                let (skills_pos, skills_size): (Vec2, Vec2) = Self::skills_button_rect(width);
                if Self::point_in_rect(mouse, skills_pos, skills_size) {
                    self.toggle_skills_panel();
                    return;
                } else {
                    self.handle_click(mouse, width, height);
                }
            }

            self.update_battle(dt, width, height);
        } else {
            self.update_effects(dt);
            if ctx.input().just_pressed(Action::Click) {
                self.reset();
            }
        }
    }

    fn on_render(&mut self, ctx: &mut dyn GameContext<Self::Action>) {
        let width: f32 = ctx.surface_width() as f32;
        let height: f32 = ctx.surface_height() as f32;
        if self.screen == ScreenState::Menu {
            self.draw_menu(ctx, width, height);
            return;
        }
        if self.screen == ScreenState::StageSelect {
            self.draw_stage_select(ctx, width, height);
            return;
        }

        Self::draw_background(ctx, width, height);
        Self::draw_castle(ctx, &self.player_castle, Faction::Player, width, height);
        Self::draw_castle(ctx, &self.enemy_castle, Faction::Enemy, width, height);

        for unit in &self.units {
            self.draw_unit(ctx, unit);
        }
        self.draw_hero(ctx);
        self.draw_hero_aim(ctx, width);
        self.draw_effects(ctx);
        self.draw_hud(ctx, width, height);
        self.draw_skills_panel(ctx, width, height);
        self.draw_end_overlay(ctx, width, height);
    }
}

#[cfg(test)]
mod tests;

use std::collections::VecDeque;

use redixel::prelude::*;

mod arrows;
mod cannons;
mod tower_upgrades;
use cannons::{CannonSprites, Cannonball};
use tower_upgrades::TowerUpgrade;
#[cfg(test)]
mod character_combat_tests;
mod character_sprites;
use arrows::Arrow;
mod clan;
mod enemy_ai;
mod forest;
mod forest_sprites;
mod goblin_sprites;
mod sidebar;
use sidebar::UiIcons;
mod war_hud;
use clan::*;
mod render;

use character_sprites::ArmySprites;
use forest_sprites::ForestSprites;
use goblin_sprites::RunnerSprites;

const STARTING_COINS: f32 = 120.0;
const PASSIVE_COIN_RATE: f32 = 6.0;
const ENEMY_PASSIVE_COIN_RATE: f32 = 6.0;
const MAX_COINS: f32 = 9_999.0;
const CASTLE_MAX_HEALTH: f32 = 1_000.0;
const CASTLE_WIDTH: f32 = 92.0;
const CASTLE_ATTACK_RANGE: f32 = 235.0;
const SPELL_COOLDOWN: f32 = 15.0;
const SPELL_RADIUS: f32 = 135.0;
const SPELL_DAMAGE: f32 = 85.0;
const TOOLBAR_HEIGHT: f32 = 104.0;
const HUD_BASE_WIDTH: f32 = 960.0;
const HUD_BASE_HEIGHT: f32 = 540.0;
const HUD_MAX_SCALE: f32 = 2.0;
const MAX_UNITS: usize = 80;
const PLAYER_RECRUIT_INTERVAL: f32 = 0.85;
const MENU_BUTTON_WIDTH: f32 = 250.0;
const MENU_BUTTON_HEIGHT: f32 = 52.0;
const MENU_BUTTON_GAP: f32 = 12.0;
const HERO_MAX_HEALTH: f32 = 115.0;
const HERO_DAMAGE: f32 = 48.0;
const HERO_TOWER_DAMAGE_MULTIPLIER: f32 = 1.8;
const HERO_RANGE: f32 = 155.0;
const HERO_MOVE_SPEED: f32 = 112.0;
const HERO_ATTACK_COOLDOWN: f32 = 0.92;
const HERO_SHOT_PAUSE: f32 = 0.2;
const HERO_RETURN_COOLDOWN: f32 = 30.0;
const HERO_SIZE: Vec2 = Vec2::new(31.0, 49.0);
const RUNNER_ATTACK_ANIMATION: f32 = goblin_sprites::ATTACK_SECONDS;
const HERO_ATTACK_ANIMATION: f32 = if character_sprites::HERO_ATTACK_SECONDS < HERO_ATTACK_COOLDOWN * 0.9 {
    character_sprites::HERO_ATTACK_SECONDS
} else {
    HERO_ATTACK_COOLDOWN * 0.9
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Resolution {
    width: u32,
    height: u32,
}

const RESOLUTIONS: [Resolution; 5] = [
    Resolution {
        width: 960,
        height: 540,
    },
    Resolution {
        width: 1280,
        height: 720,
    },
    Resolution {
        width: 1366,
        height: 768,
    },
    Resolution {
        width: 1600,
        height: 900,
    },
    Resolution {
        width: 1920,
        height: 1080,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Action {
    RecruitRunner,
    RecruitGuard,
    RecruitArcher,
    RecruitBrute,
    SummonHero,
    CastSpell,
    Upgrade,
    TroopsPanel,
    UpgradesPanel,
    MagicPanel,
    HeroPanel,
    Confirm,
    Click,
    Restart,
    Exit,
    CameraLeft,
    CameraRight,
    Home,
    Pause,
    Meteor,
    Fury,
    TestRunner,
    TestMap,
    TestArmy,
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

    fn attack_animation_duration(self) -> f32 {
        character_sprites::attack_seconds(self).min(self.stats().cooldown * 0.9)
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
    id: u64,
    kind: UnitKind,
    faction: Faction,
    pos: Vec2,
    health: f32,
    max_health: f32,
    attack_cooldown: f32,
    attack_animation: f32,
    attack_duration: f32,
    pending_attack: Option<f32>,
    animation_time: f32,
    moving: bool,
    hit_flash: f32,
}

impl Unit {
    fn new(kind: UnitKind, faction: Faction, pos: Vec2, level: u8) -> Self {
        let stats: UnitStats = kind.stats();
        let max_health: f32 = stats.health * level_multiplier(level);
        Self {
            id: 0,
            kind,
            faction,
            pos,
            health: max_health,
            max_health,
            attack_cooldown: 0.15,
            attack_animation: 0.0,
            attack_duration: kind.attack_animation_duration(),
            pending_attack: None,
            animation_time: 0.0,
            moving: false,
            hit_flash: 0.0,
        }
    }

    fn alive(&self) -> bool {
        self.health > 0.0
    }

    fn animation_pose(&self) -> Vec2 {
        Vec2::new(
            if self.moving {
                (self.animation_time * 13.0).sin()
            } else {
                0.0
            },
            if self.attack_animation > 0.0 {
                (self.attack_animation / self.attack_duration * std::f32::consts::PI).sin()
            } else {
                0.0
            },
        )
    }
}

#[derive(Debug, Clone, Copy)]
struct Castle {
    health: f32,
    max_health: f32,
    upgrades: [u8; 5],
    cannon_angle: f32,
    attack_cooldown: f32,
}

#[derive(Debug, Clone, Copy)]
struct Hero {
    pos: Vec2,
    health: f32,
    max_health: f32,
    attack_cooldown: f32,
    shot_pause: f32,
    hit_flash: f32,
    aim_style: u8,
    summoned: bool,
    in_castle: bool,
    move_direction: f32,
    attack_animation: f32,
    attack_pending: bool,
    animation_time: f32,
    moving: bool,
}

impl Hero {
    fn new() -> Self {
        Self {
            pos: Vec2::new(158.0, 400.0),
            health: HERO_MAX_HEALTH,
            max_health: HERO_MAX_HEALTH,
            attack_cooldown: 0.0,
            shot_pause: 0.0,
            hit_flash: 0.0,
            aim_style: 0,
            summoned: false,
            in_castle: false,
            move_direction: 1.0,
            attack_animation: 0.0,
            attack_pending: false,
            animation_time: 0.0,
            moving: false,
        }
    }

    fn alive(&self) -> bool {
        self.summoned && !self.in_castle && self.health > 0.0
    }
}

impl Castle {
    fn new() -> Self {
        Self {
            health: CASTLE_MAX_HEALTH,
            max_health: CASTLE_MAX_HEALTH,
            upgrades: [0; 5],
            cannon_angle: -10.0_f32.to_radians(),
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
    Settings,
    StageSelect,
    Battle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum AttackEffectKind {
    ArrowImpact,
    MeleeImpact,
    HeavyImpact,
    CannonImpact { radius: f32 },
    Lightning,
    Meteor,
}

struct AttackEffect {
    start: Vec2,
    end: Vec2,
    life: f32,
    max_life: f32,
    color: Color,
    kind: AttackEffectKind,
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
    forest_sprites: Option<ForestSprites>,
    runner_sprites: Option<RunnerSprites>,
    army_sprites: Option<ArmySprites>,
    cannon_sprites: Option<CannonSprites>,
    ui_icons: Option<UiIcons>,
    clan: ClanWar,
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
    enemy_investment_timer: f32,
    effects: Vec<AttackEffect>,
    arrows: Vec<Arrow>,
    cannonballs: Vec<Cannonball>,
    next_unit_id: u64,
    screen: ScreenState,
    state: BattleState,
    rng: u32,
    selected_resolution: usize,
}

impl UnitesWar {
    pub(crate) fn new() -> Self {
        Self {
            forest_sprites: None,
            runner_sprites: None,
            army_sprites: None,
            cannon_sprites: None,
            ui_icons: None,
            clan: ClanWar::default(),
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
            enemy_investment_timer: 0.0,
            effects: Vec::new(),
            arrows: Vec::new(),
            cannonballs: Vec::new(),
            next_unit_id: 0,
            screen: ScreenState::Menu,
            state: BattleState::Playing,
            rng: 0xC1A4_5EED,
            selected_resolution: 1,
        }
    }

    fn reset(&mut self) {
        let difficulty = self.clan.difficulty;
        let selected_resolution: usize = self.selected_resolution;
        let runner_sprites = self.runner_sprites;
        let army_sprites = self.army_sprites.take();
        let forest_sprites = self.forest_sprites;
        let cannon_sprites = self.cannon_sprites;
        let ui_icons = self.ui_icons;
        *self = Self::new();
        self.forest_sprites = forest_sprites;
        self.cannon_sprites = cannon_sprites;
        self.ui_icons = ui_icons;
        self.runner_sprites = runner_sprites;
        self.army_sprites = army_sprites;
        self.clan.difficulty = difficulty;
        self.selected_resolution = selected_resolution;
        self.screen = ScreenState::Battle;
    }

    fn start_battle(&mut self) {
        self.reset();
    }

    fn open_stage_select(&mut self) {
        self.screen = ScreenState::StageSelect;
    }

    fn open_settings(&mut self) {
        self.screen = ScreenState::Settings;
    }

    fn closest_resolution(width: u32, height: u32) -> usize {
        RESOLUTIONS
            .iter()
            .enumerate()
            .min_by_key(|(_, resolution)| {
                resolution.width.abs_diff(width) as u64 + resolution.height.abs_diff(height) as u64
            })
            .map(|(index, _)| index)
            .unwrap_or(0)
    }

    fn apply_resolution(&mut self, index: usize, ctx: &mut dyn GameContext<Action>) {
        let Some(resolution) = RESOLUTIONS.get(index).copied() else {
            return;
        };
        self.selected_resolution = index;
        if ctx.is_fullscreen() {
            ctx.request_fullscreen(false);
        }
        ctx.request_surface_size(resolution.width, resolution.height);
    }

    fn apply_window_mode(&self, fullscreen: bool, ctx: &mut dyn GameContext<Action>) {
        ctx.request_fullscreen(fullscreen);
        if !fullscreen && let Some(resolution) = RESOLUTIONS.get(self.selected_resolution) {
            ctx.request_surface_size(resolution.width, resolution.height);
        }
    }

    fn ground_y(height: f32) -> f32 {
        let vertical_scale: f32 = (height / HUD_BASE_HEIGHT).clamp(1.0, HUD_MAX_SCALE);
        (height - (TOOLBAR_HEIGHT + 42.0) * vertical_scale).max(260.0)
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

        let level: u8 = 0;
        let mut unit = Unit::new(
            kind,
            faction,
            Vec2::new(Self::spawn_x(faction, width), Self::unit_y(kind, height)),
            level,
        );
        unit.max_health *= 1.0 + self.research_level(faction, Research::Armor) as f32 * 0.2;
        unit.health = unit.max_health;
        self.units.push(unit);
        true
    }

    fn recruit(&mut self, faction: Faction, kind: UnitKind, width: f32, height: f32) -> bool {
        if self.state != BattleState::Playing || self.units.len() >= MAX_UNITS {
            return false;
        }

        if !self.unit_unlocked(kind, faction)
            || self.population(faction) + kind.population() > self.population_cap(faction)
        {
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
        if !self.unit_unlocked(kind, Faction::Player) {
            self.announce(format!("{} REQUER {} XP", kind.name(), kind.unlock_xp() as u32));
            return false;
        }
        if self.population(Faction::Player) + kind.population() > self.population_cap(Faction::Player) {
            self.announce("POPULACAO CHEIA. LIMITE DE 20.");
            return false;
        }
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

    fn cast_spell(&mut self, target: Vec2, width: f32, height: f32) -> bool {
        if self.state != BattleState::Playing || self.spell_cooldown > 0.0 {
            return false;
        }

        if target.y < 0.0 || target.y >= height || target.x < 0.0 || target.x > width {
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
        let damage: f32 = SPELL_DAMAGE;
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
                kind: AttackEffectKind::Lightning,
            });
        }
        true
    }

    fn menu_button_rect(index: usize, width: f32, height: f32) -> (Vec2, Vec2) {
        let scale = Self::view_scale(width, height);
        let x = (width - MENU_BUTTON_WIDTH * scale) * 0.5;
        let y = height * 0.47 + index as f32 * (MENU_BUTTON_HEIGHT + MENU_BUTTON_GAP) * scale;
        (Vec2::new(x, y), Vec2::new(MENU_BUTTON_WIDTH, MENU_BUTTON_HEIGHT) * scale)
    }

    fn settings_panel_rect(width: f32, height: f32) -> (Vec2, Vec2) {
        let size: Vec2 = Vec2::new((width - 32.0).min(720.0), (height - 28.0).min(520.0));
        (Vec2::new((width - size.x) * 0.5, (height - size.y) * 0.5), size)
    }

    fn resolution_button_rect(index: usize, width: f32, height: f32) -> (Vec2, Vec2) {
        let (panel_pos, panel_size): (Vec2, Vec2) = Self::settings_panel_rect(width, height);
        let columns: usize = 2;
        let gap: f32 = 12.0;
        let grid_pos: Vec2 = panel_pos + Vec2::new(28.0, 143.0);
        let button_width: f32 = (panel_size.x - 56.0 - gap) / columns as f32;
        let button_size: Vec2 = Vec2::new(button_width, 58.0);
        let column: f32 = (index % columns) as f32;
        let row: f32 = (index / columns) as f32;
        (
            grid_pos + Vec2::new(column * (button_size.x + gap), row * (button_size.y + gap)),
            button_size,
        )
    }

    fn window_mode_button_rect(index: usize, width: f32, height: f32) -> (Vec2, Vec2) {
        let (panel_pos, panel_size): (Vec2, Vec2) = Self::settings_panel_rect(width, height);
        let gap: f32 = 12.0;
        let button_width: f32 = (panel_size.x - 56.0 - gap) * 0.5;
        let button_size: Vec2 = Vec2::new(button_width, 45.0);
        (
            panel_pos + Vec2::new(28.0 + index as f32 * (button_width + gap), 373.0),
            button_size,
        )
    }

    fn settings_back_button_rect(width: f32, height: f32) -> (Vec2, Vec2) {
        let (panel_pos, panel_size): (Vec2, Vec2) = Self::settings_panel_rect(width, height);
        let size: Vec2 = Vec2::new(190.0_f32.min(panel_size.x - 56.0), 43.0);
        (
            Vec2::new(panel_pos.x + (panel_size.x - size.x) * 0.5, panel_pos.y + panel_size.y - 61.0),
            size,
        )
    }

    fn stage_select_panel_rect(width: f32, height: f32) -> (Vec2, Vec2) {
        let size: Vec2 = Vec2::new((width - 36.0).min(1060.0), (height - 34.0).min(690.0));
        (Vec2::new((width - size.x) * 0.5, (height - size.y) * 0.46), size)
    }

    fn stage_back_button_rect(width: f32, height: f32) -> (Vec2, Vec2) {
        let (panel_pos, panel_size): (Vec2, Vec2) = Self::stage_select_panel_rect(width, height);
        let size: Vec2 = Vec2::new(190.0_f32.min(panel_size.x - 48.0), 43.0);
        (
            Vec2::new(panel_pos.x + (panel_size.x - size.x) * 0.5, panel_pos.y + panel_size.y - 58.0),
            size,
        )
    }

    fn point_in_rect(point: Vec2, pos: Vec2, size: Vec2) -> bool {
        point.x >= pos.x && point.x <= pos.x + size.x && point.y >= pos.y && point.y <= pos.y + size.y
    }

    fn summon_hero(&mut self, width: f32, height: f32) -> bool {
        if self.screen != ScreenState::Battle || self.state != BattleState::Playing || self.hero.summoned {
            return false;
        }
        if self.clan.experience[0] < HERO_UNLOCK_XP {
            self.announce("O CHEFE REQUER 250 XP");
            return false;
        }

        self.hero.summoned = true;
        self.hero.move_direction = 1.0;
        self.hero.shot_pause = 0.0;
        self.hero.pos = Vec2::new(
            Self::spawn_x(Faction::Player, width),
            Self::ground_y(height) - HERO_SIZE.y * 0.5,
        );
        true
    }

    fn set_hero_direction(&mut self, direction: f32) -> bool {
        if !self.hero.alive() || direction.abs() <= f32::EPSILON {
            return false;
        }

        self.hero.move_direction = direction.signum();
        true
    }

    fn update_hero_movement(&mut self, dt: f32, width: f32, _height: f32) {
        self.hero.moving = false;
        self.hero.pos.y = Self::ground_at(self.hero.pos.x) - HERO_SIZE.y * 0.5;
        if !self.hero.alive()
            || (self.hero.move_direction > 0.0
                && (self.hero.shot_pause > 0.0 || self.hero.attack_animation > 0.0 || self.hero_in_combat(width)))
        {
            return;
        }

        let min_x: f32 = Self::castle_x(Faction::Player, width);
        let max_x: f32 = Self::castle_x(Faction::Enemy, width) - CASTLE_WIDTH * 0.72;
        let previous_x = self.hero.pos.x;
        self.hero.pos.x =
            (self.hero.pos.x + self.hero.move_direction * HERO_MOVE_SPEED * dt).clamp(min_x, max_x.max(min_x));
        self.hero.pos.y = Self::ground_at(self.hero.pos.x) - HERO_SIZE.y * 0.5;
        self.hero.moving = (self.hero.pos.x - previous_x).abs() > f32::EPSILON;
        if self.hero.move_direction < 0.0 && self.hero.pos.x <= min_x {
            self.hero.in_castle = true;
            self.hero.moving = false;
            self.hero.attack_pending = false;
            self.hero.attack_animation = 0.0;
            self.hero.shot_pause = 0.0;
            self.clan.hero_respawn = 0.0;
            self.clan.fury_time = 0.0;
            self.announce("CHEFE NO CASTELO. NOVA CONVOCACAO EM 30 S.");
        }
    }

    fn update_hero_attack(&mut self, dt: f32, width: f32) {
        self.hero.attack_cooldown = (self.hero.attack_cooldown - dt).max(0.0);
        self.hero.shot_pause = (self.hero.shot_pause - dt).max(0.0);
        self.hero.attack_animation = (self.hero.attack_animation - dt).max(0.0);
        if self.hero.alive() {
            self.hero.animation_time += dt;
        }
        if !self.hero.alive() || self.hero.move_direction < 0.0 {
            self.hero.attack_pending = false;
            self.hero.attack_animation = 0.0;
            self.hero.shot_pause = 0.0;
            return;
        }
        if self.hero.attack_pending {
            if !self.hero_in_combat(width) {
                self.hero.attack_pending = false;
                self.hero.attack_animation = 0.0;
                self.hero.shot_pause = 0.0;
            } else if self.hero.attack_animation
                <= HERO_ATTACK_ANIMATION * (1.0 - character_sprites::HERO_HIT_FRACTION) + 0.00001
            {
                self.hero.attack_pending = false;
                if self.fire_hero(width) {
                    // Preserve the original interval from the start of one
                    // draw to the next, including the time spent winding up.
                    self.hero.attack_cooldown =
                        (HERO_ATTACK_COOLDOWN - (HERO_ATTACK_ANIMATION - self.hero.attack_animation)).max(0.0);
                }
            }
        } else if self.hero.attack_cooldown <= 0.0 && self.hero.attack_animation <= 0.0 && self.hero_in_combat(width) {
            self.hero.attack_pending = true;
            self.hero.attack_animation = HERO_ATTACK_ANIMATION;
            self.hero.shot_pause = HERO_ATTACK_ANIMATION;
        }
    }

    fn hero_target(&self) -> Option<usize> {
        self.units
            .iter()
            .enumerate()
            .filter(|(_, unit)| unit.faction == Faction::Enemy && unit.alive())
            .filter(|(_, unit)| (0.0..=HERO_RANGE).contains(&(unit.pos.x - self.hero.pos.x)))
            .min_by(|(_, a), (_, b)| a.pos.x.total_cmp(&b.pos.x))
            .map(|(index, _)| index)
    }

    fn enemy_castle_in_hero_range(&self, width: f32) -> bool {
        let enemy_castle_x: f32 = Self::castle_x(Faction::Enemy, width);
        self.enemy_castle.health > 0.0
            && enemy_castle_x >= self.hero.pos.x
            && enemy_castle_x - self.hero.pos.x <= HERO_RANGE
    }

    fn hero_in_combat(&self, width: f32) -> bool {
        self.hero_target().is_some() || self.enemy_castle_in_hero_range(width)
    }

    fn fire_hero(&mut self, width: f32) -> bool {
        if self.state != BattleState::Playing || !self.hero.alive() || self.hero.attack_cooldown > 0.0 {
            return false;
        }

        let multiplier = if self.clan.fury_time > 0.0 { 2.0 } else { 1.0 };
        let hit = if let Some(target) = self.hero_target() {
            (AttackTarget::Unit(target), HERO_DAMAGE * multiplier)
        } else if self.enemy_castle_in_hero_range(width) {
            (
                AttackTarget::Castle(Faction::Enemy),
                HERO_DAMAGE * HERO_TOWER_DAMAGE_MULTIPLIER * multiplier,
            )
        } else {
            return false;
        };

        let arrow_color: Color = match self.hero.aim_style {
            0 => Color::from_rgba8(205, 133, 255, 255),
            1 => Color::from_rgba8(108, 224, 210, 255),
            _ => Color::from_rgba8(248, 194, 78, 255),
        };
        self.launch_arrow(self.hero.pos + Vec2::new(15.0, -7.0), vec![hit], arrow_color, width);
        self.hero.attack_cooldown = HERO_ATTACK_COOLDOWN;
        self.hero.shot_pause = HERO_SHOT_PAUSE;
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

    fn apply_damage(&mut self, target_index: usize, amount: f32, hit_flash: f32) -> f32 {
        let Some(target) = self.units.get_mut(target_index).filter(|unit| unit.alive()) else {
            return 0.0;
        };
        target.health -= amount;
        target.hit_flash = hit_flash;
        amount
    }

    fn update_units(&mut self, dt: f32, width: f32) {
        for unit in &mut self.units {
            unit.attack_cooldown = (unit.attack_cooldown - dt).max(0.0);
            unit.attack_animation = (unit.attack_animation - dt).max(0.0);
            unit.moving = false;
            unit.hit_flash = (unit.hit_flash - dt).max(0.0);
        }

        let snapshot: Vec<(Faction, Vec2, Vec2, bool)> = self
            .units
            .iter()
            .map(|unit: &Unit| (unit.faction, unit.pos, unit.kind.stats().size, unit.alive()))
            .collect();
        let mut attacks: Vec<AttackIntent> = Vec::new();
        let mut holding_line: Vec<bool> = self
            .units
            .iter()
            .map(|unit| {
                let stats = unit.kind.stats();
                unit.alive()
                    && (unit.attack_animation > 0.0
                        || snapshot.iter().any(|(faction, pos, size, alive)| {
                            *alive
                                && *faction != unit.faction
                                && (pos.x - unit.pos.x).abs() <= stats.range + size.x * 0.5
                        })
                        || (unit.faction == Faction::Enemy
                            && self.hero.alive()
                            && (self.hero.pos.x - unit.pos.x).abs() <= stats.range + HERO_SIZE.x * 0.5)
                        || (Self::castle_x(unit.faction.opposite(), width) - unit.pos.x).abs()
                            <= stats.range + CASTLE_WIDTH * 0.5)
            })
            .collect();

        // Resolve the front of each faction first so followers use the updated
        // position of their neighbour and cannot step through a fighting line.
        let mut order: Vec<usize> = (0..self.units.len()).collect();
        order.sort_by(|&a, &b| {
            let a = &self.units[a];
            let b = &self.units[b];
            (b.pos.x * b.faction.direction()).total_cmp(&(a.pos.x * a.faction.direction()))
        });
        for index in order {
            if !self.units[index].alive() {
                continue;
            }

            let faction: Faction = self.units[index].faction;
            let mut pos: Vec2 = self.units[index].pos;
            let stats: UnitStats = self.units[index].kind.stats();
            // Marching units may already overlap when the front enters combat.
            // Restore their spacing before choosing attacks from the new position.
            let original_x = snapshot[index].1.x;
            for (other_index, (other_faction, other_pos, other_size, alive)) in snapshot.iter().enumerate() {
                if other_index == index || !*alive || *other_faction != faction || !holding_line[other_index] {
                    continue;
                }
                let ahead = (other_pos.x - original_x) * faction.direction();
                if ahead > 0.0 || (ahead == 0.0 && other_index < index) {
                    let limit =
                        self.units[other_index].pos.x * faction.direction() - (stats.size.x + other_size.x) * 0.58;
                    if pos.x * faction.direction() > limit {
                        pos.x = limit * faction.direction();
                        holding_line[index] = true;
                    }
                }
            }
            self.units[index].pos.x = pos.x;
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

            if faction == Faction::Enemy
                && self.units[index].kind == UnitKind::Archer
                && self.clan.difficulty == Difficulty::Warlord
                && let Some(target) = self.enemy_archer_focus(index)
            {
                nearest = Some(target);
            }

            if let Some((target, distance, target_width)) = nearest
                && distance <= stats.range + target_width * 0.5
            {
                if self.units[index].attack_cooldown <= 0.0 || self.units[index].pending_attack.is_some() {
                    attacks.push(AttackIntent {
                        attacker: index,
                        target,
                    });
                }
                continue;
            }

            let enemy_castle_x: f32 = Self::castle_x(faction.opposite(), width);
            if (enemy_castle_x - pos.x).abs() <= stats.range + CASTLE_WIDTH * 0.5 {
                if self.units[index].attack_cooldown <= 0.0 || self.units[index].pending_attack.is_some() {
                    attacks.push(AttackIntent {
                        attacker: index,
                        target: AttackTarget::Castle(faction.opposite()),
                    });
                }
                continue;
            }

            // Reacquire targets each update: no stored vector index can point
            // at a different unit after defeated units are removed.
            if self.units[index].pending_attack.take().is_some() {
                self.units[index].attack_animation = 0.0;
            }
            if self.units[index].attack_animation > 0.0 {
                continue;
            }
            let direction: f32 = faction.direction();
            let mut travel = stats.speed * dt.max(0.0);
            for (other_index, (other_faction, other_pos, other_size, alive)) in snapshot.iter().enumerate() {
                if other_index == index || !*alive || *other_faction != faction {
                    continue;
                }
                if !holding_line[other_index] {
                    continue;
                }
                let ahead = (other_pos.x - pos.x) * direction;
                if ahead > 0.0 || (ahead == 0.0 && other_index < index) {
                    let spacing = (stats.size.x + other_size.x) * 0.58;
                    let available = (self.units[other_index].pos.x - pos.x) * direction - spacing;
                    if available <= travel {
                        holding_line[index] = true;
                    }
                    travel = travel.min(available.max(0.0));
                }
            }
            // Stop at enemy attack range even when a frame has a large dt.
            if let Some((_, distance, target_width)) = nearest {
                let available = (distance - stats.range - target_width * 0.5).max(0.0);
                if available <= travel {
                    holding_line[index] = true;
                }
                travel = travel.min(available);
            }
            let castle_gap = ((enemy_castle_x - pos.x).abs() - stats.range - CASTLE_WIDTH * 0.5).max(0.0);
            if castle_gap <= travel {
                holding_line[index] = true;
            }
            travel = travel.min(castle_gap);
            if travel > 0.001 && !self.enemy_archer_waits_for_cover(index) {
                self.units[index].pos.x += direction * travel;
                self.units[index].animation_time += dt;
                self.units[index].moving = true;
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
            let level: u8 = 0;
            if self.units[intent.attacker].pending_attack.is_none() {
                let damage = stats.damage
                    * level_multiplier(level)
                    * (1.0 + self.research_level(faction, Research::Weapons) as f32 * 0.2);
                let unit = &mut self.units[intent.attacker];
                unit.attack_cooldown = stats.cooldown;
                unit.attack_duration = character_sprites::attack_seconds(kind).min(stats.cooldown * 0.9);
                unit.attack_animation = unit.attack_duration;
                unit.pending_attack = Some(damage);
            }
            let unit = &mut self.units[intent.attacker];
            if unit.attack_animation > unit.attack_duration * (1.0 - character_sprites::hit_fraction(kind)) + 0.00001 {
                continue;
            }
            let damage = unit.pending_attack.take().expect("An attack was started above");

            if kind == UnitKind::Archer {
                self.launch_arrow(
                    attacker_pos + Vec2::new(faction.direction() * 10.0, -7.0),
                    vec![(intent.target, damage)],
                    Color::from_rgba8(247, 218, 122, 255),
                    width,
                );
                continue;
            }

            let end: Vec2 = match intent.target {
                AttackTarget::Unit(target_index) => {
                    if target_index >= self.units.len()
                        || !self.units[target_index].alive()
                        || self.units[target_index].faction == faction
                    {
                        continue;
                    }
                    let end: Vec2 = self.units[target_index].pos;
                    self.apply_damage(target_index, damage, 0.11);

                    end
                }
                AttackTarget::Hero => {
                    if faction != Faction::Enemy || !self.hero.alive() {
                        continue;
                    }
                    let end: Vec2 = self.hero.pos;
                    self.apply_hero_damage(damage, 0.11);
                    end
                }
                AttackTarget::Castle(target_faction) => {
                    let castle: &mut Castle = match target_faction {
                        Faction::Player => &mut self.player_castle,
                        Faction::Enemy => &mut self.enemy_castle,
                    };
                    castle.health -= damage;
                    Vec2::new(Self::castle_x(target_faction, width), attacker_pos.y)
                }
            };

            let effect_kind: AttackEffectKind = match kind {
                UnitKind::Archer => unreachable!("Arrows are launched separately"),
                UnitKind::Brute => AttackEffectKind::HeavyImpact,
                UnitKind::Runner | UnitKind::Guard => AttackEffectKind::MeleeImpact,
            };
            let effect_life: f32 = match effect_kind {
                AttackEffectKind::HeavyImpact => 0.22,
                AttackEffectKind::MeleeImpact => 0.16,
                AttackEffectKind::ArrowImpact
                | AttackEffectKind::CannonImpact { .. }
                | AttackEffectKind::Lightning
                | AttackEffectKind::Meteor => unreachable!(),
            };
            let effect_color = kind.color(faction).lerp(Color::WHITE, 0.48);
            self.effects.push(AttackEffect {
                start: attacker_pos,
                end,
                life: effect_life,
                max_life: effect_life,
                color: effect_color,
                kind: effect_kind,
            });
        }
    }

    fn collect_defeated_units(&mut self) -> (f32, f32) {
        let mut player_reward: f32 = 0.0;
        let mut enemy_reward: f32 = 0.0;

        let old_xp = self.clan.experience[0];
        for unit in self.units.iter().filter(|unit: &&Unit| !unit.alive()) {
            let progress = ((unit.pos.x - 70.0) / (WORLD_WIDTH - 140.0)).clamp(0.0, 1.0);
            let advance = if unit.faction == Faction::Enemy {
                progress
            } else {
                1.0 - progress
            };
            let reward = unit.kind.stats().bounty * (1.0 + advance);
            let winner = Self::faction_index(unit.faction.opposite());
            self.clan.experience[winner] += reward * 4.0;
            match unit.faction {
                Faction::Enemy => {
                    player_reward += reward;
                    self.clan.reputation += 1;
                }
                Faction::Player => enemy_reward += reward,
            }
        }
        for (threshold, label) in [
            (120.0, "ARQUEIROS LIBERADOS"),
            (250.0, "CHEFE DO CLA LIBERADO - TECLA 5"),
            (360.0, "OGROS LIBERADOS"),
            (500.0, "METEORO LIBERADO - TECLA E"),
        ] {
            if old_xp < threshold && self.clan.experience[0] >= threshold {
                self.announce(label);
            }
        }

        self.player_coins = (self.player_coins + player_reward).min(MAX_COINS);
        self.enemy_coins = (self.enemy_coins + enemy_reward).min(MAX_COINS);
        self.units.retain(Unit::alive);
        (player_reward, enemy_reward)
    }

    fn update_economy(&mut self, dt: f32) {
        self.player_coins = (self.player_coins + PASSIVE_COIN_RATE * dt).min(MAX_COINS);
        self.enemy_coins =
            (self.enemy_coins + ENEMY_PASSIVE_COIN_RATE * self.clan.difficulty.income() * dt).min(MAX_COINS);
    }

    fn update_effects(&mut self, dt: f32) {
        for effect in &mut self.effects {
            effect.life -= dt;
        }
        self.effects.retain(|effect: &AttackEffect| effect.life > 0.0);
    }

    fn update_battle(&mut self, dt: f32, width: f32, height: f32) {
        if self.state != BattleState::Playing || self.clan.paused {
            return;
        }
        let active_arrows = self.arrows.len();
        let active_cannonballs = self.cannonballs.len();
        self.update_effects(dt);
        self.update_clan(dt);
        self.update_hero_attack(dt, width);
        self.update_hero_movement(dt, width, height);
        self.update_economy(dt);
        self.update_tower_repairs(dt);
        self.spell_cooldown = (self.spell_cooldown - dt).max(0.0);
        self.hero.hit_flash = (self.hero.hit_flash - dt).max(0.0);

        self.update_player_recruit_queue(dt, width, height);
        self.update_enemy_ai(dt, width, height);
        self.update_units(dt, width);
        self.update_castle_attack(Faction::Player, dt, width);
        self.update_castle_attack(Faction::Enemy, dt, width);
        self.update_arrows(dt, width, active_arrows);
        self.update_cannonballs(dt, active_cannonballs);
        self.collect_defeated_units();

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
        self.forest_sprites = Some(ForestSprites::load(ctx));
        self.runner_sprites = Some(RunnerSprites::load(ctx));
        self.army_sprites = Some(ArmySprites::load(ctx));
        self.cannon_sprites = Some(CannonSprites::load(ctx));
        self.ui_icons = Some(UiIcons::load(ctx));
        self.selected_resolution = Self::closest_resolution(ctx.surface_width(), ctx.surface_height());
        ctx.input_mut().bind(Action::RecruitRunner, KeyCode::Digit1.into());
        ctx.input_mut().bind(Action::RecruitGuard, KeyCode::Digit2.into());
        ctx.input_mut().bind(Action::RecruitArcher, KeyCode::Digit3.into());
        ctx.input_mut().bind(Action::RecruitBrute, KeyCode::Digit4.into());
        ctx.input_mut().bind(Action::SummonHero, KeyCode::Digit5.into());
        ctx.input_mut().bind(Action::CastSpell, KeyCode::KeyQ.into());
        ctx.input_mut().bind(Action::Upgrade, KeyCode::KeyU.into());
        ctx.input_mut().bind(Action::TroopsPanel, KeyCode::KeyT.into());
        ctx.input_mut().bind(Action::UpgradesPanel, KeyCode::KeyM.into());
        ctx.input_mut().bind(Action::MagicPanel, KeyCode::KeyG.into());
        ctx.input_mut().bind(Action::HeroPanel, KeyCode::KeyC.into());
        ctx.input_mut().bind(Action::Confirm, KeyCode::Enter.into());
        ctx.input_mut().bind(Action::Confirm, KeyCode::NumpadEnter.into());
        ctx.input_mut().bind(Action::Restart, KeyCode::KeyR.into());
        ctx.input_mut().bind(Action::Exit, KeyCode::Escape.into());
        ctx.input_mut().bind(Action::Click, MouseButton::Left.into());
        ctx.input_mut().bind(Action::CameraRight, KeyCode::ArrowRight.into());
        ctx.input_mut().bind(Action::CameraLeft, KeyCode::ArrowLeft.into());
        ctx.input_mut().bind(Action::Home, KeyCode::Home.into());
        ctx.input_mut().bind(Action::Pause, KeyCode::Space.into());
        ctx.input_mut().bind(Action::Meteor, KeyCode::KeyE.into());
        ctx.input_mut().bind(Action::Fury, KeyCode::KeyW.into());
        ctx.input_mut().bind(Action::TestRunner, KeyCode::F6.into());
        ctx.input_mut().bind(Action::TestMap, KeyCode::F7.into());
        ctx.input_mut().bind(Action::TestArmy, KeyCode::F8.into());
        #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
        if std::env::args().any(|arg| arg == "--test-cannons") {
            self.start_cannon_test();
        } else if std::env::args().any(|arg| arg == "--test-army") {
            self.start_army_test();
        } else if std::env::args().any(|arg| arg == "--test-map") {
            self.start_map_test();
        } else if std::env::args().any(|arg| arg == "--test-runner") {
            self.start_runner_test();
        }
        log::info!("Unites War: 1-4 recruit | 5 chief | arrows camera | Q/E spells | W fury | Space pause");
    }

    fn on_update(&mut self, ctx: &mut dyn GameContext<Self::Action>) {
        let width: f32 = ctx.surface_width() as f32;
        let height: f32 = ctx.surface_height() as f32;
        let dt: f32 = (ctx.delta_time() as f32).min(0.05);

        if ctx.input().just_pressed(Action::TestArmy) {
            self.start_army_test();
            return;
        }
        if ctx.input().just_pressed(Action::TestMap) {
            self.start_map_test();
            return;
        }
        if ctx.input().just_pressed(Action::TestRunner) {
            self.start_runner_test();
            return;
        }

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
                let (settings_pos, settings_size): (Vec2, Vec2) = Self::menu_button_rect(1, width, height);
                let (exit_pos, exit_size): (Vec2, Vec2) = Self::menu_button_rect(2, width, height);
                if Self::point_in_rect(mouse, play_pos, play_size) {
                    self.open_stage_select();
                } else if Self::point_in_rect(mouse, settings_pos, settings_size) {
                    self.open_settings();
                } else if Self::point_in_rect(mouse, exit_pos, exit_size) {
                    ctx.exit();
                }
            }
            return;
        }

        if self.screen == ScreenState::Settings {
            if ctx.input().just_pressed(Action::Exit) {
                self.screen = ScreenState::Menu;
                return;
            }
            if ctx.input().just_pressed(Action::Click)
                && let Some(mouse) = ctx.input().mouse_position()
            {
                for (index, fullscreen) in [false, true].into_iter().enumerate() {
                    let (button_pos, button_size): (Vec2, Vec2) = Self::window_mode_button_rect(index, width, height);
                    if Self::point_in_rect(mouse, button_pos, button_size) {
                        self.apply_window_mode(fullscreen, ctx);
                        return;
                    }
                }

                for index in 0..RESOLUTIONS.len() {
                    let (button_pos, button_size): (Vec2, Vec2) = Self::resolution_button_rect(index, width, height);
                    if Self::point_in_rect(mouse, button_pos, button_size) {
                        self.apply_resolution(index, ctx);
                        return;
                    }
                }

                let (back_pos, back_size): (Vec2, Vec2) = Self::settings_back_button_rect(width, height);
                if Self::point_in_rect(mouse, back_pos, back_size) {
                    self.screen = ScreenState::Menu;
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
                for (index, difficulty) in Difficulty::ALL.into_iter().enumerate() {
                    let (pos, size) = Self::difficulty_rect(index, width, height);
                    if Self::point_in_rect(mouse, pos, size) {
                        self.clan.difficulty = difficulty;
                        self.start_battle();
                        return;
                    }
                }
                let (back_pos, back_size): (Vec2, Vec2) = Self::stage_back_button_rect(width, height);
                if Self::point_in_rect(mouse, back_pos, back_size) {
                    self.screen = ScreenState::Menu;
                }
            }
            return;
        }

        if ctx.input().just_pressed(Action::Exit) {
            if self.clan.selected_spell.take().is_none() {
                if self.clan.panel_open {
                    self.clan.panel_open = false;
                } else {
                    self.screen = ScreenState::Menu;
                }
            }
            return;
        }
        if ctx.input().just_pressed(Action::Restart) {
            self.reset();
            return;
        }
        for (action, tab) in [
            (Action::TroopsPanel, CommandTab::Army),
            (Action::UpgradesPanel, CommandTab::Research),
            (Action::MagicPanel, CommandTab::Magic),
            (Action::HeroPanel, CommandTab::Hero),
            (Action::Upgrade, CommandTab::Tower),
        ] {
            if ctx.input().just_pressed(action) {
                self.toggle_sidebar(tab);
            }
        }
        if ctx.input().just_pressed(Action::Pause) && self.state == BattleState::Playing {
            self.clan.paused = !self.clan.paused;
        }
        let mouse = ctx.input().mouse_position();
        let edge = mouse.filter(|p| self.is_battlefield_point(*p, width, height));
        let pan = if ctx.input().is_down(Action::CameraRight) || edge.is_some_and(|p| p.x > width - 14.0) {
            1.0
        } else if ctx.input().is_down(Action::CameraLeft) || edge.is_some_and(|p| p.x < 14.0) {
            -1.0
        } else {
            0.0
        };
        self.move_camera(pan * 460.0 * dt, width, height);
        if ctx.input().just_pressed(Action::Home) {
            self.clan.camera = 0.0;
        }
        if ctx.input().just_pressed(Action::Click)
            && let Some(mouse) = mouse
        {
            self.handle_war_click(mouse, width, height);
        }
        if self.state != BattleState::Playing || self.clan.paused {
            return;
        }
        if ctx.input().just_pressed(Action::SummonHero) {
            self.summon_hero(WORLD_WIDTH, WORLD_HEIGHT);
        }
        for (action, kind) in [
            (Action::RecruitRunner, UnitKind::Runner),
            (Action::RecruitGuard, UnitKind::Guard),
            (Action::RecruitArcher, UnitKind::Archer),
            (Action::RecruitBrute, UnitKind::Brute),
        ] {
            if ctx.input().just_pressed(action) {
                self.enqueue_player_recruit(kind);
            }
        }
        if ctx.input().just_pressed(Action::CastSpell) {
            self.select_spell(Spell::Lightning);
        }
        if ctx.input().just_pressed(Action::Meteor) {
            self.select_spell(Spell::Meteor);
        }
        if ctx.input().just_pressed(Action::Fury) {
            self.cast_fury();
        }
        self.update_battle(dt, WORLD_WIDTH, WORLD_HEIGHT);
        self.align_forest_units();
    }

    fn on_render(&mut self, ctx: &mut dyn GameContext<Self::Action>) {
        let width: f32 = ctx.surface_width() as f32;
        let height: f32 = ctx.surface_height() as f32;
        if self.screen == ScreenState::Menu {
            self.draw_war_menu(ctx, width, height);
            return;
        }
        if self.screen == ScreenState::Settings {
            self.draw_settings(ctx, width, height);
            return;
        }
        if self.screen == ScreenState::StageSelect {
            self.draw_war_stage_select(ctx, width, height);
            return;
        }

        self.draw_forest_world(ctx, width, height);
        self.draw_war_hud(ctx, width, height);
        self.draw_end_overlay(ctx, width, height);
    }
}

#[cfg(test)]
mod tests;

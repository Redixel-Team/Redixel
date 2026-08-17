use super::*;

pub(super) const MOMENTUM_MAX_STACKS: u8 = 5;
pub(super) const MOMENTUM_DURATION: f32 = 2.4;
const MOMENTUM_ATTACK_SPEED_PER_STACK: f32 = 0.1;

const BULWARK_SELF_DAMAGE_REDUCTION: f32 = 0.25;
const BULWARK_ALLY_DAMAGE_REDUCTION: f32 = 0.18;
const BULWARK_PROTECTION_RANGE: f32 = 105.0;

pub(super) const PIERCING_ATTACK_INTERVAL: u8 = 3;
const PIERCING_DAMAGE_MULTIPLIER: f32 = 0.65;
const PIERCING_SEARCH_RANGE: f32 = 175.0;

pub(super) const RAGE_HEALTH_THRESHOLD: f32 = 0.4;
const RAGE_DAMAGE_MULTIPLIER: f32 = 1.4;
const RAGE_ATTACK_SPEED_MULTIPLIER: f32 = 1.3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PassiveKind {
    Momentum,
    Bulwark,
    PiercingShot,
    Rage,
}

impl PassiveKind {
    pub(super) const ALL: [Self; 4] = [Self::Momentum, Self::Bulwark, Self::PiercingShot, Self::Rage];

    pub(super) const fn index(self) -> usize {
        match self {
            Self::Momentum => 0,
            Self::Bulwark => 1,
            Self::PiercingShot => 2,
            Self::Rage => 3,
        }
    }

    pub(super) const fn cost(self) -> f32 {
        match self {
            Self::Momentum => 100.0,
            Self::Bulwark => 140.0,
            Self::PiercingShot => 180.0,
            Self::Rage => 220.0,
        }
    }

    pub(super) const fn title(self) -> &'static str {
        match self {
            Self::Momentum => "MOMENTUM",
            Self::Bulwark => "BULWARK",
            Self::PiercingShot => "PIERCING SHOT",
            Self::Rage => "RAGE",
        }
    }

    pub(super) const fn description(self) -> [&'static str; 2] {
        match self {
            Self::Momentum => ["ATACA MAIS RAPIDO", "ENQUANTO LUTA"],
            Self::Bulwark => ["REDUZ DANO E PROTEGE", "ALIADOS ATRAS"],
            Self::PiercingShot => ["CADA 3 ATAQUES", "ATRAVESSA O ALVO"],
            Self::Rage => ["POUCA VIDA AUMENTA", "DANO E VELOCIDADE"],
        }
    }

    pub(super) const fn unit_kind(self) -> UnitKind {
        match self {
            Self::Momentum => UnitKind::Runner,
            Self::Bulwark => UnitKind::Guard,
            Self::PiercingShot => UnitKind::Archer,
            Self::Rage => UnitKind::Brute,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct PassiveUnlocks {
    unlocked: [bool; 4],
}

impl PassiveUnlocks {
    pub(super) fn contains(self, passive: PassiveKind) -> bool {
        self.unlocked[passive.index()]
    }

    fn unlock(&mut self, passive: PassiveKind) {
        self.unlocked[passive.index()] = true;
    }
}

pub(super) struct AttackModifiers {
    pub(super) damage: f32,
    pub(super) cooldown: f32,
    pub(super) piercing_damage: Option<f32>,
}

impl UnitesWar {
    pub(super) fn passive_enabled(&self, faction: Faction, passive: PassiveKind) -> bool {
        faction == Faction::Player && self.player_passives.contains(passive)
    }

    pub(super) fn buy_passive(&mut self, passive: PassiveKind) -> bool {
        if self.screen != ScreenState::Battle
            || self.state != BattleState::Playing
            || self.player_passives.contains(passive)
            || self.player_coins < passive.cost()
        {
            return false;
        }

        self.player_coins -= passive.cost();
        self.player_passives.unlock(passive);
        true
    }

    pub(super) fn update_passive_states(&mut self, dt: f32) {
        for unit in &mut self.units {
            if unit.momentum_timer > 0.0 {
                unit.momentum_timer = (unit.momentum_timer - dt).max(0.0);
                if unit.momentum_timer <= 0.0 {
                    unit.momentum_stacks = 0;
                }
            }
        }
    }

    pub(super) fn prepare_attack(
        &mut self,
        attacker_index: usize,
        base_damage: f32,
        base_cooldown: f32,
        can_pierce: bool,
    ) -> AttackModifiers {
        let faction: Faction = self.units[attacker_index].faction;
        let kind: UnitKind = self.units[attacker_index].kind;
        let health_ratio: f32 = self.units[attacker_index].health / self.units[attacker_index].max_health;
        let mut damage: f32 = base_damage;
        let mut attack_speed: f32 = 1.0;
        let mut piercing_damage: Option<f32> = None;

        if kind == UnitKind::Runner && self.passive_enabled(faction, PassiveKind::Momentum) {
            let unit: &mut Unit = &mut self.units[attacker_index];
            unit.momentum_stacks = (unit.momentum_stacks + 1).min(MOMENTUM_MAX_STACKS);
            unit.momentum_timer = MOMENTUM_DURATION;
            attack_speed += unit.momentum_stacks as f32 * MOMENTUM_ATTACK_SPEED_PER_STACK;
        }

        if kind == UnitKind::Archer && can_pierce && self.passive_enabled(faction, PassiveKind::PiercingShot) {
            let unit: &mut Unit = &mut self.units[attacker_index];
            unit.attacks_until_piercing += 1;
            if unit.attacks_until_piercing >= PIERCING_ATTACK_INTERVAL {
                unit.attacks_until_piercing = 0;
                piercing_damage = Some(damage * PIERCING_DAMAGE_MULTIPLIER);
            }
        }

        if kind == UnitKind::Brute
            && health_ratio <= RAGE_HEALTH_THRESHOLD
            && self.passive_enabled(faction, PassiveKind::Rage)
        {
            damage *= RAGE_DAMAGE_MULTIPLIER;
            attack_speed *= RAGE_ATTACK_SPEED_MULTIPLIER;
        }

        AttackModifiers {
            damage,
            cooldown: base_cooldown / attack_speed,
            piercing_damage,
        }
    }

    pub(super) fn apply_damage(&mut self, target_index: usize, amount: f32, hit_flash: f32) -> f32 {
        if target_index >= self.units.len() || !self.units[target_index].alive() {
            return 0.0;
        }

        let target_faction: Faction = self.units[target_index].faction;
        let target_kind: UnitKind = self.units[target_index].kind;
        let target_pos: Vec2 = self.units[target_index].pos;
        let bulwark_enabled: bool = self.passive_enabled(target_faction, PassiveKind::Bulwark);
        let damage_multiplier: f32 = if bulwark_enabled && target_kind == UnitKind::Guard {
            1.0 - BULWARK_SELF_DAMAGE_REDUCTION
        } else if bulwark_enabled
            && self.units.iter().enumerate().any(|(index, guard)| {
                index != target_index
                    && guard.alive()
                    && guard.faction == target_faction
                    && guard.kind == UnitKind::Guard
                    && (guard.pos.x - target_pos.x) * target_faction.direction() > 0.0
                    && (guard.pos.x - target_pos.x).abs() <= BULWARK_PROTECTION_RANGE
            })
        {
            1.0 - BULWARK_ALLY_DAMAGE_REDUCTION
        } else {
            1.0
        };

        let final_damage: f32 = amount * damage_multiplier;
        self.units[target_index].health -= final_damage;
        self.units[target_index].hit_flash = hit_flash;
        final_damage
    }

    pub(super) fn piercing_target(&self, attacker_index: usize, first_target: usize) -> Option<usize> {
        if attacker_index >= self.units.len() || first_target >= self.units.len() {
            return None;
        }

        let attacker: &Unit = &self.units[attacker_index];
        let first_target_pos: Vec2 = self.units[first_target].pos;
        let direction: f32 = attacker.faction.direction();

        self.units
            .iter()
            .enumerate()
            .filter(|(index, unit)| {
                *index != first_target
                    && unit.alive()
                    && unit.faction == attacker.faction.opposite()
                    && (unit.pos.x - first_target_pos.x) * direction > 0.0
                    && (unit.pos.x - first_target_pos.x).abs() <= PIERCING_SEARCH_RANGE
            })
            .min_by(|(_, a), (_, b)| {
                (a.pos.x - first_target_pos.x)
                    .abs()
                    .total_cmp(&(b.pos.x - first_target_pos.x).abs())
            })
            .map(|(index, _)| index)
    }
}

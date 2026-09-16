use super::*;

fn duel(kind: UnitKind) -> UnitesWar {
    let mut game = UnitesWar::new();
    game.start_battle();
    game.units
        .push(Unit::new(kind, Faction::Player, Vec2::new(500.0, 0.0), 0));
    game.units
        .push(Unit::new(UnitKind::Guard, Faction::Enemy, Vec2::new(525.0, 0.0), 0));
    game.units[0].attack_cooldown = 0.0;
    game.units[1].attack_cooldown = 10.0;
    game
}

#[test]
fn melee_troops_deal_damage_once_at_the_authored_impact_frame() {
    for kind in [UnitKind::Runner, UnitKind::Guard, UnitKind::Brute] {
        let mut game = duel(kind);
        let health = game.units[1].health;
        game.update_units(0.0, WORLD_WIDTH);
        assert_eq!(game.units[1].health, health);
        assert!(game.effects.is_empty());
        let windup = game.units[0].attack_duration * character_sprites::hit_fraction(kind);
        game.update_units(windup - 0.01, WORLD_WIDTH);
        assert_eq!(game.units[1].health, health);
        game.update_units(0.011, WORLD_WIDTH);
        assert_eq!(game.units[1].health, health - kind.stats().damage);
        assert_eq!(game.effects.len(), 1);
        assert!(game.units[0].pending_attack.is_none());
        game.update_units(0.01, WORLD_WIDTH);
        assert_eq!(game.units[1].health, health - kind.stats().damage);
        assert!(!game.units[0].moving, "Recovery keeps planted feet still");
    }
}

#[test]
fn an_attack_cancels_if_the_target_leaves_range() {
    let mut game = duel(UnitKind::Archer);
    game.update_units(0.0, WORLD_WIDTH);
    let health = game.units[1].health;
    game.units[1].pos.x = 1000.0;
    game.update_units(0.6, WORLD_WIDTH);
    assert_eq!(game.units[1].health, health);
    assert!(game.effects.is_empty());
    assert!(game.units[0].pending_attack.is_none());
    assert_eq!(game.units[0].attack_animation, 0.0);
    assert!(game.units[0].moving);
}

#[test]
fn removing_a_target_during_windup_does_not_damage_an_ally_at_its_old_index() {
    let mut game = duel(UnitKind::Guard);
    game.units
        .push(Unit::new(UnitKind::Guard, Faction::Player, Vec2::new(475.0, 0.0), 0));
    game.units
        .push(Unit::new(UnitKind::Guard, Faction::Enemy, Vec2::new(530.0, 0.0), 0));
    game.units[3].attack_cooldown = 10.0;
    game.update_units(0.0, WORLD_WIDTH);
    game.units[1].health = 0.0;
    game.collect_defeated_units();
    let ally_health = game.units[1].health;
    // The next enemy enters the gap left by the defeated front unit.
    game.units[2].pos.x = 530.0;
    let enemy_health = game.units[2].health;
    let windup = game.units[0].attack_duration * character_sprites::hit_fraction(UnitKind::Guard);
    game.update_units(windup, WORLD_WIDTH);
    assert_eq!(game.units[1].faction, Faction::Player);
    assert_eq!(game.units[1].health, ally_health);
    assert!(game.units[2].health < enemy_health);
}

#[test]
fn hero_draws_before_firing_and_keeps_the_original_attack_interval() {
    let mut game = duel(UnitKind::Guard);
    game.hero.summoned = true;
    game.hero.pos = Vec2::new(400.0, 0.0);
    let health = game.units[1].health;
    game.update_hero_attack(0.0, WORLD_WIDTH);
    assert!(game.hero.attack_pending);
    assert_eq!(game.units[1].health, health);
    let windup = HERO_ATTACK_ANIMATION * character_sprites::HERO_HIT_FRACTION;
    game.update_hero_attack(windup - 0.01, WORLD_WIDTH);
    assert_eq!(game.units[1].health, health);
    game.update_hero_attack(0.01, WORLD_WIDTH);
    assert_eq!(game.units[1].health, health);
    assert_eq!(game.arrows.len(), 1);
    game.update_arrows(0.5, WORLD_WIDTH, game.arrows.len());
    assert_eq!(game.units[1].health, health - HERO_DAMAGE);
    assert!(!game.hero.attack_pending);
    assert!((game.hero.attack_cooldown + windup - HERO_ATTACK_COOLDOWN).abs() < 0.0001);
    game.update_hero_attack(0.01, WORLD_WIDTH);
    assert_eq!(game.units[1].health, health - HERO_DAMAGE);
}

#[test]
fn retreat_cancels_hero_windup_and_plays_running_in_the_new_direction() {
    let mut game = duel(UnitKind::Guard);
    game.hero.summoned = true;
    game.hero.pos = Vec2::new(400.0, 0.0);
    let health = game.units[1].health;
    game.update_hero_attack(0.0, WORLD_WIDTH);
    game.set_hero_direction(-1.0);
    game.update_hero_attack(0.1, WORLD_WIDTH);
    game.update_hero_movement(0.1, WORLD_WIDTH, WORLD_HEIGHT);
    assert!(!game.hero.attack_pending);
    assert_eq!(game.hero.attack_animation, 0.0);
    assert!(game.hero.moving && game.hero.animation_time > 0.0);
    assert!(game.hero.pos.x < 400.0);
    assert_eq!(game.units[1].health, health);
}

#[test]
fn pausing_freezes_troop_and_hero_attacks_together() {
    let mut game = duel(UnitKind::Archer);
    game.hero.summoned = true;
    game.hero.pos = Vec2::new(400.0, 0.0);
    game.update_units(0.0, WORLD_WIDTH);
    game.update_hero_attack(0.0, WORLD_WIDTH);
    let timers = (game.units[0].attack_animation, game.hero.attack_animation);
    let health = game.units[1].health;
    game.clan.paused = true;
    game.update_battle(1.0, WORLD_WIDTH, WORLD_HEIGHT);
    assert_eq!((game.units[0].attack_animation, game.hero.attack_animation), timers);
    assert_eq!(game.units[1].health, health);
}

use super::*;

#[test]
fn game_opens_in_menu_and_can_select_the_first_stage() {
    let mut game: UnitesWar = UnitesWar::new();
    assert_eq!(game.screen, ScreenState::Menu);
    game.open_stage_select();
    assert_eq!(game.screen, ScreenState::StageSelect);
    game.start_battle();
    assert_eq!(game.screen, ScreenState::Battle);
    assert_eq!(game.state, BattleState::Playing);
}

#[test]
fn battle_reset_preserves_loaded_texture_handles() {
    let mut game: UnitesWar = UnitesWar::new();
    game.textures = GameTextures {
        stage_01_background: Some(TextureId::new(4)),
        stage_01_fortress: Some(TextureId::new(7)),
    };
    let loaded_textures: GameTextures = game.textures;

    game.start_battle();

    assert_eq!(game.textures, loaded_textures);
}

#[test]
fn stage_select_has_one_playable_stage_and_future_slots() {
    let first: (Vec2, Vec2) = UnitesWar::stage_card_rect(0, 1280.0, 720.0);
    let second: (Vec2, Vec2) = UnitesWar::stage_card_rect(1, 1280.0, 720.0);
    assert!(second.0.x > first.0.x);
    assert_eq!(first.1, second.1);
}

#[test]
fn shadow_archer_can_advance_and_retreat_within_the_battlefield() {
    let mut game: UnitesWar = UnitesWar::new();
    let width: f32 = 1280.0;
    let height: f32 = 720.0;
    game.update_hero_movement(0.0, 0.0, width, height);
    let initial_x: f32 = game.hero.pos.x;

    game.update_hero_movement(1.0, 0.5, width, height);
    assert!(game.hero.pos.x > initial_x);

    game.update_hero_movement(-1.0, 0.5, width, height);
    assert!((game.hero.pos.x - initial_x).abs() < 0.001);

    game.update_hero_movement(-1.0, 100.0, width, height);
    assert!(game.hero.pos.x >= UnitesWar::castle_x(Faction::Player, width) + CASTLE_WIDTH * 0.62);
}

#[test]
fn shadow_archer_arrow_pierces_up_to_four_enemy_troops() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    game.hero.pos = Vec2::new(200.0, 400.0);
    for index in 0..5 {
        game.units.push(Unit::new(
            UnitKind::Guard,
            Faction::Enemy,
            Vec2::new(300.0 + index as f32 * 50.0, 400.0),
            0,
        ));
    }
    let health_before: Vec<f32> = game.units.iter().map(|unit| unit.health).collect();

    assert!(game.fire_hero(1280.0));
    assert_eq!(game.hero.aim_style, 1);
    assert_eq!(game.hero.attack_cooldown, HERO_ATTACK_COOLDOWN);
    for (index, unit) in game.units.iter().enumerate() {
        if index < HERO_MAX_PIERCED_TARGETS {
            assert!(unit.health < health_before[index]);
        } else {
            assert_eq!(unit.health, health_before[index]);
        }
    }

    game.hero.attack_cooldown = 0.0;
    assert!(game.fire_hero(1280.0));
    assert_eq!(game.hero.aim_style, 2);
    game.hero.attack_cooldown = 0.0;
    assert!(game.fire_hero(1280.0));
    assert_eq!(game.hero.aim_style, 0);
}

#[test]
fn shadow_archer_deals_bonus_damage_to_the_enemy_castle() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    let width: f32 = 1280.0;
    game.hero.pos.x = UnitesWar::castle_x(Faction::Enemy, width) - HERO_RANGE + 1.0;
    let health_before: f32 = game.enemy_castle.health;

    assert!(game.fire_hero(width));
    assert!((game.enemy_castle.health - (health_before - HERO_DAMAGE * HERO_TOWER_DAMAGE_MULTIPLIER)).abs() < 0.001);
}

#[test]
fn shadow_archer_has_low_health_and_can_be_defeated() {
    let mut game: UnitesWar = UnitesWar::new();
    assert!(game.hero.max_health < UnitKind::Guard.stats().health);
    assert_eq!(game.apply_hero_damage(HERO_MAX_HEALTH * 2.0, 0.1), HERO_MAX_HEALTH);
    assert!(!game.hero.alive());
    assert!(!game.fire_hero(1280.0));
}

#[test]
fn shadow_archer_waits_for_a_target_before_firing_automatically() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();

    assert!(!game.fire_hero(1280.0));
    assert_eq!(game.hero.attack_cooldown, 0.0);
    assert_eq!(game.hero.aim_style, 0);

    game.units.push(Unit::new(
        UnitKind::Runner,
        Faction::Enemy,
        game.hero.pos + Vec2::new(200.0, 0.0),
        0,
    ));
    assert!(game.fire_hero(1280.0));
}

#[test]
fn enemy_troops_can_attack_the_shadow_archer() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    game.hero.pos = Vec2::new(300.0, 400.0);
    game.units
        .push(Unit::new(UnitKind::Guard, Faction::Enemy, Vec2::new(330.0, 400.0), 0));
    game.units[0].attack_cooldown = 0.0;
    let health_before: f32 = game.hero.health;

    game.update_units(0.0, 1280.0);

    assert!(game.hero.health < health_before);
}

#[test]
fn bitmap_font_has_letters_and_spacing() {
    assert_ne!(UnitesWar::glyph_rows('A'), [0; 7]);
    assert_eq!(UnitesWar::glyph_rows(' '), [0; 7]);
    assert!((UnitesWar::text_width("WAR", 2.0) - 34.0).abs() < f32::EPSILON);
}

#[test]
fn skills_panel_only_toggles_during_battle() {
    let mut game: UnitesWar = UnitesWar::new();
    game.toggle_skills_panel();
    assert!(!game.skills_panel_open);

    game.start_battle();
    game.toggle_skills_panel();
    assert!(game.skills_panel_open);
    game.toggle_skills_panel();
    assert!(!game.skills_panel_open);

    game.state = BattleState::Victory;
    game.toggle_skills_panel();
    assert!(!game.skills_panel_open);
}

#[test]
fn stronger_units_cost_more() {
    assert!(UnitKind::Runner.stats().cost < UnitKind::Guard.stats().cost);
    assert!(UnitKind::Guard.stats().cost < UnitKind::Archer.stats().cost);
    assert!(UnitKind::Archer.stats().cost < UnitKind::Brute.stats().cost);
}

#[test]
fn recruit_spends_coins_and_creates_unit() {
    let mut game: UnitesWar = UnitesWar::new();
    let initial_coins: f32 = game.player_coins;
    assert!(game.recruit(Faction::Player, UnitKind::Runner, 1280.0, 720.0));
    assert_eq!(game.units.len(), 1);
    assert_eq!(game.units[0].faction, Faction::Player);
    assert!((game.player_coins - (initial_coins - UnitKind::Runner.stats().cost)).abs() < 0.001);
}

#[test]
fn unaffordable_unit_is_not_recruited() {
    let mut game: UnitesWar = UnitesWar::new();
    game.player_coins = 0.0;
    assert!(!game.recruit(Faction::Player, UnitKind::Brute, 1280.0, 720.0));
    assert!(game.units.is_empty());
}

#[test]
fn player_recruits_are_paid_and_added_to_the_queue_in_order() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    game.player_coins = MAX_COINS;
    let coins_before: f32 = game.player_coins;

    assert!(game.enqueue_player_recruit(UnitKind::Guard));
    assert!(game.enqueue_player_recruit(UnitKind::Runner));

    assert!(game.units.is_empty());
    assert_eq!(
        game.player_recruit_queue.iter().copied().collect::<Vec<_>>(),
        vec![UnitKind::Guard, UnitKind::Runner]
    );
    assert!(
        (game.player_coins - (coins_before - UnitKind::Guard.stats().cost - UnitKind::Runner.stats().cost)).abs()
            < 0.001
    );
    assert_eq!(game.player_recruit_timer, PLAYER_RECRUIT_INTERVAL);
}

#[test]
fn player_recruit_queue_spawns_only_one_unit_per_interval() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    game.player_coins = MAX_COINS;
    assert!(game.enqueue_player_recruit(UnitKind::Guard));
    assert!(game.enqueue_player_recruit(UnitKind::Runner));

    game.update_player_recruit_queue(PLAYER_RECRUIT_INTERVAL * 0.5, 1280.0, 720.0);
    assert!(game.units.is_empty());

    game.update_player_recruit_queue(PLAYER_RECRUIT_INTERVAL * 0.5, 1280.0, 720.0);
    assert_eq!(game.units.len(), 1);
    assert_eq!(game.units[0].kind, UnitKind::Guard);
    assert_eq!(game.player_recruit_queue.len(), 1);

    game.update_player_recruit_queue(PLAYER_RECRUIT_INTERVAL, 1280.0, 720.0);
    assert_eq!(game.units.len(), 2);
    assert_eq!(game.units[1].kind, UnitKind::Runner);
    assert!(game.player_recruit_queue.is_empty());
}

#[test]
fn spell_only_damages_enemies_in_radius() {
    let mut game: UnitesWar = UnitesWar::new();
    game.player_coins = 0.0;
    game.units
        .push(Unit::new(UnitKind::Guard, Faction::Enemy, Vec2::new(600.0, 400.0), 0));
    game.units
        .push(Unit::new(UnitKind::Guard, Faction::Player, Vec2::new(610.0, 400.0), 0));
    let enemy_health: f32 = game.units[0].health;
    let ally_health: f32 = game.units[1].health;

    assert!(game.cast_spell(Vec2::new(600.0, 400.0), 1280.0, 720.0));
    assert!(game.units[0].health < enemy_health);
    assert_eq!(game.units[1].health, ally_health);
    assert_eq!(game.player_coins, 0.0);
    assert_eq!(game.spell_cooldown, SPELL_COOLDOWN);
    assert!(!game.cast_spell(Vec2::new(600.0, 400.0), 1280.0, 720.0));
}

#[test]
fn upgrade_improves_and_repairs_castle() {
    let mut game: UnitesWar = UnitesWar::new();
    game.player_coins = MAX_COINS;
    game.player_castle.health = 500.0;
    let previous_max: f32 = game.player_castle.max_health;

    assert!(game.upgrade(Faction::Player));
    assert_eq!(game.player_castle.level, 1);
    assert!(game.player_castle.health > 500.0);
    assert!(game.player_castle.max_health > previous_max);
}

#[test]
fn economy_generates_passive_coins() {
    let mut game: UnitesWar = UnitesWar::new();
    let player_before: f32 = game.player_coins;
    let enemy_before: f32 = game.enemy_coins;
    game.update_economy(2.0);
    assert!((game.player_coins - player_before - PASSIVE_COIN_RATE * 2.0).abs() < 0.001);
    assert!((game.enemy_coins - enemy_before - ENEMY_PASSIVE_COIN_RATE * 2.0).abs() < 0.001);
}

#[test]
fn defeated_enemy_grants_bounty_once() {
    let mut game: UnitesWar = UnitesWar::new();
    game.player_coins = 0.0;
    let mut enemy: Unit = Unit::new(UnitKind::Guard, Faction::Enemy, Vec2::ZERO, 0);
    enemy.health = 0.0;
    game.units.push(enemy);

    let (player_reward, enemy_reward): (f32, f32) = game.collect_defeated_units();
    assert_eq!(player_reward, UnitKind::Guard.stats().bounty);
    assert_eq!(enemy_reward, 0.0);
    assert_eq!(game.player_coins, UnitKind::Guard.stats().bounty);

    let coins_after_first_collection: f32 = game.player_coins;
    assert_eq!(game.collect_defeated_units(), (0.0, 0.0));
    assert_eq!(game.player_coins, coins_after_first_collection);
}

#[test]
fn passive_purchase_spends_gold_and_can_only_happen_once() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    game.player_coins = MAX_COINS;
    let initial_coins: f32 = game.player_coins;

    assert!(game.buy_passive(PassiveKind::Momentum));
    assert!(game.player_passives.contains(PassiveKind::Momentum));
    assert!((game.player_coins - (initial_coins - PassiveKind::Momentum.cost())).abs() < 0.001);

    let coins_after_purchase: f32 = game.player_coins;
    assert!(!game.buy_passive(PassiveKind::Momentum));
    assert_eq!(game.player_coins, coins_after_purchase);
}

#[test]
fn momentum_builds_attack_speed_while_runner_is_fighting() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    game.player_coins = MAX_COINS;
    assert!(game.buy_passive(PassiveKind::Momentum));

    game.units
        .push(Unit::new(UnitKind::Runner, Faction::Player, Vec2::new(300.0, 400.0), 0));
    game.units
        .push(Unit::new(UnitKind::Guard, Faction::Enemy, Vec2::new(325.0, 400.0), 0));
    game.units[0].attack_cooldown = 0.0;

    game.update_units(0.0, 1280.0);

    assert_eq!(game.units[0].momentum_stacks, 1);
    assert_eq!(game.units[0].momentum_timer, MOMENTUM_DURATION);
    assert!(game.units[0].attack_cooldown < UnitKind::Runner.stats().cooldown);
}

#[test]
fn bulwark_reduces_guard_damage_and_protects_allies_behind() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    game.player_coins = MAX_COINS;
    assert!(game.buy_passive(PassiveKind::Bulwark));

    game.units
        .push(Unit::new(UnitKind::Guard, Faction::Player, Vec2::new(300.0, 400.0), 0));
    game.units
        .push(Unit::new(UnitKind::Runner, Faction::Player, Vec2::new(250.0, 400.0), 0));
    game.units
        .push(Unit::new(UnitKind::Runner, Faction::Player, Vec2::new(350.0, 400.0), 0));

    assert!((game.apply_damage(0, 100.0, 0.1) - 75.0).abs() < 0.001);
    assert!((game.apply_damage(1, 100.0, 0.1) - 82.0).abs() < 0.001);
    assert!((game.apply_damage(2, 100.0, 0.1) - 100.0).abs() < 0.001);
}

#[test]
fn piercing_shot_hits_a_second_target_every_third_attack() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    game.player_coins = MAX_COINS;
    assert!(game.buy_passive(PassiveKind::PiercingShot));

    game.units
        .push(Unit::new(UnitKind::Archer, Faction::Player, Vec2::new(300.0, 400.0), 0));
    game.units
        .push(Unit::new(UnitKind::Guard, Faction::Enemy, Vec2::new(400.0, 400.0), 0));
    game.units
        .push(Unit::new(UnitKind::Guard, Faction::Enemy, Vec2::new(500.0, 400.0), 0));
    game.units[0].attack_cooldown = 0.0;
    game.units[0].attacks_until_piercing = PIERCING_ATTACK_INTERVAL - 1;
    let first_health: f32 = game.units[1].health;
    let second_health: f32 = game.units[2].health;

    game.update_units(0.0, 1280.0);

    assert!(game.units[1].health < first_health);
    assert!(game.units[2].health < second_health);
    assert_eq!(game.units[0].attacks_until_piercing, 0);
}

#[test]
fn rage_increases_brute_damage_and_attack_speed_at_low_health() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    game.player_coins = MAX_COINS;
    assert!(game.buy_passive(PassiveKind::Rage));

    game.units
        .push(Unit::new(UnitKind::Brute, Faction::Player, Vec2::new(300.0, 400.0), 0));
    game.units
        .push(Unit::new(UnitKind::Brute, Faction::Enemy, Vec2::new(330.0, 400.0), 0));
    game.units[0].health = game.units[0].max_health * (RAGE_HEALTH_THRESHOLD - 0.05);
    game.units[0].attack_cooldown = 0.0;
    let enemy_health: f32 = game.units[1].health;

    game.update_units(0.0, 1280.0);

    assert!(enemy_health - game.units[1].health > UnitKind::Brute.stats().damage);
    assert!(game.units[0].attack_cooldown < UnitKind::Brute.stats().cooldown);
}

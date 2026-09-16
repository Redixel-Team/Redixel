use super::*;

#[test]
fn game_opens_in_menu_and_can_select_the_first_stage() {
    let mut game: UnitesWar = UnitesWar::new();
    assert_eq!(game.screen, ScreenState::Menu);
    game.open_settings();
    assert_eq!(game.screen, ScreenState::Settings);
    game.screen = ScreenState::Menu;
    game.open_stage_select();
    assert_eq!(game.screen, ScreenState::StageSelect);
    game.start_battle();
    game.clan.experience[0] = HERO_UNLOCK_XP;
    assert_eq!(game.screen, ScreenState::Battle);
    assert_eq!(game.state, BattleState::Playing);
}

#[test]
fn settings_offer_common_resolutions_through_full_hd() {
    assert_eq!(
        RESOLUTIONS.first(),
        Some(&Resolution {
            width: 960,
            height: 540
        })
    );
    assert_eq!(
        RESOLUTIONS.last(),
        Some(&Resolution {
            width: 1920,
            height: 1080
        })
    );
    assert_eq!(UnitesWar::closest_resolution(1278, 718), 1);

    let first: (Vec2, Vec2) = UnitesWar::resolution_button_rect(0, 960.0, 540.0);
    let second: (Vec2, Vec2) = UnitesWar::resolution_button_rect(1, 960.0, 540.0);
    let third: (Vec2, Vec2) = UnitesWar::resolution_button_rect(2, 960.0, 540.0);
    assert!(second.0.x > first.0.x);
    assert_eq!(second.0.y, first.0.y);
    assert_eq!(third.0.x, first.0.x);
    assert!(third.0.y > first.0.y);

    let windowed: (Vec2, Vec2) = UnitesWar::window_mode_button_rect(0, 960.0, 540.0);
    let fullscreen: (Vec2, Vec2) = UnitesWar::window_mode_button_rect(1, 960.0, 540.0);
    let back: (Vec2, Vec2) = UnitesWar::settings_back_button_rect(960.0, 540.0);
    assert!(fullscreen.0.x > windowed.0.x);
    assert_eq!(fullscreen.0.y, windowed.0.y);
    assert!(windowed.0.y + windowed.1.y < back.0.y);
}

#[test]
fn compact_battle_hud_fits_the_smallest_supported_resolution() {
    let width: f32 = 960.0;
    let height: f32 = 540.0;
    let (panel_pos, panel_size): (Vec2, Vec2) = UnitesWar::command_panel_rect(width, height);
    let (first_pos, _): (Vec2, Vec2) = UnitesWar::button_rect(0, width, height);
    let (last_pos, last_size): (Vec2, Vec2) = UnitesWar::button_rect(3, width, height);
    let (status_pos, status_size): (Vec2, Vec2) = UnitesWar::minimap_rect(width, height);

    assert!(first_pos.x >= panel_pos.x && first_pos.y >= panel_pos.y);
    assert!(last_pos.x + last_size.x <= panel_pos.x + panel_size.x);
    assert!(last_pos.y + last_size.y <= panel_pos.y + panel_size.y);
    assert!(panel_pos.x + panel_size.x < status_pos.x);
    assert!(status_pos.x + status_size.x <= width);
    assert!(status_pos.y + status_size.y <= height);
}

#[test]
fn battle_hud_scales_with_the_selected_resolution() {
    assert_eq!(UnitesWar::view_scale(960.0, 540.0), 1.0);
    assert!((UnitesWar::view_scale(1280.0, 720.0) - 4.0 / 3.0).abs() < 0.0001);
    assert_eq!(UnitesWar::view_scale(1920.0, 1080.0), 2.0);

    let (_, base_size): (Vec2, Vec2) = UnitesWar::command_panel_rect(960.0, 540.0);
    let (_, full_hd_size): (Vec2, Vec2) = UnitesWar::command_panel_rect(1920.0, 1080.0);
    assert_eq!(full_hd_size, base_size * 2.0);
}

#[test]
fn clan_chief_can_advance_and_retreat_within_the_battlefield() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    game.clan.experience[0] = HERO_UNLOCK_XP;
    let width: f32 = 1280.0;
    let height: f32 = 720.0;
    assert!(!game.hero.alive());
    assert!(game.summon_hero(width, height));
    let initial_x: f32 = game.hero.pos.x;

    game.update_hero_movement(0.5, width, height);
    assert!(game.hero.pos.x > initial_x);

    assert!(game.set_hero_direction(-1.0));
    game.update_hero_movement(0.5, width, height);
    assert!((game.hero.pos.x - initial_x).abs() < 0.001);

    game.update_hero_movement(100.0, width, height);
    assert_eq!(game.hero.pos.x, UnitesWar::castle_x(Faction::Player, width));
    assert!(game.hero.in_castle);
    assert!(!game.hero.alive());
}

#[test]
fn clan_chief_cannot_be_summoned_twice_while_active() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    game.clan.experience[0] = HERO_UNLOCK_XP;

    assert!(!game.hero.summoned);
    assert!(game.summon_hero(1280.0, 720.0));
    assert!(game.hero.alive());
    assert_eq!(game.hero.move_direction, 1.0);
    assert!(!game.summon_hero(1280.0, 720.0));
}

#[test]
fn clan_chief_holds_position_until_combat_ends() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    game.clan.experience[0] = HERO_UNLOCK_XP;
    let width: f32 = 1280.0;
    assert!(game.summon_hero(width, 720.0));
    let initial_x: f32 = game.hero.pos.x;
    game.units.push(Unit::new(
        UnitKind::Guard,
        Faction::Enemy,
        Vec2::new(initial_x + HERO_RANGE - 10.0, game.hero.pos.y),
        0,
    ));

    assert!(game.fire_hero(width));
    assert_eq!(game.hero.shot_pause, HERO_SHOT_PAUSE);

    game.update_hero_movement(0.1, width, 720.0);
    assert_eq!(game.hero.pos.x, initial_x);

    game.hero.shot_pause = 0.0;
    assert!(game.units[0].alive());
    game.update_hero_movement(0.1, width, 720.0);
    assert_eq!(game.hero.pos.x, initial_x);

    game.units[0].health = 0.0;
    game.update_hero_movement(0.1, width, 720.0);
    assert!(game.hero.pos.x > initial_x);
}

#[test]
fn clan_chief_holds_position_while_attacking_the_enemy_castle() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    game.clan.experience[0] = HERO_UNLOCK_XP;
    let width: f32 = 1280.0;
    assert!(game.summon_hero(width, 720.0));
    game.hero.pos.x = UnitesWar::castle_x(Faction::Enemy, width) - HERO_RANGE + 1.0;
    let initial_x: f32 = game.hero.pos.x;

    game.update_hero_movement(0.5, width, 720.0);

    assert_eq!(game.hero.pos.x, initial_x);
}

#[test]
fn clan_chief_has_the_same_range_as_a_regular_archer() {
    assert_eq!(HERO_RANGE, UnitKind::Archer.stats().range);
}

#[test]
fn clan_chief_arrow_hits_only_the_first_enemy() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    game.clan.experience[0] = HERO_UNLOCK_XP;
    assert!(game.summon_hero(1280.0, 720.0));
    game.hero.pos = Vec2::new(200.0, 400.0);
    for index in 0..5 {
        game.units.push(Unit::new(
            UnitKind::Guard,
            Faction::Enemy,
            Vec2::new(300.0 + index as f32 * 12.0, 400.0),
            0,
        ));
    }
    let health_before: Vec<f32> = game.units.iter().map(|unit| unit.health).collect();

    assert!(game.fire_hero(1280.0));
    game.update_arrows(1.0, 1280.0, game.arrows.len());
    assert_eq!(game.hero.aim_style, 1);
    assert_eq!(game.hero.attack_cooldown, HERO_ATTACK_COOLDOWN);
    for (index, unit) in game.units.iter().enumerate() {
        if index == 0 {
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
fn clan_chief_deals_bonus_damage_to_the_enemy_castle() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    game.clan.experience[0] = HERO_UNLOCK_XP;
    let width: f32 = 1280.0;
    assert!(game.summon_hero(width, 720.0));
    game.hero.pos.x = UnitesWar::castle_x(Faction::Enemy, width) - HERO_RANGE + 1.0;
    let health_before: f32 = game.enemy_castle.health;

    assert!(game.fire_hero(width));
    game.update_arrows(1.0, 1280.0, game.arrows.len());
    assert!((game.enemy_castle.health - (health_before - HERO_DAMAGE * HERO_TOWER_DAMAGE_MULTIPLIER)).abs() < 0.001);
}

#[test]
fn clan_chief_has_low_health_and_can_be_defeated() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    game.clan.experience[0] = HERO_UNLOCK_XP;
    assert!(game.summon_hero(1280.0, 720.0));
    assert!(game.hero.max_health < UnitKind::Guard.stats().health);
    assert_eq!(game.apply_hero_damage(HERO_MAX_HEALTH * 2.0, 0.1), HERO_MAX_HEALTH);
    assert!(!game.hero.alive());
    assert!(!game.fire_hero(1280.0));
}

#[test]
fn clan_chief_waits_for_a_target_before_firing_automatically() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    game.clan.experience[0] = HERO_UNLOCK_XP;
    assert!(game.summon_hero(1280.0, 720.0));

    assert!(!game.fire_hero(1280.0));
    assert_eq!(game.hero.attack_cooldown, 0.0);
    assert_eq!(game.hero.aim_style, 0);

    game.units.push(Unit::new(
        UnitKind::Runner,
        Faction::Enemy,
        game.hero.pos + Vec2::new(HERO_RANGE + 1.0, 0.0),
        0,
    ));
    assert!(!game.fire_hero(1280.0));

    game.units[0].pos.x = game.hero.pos.x + HERO_RANGE;
    assert!(game.fire_hero(1280.0));
}

#[test]
fn enemy_troops_can_attack_the_clan_chief() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    game.clan.experience[0] = HERO_UNLOCK_XP;
    assert!(game.summon_hero(1280.0, 720.0));
    game.hero.pos = Vec2::new(300.0, 400.0);
    game.units
        .push(Unit::new(UnitKind::Guard, Faction::Enemy, Vec2::new(330.0, 400.0), 0));
    game.units[0].attack_cooldown = 0.0;
    let health_before: f32 = game.hero.health;

    game.update_units(0.0, 1280.0);
    let windup = game.units[0].attack_duration * character_sprites::hit_fraction(game.units[0].kind);
    game.update_units(windup, 1280.0);

    assert!(game.hero.health < health_before);
}

#[test]
fn melee_troops_lunge_and_only_create_an_impact_at_the_target() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    game.clan.experience[0] = HERO_UNLOCK_XP;
    game.units
        .push(Unit::new(UnitKind::Runner, Faction::Player, Vec2::new(300.0, 400.0), 0));
    game.units
        .push(Unit::new(UnitKind::Guard, Faction::Enemy, Vec2::new(325.0, 400.0), 0));
    game.units[0].attack_cooldown = 0.0;

    game.update_units(0.0, 1280.0);

    assert_eq!(game.units[0].attack_animation, RUNNER_ATTACK_ANIMATION);
    assert!(game.effects.is_empty());
    game.update_units(
        RUNNER_ATTACK_ANIMATION * character_sprites::hit_fraction(UnitKind::Runner),
        1280.0,
    );
    assert!(
        game.effects
            .iter()
            .any(|effect| effect.kind == AttackEffectKind::MeleeImpact)
    );
    assert!(game.arrows.is_empty());
}

#[test]
fn regular_archers_create_visible_arrows_and_start_their_attack_animation() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    game.clan.experience[0] = HERO_UNLOCK_XP;
    game.units
        .push(Unit::new(UnitKind::Archer, Faction::Player, Vec2::new(300.0, 400.0), 0));
    game.units
        .push(Unit::new(UnitKind::Guard, Faction::Enemy, Vec2::new(420.0, 400.0), 0));
    game.units[0].attack_cooldown = 0.0;

    game.update_units(0.0, 1280.0);

    assert_eq!(game.units[0].attack_animation, UnitKind::Archer.attack_animation_duration());
    assert!(game.effects.is_empty(), "The arrow waits for the drawn bow to release");
    let windup = game.units[0].attack_duration * character_sprites::hit_fraction(UnitKind::Archer);
    game.update_units(windup, 1280.0);
    assert_eq!(game.arrows.len(), 1);
}

#[test]
fn allied_troops_move_freely_outside_combat() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    game.clan.experience[0] = HERO_UNLOCK_XP;
    game.units
        .push(Unit::new(UnitKind::Guard, Faction::Player, Vec2::new(300.0, 400.0), 0));
    game.units
        .push(Unit::new(UnitKind::Guard, Faction::Player, Vec2::new(315.0, 400.0), 0));
    let initial_positions: Vec<f32> = game.units.iter().map(|unit| unit.pos.x).collect();

    game.update_units(0.1, 1280.0);

    assert!(game.units[0].pos.x > initial_positions[0]);
    assert!(game.units[1].pos.x > initial_positions[1]);
    assert!(game.units[0].moving);
    assert!(game.units[1].moving);
}

#[test]
fn enemy_troops_share_allied_marching_rules_outside_combat() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    game.clan.experience[0] = HERO_UNLOCK_XP;
    game.units
        .push(Unit::new(UnitKind::Guard, Faction::Enemy, Vec2::new(900.0, 400.0), 0));
    game.units
        .push(Unit::new(UnitKind::Guard, Faction::Enemy, Vec2::new(885.0, 400.0), 0));
    let initial_positions: Vec<f32> = game.units.iter().map(|unit| unit.pos.x).collect();

    game.update_units(0.1, 1280.0);

    assert!(game.units[0].pos.x < initial_positions[0]);
    assert!(game.units[1].pos.x < initial_positions[1]);
}

#[test]
fn allied_troops_stop_moving_when_they_enter_combat() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    game.clan.experience[0] = HERO_UNLOCK_XP;
    game.units
        .push(Unit::new(UnitKind::Guard, Faction::Player, Vec2::new(300.0, 400.0), 0));
    game.units
        .push(Unit::new(UnitKind::Guard, Faction::Enemy, Vec2::new(325.0, 400.0), 0));
    let initial_x: f32 = game.units[0].pos.x;

    game.update_units(0.1, 1280.0);

    assert_eq!(game.units[0].pos.x, initial_x);
    assert!(!game.units[0].moving);
}

#[test]
fn bitmap_font_has_letters_and_spacing() {
    assert_ne!(UnitesWar::glyph_rows('A'), [0; 7]);
    assert_eq!(UnitesWar::glyph_rows(' '), [0; 7]);
    assert!((UnitesWar::text_width("WAR", 2.0) - 34.0).abs() < f32::EPSILON);
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
    game.clan.experience[0] = HERO_UNLOCK_XP;
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
    game.clan.experience[0] = HERO_UNLOCK_XP;
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

    assert!(game.buy_tower_upgrade(Faction::Player, TowerUpgrade::Health));
    assert_eq!(game.player_castle.upgrades[TowerUpgrade::Health.index()], 1);
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
fn runner_uses_base_attack_speed() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    game.clan.experience[0] = HERO_UNLOCK_XP;
    game.player_coins = MAX_COINS;

    game.units
        .push(Unit::new(UnitKind::Runner, Faction::Player, Vec2::new(300.0, 400.0), 0));
    game.units
        .push(Unit::new(UnitKind::Guard, Faction::Enemy, Vec2::new(325.0, 400.0), 0));
    game.units[0].attack_cooldown = 0.0;

    game.update_units(0.0, 1280.0);

    assert_eq!(game.units[0].attack_cooldown, UnitKind::Runner.stats().cooldown);
}

#[test]
fn guards_and_nearby_allies_take_full_damage() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    game.clan.experience[0] = HERO_UNLOCK_XP;
    game.player_coins = MAX_COINS;

    game.units
        .push(Unit::new(UnitKind::Guard, Faction::Player, Vec2::new(300.0, 400.0), 0));
    game.units
        .push(Unit::new(UnitKind::Runner, Faction::Player, Vec2::new(250.0, 400.0), 0));
    game.units
        .push(Unit::new(UnitKind::Runner, Faction::Player, Vec2::new(350.0, 400.0), 0));

    assert!((game.apply_damage(0, 100.0, 0.1) - 100.0).abs() < 0.001);
    assert!((game.apply_damage(1, 100.0, 0.1) - 100.0).abs() < 0.001);
    assert!((game.apply_damage(2, 100.0, 0.1) - 100.0).abs() < 0.001);
}

#[test]
fn archer_arrows_only_damage_the_primary_target() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    game.clan.experience[0] = HERO_UNLOCK_XP;
    game.player_coins = MAX_COINS;

    game.units
        .push(Unit::new(UnitKind::Archer, Faction::Player, Vec2::new(300.0, 400.0), 0));
    game.units
        .push(Unit::new(UnitKind::Guard, Faction::Enemy, Vec2::new(400.0, 400.0), 0));
    game.units
        .push(Unit::new(UnitKind::Guard, Faction::Enemy, Vec2::new(500.0, 400.0), 0));
    game.units[0].attack_cooldown = 0.0;
    let first_health: f32 = game.units[1].health;
    let second_health: f32 = game.units[2].health;

    for _ in 0..3 {
        game.units[0].attack_cooldown = 0.0;
        game.update_units(0.0, 1280.0);
        let windup = game.units[0].attack_duration * character_sprites::hit_fraction(UnitKind::Archer);
        game.update_units(windup, 1280.0);
        game.update_arrows(0.5, 1280.0, game.arrows.len());
        assert_eq!(game.units[2].health, second_health);
    }

    assert!((first_health - game.units[1].health - 3.0 * UnitKind::Archer.stats().damage).abs() < 0.001);
}

#[test]
fn low_health_does_not_change_brute_damage_or_attack_speed() {
    let mut game: UnitesWar = UnitesWar::new();
    game.start_battle();
    game.clan.experience[0] = HERO_UNLOCK_XP;
    game.player_coins = MAX_COINS;

    game.units
        .push(Unit::new(UnitKind::Brute, Faction::Player, Vec2::new(300.0, 400.0), 0));
    game.units
        .push(Unit::new(UnitKind::Brute, Faction::Enemy, Vec2::new(330.0, 400.0), 0));
    game.units[0].health = game.units[0].max_health * 0.35;
    game.units[0].attack_cooldown = 0.0;
    let enemy_health: f32 = game.units[1].health;

    game.update_units(0.0, 1280.0);
    let windup = game.units[0].attack_duration * character_sprites::hit_fraction(UnitKind::Brute);
    game.update_units(windup, 1280.0);

    assert!((enemy_health - game.units[1].health - UnitKind::Brute.stats().damage).abs() < 0.001);
    assert!((game.units[0].attack_cooldown + windup - UnitKind::Brute.stats().cooldown).abs() < 0.001);
}

#[test]
fn progression_unlocks_units_and_chief_without_spending_experience() {
    let mut game = UnitesWar::new();
    game.start_battle();
    game.player_coins = 1000.0;
    assert!(game.enqueue_player_recruit(UnitKind::Runner));
    assert!(!game.enqueue_player_recruit(UnitKind::Archer));
    assert!(!game.summon_hero(WORLD_WIDTH, WORLD_HEIGHT));
    game.clan.experience[0] = 120.0;
    assert!(game.enqueue_player_recruit(UnitKind::Archer));
    assert!(!game.enqueue_player_recruit(UnitKind::Brute));
    game.clan.experience[0] = HERO_UNLOCK_XP;
    assert!(game.summon_hero(WORLD_WIDTH, WORLD_HEIGHT));
    assert_eq!(game.clan.experience[0], HERO_UNLOCK_XP);
    game.clan.experience[0] = 360.0;
    assert!(game.enqueue_player_recruit(UnitKind::Brute));
}

#[test]
fn population_reserves_queued_troops_and_returns_slots_after_death() {
    let mut game = UnitesWar::new();
    game.player_coins = MAX_COINS;
    for _ in 0..10 {
        assert!(game.enqueue_player_recruit(UnitKind::Guard));
    }
    let coins = game.player_coins;
    assert_eq!(game.population(Faction::Player), 20);
    assert!(!game.enqueue_player_recruit(UnitKind::Runner));
    assert_eq!(game.player_coins, coins);
    game.update_player_recruit_queue(1.0, WORLD_WIDTH, WORLD_HEIGHT);
    assert_eq!(game.population(Faction::Player), 20);
    game.units[0].health = 0.0;
    game.collect_defeated_units();
    assert_eq!(game.population(Faction::Player), 18);
    assert!(game.enqueue_player_recruit(UnitKind::Guard));
    assert!(game.buy_tower_upgrade(Faction::Player, TowerUpgrade::Health));
    assert_eq!(game.population_cap(Faction::Player), 20);
    assert!(!game.enqueue_player_recruit(UnitKind::Runner));
}

#[test]
fn enemy_population_and_unlocks_are_enforced_too() {
    let mut game = UnitesWar::new();
    game.enemy_coins = MAX_COINS;
    assert!(!game.recruit(Faction::Enemy, UnitKind::Brute, WORLD_WIDTH, WORLD_HEIGHT));
    for _ in 0..20 {
        assert!(game.recruit(Faction::Enemy, UnitKind::Runner, WORLD_WIDTH, WORLD_HEIGHT));
    }
    let coins = game.enemy_coins;
    assert!(!game.recruit(Faction::Enemy, UnitKind::Guard, WORLD_WIDTH, WORLD_HEIGHT));
    assert_eq!(game.enemy_coins, coins);
}

#[test]
fn attacking_deeper_rewards_more_gold_and_xp_once() {
    let reward = |x| {
        let mut game = UnitesWar::new();
        let mut victim = Unit::new(UnitKind::Guard, Faction::Enemy, Vec2::new(x, 320.0), 0);
        victim.health = 0.0;
        game.units.push(victim);
        let coins = game.collect_defeated_units().0;
        let xp = game.clan.experience[0];
        assert_eq!(game.clan.reputation, 1);
        assert_eq!(game.collect_defeated_units(), (0.0, 0.0));
        assert_eq!(game.clan.experience[0], xp);
        (coins, xp)
    };
    let home = reward(70.0);
    let away = reward(WORLD_WIDTH - 70.0);
    assert_eq!(away.0, home.0 * 2.0);
    assert_eq!(away.1, home.1 * 2.0);
}

#[test]
fn armor_preserves_health_ratio_and_applies_to_future_units() {
    let mut game = UnitesWar::new();
    game.player_coins = MAX_COINS;
    game.spawn_unit(Faction::Player, UnitKind::Guard, WORLD_WIDTH, WORLD_HEIGHT);
    game.units[0].health *= 0.5;
    let base = game.units[0].max_health;
    assert!(game.buy_research(Research::Armor));
    assert!((game.units[0].max_health - base * 1.2).abs() < 0.01);
    assert!((game.units[0].health / game.units[0].max_health - 0.5).abs() < 0.001);
    game.spawn_unit(Faction::Player, UnitKind::Guard, WORLD_WIDTH, WORLD_HEIGHT);
    assert_eq!(game.units[1].max_health, game.units[0].max_health);
    assert_eq!(game.units[1].health, game.units[1].max_health);
}

#[test]
fn research_costs_scale_and_stop_at_three_levels() {
    let mut game = UnitesWar::new();
    game.player_coins = MAX_COINS;
    for cost in [100.0, 210.0, 320.0] {
        let coins = game.player_coins;
        assert!(game.buy_research(Research::Weapons));
        assert_eq!(game.player_coins, coins - cost);
    }
    let coins = game.player_coins;
    assert!(!game.buy_research(Research::Weapons));
    assert_eq!(game.player_coins, coins);
    game.player_coins = 0.0;
    assert!(!game.buy_research(Research::Armor));
}

#[test]
fn weapons_increase_actual_combat_damage() {
    let damage = |level| {
        let mut game = UnitesWar::new();
        game.clan.research[Research::Weapons.index()] = level;
        game.units
            .push(Unit::new(UnitKind::Runner, Faction::Player, Vec2::new(400.0, 330.0), 0));
        game.units
            .push(Unit::new(UnitKind::Guard, Faction::Enemy, Vec2::new(420.0, 330.0), 0));
        let health = game.units[1].health;
        game.update_units(0.2, WORLD_WIDTH);
        let windup = game.units[0].attack_duration * character_sprites::hit_fraction(UnitKind::Runner);
        game.update_units(windup, WORLD_WIDTH);
        health - game.units[1].health
    };
    assert_eq!(damage(0), UnitKind::Runner.stats().damage);
    assert!((damage(1) - damage(0) * 1.2).abs() < 0.001);
}

#[test]
fn troop_research_does_not_change_passive_income() {
    let mut game = UnitesWar::new();
    game.clan.research[Research::Armor.index()] = 2;
    game.clan.difficulty = Difficulty::Warlord;
    game.player_coins = 0.0;
    game.enemy_coins = 0.0;
    game.update_economy(10.0);
    assert_eq!(game.player_coins, 60.0);
    assert!((game.enemy_coins - 78.0).abs() < 0.001);
}

#[test]
fn camera_transforms_spell_target_consistently_across_resolutions() {
    let mut game = UnitesWar::new();
    game.clan.camera = 900.0;
    for (width, height) in [(960.0, 540.0), (1280.0, 720.0), (1920.0, 1080.0), (3840.0, 2160.0)] {
        let target = game.screen_to_world(Vec2::new(width * 0.5, height * 0.5), width, height);
        assert!((target.x - 1380.0).abs() < 0.001);
        game.move_camera(10000.0, width, height);
        assert_eq!(game.clan.camera, WORLD_WIDTH - 960.0);
        game.move_camera(-10000.0, width, height);
        assert_eq!(game.clan.camera, 0.0);
        game.clan.camera = 900.0;
    }
}

#[test]
fn scenery_grounding_does_not_change_world_progress_on_resize() {
    let mut game = UnitesWar::new();
    game.spawn_unit(Faction::Player, UnitKind::Guard, WORLD_WIDTH, WORLD_HEIGHT);
    game.units[0].pos.x = 1200.0;
    game.align_forest_units();
    let before = game.units[0].pos;
    game.move_camera(1000.0, 1920.0, 1080.0);
    game.align_forest_units();
    assert_eq!(game.units[0].pos, before);
    assert_eq!(before.y + UnitKind::Guard.stats().size.y * 0.5, UnitesWar::ground_at(1200.0));
    assert!(UnitesWar::ground_at(1200.0) > UnitesWar::ground_at(70.0));
}

#[test]
fn hud_clicks_do_not_cast_selected_spell_and_field_uses_camera() {
    let mut game = UnitesWar::new();
    game.start_battle();
    game.clan.camera = 800.0;
    game.clan.panel_open = true;
    game.select_spell(Spell::Lightning);
    game.handle_war_click(Vec2::new(20.0, 20.0), 960.0, 540.0);
    assert_eq!(game.spell_cooldown, 0.0);
    let (p, z) = UnitesWar::button_rect(0, 960.0, 540.0);
    game.handle_war_click(p + z * 0.5, 960.0, 540.0);
    assert_eq!(game.player_recruit_queue.len(), 1);
    assert_eq!(game.spell_cooldown, 0.0);
    game.units
        .push(Unit::new(UnitKind::Guard, Faction::Enemy, Vec2::new(1200.0, 360.0), 0));
    game.handle_war_click(Vec2::new(400.0, 320.0), 960.0, 540.0);
    assert!(game.units[0].health < game.units[0].max_health);
    assert_eq!(game.spell_cooldown, SPELL_COOLDOWN);
    assert!(game.clan.selected_spell.is_none());
}

#[test]
fn meteor_requires_xp_and_only_damages_enemies_in_area() {
    let mut game = UnitesWar::new();
    game.select_spell(Spell::Meteor);
    assert_eq!(game.clan.selected_spell, None);
    game.clan.experience[0] = 500.0;
    game.select_spell(Spell::Meteor);
    for (faction, x) in [
        (Faction::Enemy, 2300.0),
        (Faction::Player, 2290.0),
        (Faction::Enemy, 1900.0),
    ] {
        game.units
            .push(Unit::new(UnitKind::Brute, faction, Vec2::new(x, 280.0), 0));
    }
    assert!(game.cast_selected_spell(Vec2::new(2300.0, 310.0)));
    assert_eq!(game.units[0].health, 140.0);
    assert_eq!(game.units[1].health, 310.0);
    assert_eq!(game.units[2].health, 310.0);
    assert_eq!(game.enemy_castle.health, 900.0);
    assert_eq!(game.clan.meteor_cooldown, 32.0);
    game.select_spell(Spell::Meteor);
    assert_eq!(game.clan.selected_spell, None);
}

#[test]
fn chief_can_recall_under_attack_enter_castle_and_return_after_30_seconds() {
    let mut game = UnitesWar::new();
    game.start_battle();
    game.clan.experience[0] = HERO_UNLOCK_XP;
    game.summon_hero(WORLD_WIDTH, WORLD_HEIGHT);
    game.hero.pos.x = 500.0;
    game.units
        .push(Unit::new(UnitKind::Guard, Faction::Enemy, Vec2::new(520.0, 300.0), 0));
    assert!(game.hero_in_combat(WORLD_WIDTH));
    game.set_hero_direction(-1.0);
    game.update_hero_movement(0.1, WORLD_WIDTH, WORLD_HEIGHT);
    assert!(game.hero.pos.x < 500.0);
    game.hero.pos.x = 140.0;
    game.hero.health = 40.0;
    game.update_clan(1.0);
    assert_eq!(game.hero.health, 40.0);
    assert!(game.hero.alive());
    game.update_hero_movement(5.0, WORLD_WIDTH, WORLD_HEIGHT);
    assert!(game.hero.in_castle);
    assert!(!game.hero.alive());
    assert_eq!(game.hero.pos.x, UnitesWar::castle_x(Faction::Player, WORLD_WIDTH));
    assert!(!game.set_hero_direction(1.0));
    assert_eq!(game.apply_hero_damage(100.0, 0.1), 0.0);
    game.update_clan(29.0);
    assert!(!game.summon_hero(WORLD_WIDTH, WORLD_HEIGHT));
    game.update_clan(1.0);
    assert!(game.summon_hero(WORLD_WIDTH, WORLD_HEIGHT));
    assert_eq!(game.hero.health, HERO_MAX_HEALTH);
}

#[test]
fn fury_doubles_hero_damage_and_expires() {
    let mut game = UnitesWar::new();
    game.start_battle();
    assert!(!game.cast_fury());
    game.clan.experience[0] = HERO_UNLOCK_XP;
    game.summon_hero(WORLD_WIDTH, WORLD_HEIGHT);
    let x = game.hero.pos.x + 50.0;
    game.units
        .push(Unit::new(UnitKind::Brute, Faction::Enemy, Vec2::new(x, 280.0), 0));
    assert!(game.cast_fury());
    assert!(!game.cast_fury());
    game.fire_hero(WORLD_WIDTH);
    assert_eq!(game.units[0].health, 310.0);
    game.update_arrows(1.0, WORLD_WIDTH, game.arrows.len());
    assert_eq!(game.units[0].health, 310.0 - HERO_DAMAGE * 2.0);
    game.update_clan(8.0);
    assert_eq!(game.clan.fury_time, 0.0);
    assert!(game.clan.fury_cooldown > 0.0);
}

#[test]
fn pause_freezes_the_entire_simulation() {
    let mut game = UnitesWar::new();
    game.start_battle();
    game.enqueue_player_recruit(UnitKind::Runner);
    game.clan.paused = true;
    let coins = game.player_coins;
    let timer = game.player_recruit_timer;
    game.update_battle(10.0, WORLD_WIDTH, WORLD_HEIGHT);
    assert_eq!(game.player_coins, coins);
    assert_eq!(game.player_recruit_timer, timer);
    assert_eq!(game.clan.elapsed, 0.0);
    assert!(game.units.is_empty());
}

#[test]
fn upgrades_tab_buys_research_and_ignores_removed_talent_slot() {
    let mut game = UnitesWar::new();
    game.start_battle();
    game.player_coins = 1000.0;
    game.clan.tab = CommandTab::Research;
    game.clan.panel_open = true;
    let (pos, size) = UnitesWar::button_rect(4, 1280.0, 720.0);
    game.handle_war_click(pos + size * 0.5, 1280.0, 720.0);
    assert_eq!(game.player_coins, 1000.0);
    assert_eq!(game.clan.research, [0; 2]);
    game.update_battle(0.1, WORLD_WIDTH, WORLD_HEIGHT);
    assert!(game.clan.elapsed > 0.0);

    let coins = game.player_coins;
    let cost = game.research_cost(Research::Weapons);
    game.clan.panel_open = true;
    let (pos, size) = UnitesWar::button_rect(0, 1280.0, 720.0);
    game.handle_war_click(pos + size * 0.5, 1280.0, 720.0);
    assert_eq!(game.clan.research[Research::Weapons.index()], 1);
    assert!((game.player_coins - (coins - cost)).abs() < 0.001);
}

#[test]
fn restart_preserves_preferences_and_clears_battle_progress() {
    let mut game = UnitesWar::new();
    game.clan.difficulty = Difficulty::Warlord;
    game.selected_resolution = 4;
    game.clan.experience[0] = 2000.0;
    game.clan.research = [2; 2];
    game.clan.enemy_research = [2; 2];
    game.clan.camera = 900.0;
    game.clan.paused = true;
    game.clan.selected_spell = Some(Spell::Meteor);
    game.reset();
    assert_eq!(game.clan.difficulty, Difficulty::Warlord);
    assert_eq!(game.selected_resolution, 4);
    assert_eq!(game.clan.experience, [0.0; 2]);
    assert_eq!(game.clan.research, [0; 2]);
    assert_eq!(game.clan.enemy_research, [0; 2]);
    assert_eq!(game.clan.camera, 0.0);
    assert!(!game.clan.paused);
    assert!(game.clan.selected_spell.is_none());
    assert_eq!(game.player_coins, STARTING_COINS);
}

#[test]
fn animation_moves_limbs_without_moving_the_unit_off_the_ground() {
    for kind in UnitKind::ALL {
        let mut unit = Unit::new(kind, Faction::Player, Vec2::new(600.0, 320.0), 0);
        assert_eq!(unit.animation_pose(), Vec2::ZERO);
        unit.moving = true;
        unit.animation_time = 0.1;
        assert!(unit.animation_pose().x > 0.0);
        unit.attack_animation = kind.attack_animation_duration() * 0.5;
        assert!((unit.animation_pose().y - 1.0).abs() < 0.001);
        assert_eq!(unit.pos, Vec2::new(600.0, 320.0));
    }
}

#[test]
fn an_undefended_fortress_eventually_loses_on_every_difficulty() {
    for difficulty in Difficulty::ALL {
        let mut game = UnitesWar::new();
        game.clan.difficulty = difficulty;
        game.start_battle();
        for _ in 0..18000 {
            game.update_battle(0.05, WORLD_WIDTH, WORLD_HEIGHT);
            game.align_forest_units();
            if game.state != BattleState::Playing {
                break;
            }
        }
        assert_eq!(
            game.state,
            BattleState::Defeat,
            "{:?} stalled after {}s",
            difficulty,
            game.clan.elapsed
        );
        assert_eq!(game.player_castle.health, 0.0);
    }
}

#[test]
fn a_recruited_army_can_complete_a_battle() {
    let mut game = UnitesWar::new();
    game.clan.difficulty = Difficulty::Scout;
    game.start_battle();
    for tick in 0..18000 {
        if tick % 30 == 0 {
            let kind = if game.unit_unlocked(UnitKind::Brute, Faction::Player) && tick % 90 == 0 {
                UnitKind::Brute
            } else if game.unit_unlocked(UnitKind::Archer, Faction::Player) && tick % 60 == 0 {
                UnitKind::Archer
            } else {
                UnitKind::Guard
            };
            game.enqueue_player_recruit(kind);
            game.summon_hero(WORLD_WIDTH, WORLD_HEIGHT);
        }
        game.update_battle(0.05, WORLD_WIDTH, WORLD_HEIGHT);
        game.align_forest_units();
        assert!(game.population(Faction::Player) <= game.population_cap(Faction::Player));
        assert!(game.population(Faction::Enemy) <= game.population_cap(Faction::Enemy));
        assert!(game.player_coins.is_finite() && game.player_coins >= 0.0);
        if game.state != BattleState::Playing {
            break;
        }
    }
    assert_eq!(
        game.state,
        BattleState::Victory,
        "scripted army ended at {:?} after {}s",
        game.state,
        game.clan.elapsed
    );
}

#[test]
fn hero_arrow_does_not_continue_into_castle_behind_target() {
    let mut game = UnitesWar::new();
    game.start_battle();
    game.clan.experience[0] = HERO_UNLOCK_XP;
    game.summon_hero(WORLD_WIDTH, WORLD_HEIGHT);
    let x = UnitesWar::castle_x(Faction::Enemy, WORLD_WIDTH);
    game.hero.pos = Vec2::new(x - 150.0, 300.0);
    game.units
        .push(Unit::new(UnitKind::Guard, Faction::Enemy, Vec2::new(x - 80.0, 300.0), 0));
    let castle_health = game.enemy_castle.health;
    assert!(game.fire_hero(WORLD_WIDTH));
    game.update_arrows(2.0, WORLD_WIDTH, game.arrows.len());
    assert!(game.units[0].health < game.units[0].max_health);
    assert_eq!(game.enemy_castle.health, castle_health);
    assert!(game.arrows.is_empty());
}

#[test]
fn hero_recall_cooldown_freezes_when_paused_and_clears_on_restart() {
    let mut game = UnitesWar::new();
    game.start_battle();
    game.clan.experience[0] = HERO_UNLOCK_XP;
    game.summon_hero(WORLD_WIDTH, WORLD_HEIGHT);
    game.set_hero_direction(-1.0);
    game.update_hero_movement(10.0, WORLD_WIDTH, WORLD_HEIGHT);
    game.clan.paused = true;
    game.update_battle(40.0, WORLD_WIDTH, WORLD_HEIGHT);
    assert_eq!(game.clan.hero_respawn, 0.0);
    assert!(!game.summon_hero(WORLD_WIDTH, WORLD_HEIGHT));
    game.reset();
    assert!(!game.hero.in_castle);
    assert!(!game.hero.summoned);
    assert_eq!(game.clan.hero_respawn, 0.0);
}

#[test]
fn defeated_hero_still_requires_30_seconds_before_resummoning() {
    let mut game = UnitesWar::new();
    game.start_battle();
    game.clan.experience[0] = HERO_UNLOCK_XP;
    game.summon_hero(WORLD_WIDTH, WORLD_HEIGHT);
    game.apply_hero_damage(HERO_MAX_HEALTH, 0.1);
    game.update_clan(29.99);
    assert!(!game.summon_hero(WORLD_WIDTH, WORLD_HEIGHT));
    game.update_clan(0.02);
    assert!(game.summon_hero(WORLD_WIDTH, WORLD_HEIGHT));
    assert_eq!(game.hero.health, HERO_MAX_HEALTH);
}

#[test]
fn reinforcements_queue_behind_fighting_allies_in_both_directions() {
    for faction in [Faction::Player, Faction::Enemy] {
        let mut game = UnitesWar::new();
        let d = faction.direction();
        let front = if faction == Faction::Player { 500.0 } else { 1900.0 };
        // Deliberately store the rear first; resolution must follow position.
        for (kind, offset) in [
            (UnitKind::Runner, -200.0),
            (UnitKind::Guard, 0.0),
            (UnitKind::Runner, -100.0),
        ] {
            game.units
                .push(Unit::new(kind, faction, Vec2::new(front + d * offset, 300.0), 0));
        }
        game.units.push(Unit::new(
            UnitKind::Guard,
            faction.opposite(),
            Vec2::new(front + d * 40.0, 300.0),
            0,
        ));
        game.update_units(10.0, WORLD_WIDTH);
        assert_eq!(game.units[1].pos.x, front);
        let front_gap = (game.units[1].pos.x - game.units[2].pos.x) * d;
        let back_gap = (game.units[2].pos.x - game.units[0].pos.x) * d;
        assert!(front_gap >= (31.0 + 25.0) * 0.58 - 0.001);
        assert!(back_gap >= 50.0 * 0.58 - 0.001);
        game.update_units(0.01, WORLD_WIDTH);
        assert!(!game.units[0].moving);
        assert!(!game.units[2].moving);
        game.units[3].health = 0.0;
        game.units[1].pending_attack = None;
        game.units[1].attack_animation = 0.0;
        let previous = game.units[0].pos.x;
        game.update_units(0.1, WORLD_WIDTH);
        assert!((game.units[0].pos.x - previous) * d > 0.0);
    }
}

#[test]
fn archers_can_fire_from_behind_a_fighting_front_line() {
    let mut game = UnitesWar::new();
    for (kind, faction, x) in [
        (UnitKind::Guard, Faction::Player, 500.0),
        (UnitKind::Archer, Faction::Player, 400.0),
        (UnitKind::Guard, Faction::Enemy, 540.0),
    ] {
        game.units.push(Unit::new(kind, faction, Vec2::new(x, 300.0), 0));
    }
    game.units[1].attack_cooldown = 0.0;
    game.update_units(0.1, WORLD_WIDTH);
    assert_eq!(game.units[1].pos.x, 400.0);
    assert!(game.units[1].pending_attack.is_some());
}

#[test]
fn overlapping_reinforcements_separate_when_front_enters_combat() {
    let mut game = UnitesWar::new();
    for (faction, x) in [
        (Faction::Player, 500.0),
        (Faction::Player, 495.0),
        (Faction::Enemy, 540.0),
    ] {
        game.units
            .push(Unit::new(UnitKind::Guard, faction, Vec2::new(x, 300.0), 0));
    }
    game.update_units(0.1, WORLD_WIDTH);
    assert!((game.units[0].pos.x - game.units[1].pos.x) >= 62.0 * 0.58 - 0.001);
    assert!(!game.units[1].moving);
    assert!(game.units[1].pending_attack.is_none());
}

#[test]
fn enemy_reinforcements_hold_the_line_on_every_difficulty() {
    for difficulty in Difficulty::ALL {
        for front_kind in UnitKind::ALL {
            let mut game = UnitesWar::new();
            game.clan.difficulty = difficulty;
            game.units
                .push(Unit::new(front_kind, Faction::Enemy, Vec2::new(1000.0, 300.0), 0));
            game.units
                .push(Unit::new(UnitKind::Runner, Faction::Enemy, Vec2::new(1100.0, 300.0), 0));
            let distance = front_kind.stats().range + 10.0;
            game.units.push(Unit::new(
                UnitKind::Guard,
                Faction::Player,
                Vec2::new(1000.0 - distance, 300.0),
                0,
            ));
            game.update_units(2.0, WORLD_WIDTH);
            assert_eq!(game.units[0].pos.x, 1000.0);
            let gap = game.units[1].pos.x - game.units[0].pos.x;
            assert!(
                gap >= (front_kind.stats().size.x + 25.0) * 0.58 - 0.001,
                "{difficulty:?} {front_kind:?}"
            );
            let previous = game.units[1].pos.x;
            game.update_units(0.01, WORLD_WIDTH);
            assert!((game.units[1].pos.x - previous).abs() < 0.001);
        }
    }
}

#[test]
fn fast_troops_overtake_slow_allies_without_interrupting_their_run_cycle() {
    for faction in [Faction::Player, Faction::Enemy] {
        let mut game = UnitesWar::new();
        let d = faction.direction();
        game.units
            .push(Unit::new(UnitKind::Brute, faction, Vec2::new(1100.0, 300.0), 0));
        game.units
            .push(Unit::new(UnitKind::Runner, faction, Vec2::new(1100.0 - d * 30.0, 300.0), 0));
        let start = game.units[1].pos.x;
        for step in 1..=60 {
            game.update_units(1.0 / 60.0, WORLD_WIDTH);
            assert!(game.units[1].moving, "{faction:?}, frame {step}");
            assert!((game.units[1].animation_time - step as f32 / 60.0).abs() < 0.001);
        }
        assert!((game.units[1].pos.x - start - d * UnitKind::Runner.stats().speed).abs() < 0.01);
        assert!((game.units[1].pos.x - game.units[0].pos.x) * d > 0.0);
    }
}

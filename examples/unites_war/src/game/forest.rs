use super::*;

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgba8(r, g, b, 255)
}

impl UnitesWar {
    pub(super) fn ellipse(ctx: &mut dyn GameContext<Action>, center: Vec2, radius: Vec2, color: Color) {
        for i in 0..32 {
            let a = i as f32 * std::f32::consts::TAU / 32.0;
            let b = (i + 1) as f32 * std::f32::consts::TAU / 32.0;
            ctx.draw_triangle(
                center,
                center + Vec2::new(a.cos() * radius.x, a.sin() * radius.y),
                center + Vec2::new(b.cos() * radius.x, b.sin() * radius.y),
                color,
            );
        }
    }

    fn forest_tree(ctx: &mut dyn GameContext<Action>, foot: Vec2, scale: f32, distant: bool) {
        let bark = if distant { rgb(76, 106, 69) } else { rgb(73, 67, 39) };
        let p = |x, y| foot + Vec2::new(x, y) * scale;
        Self::draw_line(ctx, p(0.0, 0.0), p(-3.0, -77.0), 10.0 * scale, bark);
        Self::draw_line(ctx, p(-1.0, -38.0), p(-22.0, -67.0), 5.0 * scale, bark);
        Self::draw_line(ctx, p(-1.0, -49.0), p(25.0, -86.0), 5.0 * scale, bark);
        for (x, y, radius) in [
            (-24.0, -73.0, 24.0),
            (22.0, -85.0, 27.0),
            (-3.0, -103.0, 31.0),
            (-5.0, -69.0, 27.0),
        ] {
            Self::ellipse(
                ctx,
                p(x, y),
                Vec2::new(radius, radius * 0.82) * scale,
                if distant { rgb(84, 130, 74) } else { rgb(44, 83, 38) },
            );
            Self::ellipse(
                ctx,
                p(x - 3.0, y - 4.0),
                Vec2::new(radius - 4.0, radius * 0.69) * scale,
                if distant { rgb(104, 147, 81) } else { rgb(76, 124, 45) },
            );
            if !distant {
                Self::ellipse(
                    ctx,
                    p(x - 8.0, y - 9.0),
                    Vec2::new(radius * 0.53, radius * 0.37) * scale,
                    rgb(119, 158, 52),
                );
            }
        }
    }

    fn draw_procedural_forest(&self, ctx: &mut dyn GameContext<Action>, width: f32, height: f32) {
        let scale = Self::view_scale(width, height);
        let visible = width / scale;
        let camera = self.clan.camera;
        let point = |x: f32, y: f32| Vec2::new((x - camera) * scale, y * scale);
        ctx.clear_color(rgb(134, 192, 211));
        for band in 0..32 {
            let t = band as f32 / 31.0;
            ctx.draw_rect(
                Vec2::new(0.0, band as f32 * 10.0 * scale),
                Vec2::new(width, 11.0 * scale),
                rgb(105, 173, 200).lerp(rgb(216, 223, 177), t),
            );
        }
        let sun = Vec2::new((720.0 - camera * 0.08) * scale, 109.0 * scale);
        Self::ellipse(ctx, sun, Vec2::splat(43.0 * scale), Color::from_rgba8(255, 240, 182, 45));
        Self::ellipse(ctx, sun, Vec2::splat(31.0 * scale), rgb(255, 239, 172));
        for i in 0..8 {
            let x = (i as f32 * 220.0 - camera * 0.15).rem_euclid(visible + 200.0) - 100.0;
            let y = 93.0 + (i % 3) as f32 * 25.0;
            Self::ellipse(
                ctx,
                Vec2::new(x, y) * scale,
                Vec2::new(54.0, 9.0) * scale,
                Color::from_rgba8(241, 243, 219, 155),
            );
            Self::ellipse(
                ctx,
                Vec2::new(x - 11.0, y - 9.0) * scale,
                Vec2::new(27.0, 17.0) * scale,
                Color::from_rgba8(241, 243, 219, 155),
            );
        }
        for i in 0..9 {
            let x = i as f32 * 235.0 - camera * 0.28 - 100.0;
            let peak = 132.0 + (i % 3) as f32 * 19.0;
            ctx.draw_triangle(
                Vec2::new(x - 165.0, 280.0) * scale,
                Vec2::new(x, peak) * scale,
                Vec2::new(x + 185.0, 280.0) * scale,
                rgb(121, 163, 167),
            );
            ctx.draw_triangle(
                Vec2::new(x, peak) * scale,
                Vec2::new(x + 185.0, 280.0) * scale,
                Vec2::new(x + 37.0, 253.0) * scale,
                rgb(100, 147, 152),
            );
            ctx.draw_triangle(
                Vec2::new(x - 34.0, peak + 31.0) * scale,
                Vec2::new(x, peak) * scale,
                Vec2::new(x + 39.0, peak + 31.0) * scale,
                rgb(224, 231, 210),
            );
        }
        for layer in 0..3 {
            let base = 252.0 + layer as f32 * 30.0;
            let color = [rgb(120, 157, 108), rgb(104, 146, 81), rgb(127, 161, 74)][layer];
            for i in 0..(visible / 16.0) as usize + 1 {
                let x = i as f32 * 16.0;
                let y = |xx: f32| base + ((xx + camera * (0.32 + layer as f32 * 0.12)) / 185.0).sin() * 28.0;
                Self::draw_quad(
                    ctx,
                    Vec2::new(x, y(x)) * scale,
                    Vec2::new(x + 17.0, y(x + 17.0)) * scale,
                    Vec2::new(x + 17.0, 415.0) * scale,
                    Vec2::new(x, 415.0) * scale,
                    color,
                );
            }
        }
        // The river belongs to the distant valley; the army walks along its near bank.
        Self::draw_quad(
            ctx,
            point(1115.0, 260.0),
            point(1130.0, 260.0),
            point(1350.0, 403.0),
            point(1190.0, 403.0),
            rgb(107, 174, 169),
        );
        Self::draw_line(ctx, point(1122.0, 268.0), point(1270.0, 390.0), 5.0 * scale, rgb(181, 211, 183));
        for i in 0..40 {
            let x = i as f32 * 72.0 + 23.0;
            if x < camera - 100.0 || x > camera + visible + 100.0 {
                continue;
            }
            let size = 0.48 + (i * 17 % 9) as f32 * 0.075;
            Self::forest_tree(ctx, point(x, Self::ground_at(x) - 25.0), size * scale, true);
        }
        for i in 0..(visible / 10.0) as usize + 2 {
            let x = camera + i as f32 * 10.0 - 10.0;
            let y = Self::ground_at(x);
            let next = Self::ground_at(x + 11.0);
            Self::draw_quad(
                ctx,
                point(x, y - 5.0),
                point(x + 11.0, next - 5.0),
                point(x + 11.0, height / scale),
                point(x, height / scale),
                rgb(116, 92, 52),
            );
            Self::draw_quad(
                ctx,
                point(x, y + 9.0),
                point(x + 11.0, next + 9.0),
                point(x + 11.0, height / scale),
                point(x, height / scale),
                rgb(88, 76, 45),
            );
            Self::draw_line(
                ctx,
                point(x, y - 3.0),
                point(x + 11.0, next - 3.0),
                8.0 * scale,
                rgb(67, 103, 40),
            );
            Self::draw_line(
                ctx,
                point(x, y - 6.0),
                point(x + 11.0, next - 6.0),
                3.0 * scale,
                rgb(166, 183, 74),
            );
        }
        for i in 0..110 {
            let x = i as f32 * 23.0 + 8.0;
            if x < camera || x > camera + visible {
                continue;
            }
            let y = Self::ground_at(x) + 18.0 + (i * 11 % 23) as f32;
            Self::ellipse(
                ctx,
                point(x, y),
                Vec2::new(3.0 + (i % 4) as f32, 1.8) * scale,
                rgb(116, 100, 59),
            );
        }
        for x in [225.0, 426.0, 1870.0, 2180.0] {
            if x >= camera - 80.0 && x <= camera + visible + 80.0 {
                Self::forest_tree(ctx, point(x, Self::ground_at(x) - 9.0), 1.05 * scale, false);
            }
        }
    }

    pub(super) fn draw_forest_world(&self, ctx: &mut dyn GameContext<Action>, width: f32, height: f32) {
        let scale = Self::view_scale(width, height);
        let visible = width / scale;
        let camera = self.clan.camera;
        let point = |x: f32, y: f32| Vec2::new((x - camera) * scale, y * scale);
        if let Some(sprites) = &self.forest_sprites {
            sprites.draw(ctx, camera, width, height);
        } else {
            self.draw_procedural_forest(ctx, width, height);
        }
        for (faction, castle) in [
            (Faction::Player, &self.player_castle),
            (Faction::Enemy, &self.enemy_castle),
        ] {
            let x = Self::castle_x(faction, WORLD_WIDTH);
            self.draw_forest_tower(ctx, point(x, Self::ground_at(x)), scale, castle, faction);
        }
        for unit in &self.units {
            if !unit.alive() || unit.pos.x < camera - 75.0 || unit.pos.x > camera + visible + 75.0 {
                continue;
            }
            let foot = point(unit.pos.x, Self::ground_at(unit.pos.x));
            if !self.draw_blender_unit(ctx, foot, scale, unit) {
                Self::draw_goblin(
                    ctx,
                    foot,
                    scale * unit.kind.stats().size.y / 43.0,
                    unit.kind,
                    unit.faction,
                    unit.animation_pose(),
                    unit.hit_flash > 0.0,
                    false,
                );
            }
            if unit.health < unit.max_health || unit.attack_animation > 0.0 {
                Self::draw_bar(
                    ctx,
                    foot + Vec2::new(-15.0, -unit.kind.stats().size.y - 9.0) * scale,
                    Vec2::new(30.0, 4.0) * scale,
                    unit.health / unit.max_health,
                    if unit.faction == Faction::Player {
                        rgb(143, 216, 72)
                    } else {
                        rgb(225, 97, 68)
                    },
                );
            }
        }
        if self.hero.alive() {
            let foot = point(self.hero.pos.x, Self::ground_at(self.hero.pos.x));
            Self::ellipse(
                ctx,
                foot + Vec2::new(0.0, -2.0) * scale,
                Vec2::new(24.0, 5.0) * scale,
                if self.clan.fury_time > 0.0 {
                    rgb(228, 137, 40)
                } else {
                    rgb(214, 192, 92)
                },
            );
            if !self.draw_blender_hero(ctx, foot, scale) {
                Self::draw_goblin(
                    ctx,
                    foot,
                    scale * 1.25,
                    UnitKind::Archer,
                    Faction::Player,
                    Vec2::new(
                        (self.clan.elapsed * 11.0).sin() * if self.hero_in_combat(WORLD_WIDTH) { 0.0 } else { 1.0 },
                        self.hero.shot_pause / HERO_SHOT_PAUSE,
                    ),
                    self.hero.hit_flash > 0.0,
                    true,
                );
            }
            Self::draw_bar(
                ctx,
                foot + Vec2::new(-22.0, -70.0) * scale,
                Vec2::new(44.0, 5.0) * scale,
                self.hero.health / self.hero.max_health,
                rgb(235, 196, 86),
            );
        }
        self.draw_cannonballs(ctx, scale);
        for arrow in &self.arrows {
            // The rendered tip is the same position used to resolve impact.
            let tip = point(arrow.pos.x, arrow.pos.y);
            let direction = arrow.direction;
            let base = tip - direction * 4.0 * scale;
            let side = Vec2::new(-direction.y, direction.x) * 3.0 * scale;
            Self::draw_line(ctx, tip - direction * 24.0 * scale, base, 2.0 * scale, arrow.color);
            ctx.draw_triangle(tip, base + side, base - side, rgb(248, 228, 177));
        }
        for effect in &self.effects {
            let t = 1.0 - effect.life / effect.max_life;
            let end = point(effect.end.x, effect.end.y);
            let start = point(effect.start.x, effect.start.y);
            match effect.kind {
                AttackEffectKind::Lightning => {
                    let mut previous = start;
                    for i in 1..9 {
                        let p = start
                            + (end - start) * (i as f32 / 8.0)
                            + Vec2::new(
                                if i == 8 {
                                    0.0
                                } else {
                                    ((i * 19 % 29) as f32 - 14.0) * scale
                                },
                                0.0,
                            );
                        Self::draw_line(ctx, previous, p, 5.0 * scale * (1.0 - t), rgb(253, 242, 161));
                        Self::draw_line(ctx, previous, p, 1.7 * scale, Color::WHITE);
                        previous = p;
                    }
                }
                AttackEffectKind::Meteor => {
                    let head = start + (end - start) * (t * 1.8).min(1.0);
                    Self::draw_line(ctx, head - (end - start) * 0.13, head, 11.0 * scale, rgb(225, 96, 36));
                    Self::ellipse(ctx, head, Vec2::splat(10.0 * scale), rgb(255, 199, 77));
                    if t > 0.55 {
                        Self::ellipse(
                            ctx,
                            end,
                            Vec2::new(150.0 * (t - 0.5), 16.0) * scale,
                            Color::from_rgba8(255, 167, 49, ((1.0 - t) * 255.0) as u8),
                        );
                    }
                }
                AttackEffectKind::CannonImpact { radius } => {
                    let alpha = ((1.0 - t) * 190.0) as u8;
                    Self::ellipse(
                        ctx,
                        end,
                        Vec2::splat(radius * t.sqrt() * scale),
                        Color::from_rgba8(212, 128, 51, alpha / 2),
                    );
                    Self::ellipse(
                        ctx,
                        end,
                        Vec2::splat((5.0 + 18.0 * t) * scale),
                        Color::from_rgba8(255, 207, 109, alpha),
                    );
                    for i in 0..8 {
                        let angle = i as f32 * std::f32::consts::TAU / 8.0;
                        let direction = Vec2::new(angle.cos(), angle.sin());
                        Self::draw_line(
                            ctx,
                            end + direction * 10.0 * t * scale,
                            end + direction * (12.0 + 40.0 * t) * scale,
                            2.0 * scale,
                            Color::from_rgba8(224, 171, 94, alpha),
                        );
                    }
                }
                _ => {
                    for i in 0..5 {
                        let angle = i as f32 * 1.25;
                        let delta = Vec2::new(angle.cos(), angle.sin());
                        Self::draw_line(
                            ctx,
                            end + delta * 3.0 * scale,
                            end + delta * (8.0 + t * 12.0) * scale,
                            2.0 * scale * (1.0 - t),
                            effect.color,
                        );
                    }
                }
            }
        }
        if self.clan.selected_spell.is_some()
            && let Some(mouse) = ctx.input().mouse_position()
            && self.is_battlefield_point(mouse, width, height)
        {
            let x = mouse.x / scale + camera;
            Self::ellipse(
                ctx,
                point(x, Self::ground_at(x)),
                Vec2::new(SPELL_RADIUS, 11.0) * scale,
                Color::from_rgba8(255, 227, 123, 65),
            );
            Self::draw_line(
                ctx,
                point(x, Self::ground_at(x) - 55.0),
                point(x, Self::ground_at(x) - 15.0),
                2.0 * scale,
                rgb(255, 235, 161),
            );
        }
    }

    fn draw_forest_tower(
        &self,
        ctx: &mut dyn GameContext<Action>,
        foot: Vec2,
        scale: f32,
        castle: &Castle,
        faction: Faction,
    ) {
        if self.forest_sprites.is_some() {
            // The Blender background includes both towers, but health and
            // upgrade indicators remain live game data.
            self.draw_cannon(ctx, foot, scale, castle, faction);
            Self::draw_forest_tower_status(ctx, foot, scale, castle, faction);
            return;
        }
        let p = |x, y| foot + Vec2::new(x, y) * scale;
        let banner = if faction == Faction::Player {
            rgb(90, 128, 59)
        } else {
            rgb(158, 60, 43)
        };
        Self::ellipse(
            ctx,
            p(0.0, 0.0),
            Vec2::new(65.0, 8.0) * scale,
            Color::from_rgba8(29, 36, 25, 110),
        );
        ctx.draw_rect(p(-44.0, -136.0), Vec2::new(88.0, 136.0) * scale, rgb(46, 49, 42));
        ctx.draw_rect(p(-40.0, -132.0), Vec2::new(80.0, 132.0) * scale, rgb(121, 120, 96));
        ctx.draw_rect(p(17.0, -132.0), Vec2::new(23.0, 132.0) * scale, rgb(88, 92, 75));
        for row in 0..8 {
            let y = -128.0 + row as f32 * 16.0;
            Self::draw_line(ctx, p(-39.0, y), p(39.0, y), scale, rgb(69, 73, 59));
            for col in 0..3 {
                let x = -30.0 + col as f32 * 27.0 + (row % 2) as f32 * 10.0;
                Self::draw_line(ctx, p(x, y), p(x, y + 15.0), scale, rgb(69, 73, 59));
            }
        }
        ctx.draw_rect(p(-49.0, -144.0), Vec2::new(98.0, 18.0) * scale, rgb(49, 53, 44));
        ctx.draw_rect(p(-46.0, -142.0), Vec2::new(92.0, 10.0) * scale, rgb(143, 140, 112));
        for i in 0..5 {
            ctx.draw_rect(
                p(-48.0 + i as f32 * 21.0, -157.0),
                Vec2::new(15.0, 17.0) * scale,
                rgb(54, 58, 46),
            );
            ctx.draw_rect(
                p(-46.0 + i as f32 * 21.0, -155.0),
                Vec2::new(11.0, 14.0) * scale,
                rgb(139, 137, 108),
            );
        }
        Self::ellipse(ctx, p(0.0, -36.0), Vec2::new(19.0, 23.0) * scale, rgb(43, 44, 35));
        ctx.draw_rect(p(-19.0, -38.0), Vec2::new(38.0, 38.0) * scale, rgb(43, 44, 35));
        ctx.draw_rect(p(-14.0, -38.0), Vec2::new(28.0, 37.0) * scale, rgb(73, 58, 35));
        for x in [-9.0, 0.0, 9.0] {
            Self::draw_line(ctx, p(x, -38.0), p(x, -1.0), 2.0 * scale, rgb(42, 40, 29));
        }
        ctx.draw_rect(p(-15.0, -96.0), Vec2::new(30.0, 24.0) * scale, rgb(40, 45, 36));
        self.draw_cannon(ctx, foot, scale, castle, faction);
        Self::draw_line(ctx, p(-13.0, -157.0), p(-13.0, -213.0), 3.0 * scale, rgb(64, 59, 38));
        Self::draw_quad(
            ctx,
            p(-12.0, -211.0),
            p(26.0, -204.0),
            p(24.0, -177.0),
            p(-12.0, -184.0),
            banner,
        );
        Self::ellipse(ctx, p(5.0, -194.0), Vec2::new(7.0, 6.0) * scale, rgb(220, 207, 155));
        Self::draw_forest_tower_status(ctx, foot, scale, castle, faction);
    }

    fn draw_forest_tower_status(
        ctx: &mut dyn GameContext<Action>,
        foot: Vec2,
        scale: f32,
        castle: &Castle,
        faction: Faction,
    ) {
        let p = |x, y| foot + Vec2::new(x, y) * scale;
        let banner = if faction == Faction::Player {
            rgb(90, 128, 59)
        } else {
            rgb(158, 60, 43)
        };
        for (i, level) in castle.upgrades.iter().enumerate() {
            if *level == 0 {
                continue;
            }
            Self::ellipse(
                ctx,
                p(-30.0 + i as f32 * 15.0, -116.0),
                Vec2::splat(4.0 * scale),
                rgb(226, 186, 84),
            );
        }
        Self::draw_bar(
            ctx,
            p(-47.0, -170.0),
            Vec2::new(94.0, 6.0) * scale,
            castle.health / castle.max_health,
            banner.lerp(Color::WHITE, 0.25),
        );
    }

    // Art is drawn in local coordinates so weapons and legs animate continuously.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_goblin(
        ctx: &mut dyn GameContext<Action>,
        foot: Vec2,
        scale: f32,
        kind: UnitKind,
        faction: Faction,
        pose: Vec2,
        flash: bool,
        chief: bool,
    ) {
        let d = faction.direction();
        let p = |x, y| foot + Vec2::new(x * d, y) * scale;
        let skin = if flash {
            rgb(237, 242, 181)
        } else if faction == Faction::Player {
            rgb(112, 156, 62)
        } else {
            rgb(134, 153, 67)
        };
        let edge = rgb(34, 45, 27);
        let cloth = if chief {
            rgb(113, 56, 78)
        } else if faction == Faction::Player {
            rgb(81, 95, 45)
        } else {
            rgb(136, 57, 38)
        };
        Self::ellipse(
            ctx,
            p(0.0, -1.0),
            Vec2::new(16.0, 3.5) * scale,
            Color::from_rgba8(23, 31, 18, 100),
        );
        for side in [-1.0, 1.0] {
            let step = pose.x * side * 6.0;
            Self::draw_line(ctx, p(side * 4.0, -18.0), p(side * 6.0 + step, -4.0), 6.0 * scale, edge);
            Self::draw_line(ctx, p(side * 4.0, -17.0), p(side * 6.0 + step, -5.0), 3.5 * scale, skin);
            Self::ellipse(
                ctx,
                p(side * 6.0 + step + 2.0, -3.0),
                Vec2::new(6.0, 3.0) * scale,
                rgb(56, 48, 29),
            );
        }
        if chief {
            ctx.draw_triangle(p(-8.0, -36.0), p(-22.0, -6.0), p(6.0, -12.0), cloth);
        }
        Self::ellipse(ctx, p(0.0, -25.0), Vec2::new(10.0, 13.0) * scale, edge);
        Self::ellipse(ctx, p(0.0, -26.0), Vec2::new(8.0, 11.0) * scale, cloth);
        Self::draw_line(ctx, p(-8.0, -18.0), p(8.0, -18.0), 4.0 * scale, rgb(66, 47, 28));
        Self::ellipse(ctx, p(2.0, -18.0), Vec2::splat(2.0 * scale), rgb(200, 172, 79));
        ctx.draw_triangle(p(-7.0, -42.0), p(-23.0, -43.0), p(-9.0, -32.0), edge);
        ctx.draw_triangle(p(-8.0, -40.0), p(-19.0, -41.0), p(-8.0, -34.0), skin);
        ctx.draw_triangle(p(10.0, -43.0), p(21.0, -45.0), p(13.0, -34.0), edge);
        ctx.draw_triangle(p(11.0, -41.0), p(18.0, -43.0), p(12.0, -35.0), skin);
        Self::ellipse(ctx, p(2.0, -39.0), Vec2::new(13.0, 11.0) * scale, edge);
        Self::ellipse(ctx, p(3.0, -40.0), Vec2::new(11.0, 9.0) * scale, skin);
        Self::ellipse(ctx, p(12.0, -37.0), Vec2::new(7.0, 4.0) * scale, skin);
        Self::draw_line(ctx, p(3.0, -43.0), p(13.0, -41.0), 3.0 * scale, edge);
        Self::ellipse(ctx, p(10.0, -40.0), Vec2::new(3.0, 1.6) * scale, rgb(239, 226, 150));
        Self::ellipse(ctx, p(11.0, -40.0), Vec2::new(1.0, 1.6) * scale, edge);
        Self::draw_line(ctx, p(8.0, -33.0), p(17.0, -34.0), 1.4 * scale, edge);
        ctx.draw_triangle(p(10.0, -33.0), p(13.0, -33.0), p(12.0, -29.0), rgb(236, 221, 173));
        if kind == UnitKind::Guard || chief {
            Self::ellipse(
                ctx,
                p(1.0, -47.0),
                Vec2::new(13.0, 6.0) * scale,
                if chief { rgb(193, 153, 58) } else { rgb(118, 121, 102) },
            );
            Self::draw_line(ctx, p(-11.0, -45.0), p(14.0, -45.0), 2.0 * scale, edge);
            if chief {
                for x in [-8.0, 0.0, 8.0] {
                    ctx.draw_triangle(p(x - 3.0, -49.0), p(x, -58.0), p(x + 4.0, -49.0), rgb(224, 188, 76));
                }
            }
        }
        let reach = pose.y * 12.0;
        Self::draw_line(ctx, p(5.0, -29.0), p(17.0 + reach, -24.0), 6.0 * scale, edge);
        Self::draw_line(ctx, p(6.0, -29.0), p(17.0 + reach, -25.0), 3.5 * scale, skin);
        match kind {
            UnitKind::Archer => {
                let x = 23.0;
                Self::draw_line(ctx, p(x, -45.0), p(x + 6.0, -29.0), 2.5 * scale, rgb(126, 82, 39));
                Self::draw_line(ctx, p(x + 6.0, -29.0), p(x, -12.0), 2.5 * scale, rgb(126, 82, 39));
                Self::draw_line(ctx, p(x, -45.0), p(x - 6.0 * pose.y, -29.0), scale, rgb(221, 204, 147));
                Self::draw_line(ctx, p(x - 6.0 * pose.y, -29.0), p(x, -12.0), scale, rgb(221, 204, 147));
                Self::draw_line(ctx, p(10.0, -29.0), p(34.0, -29.0), 1.4 * scale, rgb(224, 210, 161));
            }
            UnitKind::Brute => {
                Self::draw_line(
                    ctx,
                    p(17.0 + reach, -20.0),
                    p(23.0 + reach, -49.0 + reach),
                    5.0 * scale,
                    rgb(98, 62, 32),
                );
                Self::ellipse(
                    ctx,
                    p(23.0 + reach, -48.0 + reach),
                    Vec2::new(9.0, 12.0) * scale,
                    rgb(92, 88, 71),
                );
                for y in [-54.0, -44.0] {
                    ctx.draw_triangle(
                        p(28.0 + reach, y + reach),
                        p(37.0 + reach, y - 3.0 + reach),
                        p(30.0 + reach, y + 4.0 + reach),
                        rgb(178, 178, 146),
                    );
                }
            }
            _ => {
                Self::draw_line(ctx, p(17.0 + reach, -21.0), p(25.0 + reach, -46.0 + reach), 4.0 * scale, edge);
                Self::draw_line(
                    ctx,
                    p(19.0 + reach, -27.0),
                    p(25.0 + reach, -47.0 + reach),
                    2.8 * scale,
                    rgb(198, 201, 169),
                );
                Self::draw_line(
                    ctx,
                    p(14.0 + reach, -28.0),
                    p(25.0 + reach, -25.0),
                    3.0 * scale,
                    rgb(174, 141, 69),
                );
                if kind == UnitKind::Guard {
                    Self::ellipse(ctx, p(-7.0, -25.0), Vec2::new(10.0, 14.0) * scale, edge);
                    Self::ellipse(ctx, p(-7.0, -25.0), Vec2::new(8.0, 12.0) * scale, rgb(127, 106, 59));
                    Self::draw_line(ctx, p(-7.0, -36.0), p(-7.0, -14.0), 2.0 * scale, rgb(184, 167, 108));
                    Self::ellipse(ctx, p(-7.0, -25.0), Vec2::splat(3.0 * scale), rgb(158, 163, 133));
                }
            }
        }
    }
}

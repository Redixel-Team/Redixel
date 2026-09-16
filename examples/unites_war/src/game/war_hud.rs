use super::*;

fn gold() -> Color {
    Color::from_rgba8(230, 199, 123, 255)
}
fn parchment() -> Color {
    Color::from_rgba8(213, 210, 175, 255)
}

impl UnitesWar {
    pub(super) fn wood_panel(ctx: &mut dyn GameContext<Action>, pos: Vec2, size: Vec2, scale: f32, selected: bool) {
        Self::draw_hud_panel(
            ctx,
            pos,
            size,
            scale,
            if selected {
                Color::from_rgba8(74, 84, 49, 255)
            } else {
                Color::from_rgba8(44, 48, 35, 255)
            },
            if selected {
                gold()
            } else {
                Color::from_rgba8(109, 108, 73, 255)
            },
        );
        for x in [6.0 * scale, size.x - 6.0 * scale] {
            for y in [6.0 * scale, size.y - 6.0 * scale] {
                Self::ellipse(
                    ctx,
                    pos + Vec2::new(x, y),
                    Vec2::splat(1.4 * scale),
                    Color::from_rgba8(163, 149, 96, 255),
                );
            }
        }
    }

    pub(super) fn difficulty_rect(index: usize, width: f32, height: f32) -> (Vec2, Vec2) {
        let scale = Self::view_scale(width, height);
        (
            Vec2::new(width * 0.5 - 345.0 * scale + index as f32 * 235.0 * scale, height * 0.43),
            Vec2::new(220.0, 148.0) * scale,
        )
    }

    pub(super) fn draw_war_menu(&self, ctx: &mut dyn GameContext<Action>, width: f32, height: f32) {
        self.draw_forest_world(ctx, width, height);
        let s = Self::view_scale(width, height);
        ctx.draw_rect(Vec2::ZERO, Vec2::new(width, height), Color::from_rgba8(20, 31, 19, 115));
        let panel = Vec2::new(width * 0.5 - 225.0 * s, height * 0.12);
        Self::wood_panel(ctx, panel, Vec2::new(450.0 * s, height * 0.81), s, false);
        Self::draw_text_centered(ctx, "UNITES WAR", width * 0.5, height * 0.19, 4.4 * s, gold());
        Self::draw_text_centered(ctx, "A FLORESTA DOS GOBLINS", width * 0.5, height * 0.28, 1.65 * s, parchment());
        Self::draw_text_centered(
            ctx,
            "UM CLA. UMA TORRE. TODA A FLORESTA.",
            width * 0.5,
            height * 0.36,
            0.95 * s,
            Color::from_rgba8(156, 180, 123, 255),
        );
        for (i, label) in ["JOGAR", "CONFIGURACOES", "SAIR"].into_iter().enumerate() {
            let (p, z) = Self::menu_button_rect(i, width, height);
            let hovered = ctx
                .input()
                .mouse_position()
                .is_some_and(|m| Self::point_in_rect(m, p, z));
            Self::wood_panel(ctx, p, z, s, hovered);
            Self::draw_text_centered(ctx, label, width * 0.5, p.y + 18.0 * s, 1.8 * s, gold());
        }
        Self::draw_text_centered(
            ctx,
            "RECRUTE. EVOLUA. CONQUISTE.",
            width * 0.5,
            height * 0.86,
            1.1 * s,
            parchment(),
        );
    }

    pub(super) fn draw_war_stage_select(&self, ctx: &mut dyn GameContext<Action>, width: f32, height: f32) {
        self.draw_forest_world(ctx, width, height);
        let s = Self::view_scale(width, height);
        ctx.draw_rect(Vec2::ZERO, Vec2::new(width, height), Color::from_rgba8(19, 28, 18, 195));
        Self::draw_text_centered(ctx, "A FLORESTA DOS GOBLINS", width * 0.5, height * 0.16, 2.8 * s, gold());
        Self::draw_text_centered(
            ctx,
            "ESCOLHA O DESAFIO DO SEU CLA",
            width * 0.5,
            height * 0.25,
            1.3 * s,
            parchment(),
        );
        Self::draw_text_centered(
            ctx,
            "OURO TREINA TROPAS. ABATES RENDEM XP. DESTRUA A TORRE RIVAL.",
            width * 0.5,
            height * 0.33,
            1.05 * s,
            parchment(),
        );
        for (i, difficulty) in Difficulty::ALL.into_iter().enumerate() {
            let (p, z) = Self::difficulty_rect(i, width, height);
            let hovered = ctx
                .input()
                .mouse_position()
                .is_some_and(|m| Self::point_in_rect(m, p, z));
            Self::wood_panel(ctx, p, z, s, hovered || self.clan.difficulty == difficulty);
            let foot = p + Vec2::new(107.0, 80.0) * s;
            if !self.draw_unit_portrait(
                ctx,
                foot,
                42.0 * s,
                [UnitKind::Runner, UnitKind::Guard, UnitKind::Brute][i],
                false,
            ) {
                Self::draw_goblin(
                    ctx,
                    foot,
                    0.9 * s,
                    [UnitKind::Runner, UnitKind::Guard, UnitKind::Brute][i],
                    Faction::Player,
                    Vec2::ZERO,
                    false,
                    i == 2,
                );
            }
            Self::draw_text_centered(ctx, difficulty.name(), p.x + z.x * 0.5, p.y + 95.0 * s, 1.25 * s, gold());
            Self::draw_text_centered(
                ctx,
                [
                    "AVANCO LENTO E EXERCITO SIMPLES",
                    "ADAPTA TROPAS E MELHORIAS",
                    "REAGE RAPIDO E FOCA ALVOS",
                ][i],
                p.x + z.x * 0.5,
                p.y + 124.0 * s,
                0.9 * s,
                parchment(),
            );
        }
        let (p, z) = Self::stage_back_button_rect(width, height);
        Self::wood_panel(ctx, p, z, s, false);
        Self::draw_text_centered(ctx, "VOLTAR", p.x + z.x * 0.5, p.y + 14.0, 1.7, gold());
    }
}

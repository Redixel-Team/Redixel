use super::*;

impl UnitesWar {
    pub(super) fn draw_quad(ctx: &mut dyn GameContext<Action>, p1: Vec2, p2: Vec2, p3: Vec2, p4: Vec2, color: Color) {
        ctx.draw_triangle(p1, p2, p3, color);
        ctx.draw_triangle(p1, p3, p4, color);
    }

    pub(super) fn draw_line(ctx: &mut dyn GameContext<Action>, start: Vec2, end: Vec2, width: f32, color: Color) {
        let direction: Vec2 = (end - start).normalise();
        let normal: Vec2 = Vec2::new(-direction.y, direction.x) * width * 0.5;
        Self::draw_quad(ctx, start + normal, end + normal, end - normal, start - normal, color);
    }

    /// Draws an extruded rectangle using the engine's existing triangle API.
    /// The offset represents the projected depth of the back face.
    pub(super) fn draw_prism(ctx: &mut dyn GameContext<Action>, pos: Vec2, size: Vec2, depth: Vec2, color: Color) {
        let top_left: Vec2 = pos;
        let top_right: Vec2 = pos + Vec2::new(size.x, 0.0);
        let bottom_right: Vec2 = pos + size;
        let bottom_left: Vec2 = pos + Vec2::new(0.0, size.y);

        Self::draw_quad(
            ctx,
            top_left + depth,
            top_right + depth,
            top_right,
            top_left,
            color.lerp(Color::WHITE, 0.24),
        );
        Self::draw_quad(
            ctx,
            top_right + depth,
            bottom_right + depth,
            bottom_right,
            top_right,
            color.lerp(Color::BLACK, 0.28),
        );
        Self::draw_quad(ctx, top_left, top_right, bottom_right, bottom_left, color);
    }

    pub(super) fn draw_chamfered_rect(
        ctx: &mut dyn GameContext<Action>,
        pos: Vec2,
        size: Vec2,
        corner: f32,
        color: Color,
    ) {
        let corner: f32 = corner.min(size.x * 0.25).min(size.y * 0.25);
        let center: Vec2 = pos + size * 0.5;
        let points: [Vec2; 8] = [
            pos + Vec2::new(corner, 0.0),
            pos + Vec2::new(size.x - corner, 0.0),
            pos + Vec2::new(size.x, corner),
            pos + Vec2::new(size.x, size.y - corner),
            pos + Vec2::new(size.x - corner, size.y),
            pos + Vec2::new(corner, size.y),
            pos + Vec2::new(0.0, size.y - corner),
            pos + Vec2::new(0.0, corner),
        ];
        for index in 0..points.len() {
            ctx.draw_triangle(center, points[index], points[(index + 1) % points.len()], color);
        }
    }

    pub(super) fn draw_hud_panel(
        ctx: &mut dyn GameContext<Action>,
        pos: Vec2,
        size: Vec2,
        scale: f32,
        fill: Color,
        border: Color,
    ) {
        Self::draw_chamfered_rect(ctx, pos, size, 11.0 * scale, border);
        Self::draw_chamfered_rect(
            ctx,
            pos + Vec2::splat(2.0 * scale),
            size - Vec2::splat(4.0 * scale),
            9.0 * scale,
            fill,
        );
        ctx.draw_rect(
            pos + Vec2::new(12.0, 3.0) * scale,
            Vec2::new((size.x - 24.0 * scale).max(0.0), 2.0 * scale),
            Color::from_rgba8(225, 237, 229, 80),
        );
    }

    pub(super) fn draw_bar(ctx: &mut dyn GameContext<Action>, pos: Vec2, size: Vec2, ratio: f32, fill: Color) {
        ctx.draw_rect(pos, size, Color::from_rgba8(28, 31, 35, 255));
        let inset: f32 = 3.0_f32.min(size.x * 0.25).min(size.y * 0.25);
        ctx.draw_rect(
            pos + Vec2::splat(inset),
            Vec2::new((size.x - inset * 2.0) * ratio.clamp(0.0, 1.0), size.y - inset * 2.0),
            fill,
        );
    }

    pub(super) fn glyph_rows(character: char) -> [u8; 7] {
        match character.to_ascii_uppercase() {
            'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
            'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
            'C' => [0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111],
            'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
            'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
            'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
            'G' => [0b01111, 0b10000, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111],
            'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
            'I' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111],
            'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100],
            'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
            'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
            'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
            'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
            'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
            'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
            'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
            'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
            'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
            'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
            'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
            'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
            'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
            'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
            'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
            'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
            '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
            '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
            '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
            '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
            '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
            '5' => [0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110],
            '6' => [0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
            '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
            '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
            '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110],
            '/' => [1, 2, 2, 4, 8, 8, 16],
            '+' => [0, 4, 4, 31, 4, 4, 0],
            '%' => [17, 2, 4, 4, 8, 16, 17],
            '-' => [0, 0, 0, 0b11111, 0, 0, 0],
            ':' => [0, 0b00100, 0b00100, 0, 0b00100, 0b00100, 0],
            '.' => [0, 0, 0, 0, 0, 0b00100, 0b00100],
            '!' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0, 0b00100],
            _ => [0; 7],
        }
    }

    pub(super) fn text_width(text: &str, scale: f32) -> f32 {
        let count: usize = text.chars().count();
        if count == 0 {
            0.0
        } else {
            (count as f32 * 6.0 - 1.0) * scale
        }
    }

    pub(super) fn draw_text(ctx: &mut dyn GameContext<Action>, text: &str, pos: Vec2, scale: f32, color: Color) {
        for (character_index, character) in text.chars().enumerate() {
            let rows: [u8; 7] = Self::glyph_rows(character);
            for (row, bits) in rows.into_iter().enumerate() {
                for column in 0..5 {
                    if bits & (1 << (4 - column)) != 0 {
                        ctx.draw_rect(
                            pos + Vec2::new((character_index as f32 * 6.0 + column as f32) * scale, row as f32 * scale),
                            Vec2::splat(scale),
                            color,
                        );
                    }
                }
            }
        }
    }

    pub(super) fn draw_text_centered(
        ctx: &mut dyn GameContext<Action>,
        text: &str,
        center_x: f32,
        y: f32,
        scale: f32,
        color: Color,
    ) {
        let x: f32 = center_x - Self::text_width(text, scale) * 0.5;
        Self::draw_text(ctx, text, Vec2::new(x, y), scale, color);
    }

    pub(super) fn draw_settings(&self, ctx: &mut dyn GameContext<Action>, width: f32, height: f32) {
        self.draw_forest_world(ctx, width, height);
        ctx.draw_rect(Vec2::ZERO, Vec2::new(width, height), Color::from_rgba8(12, 18, 18, 135));

        let (panel_pos, panel_size): (Vec2, Vec2) = Self::settings_panel_rect(width, height);
        Self::draw_prism(
            ctx,
            panel_pos,
            panel_size,
            Vec2::new(12.0, -10.0),
            Color::from_rgba8(30, 40, 40, 250),
        );
        Self::draw_text_centered(
            ctx,
            "CONFIGURACOES",
            width * 0.5,
            panel_pos.y + 29.0,
            2.75,
            Color::from_rgba8(239, 197, 83, 255),
        );
        Self::draw_text_centered(
            ctx,
            "RESOLUCAO DA JANELA",
            width * 0.5,
            panel_pos.y + 72.0,
            1.6,
            Color::from_rgba8(183, 204, 190, 255),
        );
        let current_resolution: String = format!(
            "ATUAL: {} X {}  {}",
            width as u32,
            height as u32,
            if ctx.is_fullscreen() { "TELA CHEIA" } else { "JANELA" }
        );
        Self::draw_text_centered(
            ctx,
            &current_resolution,
            width * 0.5,
            panel_pos.y + 105.0,
            1.35,
            Color::from_rgba8(143, 165, 152, 255),
        );

        let mouse: Option<Vec2> = ctx.input().mouse_position();
        for (index, resolution) in RESOLUTIONS.into_iter().enumerate() {
            let (button_pos, button_size): (Vec2, Vec2) = Self::resolution_button_rect(index, width, height);
            let hovered: bool = mouse.is_some_and(|point| Self::point_in_rect(point, button_pos, button_size));
            let selected: bool = index == self.selected_resolution;
            let button_color: Color = match (selected, hovered) {
                (true, true) => Color::from_rgba8(81, 157, 99, 255),
                (true, false) => Color::from_rgba8(55, 116, 72, 255),
                (false, true) => Color::from_rgba8(63, 104, 128, 255),
                (false, false) => Color::from_rgba8(43, 64, 72, 255),
            };
            Self::draw_prism(ctx, button_pos, button_size, Vec2::new(6.0, -5.0), button_color);

            let label: String = if resolution.width == 1920 && resolution.height == 1080 {
                "1920 X 1080  FULL HD".to_owned()
            } else {
                format!("{} X {}", resolution.width, resolution.height)
            };
            Self::draw_text_centered(
                ctx,
                &label,
                button_pos.x + button_size.x * 0.5,
                button_pos.y + 20.0,
                1.75,
                if selected {
                    Color::from_rgba8(249, 220, 137, 255)
                } else {
                    Color::WHITE
                },
            );
        }

        for (index, (label, fullscreen)) in [("MODO JANELA", false), ("TELA CHEIA", true)].into_iter().enumerate() {
            let (button_pos, button_size): (Vec2, Vec2) = Self::window_mode_button_rect(index, width, height);
            let hovered: bool = mouse.is_some_and(|point| Self::point_in_rect(point, button_pos, button_size));
            let selected: bool = ctx.is_fullscreen() == fullscreen;
            Self::draw_prism(
                ctx,
                button_pos,
                button_size,
                Vec2::new(5.0, -4.0),
                match (selected, hovered) {
                    (true, true) => Color::from_rgba8(81, 157, 99, 255),
                    (true, false) => Color::from_rgba8(55, 116, 72, 255),
                    (false, true) => Color::from_rgba8(63, 104, 128, 255),
                    (false, false) => Color::from_rgba8(43, 64, 72, 255),
                },
            );
            Self::draw_text_centered(
                ctx,
                label,
                button_pos.x + button_size.x * 0.5,
                button_pos.y + 14.0,
                1.9,
                if selected {
                    Color::from_rgba8(249, 220, 137, 255)
                } else {
                    Color::WHITE
                },
            );
        }

        let (back_pos, back_size): (Vec2, Vec2) = Self::settings_back_button_rect(width, height);
        let back_hovered: bool = mouse.is_some_and(|point| Self::point_in_rect(point, back_pos, back_size));
        Self::draw_prism(
            ctx,
            back_pos,
            back_size,
            Vec2::new(5.0, -4.0),
            if back_hovered {
                Color::from_rgba8(132, 72, 61, 255)
            } else {
                Color::from_rgba8(91, 58, 54, 255)
            },
        );
        Self::draw_text_centered(ctx, "VOLTAR", width * 0.5, back_pos.y + 13.0, 1.9, Color::WHITE);
    }

    pub(super) fn draw_spell_icon(ctx: &mut dyn GameContext<Action>, center: Vec2, scale: f32, ready: bool) {
        let color: Color = if ready {
            Color::from_rgba8(111, 226, 209, 255)
        } else {
            Color::from_rgba8(91, 112, 118, 255)
        };
        Self::draw_quad(
            ctx,
            center + Vec2::new(-11.0, 0.0) * scale,
            center + Vec2::new(0.0, -15.0) * scale,
            center + Vec2::new(11.0, 0.0) * scale,
            center + Vec2::new(0.0, 15.0) * scale,
            color,
        );
        ctx.draw_triangle(
            center + Vec2::new(1.0, -10.0) * scale,
            center + Vec2::new(-5.0, 2.0) * scale,
            center + Vec2::new(2.0, 1.0) * scale,
            Color::from_rgba8(245, 236, 139, 255),
        );
        ctx.draw_triangle(
            center + Vec2::new(-2.0, 0.0) * scale,
            center + Vec2::new(5.0, -1.0) * scale,
            center + Vec2::new(-1.0, 11.0) * scale,
            Color::from_rgba8(245, 236, 139, 255),
        );
    }

    pub(super) fn draw_end_overlay(&self, ctx: &mut dyn GameContext<Action>, width: f32, height: f32) {
        if self.state == BattleState::Playing {
            return;
        }
        let scale = Self::view_scale(width, height);
        let victory = self.state == BattleState::Victory;
        let gold = Color::from_rgba8(230, 199, 123, 255);
        ctx.draw_rect(Vec2::ZERO, Vec2::new(width, height), Color::from_rgba8(17, 25, 18, 180));
        let pos = Vec2::new(width * 0.5 - 260.0 * scale, height * 0.24);
        Self::wood_panel(ctx, pos, Vec2::new(520.0, 250.0) * scale, scale, true);
        Self::draw_text_centered(
            ctx,
            if victory { "VITORIA DO CLA!" } else { "A TORRE CAIU" },
            width * 0.5,
            pos.y + 32.0 * scale,
            3.1 * scale,
            gold,
        );
        Self::draw_text_centered(
            ctx,
            if victory {
                "A FLORESTA AGORA E SUA."
            } else {
                "REUNA SEUS GOBLINS E TENTE NOVAMENTE."
            },
            width * 0.5,
            pos.y + 78.0 * scale,
            1.15 * scale,
            Color::WHITE,
        );
        Self::draw_text_centered(
            ctx,
            &format!(
                "{} ABATES   {} XP   {}:{:02}",
                self.clan.reputation,
                self.clan.experience[0] as u32,
                self.clan.elapsed as u32 / 60,
                self.clan.elapsed as u32 % 60
            ),
            width * 0.5,
            pos.y + 122.0 * scale,
            1.3 * scale,
            gold,
        );
        Self::wood_panel(
            ctx,
            pos + Vec2::new(80.0, 163.0) * scale,
            Vec2::new(360.0, 42.0) * scale,
            scale,
            false,
        );
        Self::draw_text_centered(
            ctx,
            "CLIQUE OU R PARA JOGAR NOVAMENTE",
            width * 0.5,
            pos.y + 178.0 * scale,
            1.1 * scale,
            gold,
        );
        Self::draw_text_centered(
            ctx,
            "ESC VOLTA AO MENU",
            width * 0.5,
            pos.y + 224.0 * scale,
            scale,
            Color::from_rgba8(167, 183, 140, 255),
        );
    }
}

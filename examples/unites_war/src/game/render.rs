use super::*;

impl UnitesWar {
    fn draw_quad(ctx: &mut dyn GameContext<Action>, p1: Vec2, p2: Vec2, p3: Vec2, p4: Vec2, color: Color) {
        ctx.draw_triangle(p1, p2, p3, color);
        ctx.draw_triangle(p1, p3, p4, color);
    }

    fn draw_line(ctx: &mut dyn GameContext<Action>, start: Vec2, end: Vec2, width: f32, color: Color) {
        let direction: Vec2 = (end - start).normalise();
        let normal: Vec2 = Vec2::new(-direction.y, direction.x) * width * 0.5;
        Self::draw_quad(ctx, start + normal, end + normal, end - normal, start - normal, color);
    }

    /// Draws an extruded rectangle using the engine's existing triangle API.
    /// The offset represents the projected depth of the back face.
    fn draw_prism(ctx: &mut dyn GameContext<Action>, pos: Vec2, size: Vec2, depth: Vec2, color: Color) {
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

    fn draw_chamfered_rect(ctx: &mut dyn GameContext<Action>, pos: Vec2, size: Vec2, corner: f32, color: Color) {
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

    fn draw_hud_panel(
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

    /// Builds projected depth from the texture's own alpha silhouette, then
    /// draws the original image as the lit front face.
    fn draw_extruded_sprite(
        ctx: &mut dyn GameContext<Action>,
        pos: Vec2,
        size: Vec2,
        texture: TextureId,
        front_tint: Color,
        depth_step: Vec2,
        layers: u8,
    ) {
        for layer in (1..=layers).rev() {
            let progress: f32 = layer as f32 / layers as f32;
            let side_tint: Color = front_tint.lerp(Color::BLACK, 0.38 + progress * 0.22);
            ctx.draw_sprite_tinted(pos + depth_step * layer as f32, size, texture, side_tint);
        }

        ctx.draw_sprite_tinted(pos, size, texture, front_tint);
    }

    /// Keeps the character sprite crisp while suggesting volume with one
    /// directional silhouette instead of several repeated extrusion layers.
    fn draw_depth_shadowed_sprite(
        ctx: &mut dyn GameContext<Action>,
        pos: Vec2,
        size: Vec2,
        texture: TextureId,
        front_tint: Color,
    ) {
        let depth_shadow: Color = front_tint
            .lerp(Color::from_rgba8(24, 13, 34, 255), 0.82)
            .with_alpha(0.72);
        ctx.draw_sprite_tinted(pos + Vec2::new(3.0, 2.0), size, texture, depth_shadow);
        ctx.draw_sprite_tinted(pos, size, texture, front_tint);
    }

    fn draw_shadow(ctx: &mut dyn GameContext<Action>, center: Vec2, width: f32, depth: f32) {
        let left: Vec2 = center + Vec2::new(-width * 0.5, 0.0);
        let top: Vec2 = center + Vec2::new(0.0, -depth * 0.5);
        let right: Vec2 = center + Vec2::new(width * 0.5, 0.0);
        let bottom: Vec2 = center + Vec2::new(0.0, depth * 0.5);
        Self::draw_quad(ctx, left, top, right, bottom, Color::from_rgba8(20, 24, 22, 105));
    }

    fn draw_bar(ctx: &mut dyn GameContext<Action>, pos: Vec2, size: Vec2, ratio: f32, fill: Color) {
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

    fn draw_text(ctx: &mut dyn GameContext<Action>, text: &str, pos: Vec2, scale: f32, color: Color) {
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

    fn draw_text_centered(
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

    pub(super) fn draw_menu(&self, ctx: &mut dyn GameContext<Action>, width: f32, height: f32) {
        self.draw_background(ctx, width, height);
        self.draw_castle(ctx, &self.player_castle, Faction::Player, width, height);
        self.draw_castle(ctx, &self.enemy_castle, Faction::Enemy, width, height);

        let panel_width: f32 = (width * 0.62).clamp(390.0, 700.0);
        let panel_height: f32 = (height * 0.76).clamp(440.0, 560.0);
        let panel_pos: Vec2 = Vec2::new((width - panel_width) * 0.5, (height - panel_height) * 0.43);
        Self::draw_prism(
            ctx,
            panel_pos,
            Vec2::new(panel_width, panel_height),
            Vec2::new(12.0, -10.0),
            Color::from_rgba8(31, 40, 39, 245),
        );

        let title_scale: f32 = (width / 260.0).clamp(3.2, 5.0);
        Self::draw_text_centered(
            ctx,
            "UNITES WAR",
            width * 0.5 + 3.0,
            panel_pos.y + 58.0,
            title_scale,
            Color::from_rgba8(34, 42, 39, 220),
        );
        Self::draw_text_centered(
            ctx,
            "UNITES WAR",
            width * 0.5,
            panel_pos.y + 54.0,
            title_scale,
            Color::from_rgba8(239, 197, 83, 255),
        );
        Self::draw_text_centered(
            ctx,
            "GUERRA ENTRE CLAS",
            width * 0.5,
            panel_pos.y + 111.0,
            2.0,
            Color::from_rgba8(174, 194, 177, 255),
        );
        Self::draw_text_centered(
            ctx,
            "MOEDAS POR TEMPO E ABATES",
            width * 0.5,
            panel_pos.y + 138.0,
            1.45,
            Color::from_rgba8(226, 190, 99, 255),
        );

        let mouse: Option<Vec2> = ctx.input().mouse_position();
        for (index, label) in ["JOGAR", "CONFIGURACOES", "SAIR"].into_iter().enumerate() {
            let (button_pos, button_size): (Vec2, Vec2) = Self::menu_button_rect(index, width, height);
            let hovered: bool = mouse.is_some_and(|point: Vec2| Self::point_in_rect(point, button_pos, button_size));
            let button_color: Color = match (index, hovered) {
                (0, true) => Color::from_rgba8(74, 157, 83, 255),
                (0, false) => Color::from_rgba8(52, 111, 66, 255),
                (1, true) => Color::from_rgba8(69, 128, 158, 255),
                (1, false) => Color::from_rgba8(49, 89, 111, 255),
                (2, true) => Color::from_rgba8(171, 78, 62, 255),
                _ => Color::from_rgba8(112, 59, 54, 255),
            };
            Self::draw_prism(ctx, button_pos, button_size, Vec2::new(7.0, -6.0), button_color);
            let scale: f32 = if index == 1 { 2.45 } else { 3.0 };
            Self::draw_text_centered(ctx, label, width * 0.5, button_pos.y + 17.0, scale, Color::WHITE);
        }

        Self::draw_text_centered(
            ctx,
            "ENTER PARA VER FASES",
            width * 0.5,
            height - 69.0,
            1.5,
            Color::from_rgba8(181, 190, 182, 255),
        );
        Self::draw_text_centered(
            ctx,
            "1-4 TROPAS   Q MAGIA   U MELHORIA",
            width * 0.5,
            height - 39.0,
            1.35,
            Color::from_rgba8(139, 158, 146, 255),
        );
    }

    pub(super) fn draw_settings(&self, ctx: &mut dyn GameContext<Action>, width: f32, height: f32) {
        self.draw_background(ctx, width, height);
        self.draw_castle(ctx, &self.player_castle, Faction::Player, width, height);
        self.draw_castle(ctx, &self.enemy_castle, Faction::Enemy, width, height);
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

    pub(super) fn draw_stage_select(&self, ctx: &mut dyn GameContext<Action>, width: f32, height: f32) {
        self.draw_background(ctx, width, height);
        self.draw_castle(ctx, &self.player_castle, Faction::Player, width, height);
        self.draw_castle(ctx, &self.enemy_castle, Faction::Enemy, width, height);
        ctx.draw_rect(Vec2::ZERO, Vec2::new(width, height), Color::from_rgba8(12, 18, 18, 120));

        let (panel_pos, panel_size): (Vec2, Vec2) = Self::stage_select_panel_rect(width, height);
        Self::draw_prism(
            ctx,
            panel_pos,
            panel_size,
            Vec2::new(12.0, -10.0),
            Color::from_rgba8(30, 40, 40, 250),
        );
        Self::draw_text_centered(
            ctx,
            "SELECIONE A FASE",
            width * 0.5,
            panel_pos.y + 28.0,
            2.75,
            Color::from_rgba8(239, 197, 83, 255),
        );
        Self::draw_text_centered(
            ctx,
            "A GUERRA ESTA APENAS COMECANDO",
            width * 0.5,
            panel_pos.y + 66.0,
            1.25,
            Color::from_rgba8(163, 184, 171, 255),
        );

        let mouse: Option<Vec2> = ctx.input().mouse_position();
        for index in 0..STAGE_COUNT {
            let (card_pos, card_size): (Vec2, Vec2) = Self::stage_card_rect(index, width, height);
            let hovered: bool = mouse.is_some_and(|point| Self::point_in_rect(point, card_pos, card_size));
            let available: bool = index == 0;
            let card_color: Color = if available && hovered {
                Color::from_rgba8(63, 126, 72, 255)
            } else if available {
                Color::from_rgba8(44, 91, 57, 255)
            } else if hovered {
                Color::from_rgba8(50, 58, 59, 255)
            } else {
                Color::from_rgba8(37, 44, 45, 255)
            };
            Self::draw_prism(ctx, card_pos, card_size, Vec2::new(6.0, -5.0), card_color);

            let preview_pos: Vec2 = card_pos + Vec2::new(7.0, 7.0);
            let preview_size: Vec2 = Vec2::new(card_size.x - 14.0, (card_size.y * 0.52).max(38.0));
            ctx.draw_rect(
                preview_pos,
                preview_size,
                if available {
                    Color::from_rgba8(52, 87, 92, 255)
                } else {
                    Color::from_rgba8(27, 33, 35, 255)
                },
            );

            if available {
                ctx.draw_triangle(
                    preview_pos + Vec2::new(0.0, preview_size.y * 0.48),
                    preview_pos + Vec2::new(preview_size.x * 0.36, preview_size.y * 0.08),
                    preview_pos + Vec2::new(preview_size.x * 0.64, preview_size.y * 0.48),
                    Color::from_rgba8(61, 99, 82, 255),
                );
                ctx.draw_triangle(
                    preview_pos + Vec2::new(preview_size.x * 0.35, preview_size.y * 0.48),
                    preview_pos + Vec2::new(preview_size.x * 0.72, preview_size.y * 0.14),
                    preview_pos + Vec2::new(preview_size.x, preview_size.y * 0.48),
                    Color::from_rgba8(55, 88, 76, 255),
                );
                ctx.draw_rect(
                    preview_pos + Vec2::new(0.0, preview_size.y * 0.48),
                    Vec2::new(preview_size.x, preview_size.y * 0.52),
                    Color::from_rgba8(87, 76, 49, 255),
                );
                let tower_size: Vec2 = Vec2::new((preview_size.x * 0.1).clamp(15.0, 28.0), preview_size.y * 0.43);
                Self::draw_prism(
                    ctx,
                    preview_pos + Vec2::new(12.0, preview_size.y - tower_size.y),
                    tower_size,
                    Vec2::new(3.0, -3.0),
                    Color::from_rgba8(78, 118, 91, 255),
                );
                Self::draw_prism(
                    ctx,
                    preview_pos + Vec2::new(preview_size.x - tower_size.x - 15.0, preview_size.y - tower_size.y),
                    tower_size,
                    Vec2::new(3.0, -3.0),
                    Color::from_rgba8(132, 74, 62, 255),
                );
            } else {
                let lock_center: Vec2 = preview_pos + preview_size * 0.5;
                ctx.draw_rect(
                    lock_center + Vec2::new(-13.0, -2.0),
                    Vec2::new(26.0, 22.0),
                    Color::from_rgba8(75, 82, 82, 255),
                );
                ctx.draw_rect(
                    lock_center + Vec2::new(-9.0, -14.0),
                    Vec2::new(4.0, 14.0),
                    Color::from_rgba8(75, 82, 82, 255),
                );
                ctx.draw_rect(
                    lock_center + Vec2::new(5.0, -14.0),
                    Vec2::new(4.0, 14.0),
                    Color::from_rgba8(75, 82, 82, 255),
                );
                ctx.draw_rect(
                    lock_center + Vec2::new(-5.0, -17.0),
                    Vec2::new(10.0, 4.0),
                    Color::from_rgba8(75, 82, 82, 255),
                );
            }

            let phase_label: String = format!("FASE {}", index + 1);
            Self::draw_text_centered(
                ctx,
                &phase_label,
                card_pos.x + card_size.x * 0.5,
                card_pos.y + card_size.y * 0.62,
                1.65,
                if available {
                    Color::from_rgba8(246, 218, 134, 255)
                } else {
                    Color::from_rgba8(119, 130, 129, 255)
                },
            );
            Self::draw_text_centered(
                ctx,
                if available { "PLANICIE DOS CLAS" } else { "EM BREVE" },
                card_pos.x + card_size.x * 0.5,
                card_pos.y + card_size.y * 0.79,
                if available { 1.05 } else { 1.3 },
                if available {
                    Color::from_rgba8(190, 211, 189, 255)
                } else {
                    Color::from_rgba8(93, 103, 103, 255)
                },
            );
        }

        let (back_pos, back_size): (Vec2, Vec2) = Self::stage_back_button_rect(width, height);
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

    pub(super) fn draw_background(&self, ctx: &mut dyn GameContext<Action>, width: f32, height: f32) {
        let ground: f32 = Self::ground_y(height);
        let horizon: f32 = ground - 142.0;
        ctx.clear_color(Color::from_rgba8(41, 71, 79, 255));

        if let Some(texture) = self.textures.stage_01_background {
            ctx.draw_sprite(Vec2::ZERO, Vec2::new(width, height), texture);
            return;
        }

        ctx.draw_rect(
            Vec2::new(0.0, ground - 190.0),
            Vec2::new(width, 190.0),
            Color::from_rgba8(74, 105, 86, 255),
        );
        ctx.draw_triangle(
            Vec2::new(0.0, ground - 190.0),
            Vec2::new(width * 0.28, ground - 315.0),
            Vec2::new(width * 0.52, ground - 190.0),
            Color::from_rgba8(55, 88, 77, 255),
        );
        ctx.draw_triangle(
            Vec2::new(width * 0.36, ground - 190.0),
            Vec2::new(width * 0.72, ground - 290.0),
            Vec2::new(width, ground - 190.0),
            Color::from_rgba8(49, 82, 73, 255),
        );

        Self::draw_quad(
            ctx,
            Vec2::new(0.0, horizon),
            Vec2::new(width, horizon),
            Vec2::new(width, height - TOOLBAR_HEIGHT),
            Vec2::new(0.0, height - TOOLBAR_HEIGHT),
            Color::from_rgba8(70, 91, 58, 255),
        );

        // The converging battlefield gives the side-view game a projected 3D floor.
        Self::draw_quad(
            ctx,
            Vec2::new(width * 0.38, horizon),
            Vec2::new(width * 0.62, horizon),
            Vec2::new(width + 70.0, height - TOOLBAR_HEIGHT),
            Vec2::new(-70.0, height - TOOLBAR_HEIGHT),
            Color::from_rgba8(91, 72, 49, 255),
        );
        Self::draw_quad(
            ctx,
            Vec2::new(width * 0.38, horizon),
            Vec2::new(width * 0.395, horizon),
            Vec2::new(35.0, height - TOOLBAR_HEIGHT),
            Vec2::new(0.0, height - TOOLBAR_HEIGHT),
            Color::from_rgba8(121, 101, 63, 255),
        );
        Self::draw_quad(
            ctx,
            Vec2::new(width * 0.605, horizon),
            Vec2::new(width * 0.62, horizon),
            Vec2::new(width, height - TOOLBAR_HEIGHT),
            Vec2::new(width - 35.0, height - TOOLBAR_HEIGHT),
            Color::from_rgba8(73, 57, 42, 255),
        );

        ctx.draw_rect(
            Vec2::new(0.0, ground),
            Vec2::new(width, 7.0),
            Color::from_rgba8(113, 145, 75, 255),
        );
    }

    pub(super) fn draw_castle(
        &self,
        ctx: &mut dyn GameContext<Action>,
        castle: &Castle,
        faction: Faction,
        width: f32,
        height: f32,
    ) {
        let ground: f32 = Self::ground_y(height);
        let x: f32 = Self::castle_x(faction, width);
        let stone: Color = match faction {
            Faction::Player => Color::from_rgba8(77, 111, 91, 255),
            Faction::Enemy => Color::from_rgba8(125, 72, 64, 255),
        };
        let depth: Vec2 = Vec2::new(12.0, -9.0);
        let dark: Color = stone.lerp(Color::BLACK, 0.27);

        let visual_x: f32 = x + faction.direction() * 18.0;
        Self::draw_shadow(ctx, Vec2::new(visual_x, ground + 5.0), 158.0, 32.0);

        if let Some(texture) = self.textures.stage_01_fortress {
            let sprite_size: Vec2 = Vec2::splat(180.0);
            let sprite_pos: Vec2 = Vec2::new(visual_x - sprite_size.x * 0.5, ground - 168.0);
            let tint: Color = match faction {
                Faction::Player => Color::from_rgba8(205, 255, 220, 255),
                Faction::Enemy => Color::from_rgba8(255, 214, 205, 255),
            };
            Self::draw_extruded_sprite(ctx, sprite_pos, sprite_size, texture, tint, Vec2::new(2.0, -1.0), 6);
        } else {
            Self::draw_prism(
                ctx,
                Vec2::new(x - CASTLE_WIDTH * 0.5, ground - 126.0),
                Vec2::new(CASTLE_WIDTH, 126.0),
                depth,
                dark,
            );
            Self::draw_prism(ctx, Vec2::new(x - 38.0, ground - 153.0), Vec2::new(31.0, 53.0), depth, stone);
            Self::draw_prism(ctx, Vec2::new(x + 7.0, ground - 153.0), Vec2::new(31.0, 53.0), depth, stone);
            Self::draw_prism(
                ctx,
                Vec2::new(x - 32.0, ground - 91.0),
                Vec2::new(64.0, 91.0),
                Vec2::new(9.0, -7.0),
                stone,
            );
            Self::draw_prism(
                ctx,
                Vec2::new(x - 13.0, ground - 55.0),
                Vec2::new(26.0, 55.0),
                Vec2::new(4.0, -3.0),
                Color::from_rgba8(35, 35, 31, 255),
            );

            for battlement in [-34.0_f32, -12.0, 10.0, 32.0] {
                Self::draw_prism(
                    ctx,
                    Vec2::new(x + battlement - 7.0, ground - 137.0),
                    Vec2::new(14.0, 17.0),
                    Vec2::new(5.0, -4.0),
                    stone.lerp(Color::WHITE, 0.06),
                );
            }
        }

        let flag_direction: f32 = faction.direction();
        let flag_x: f32 = x - flag_direction * 18.0;
        ctx.draw_rect(
            Vec2::new(flag_x - 2.0, ground - 206.0),
            Vec2::new(4.0, 61.0),
            Color::from_rgba8(64, 51, 38, 255),
        );
        ctx.draw_triangle(
            Vec2::new(flag_x, ground - 204.0),
            Vec2::new(flag_x + flag_direction * 35.0, ground - 191.0),
            Vec2::new(flag_x, ground - 178.0),
            if faction == Faction::Player {
                Color::from_rgba8(77, 181, 96, 255)
            } else {
                Color::from_rgba8(202, 68, 59, 255)
            },
        );

        for level in 0..castle.level {
            let marker_x: f32 = if faction == Faction::Player {
                x - 31.0 + level as f32 * 18.0
            } else {
                x + 19.0 - level as f32 * 18.0
            };
            ctx.draw_rect(
                Vec2::new(marker_x, ground - 118.0),
                Vec2::new(12.0, 18.0),
                Color::from_rgba8(247, 195, 80, 255),
            );
        }

        Self::draw_bar(
            ctx,
            Vec2::new(x - 52.0, ground - 177.0),
            Vec2::new(104.0, 13.0),
            castle.health / castle.max_health,
            if faction == Faction::Player {
                Color::from_rgba8(91, 218, 103, 255)
            } else {
                Color::from_rgba8(231, 82, 67, 255)
            },
        );
    }

    pub(super) fn draw_unit(&self, ctx: &mut dyn GameContext<Action>, unit: &Unit) {
        let stats: UnitStats = unit.kind.stats();
        let direction: f32 = unit.faction.direction();
        let attack_progress: f32 = 1.0 - unit.attack_animation / UNIT_ATTACK_ANIMATION;
        let strike: f32 = if unit.attack_animation > 0.0 && unit.kind != UnitKind::Archer {
            (attack_progress.clamp(0.0, 1.0) * std::f32::consts::PI).sin()
        } else {
            0.0
        };
        let lunge_distance: f32 = if unit.kind == UnitKind::Brute { 9.0 } else { 6.0 };
        let visual_pos: Vec2 = unit.pos + Vec2::new(direction * strike * lunge_distance, -strike * 1.5);
        let bob: f32 = (self.battle_time * 5.5 + unit.pos.x * 0.035).sin() * 1.2;
        let top_left: Vec2 = visual_pos - stats.size * 0.5 + Vec2::new(0.0, bob);
        let body_color: Color = if unit.hit_flash > 0.0 {
            Color::WHITE
        } else {
            unit.kind.color(unit.faction)
        };
        let model_depth: Vec2 = Vec2::new(5.0, -4.0);

        Self::draw_shadow(
            ctx,
            Vec2::new(visual_pos.x + direction * 3.0, visual_pos.y + stats.size.y * 0.5 + 3.0),
            stats.size.x * 1.35,
            stats.size.x * 0.48,
        );

        let leg_width: f32 = stats.size.x * 0.22;
        Self::draw_prism(
            ctx,
            top_left + Vec2::new(stats.size.x * 0.22, stats.size.y * 0.7),
            Vec2::new(leg_width, stats.size.y * 0.3),
            model_depth * 0.55,
            body_color.lerp(Color::BLACK, 0.28),
        );
        Self::draw_prism(
            ctx,
            top_left + Vec2::new(stats.size.x * 0.57, stats.size.y * 0.7),
            Vec2::new(leg_width, stats.size.y * 0.3),
            model_depth * 0.55,
            body_color.lerp(Color::BLACK, 0.2),
        );

        Self::draw_prism(
            ctx,
            top_left + Vec2::new(stats.size.x * 0.18, stats.size.y * 0.3),
            Vec2::new(stats.size.x * 0.64, stats.size.y * 0.7),
            model_depth,
            body_color,
        );
        Self::draw_prism(
            ctx,
            top_left + Vec2::new(stats.size.x * 0.23, 0.0),
            Vec2::new(stats.size.x * 0.54, stats.size.y * 0.42),
            model_depth * 0.8,
            body_color.lerp(Color::WHITE, 0.08),
        );
        let eye_x: f32 = if direction > 0.0 {
            top_left.x + stats.size.x * 0.61
        } else {
            top_left.x + stats.size.x * 0.3
        };
        ctx.draw_rect(
            Vec2::new(eye_x, top_left.y + stats.size.y * 0.12),
            Vec2::splat(3.0),
            Color::from_rgba8(242, 225, 108, 255),
        );

        match unit.kind {
            UnitKind::Runner => {
                let weapon_x: f32 = visual_pos.x + direction * (stats.size.x * 0.45 + strike * 8.0);
                ctx.draw_triangle(
                    Vec2::new(weapon_x, visual_pos.y - 4.0 - strike * 3.0),
                    Vec2::new(weapon_x + direction * 13.0, visual_pos.y + 2.0),
                    Vec2::new(weapon_x, visual_pos.y + 5.0 + strike * 3.0),
                    Color::from_rgba8(219, 215, 196, 255),
                );
            }
            UnitKind::Guard => {
                let shield_x: f32 = visual_pos.x + direction * (stats.size.x * 0.5 + strike * 7.0);
                Self::draw_prism(
                    ctx,
                    Vec2::new(shield_x - 5.0, visual_pos.y - 9.0),
                    Vec2::new(10.0, 20.0),
                    Vec2::new(3.0, -3.0),
                    Color::from_rgba8(87, 112, 129, 255),
                );
            }
            UnitKind::Archer => {
                let bow_x: f32 = visual_pos.x + direction * stats.size.x * 0.48;
                ctx.draw_triangle(
                    Vec2::new(bow_x, visual_pos.y - 12.0),
                    Vec2::new(bow_x + direction * 8.0, visual_pos.y),
                    Vec2::new(bow_x, visual_pos.y + 12.0),
                    Color::from_rgba8(111, 67, 37, 255),
                );
            }
            UnitKind::Brute => {
                Self::draw_prism(
                    ctx,
                    Vec2::new(
                        visual_pos.x + direction * (12.0 + strike * 9.0) - 5.0,
                        top_left.y - 5.0 + strike * 8.0,
                    ),
                    Vec2::new(10.0, 31.0),
                    Vec2::new(4.0, -4.0),
                    Color::from_rgba8(87, 62, 42, 255),
                );
            }
        }

        Self::draw_bar(
            ctx,
            Vec2::new(top_left.x, top_left.y - 8.0),
            Vec2::new(stats.size.x, 5.0),
            unit.health / unit.max_health,
            if unit.faction == Faction::Player {
                Color::from_rgba8(89, 220, 98, 255)
            } else {
                Color::from_rgba8(235, 82, 65, 255)
            },
        );

        if unit.faction == Faction::Player {
            let passive: PassiveKind = match unit.kind {
                UnitKind::Runner => PassiveKind::Momentum,
                UnitKind::Guard => PassiveKind::Bulwark,
                UnitKind::Archer => PassiveKind::PiercingShot,
                UnitKind::Brute => PassiveKind::Rage,
            };
            if self.player_passives.contains(passive) {
                let active: bool = match passive {
                    PassiveKind::Momentum => unit.momentum_stacks > 0,
                    PassiveKind::Rage => unit.health / unit.max_health <= RAGE_HEALTH_THRESHOLD,
                    PassiveKind::Bulwark | PassiveKind::PiercingShot => true,
                };
                let indicator_color: Color = if active {
                    Color::from_rgba8(250, 202, 75, 255)
                } else {
                    Color::from_rgba8(103, 121, 116, 255)
                };
                ctx.draw_rect(
                    Vec2::new(top_left.x, top_left.y - 14.0),
                    Vec2::new(stats.size.x * 0.72, 3.0),
                    indicator_color,
                );
            }
        }
    }

    fn hero_aim_color(&self) -> Color {
        match self.hero.aim_style {
            0 => Color::from_rgba8(205, 133, 255, 255),
            1 => Color::from_rgba8(108, 224, 210, 255),
            _ => Color::from_rgba8(248, 194, 78, 255),
        }
    }

    pub(super) fn draw_hero(&self, ctx: &mut dyn GameContext<Action>) {
        if !self.hero.alive() {
            return;
        }

        let bob: f32 = (self.battle_time * 4.8).sin() * 1.0;
        let top_left: Vec2 = self.hero.pos - HERO_SIZE * 0.5 + Vec2::new(0.0, bob);

        Self::draw_shadow(
            ctx,
            Vec2::new(self.hero.pos.x + 4.0, self.hero.pos.y + HERO_SIZE.y * 0.5 + 4.0),
            HERO_SIZE.x * 1.55,
            HERO_SIZE.x * 0.55,
        );

        if let Some(texture) = self.textures.shadow_archer {
            let tint: Color = if self.hero.hit_flash > 0.0 {
                Color::from_rgba8(255, 118, 132, 255)
            } else {
                Color::WHITE
            };
            Self::draw_depth_shadowed_sprite(
                ctx,
                self.hero.pos - HERO_SPRITE_SIZE * 0.5 + Vec2::new(0.0, bob),
                HERO_SPRITE_SIZE,
                texture,
                tint,
            );
        } else {
            let base_color: Color = if self.hero.hit_flash > 0.0 {
                Color::WHITE
            } else {
                Color::from_rgba8(67, 44, 91, 255)
            };
            let cloak_color: Color = base_color.lerp(Color::BLACK, 0.24);
            let aim_color: Color = self.hero_aim_color();

            ctx.draw_triangle(
                top_left + Vec2::new(HERO_SIZE.x * 0.5, 13.0),
                top_left + Vec2::new(2.0, HERO_SIZE.y),
                top_left + Vec2::new(HERO_SIZE.x - 4.0, HERO_SIZE.y),
                cloak_color,
            );
            Self::draw_prism(
                ctx,
                top_left + Vec2::new(10.0, 20.0),
                Vec2::new(14.0, 25.0),
                Vec2::new(4.0, -4.0),
                base_color,
            );
            Self::draw_prism(
                ctx,
                top_left + Vec2::new(8.0, 4.0),
                Vec2::new(17.0, 19.0),
                Vec2::new(4.0, -4.0),
                cloak_color,
            );
            ctx.draw_triangle(
                top_left + Vec2::new(8.0, 5.0),
                top_left + Vec2::new(16.0, -5.0),
                top_left + Vec2::new(26.0, 7.0),
                cloak_color.lerp(Color::WHITE, 0.08),
            );
            ctx.draw_rect(
                top_left + Vec2::new(20.0, 11.0),
                Vec2::splat(3.0),
                Color::from_rgba8(223, 168, 255, 255),
            );

            let bow_x: f32 = top_left.x + HERO_SIZE.x - 2.0;
            ctx.draw_triangle(
                Vec2::new(bow_x, top_left.y + 12.0),
                Vec2::new(bow_x + 10.0, top_left.y + 25.0),
                Vec2::new(bow_x, top_left.y + 38.0),
                Color::from_rgba8(74, 41, 36, 255),
            );
            ctx.draw_rect(Vec2::new(bow_x - 2.0, top_left.y + 24.0), Vec2::new(21.0, 2.0), aim_color);
            ctx.draw_triangle(
                Vec2::new(bow_x + 19.0, top_left.y + 21.0),
                Vec2::new(bow_x + 25.0, top_left.y + 25.0),
                Vec2::new(bow_x + 19.0, top_left.y + 29.0),
                aim_color,
            );
        }

        Self::draw_bar(
            ctx,
            Vec2::new(top_left.x - 5.0, top_left.y - 13.0),
            Vec2::new(HERO_SIZE.x + 10.0, 7.0),
            self.hero.health / self.hero.max_health,
            Color::from_rgba8(181, 87, 220, 255),
        );
        Self::draw_text_centered(
            ctx,
            "HEROI",
            self.hero.pos.x,
            top_left.y - 25.0,
            0.85,
            Color::from_rgba8(222, 183, 244, 255),
        );
    }

    pub(super) fn draw_hero_aim(&self, ctx: &mut dyn GameContext<Action>, width: f32) {
        if !self.hero.alive() || self.state != BattleState::Playing {
            return;
        }

        let start: Vec2 = self.hero.pos + Vec2::new(17.0, -7.0);
        let target: Vec2 = self.hero_aim_target(width);
        let direction: Vec2 = (target - start).normalise();
        let normal: Vec2 = Vec2::new(-direction.y, direction.x) * 1.2;
        let color: Color = self.hero_aim_color();
        Self::draw_quad(
            ctx,
            start + normal,
            target + normal,
            target - normal,
            start - normal,
            color.with_alpha(0.18),
        );

        match self.hero.aim_style {
            0 => {
                ctx.draw_rect(target + Vec2::new(-19.0, -2.0), Vec2::new(12.0, 4.0), color);
                ctx.draw_rect(target + Vec2::new(7.0, -2.0), Vec2::new(12.0, 4.0), color);
                ctx.draw_rect(target + Vec2::new(-2.0, -19.0), Vec2::new(4.0, 12.0), color);
                ctx.draw_rect(target + Vec2::new(-2.0, 7.0), Vec2::new(4.0, 12.0), color);
            }
            1 => {
                for offset in [
                    Vec2::new(-16.0, -16.0),
                    Vec2::new(10.0, -16.0),
                    Vec2::new(-16.0, 10.0),
                    Vec2::new(10.0, 10.0),
                ] {
                    ctx.draw_rect(target + offset, Vec2::splat(6.0), color);
                }
                Self::draw_quad(
                    ctx,
                    target + Vec2::new(-5.0, 0.0),
                    target + Vec2::new(0.0, -5.0),
                    target + Vec2::new(5.0, 0.0),
                    target + Vec2::new(0.0, 5.0),
                    color,
                );
            }
            _ => {
                Self::draw_quad(
                    ctx,
                    target + Vec2::new(-18.0, 0.0),
                    target + Vec2::new(0.0, -18.0),
                    target + Vec2::new(18.0, 0.0),
                    target + Vec2::new(0.0, 18.0),
                    color.with_alpha(0.72),
                );
                Self::draw_quad(
                    ctx,
                    target + Vec2::new(-10.0, 0.0),
                    target + Vec2::new(0.0, -10.0),
                    target + Vec2::new(10.0, 0.0),
                    target + Vec2::new(0.0, 10.0),
                    Color::from_rgba8(34, 38, 39, 255),
                );
                ctx.draw_rect(target - Vec2::splat(2.0), Vec2::splat(4.0), color);
            }
        }
    }

    pub(super) fn draw_effects(&self, ctx: &mut dyn GameContext<Action>) {
        for effect in &self.effects {
            let progress: f32 = (1.0 - effect.life / effect.max_life).clamp(0.0, 1.0);
            let direction: Vec2 = (effect.end - effect.start).normalise();
            let normal: Vec2 = Vec2::new(-direction.y, direction.x);

            match effect.kind {
                AttackEffectKind::Arrow => {
                    let flight: f32 = 1.0 - (1.0 - progress) * (1.0 - progress);
                    let pos: Vec2 = effect.start.lerp(effect.end, flight);
                    let tail: Vec2 = pos - direction * 10.0;
                    let tip: Vec2 = pos + direction * 7.0;
                    Self::draw_line(
                        ctx,
                        pos - direction * 22.0,
                        pos - direction * 4.0,
                        2.0,
                        effect.color.with_alpha(0.2),
                    );
                    Self::draw_line(ctx, tail, tip, 2.2, Color::from_rgba8(105, 70, 39, 255));
                    ctx.draw_triangle(
                        tip + direction * 5.0,
                        tip - direction * 2.0 + normal * 4.0,
                        tip - direction * 2.0 - normal * 4.0,
                        effect.color,
                    );
                    ctx.draw_triangle(
                        tail,
                        tail - direction * 6.0 + normal * 4.0,
                        tail - direction * 3.0,
                        effect.color.lerp(Color::WHITE, 0.28),
                    );
                    ctx.draw_triangle(
                        tail,
                        tail - direction * 3.0,
                        tail - direction * 6.0 - normal * 4.0,
                        effect.color.lerp(Color::WHITE, 0.12),
                    );
                    if progress > 0.78 {
                        let impact: f32 = (progress - 0.78) / 0.22;
                        let radius: f32 = 3.0 + impact * 8.0;
                        Self::draw_quad(
                            ctx,
                            effect.end - normal * radius,
                            effect.end - direction * radius,
                            effect.end + normal * radius,
                            effect.end + direction * radius,
                            effect.color.with_alpha((1.0 - impact) * 0.55),
                        );
                    }
                }
                AttackEffectKind::MeleeImpact => {
                    let alpha: f32 = (1.0 - progress) * 0.92;
                    let slash_direction: Vec2 = (direction + normal * 0.85).normalise();
                    let radius: f32 = 6.0 + progress * 10.0;
                    Self::draw_line(
                        ctx,
                        effect.end - slash_direction * radius,
                        effect.end + slash_direction * radius,
                        4.0 - progress * 2.0,
                        effect.color.with_alpha(alpha),
                    );
                    Self::draw_quad(
                        ctx,
                        effect.end - normal * (3.0 + progress * 4.0),
                        effect.end - direction * (3.0 + progress * 4.0),
                        effect.end + normal * (3.0 + progress * 4.0),
                        effect.end + direction * (3.0 + progress * 4.0),
                        Color::WHITE.with_alpha(alpha * 0.7),
                    );
                }
                AttackEffectKind::HeavyImpact => {
                    let alpha: f32 = (1.0 - progress) * 0.88;
                    let radius: f32 = 8.0 + progress * 17.0;
                    Self::draw_quad(
                        ctx,
                        effect.end - normal * radius,
                        effect.end - direction * (radius * 0.6),
                        effect.end + normal * radius,
                        effect.end + direction * (radius * 0.6),
                        effect.color.with_alpha(alpha * 0.42),
                    );
                    Self::draw_line(
                        ctx,
                        effect.end - normal * radius,
                        effect.end + normal * radius,
                        5.0 - progress * 3.0,
                        effect.color.with_alpha(alpha),
                    );
                    for offset in [-1.0_f32, 0.0, 1.0] {
                        let shard_start: Vec2 = effect.end + normal * offset * 8.0;
                        let shard_end: Vec2 = shard_start - direction * (8.0 + progress * 15.0) + normal * offset * 4.0;
                        Self::draw_line(ctx, shard_start, shard_end, 2.5, effect.color.with_alpha(alpha * 0.75));
                    }
                }
                AttackEffectKind::CastleBolt => {
                    let flight: f32 = 1.0 - (1.0 - progress) * (1.0 - progress);
                    let pos: Vec2 = effect.start.lerp(effect.end, flight);
                    Self::draw_line(
                        ctx,
                        pos - direction * 24.0,
                        pos - direction * 3.0,
                        4.0,
                        effect.color.with_alpha(0.24),
                    );
                    Self::draw_quad(
                        ctx,
                        pos - normal * 6.0,
                        pos - direction * 8.0,
                        pos + normal * 6.0,
                        pos + direction * 8.0,
                        effect.color.with_alpha(0.35),
                    );
                    Self::draw_quad(
                        ctx,
                        pos - normal * 3.0,
                        pos - direction * 4.0,
                        pos + normal * 3.0,
                        pos + direction * 4.0,
                        Color::WHITE,
                    );
                }
                AttackEffectKind::Lightning => {
                    let alpha: f32 = (1.0 - progress) * 0.9;
                    let first: Vec2 = effect.start.lerp(effect.end, 0.3) + normal * 11.0;
                    let second: Vec2 = effect.start.lerp(effect.end, 0.58) - normal * 9.0;
                    let third: Vec2 = effect.start.lerp(effect.end, 0.79) + normal * 7.0;
                    for (start, end) in [
                        (effect.start, first),
                        (first, second),
                        (second, third),
                        (third, effect.end),
                    ] {
                        Self::draw_line(ctx, start, end, 6.0, effect.color.with_alpha(alpha * 0.28));
                        Self::draw_line(ctx, start, end, 2.4, Color::WHITE.with_alpha(alpha));
                    }
                    let radius: f32 = 8.0 + progress * 25.0;
                    Self::draw_quad(
                        ctx,
                        effect.end + Vec2::new(-radius, 0.0),
                        effect.end + Vec2::new(0.0, -radius * 0.45),
                        effect.end + Vec2::new(radius, 0.0),
                        effect.end + Vec2::new(0.0, radius * 0.45),
                        effect.color.with_alpha(alpha * 0.32),
                    );
                }
            }
        }
    }

    fn draw_coin(ctx: &mut dyn GameContext<Action>, center: Vec2, radius: f32) {
        let gold: Color = Color::from_rgba8(244, 190, 57, 255);
        Self::draw_quad(
            ctx,
            center + Vec2::new(-radius, 0.0),
            center + Vec2::new(0.0, -radius),
            center + Vec2::new(radius, 0.0),
            center + Vec2::new(0.0, radius),
            gold,
        );
        Self::draw_quad(
            ctx,
            center + Vec2::new(-radius * 0.45, 0.0),
            center + Vec2::new(0.0, -radius * 0.45),
            center + Vec2::new(radius * 0.45, 0.0),
            center + Vec2::new(0.0, radius * 0.45),
            Color::from_rgba8(255, 224, 111, 255),
        );
    }

    fn draw_trophy_icon(ctx: &mut dyn GameContext<Action>, center: Vec2, scale: f32) {
        let gold: Color = Color::from_rgba8(225, 183, 82, 255);
        ctx.draw_rect(center + Vec2::new(-9.0, -10.0) * scale, Vec2::new(18.0, 10.0) * scale, gold);
        ctx.draw_rect(
            center + Vec2::new(-13.0, -8.0) * scale,
            Vec2::new(5.0, 7.0) * scale,
            gold.lerp(Color::BLACK, 0.12),
        );
        ctx.draw_rect(
            center + Vec2::new(8.0, -8.0) * scale,
            Vec2::new(5.0, 7.0) * scale,
            gold.lerp(Color::BLACK, 0.12),
        );
        ctx.draw_triangle(
            center + Vec2::new(-9.0, 0.0) * scale,
            center + Vec2::new(9.0, 0.0) * scale,
            center + Vec2::new(0.0, 7.0) * scale,
            gold,
        );
        ctx.draw_rect(center + Vec2::new(-2.0, 6.0) * scale, Vec2::new(4.0, 7.0) * scale, gold);
        ctx.draw_rect(center + Vec2::new(-8.0, 12.0) * scale, Vec2::new(16.0, 4.0) * scale, gold);
    }

    fn draw_spell_icon(ctx: &mut dyn GameContext<Action>, center: Vec2, scale: f32, ready: bool) {
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

    fn draw_troop_icon(ctx: &mut dyn GameContext<Action>, center: Vec2, scale: f32) {
        let green: Color = Color::from_rgba8(104, 205, 105, 255);
        Self::draw_prism(
            ctx,
            center + Vec2::new(-8.0, -13.0) * scale,
            Vec2::new(16.0, 16.0) * scale,
            Vec2::new(3.0, -3.0) * scale,
            green.lerp(Color::WHITE, 0.08),
        );
        Self::draw_prism(
            ctx,
            center + Vec2::new(-11.0, 4.0) * scale,
            Vec2::new(22.0, 15.0) * scale,
            Vec2::new(3.0, -3.0) * scale,
            green,
        );
    }

    fn draw_unit_icon(
        ctx: &mut dyn GameContext<Action>,
        kind: UnitKind,
        pos: Vec2,
        size: Vec2,
        scale: f32,
        available: bool,
    ) {
        let color: Color = if available {
            kind.color(Faction::Player)
        } else {
            Color::from_rgba8(67, 69, 70, 255)
        };
        Self::draw_prism(
            ctx,
            pos + Vec2::new(size.x * 0.3, 12.0 * scale),
            Vec2::new(size.x * 0.4, 35.0 * scale),
            Vec2::new(4.0, -4.0) * scale,
            color,
        );
        Self::draw_prism(
            ctx,
            pos + Vec2::new(size.x * 0.37, 7.0 * scale),
            Vec2::new(size.x * 0.26, 17.0 * scale),
            Vec2::new(3.0, -3.0) * scale,
            color.lerp(Color::WHITE, 0.1),
        );
        let cost_text: String = format!("{}", kind.stats().cost as u32);
        Self::draw_coin(ctx, pos + Vec2::new(13.0 * scale, size.y - 9.0 * scale), 6.0 * scale);
        Self::draw_text(
            ctx,
            &cost_text,
            pos + Vec2::new(23.0 * scale, size.y - 14.0 * scale),
            1.35 * scale,
            Color::from_rgba8(248, 221, 139, 255),
        );
    }

    pub(super) fn draw_hud(&self, ctx: &mut dyn GameContext<Action>, width: f32, height: f32) {
        let scale: f32 = Self::hud_scale(width, height);
        let status_pos: Vec2 = Vec2::new(14.0, 14.0) * scale;
        let status_size: Vec2 = Vec2::new((width - 28.0 * scale).min(520.0 * scale), 72.0 * scale);
        let status_fill: Color = Color::from_rgba8(58, 78, 76, 228);
        let status_border: Color = Color::from_rgba8(158, 186, 181, 238);
        Self::draw_hud_panel(ctx, status_pos, status_size, scale, status_fill, status_border);

        let cell_width: f32 = status_size.x * 0.25;
        for index in 1..4 {
            ctx.draw_rect(
                status_pos + Vec2::new(cell_width * index as f32, 10.0 * scale),
                Vec2::new(2.0 * scale, status_size.y - 20.0 * scale),
                Color::from_rgba8(184, 207, 199, 72),
            );
        }

        let player_troops: usize = self
            .units
            .iter()
            .filter(|unit| unit.faction == Faction::Player && unit.alive())
            .count()
            + self.player_recruit_queue.len();
        let labels: [&str; 4] = ["OURO", "CASTELO", "MAGIA", "TROPAS"];
        let values: [String; 4] = [
            format!("{}", self.player_coins.floor() as u32),
            format!("NVL {}", self.player_castle.level + 1),
            if self.spell_cooldown <= 0.0 {
                "PRONTA".to_owned()
            } else {
                format!("{}S", self.spell_cooldown.ceil() as u32)
            },
            format!("{} / {}", player_troops, MAX_UNITS),
        ];
        let ratios: [f32; 4] = [
            self.player_coins / COIN_METER_RANGE,
            self.player_castle.health / self.player_castle.max_health,
            1.0 - self.spell_cooldown / SPELL_COOLDOWN,
            player_troops as f32 / MAX_UNITS as f32,
        ];
        let bar_colors: [Color; 4] = [
            Color::from_rgba8(244, 190, 57, 255),
            Color::from_rgba8(103, 207, 111, 255),
            Color::from_rgba8(111, 226, 209, 255),
            Color::from_rgba8(99, 190, 94, 255),
        ];

        for index in 0..4 {
            let cell_pos: Vec2 = status_pos + Vec2::new(cell_width * index as f32, 0.0);
            Self::draw_text(
                ctx,
                labels[index],
                cell_pos + Vec2::new(43.0, 13.0) * scale,
                0.85 * scale,
                Color::from_rgba8(188, 209, 201, 255),
            );
            Self::draw_text(
                ctx,
                &values[index],
                cell_pos + Vec2::new(43.0, 32.0) * scale,
                1.45 * scale,
                if index == 0 {
                    Color::from_rgba8(255, 226, 126, 255)
                } else {
                    Color::WHITE
                },
            );
            Self::draw_bar(
                ctx,
                cell_pos + Vec2::new(9.0, 60.0) * scale,
                Vec2::new(cell_width - 18.0 * scale, 6.0 * scale),
                ratios[index],
                bar_colors[index],
            );
        }

        Self::draw_coin(ctx, status_pos + Vec2::new(24.0, 35.0) * scale, 11.0 * scale);
        Self::draw_trophy_icon(ctx, status_pos + Vec2::new(cell_width + 24.0 * scale, 34.0 * scale), scale);
        Self::draw_spell_icon(
            ctx,
            status_pos + Vec2::new(cell_width * 2.0 + 24.0 * scale, 35.0 * scale),
            scale,
            self.spell_cooldown <= 0.0,
        );
        Self::draw_troop_icon(
            ctx,
            status_pos + Vec2::new(cell_width * 3.0 + 24.0 * scale, 34.0 * scale),
            scale,
        );

        let (command_pos, command_size): (Vec2, Vec2) = Self::command_panel_rect(width, height);
        Self::draw_hud_panel(
            ctx,
            command_pos,
            command_size,
            scale,
            Color::from_rgba8(81, 87, 65, 235),
            Color::from_rgba8(166, 172, 123, 245),
        );
        let queue_progress: f32 = if self.player_recruit_queue.is_empty() {
            0.0
        } else {
            1.0 - self.player_recruit_timer / PLAYER_RECRUIT_INTERVAL
        };
        Self::draw_bar(
            ctx,
            command_pos + Vec2::new(14.0, 7.0) * scale,
            Vec2::new(command_size.x - 28.0 * scale, 5.0 * scale),
            queue_progress,
            Color::from_rgba8(245, 199, 77, 255),
        );

        if self.state == BattleState::Playing {
            let (skills_pos, skills_size): (Vec2, Vec2) = Self::skills_button_rect(width, height);
            let hovered: bool = ctx
                .input()
                .mouse_position()
                .is_some_and(|mouse: Vec2| Self::point_in_rect(mouse, skills_pos, skills_size));
            Self::draw_hud_panel(
                ctx,
                skills_pos,
                skills_size,
                scale,
                if hovered || self.skills_panel_open {
                    Color::from_rgba8(70, 94, 119, 245)
                } else {
                    Color::from_rgba8(49, 67, 82, 235)
                },
                Color::from_rgba8(151, 174, 181, 245),
            );
            Self::draw_chamfered_rect(
                ctx,
                skills_pos + Vec2::new(8.0, 8.0) * scale,
                Vec2::splat(50.0 * scale),
                7.0 * scale,
                Color::from_rgba8(22, 29, 37, 255),
            );
            let rune_center: Vec2 = skills_pos + Vec2::new(33.0, 33.0) * scale;
            Self::draw_quad(
                ctx,
                rune_center + Vec2::new(-14.0, 0.0) * scale,
                rune_center + Vec2::new(0.0, -17.0) * scale,
                rune_center + Vec2::new(14.0, 0.0) * scale,
                rune_center + Vec2::new(0.0, 17.0) * scale,
                Color::from_rgba8(122, 98, 210, 255),
            );
            Self::draw_text_centered(ctx, "H", rune_center.x, rune_center.y - 7.0 * scale, 2.0 * scale, Color::WHITE);
            Self::draw_text(
                ctx,
                "HABIL.",
                skills_pos + Vec2::new(66.0, 17.0) * scale,
                1.25 * scale,
                Color::from_rgba8(222, 232, 236, 255),
            );
            Self::draw_text(
                ctx,
                "TECLA H",
                skills_pos + Vec2::new(66.0, 39.0) * scale,
                0.8 * scale,
                Color::from_rgba8(159, 183, 191, 255),
            );
        }

        for (index, kind) in UnitKind::ALL.into_iter().enumerate() {
            let (pos, size): (Vec2, Vec2) = Self::button_rect(index, width, height);
            let available: bool = self.player_coins >= kind.stats().cost
                && self.units.len() + self.player_recruit_queue.len() < MAX_UNITS;
            let hovered: bool = ctx
                .input()
                .mouse_position()
                .is_some_and(|mouse| Self::point_in_rect(mouse, pos, size));
            Self::draw_hud_panel(
                ctx,
                pos,
                size,
                scale,
                if available && hovered {
                    Color::from_rgba8(126, 135, 93, 255)
                } else if available {
                    Color::from_rgba8(101, 109, 77, 255)
                } else {
                    Color::from_rgba8(61, 64, 57, 255)
                },
                Color::from_rgba8(182, 187, 139, 245),
            );
            Self::draw_chamfered_rect(
                ctx,
                pos + Vec2::splat(5.0 * scale),
                size - Vec2::splat(10.0 * scale),
                5.0 * scale,
                if available {
                    Color::from_rgba8(25, 31, 30, 255)
                } else {
                    Color::from_rgba8(36, 38, 37, 255)
                },
            );
            Self::draw_unit_icon(ctx, kind, pos, size, scale, available);
            let queued_count: usize = self
                .player_recruit_queue
                .iter()
                .filter(|queued_kind| **queued_kind == kind)
                .count();
            if queued_count > 0 {
                let badge_pos: Vec2 = pos + Vec2::new(size.x - 24.0 * scale, 5.0 * scale);
                ctx.draw_rect(badge_pos, Vec2::new(19.0, 16.0) * scale, Color::from_rgba8(27, 31, 30, 245));
                Self::draw_text_centered(
                    ctx,
                    &queued_count.to_string(),
                    badge_pos.x + 9.5 * scale,
                    badge_pos.y + 4.0 * scale,
                    1.05 * scale,
                    Color::from_rgba8(248, 210, 105, 255),
                );
            }
        }

        let (upgrade_pos, upgrade_size): (Vec2, Vec2) = Self::button_rect(4, width, height);
        let can_upgrade: bool =
            self.player_castle.level < 3 && self.player_coins >= Self::upgrade_cost(self.player_castle.level);
        let upgrade_hovered: bool = ctx
            .input()
            .mouse_position()
            .is_some_and(|mouse| Self::point_in_rect(mouse, upgrade_pos, upgrade_size));
        Self::draw_hud_panel(
            ctx,
            upgrade_pos,
            upgrade_size,
            scale,
            if can_upgrade && upgrade_hovered {
                Color::from_rgba8(145, 113, 49, 255)
            } else if can_upgrade {
                Color::from_rgba8(112, 87, 40, 255)
            } else {
                Color::from_rgba8(47, 43, 37, 255)
            },
            Color::from_rgba8(190, 164, 103, 245),
        );
        Self::draw_chamfered_rect(
            ctx,
            upgrade_pos + Vec2::splat(5.0 * scale),
            upgrade_size - Vec2::splat(10.0 * scale),
            5.0 * scale,
            Color::from_rgba8(31, 29, 26, 255),
        );
        for level in 0..3 {
            Self::draw_prism(
                ctx,
                upgrade_pos + Vec2::new(13.0 + level as f32 * 15.0, 28.0 - level as f32 * 7.0) * scale,
                Vec2::new(10.0, 25.0 + level as f32 * 7.0) * scale,
                Vec2::new(2.5, -2.5) * scale,
                if level < self.player_castle.level {
                    Color::from_rgba8(248, 195, 69, 255)
                } else {
                    Color::from_rgba8(106, 90, 62, 255)
                },
            );
        }
        if self.player_castle.level < 3 {
            let cost_text: String = format!("{}", Self::upgrade_cost(self.player_castle.level) as u32);
            Self::draw_text_centered(
                ctx,
                &cost_text,
                upgrade_pos.x + upgrade_size.x * 0.5,
                upgrade_pos.y + upgrade_size.y - 13.0 * scale,
                1.15 * scale,
                Color::from_rgba8(252, 218, 117, 255),
            );
        } else {
            Self::draw_text_centered(
                ctx,
                "MAX",
                upgrade_pos.x + upgrade_size.x * 0.5,
                upgrade_pos.y + upgrade_size.y - 13.0 * scale,
                1.15 * scale,
                Color::from_rgba8(252, 218, 117, 255),
            );
        }
    }

    pub(super) fn draw_skills_panel(&self, ctx: &mut dyn GameContext<Action>, width: f32, height: f32) {
        if !self.skills_panel_open {
            return;
        }

        ctx.draw_rect(Vec2::ZERO, Vec2::new(width, height), Color::from_rgba8(10, 14, 15, 205));
        let (panel_pos, panel_size): (Vec2, Vec2) = Self::skills_panel_rect(width, height);
        Self::draw_prism(
            ctx,
            panel_pos,
            panel_size,
            Vec2::new(11.0, -9.0),
            Color::from_rgba8(37, 49, 56, 255),
        );
        Self::draw_text_centered(
            ctx,
            "HABILIDADES PASSIVAS",
            width * 0.5,
            panel_pos.y + 27.0,
            2.35,
            Color::from_rgba8(228, 203, 111, 255),
        );
        Self::draw_text_centered(
            ctx,
            "COMPRE COM OURO - A BATALHA ESTA PAUSADA",
            width * 0.5,
            panel_pos.y + 63.0,
            1.15,
            Color::from_rgba8(157, 178, 186, 255),
        );

        let (close_pos, close_size): (Vec2, Vec2) = Self::skills_close_button_rect(width, height);
        let mouse: Option<Vec2> = ctx.input().mouse_position();
        for (index, passive) in PassiveKind::ALL.into_iter().enumerate() {
            let (slot_pos, slot_size): (Vec2, Vec2) = Self::passive_slot_rect(index, width, height);
            let unlocked: bool = self.player_passives.contains(passive);
            let affordable: bool = self.player_coins >= passive.cost();
            let hovered: bool = mouse.is_some_and(|mouse| Self::point_in_rect(mouse, slot_pos, slot_size));
            let slot_color: Color = if unlocked {
                Color::from_rgba8(43, 91, 64, 255)
            } else if affordable && hovered {
                Color::from_rgba8(116, 91, 45, 255)
            } else if affordable {
                Color::from_rgba8(76, 66, 43, 255)
            } else {
                Color::from_rgba8(27, 35, 40, 255)
            };
            Self::draw_prism(ctx, slot_pos, slot_size, Vec2::new(5.0, -4.0), slot_color);

            let icon_center: Vec2 = slot_pos + Vec2::new(28.0, 31.0);
            Self::draw_quad(
                ctx,
                icon_center + Vec2::new(-12.0, 0.0),
                icon_center + Vec2::new(0.0, -12.0),
                icon_center + Vec2::new(12.0, 0.0),
                icon_center + Vec2::new(0.0, 12.0),
                passive.unit_kind().color(Faction::Player),
            );
            Self::draw_text(
                ctx,
                passive.title(),
                slot_pos + Vec2::new(50.0, 22.0),
                1.5,
                if unlocked {
                    Color::from_rgba8(172, 240, 177, 255)
                } else {
                    Color::from_rgba8(231, 218, 175, 255)
                },
            );
            let description: [&str; 2] = passive.description();
            Self::draw_text(
                ctx,
                description[0],
                slot_pos + Vec2::new(16.0, slot_size.y * 0.48),
                1.05,
                Color::from_rgba8(166, 187, 194, 255),
            );
            Self::draw_text_centered(
                ctx,
                description[1],
                slot_pos.x + slot_size.x * 0.5,
                slot_pos.y + slot_size.y * 0.62,
                1.05,
                Color::from_rgba8(166, 187, 194, 255),
            );

            let status: String = if unlocked {
                "COMPRADA".to_owned()
            } else {
                format!("OURO {}", passive.cost() as u32)
            };
            Self::draw_text_centered(
                ctx,
                &status,
                slot_pos.x + slot_size.x * 0.5,
                slot_pos.y + slot_size.y - 22.0,
                1.35,
                if unlocked {
                    Color::from_rgba8(119, 231, 142, 255)
                } else if affordable {
                    Color::from_rgba8(250, 205, 83, 255)
                } else {
                    Color::from_rgba8(112, 121, 123, 255)
                },
            );
        }

        let close_hovered: bool = ctx
            .input()
            .mouse_position()
            .is_some_and(|mouse: Vec2| Self::point_in_rect(mouse, close_pos, close_size));
        Self::draw_prism(
            ctx,
            close_pos,
            close_size,
            Vec2::new(6.0, -5.0),
            if close_hovered {
                Color::from_rgba8(157, 77, 66, 255)
            } else {
                Color::from_rgba8(105, 59, 56, 255)
            },
        );
        Self::draw_text_centered(ctx, "FECHAR", width * 0.5, close_pos.y + 14.0, 2.0, Color::WHITE);
    }

    pub(super) fn draw_end_overlay(&self, ctx: &mut dyn GameContext<Action>, width: f32, height: f32) {
        if self.state == BattleState::Playing {
            return;
        }

        let color: Color = if self.state == BattleState::Victory {
            Color::from_rgba8(44, 132, 78, 225)
        } else {
            Color::from_rgba8(145, 55, 48, 225)
        };
        let panel_pos: Vec2 = Vec2::new(width * 0.25, height * 0.27);
        let panel_size: Vec2 = Vec2::new(width * 0.5, height * 0.34);
        ctx.draw_rect(panel_pos, panel_size, color);

        let emblem_center: Vec2 = panel_pos + panel_size * 0.5;
        if self.state == BattleState::Victory {
            ctx.draw_triangle(
                emblem_center + Vec2::new(-48.0, 0.0),
                emblem_center + Vec2::new(-12.0, 38.0),
                emblem_center + Vec2::new(54.0, -44.0),
                Color::WHITE,
            );
        } else {
            ctx.draw_rect(emblem_center + Vec2::new(-48.0, -8.0), Vec2::new(96.0, 16.0), Color::WHITE);
        }
        ctx.draw_rect(
            Vec2::new(emblem_center.x - 54.0, panel_pos.y + panel_size.y - 42.0),
            Vec2::new(108.0, 18.0),
            Color::from_rgba8(35, 39, 38, 255),
        );
    }
}

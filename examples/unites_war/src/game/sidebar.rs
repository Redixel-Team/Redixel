use super::*;

#[derive(Clone, Copy)]
pub(super) struct UiIcons {
    sword: TextureId,
    shield: TextureId,
    lock: TextureId,
}

impl UiIcons {
    pub(super) fn load(ctx: &mut dyn GameContext<Action>) -> Self {
        Self {
            sword: ctx.load_texture_filtered(include_bytes!("../../assets/ui_icons/sword.png"), TextureFilter::Linear),
            shield: ctx
                .load_texture_filtered(include_bytes!("../../assets/ui_icons/shield.png"), TextureFilter::Linear),
            lock: ctx.load_texture_filtered(include_bytes!("../../assets/ui_icons/lock.png"), TextureFilter::Linear),
        }
    }
}

const CATEGORIES: [(CommandTab, &str, &str); 5] = [
    (CommandTab::Army, "TROPAS", "RECRUTE UNIDADES PARA O CAMPO DE BATALHA."),
    (CommandTab::Research, "MELHORIAS", "MELHORE AS ARMAS E A ARMADURA DAS TROPAS."),
    (CommandTab::Magic, "MAGIAS", "SELECIONE UMA MAGIA E CLIQUE NO CAMPO."),
    (CommandTab::Hero, "HEROI", "CONVOQUE O CHEFE E COMANDE SEU AVANCO."),
    (CommandTab::Tower, "TORRE", "MELHORE O CANHAO, O REPARO E A VIDA DA TORRE."),
];

enum Command {
    Recruit(UnitKind),
    Research(Research),
    Tower(TowerUpgrade),
    Summon,
    Advance,
    Retreat,
    Spell(Spell),
    Fury,
}

struct Card {
    title: String,
    status: String,
    detail: String,
    key: &'static str,
    enabled: bool,
    portrait: Option<(UnitKind, bool)>,
    command: Command,
}

fn gold() -> Color {
    Color::from_rgba8(230, 199, 123, 255)
}
fn text_color() -> Color {
    Color::from_rgba8(213, 210, 175, 255)
}

impl UnitesWar {
    pub(super) fn toggle_sidebar(&mut self, tab: CommandTab) {
        self.clan.panel_open = !self.clan.panel_open || self.clan.tab != tab;
        self.clan.tab = tab;
        self.clan.selected_spell = None;
    }
    pub(super) fn sidebar_tab_rect(index: usize, width: f32, height: f32) -> (Vec2, Vec2) {
        let s = Self::view_scale(width, height);
        (
            Vec2::new((8.0 + index as f32 * 56.0) * s, height - 54.0 * s),
            Vec2::splat(46.0 * s),
        )
    }

    pub(super) fn command_panel_rect(width: f32, height: f32) -> (Vec2, Vec2) {
        let s = Self::view_scale(width, height);
        (Vec2::new(8.0, 44.0) * s, Vec2::new(180.0, 260.0) * s)
    }

    pub(super) fn button_rect(index: usize, width: f32, height: f32) -> (Vec2, Vec2) {
        let s = Self::view_scale(width, height);
        (Vec2::new(12.0, 68.0 + index as f32 * 58.0) * s, Vec2::new(172.0, 52.0) * s)
    }

    fn command_rect(&self, index: usize, width: f32, height: f32) -> (Vec2, Vec2) {
        if self.clan.tab == CommandTab::Hero {
            let s = Self::view_scale(width, height);
            (
                Vec2::new(8.0 * s, height - (224.0 - index as f32 * 40.0) * s),
                Vec2::splat(36.0 * s),
            )
        } else {
            Self::button_rect(index, width, height)
        }
    }

    pub(super) fn minimap_rect(width: f32, height: f32) -> (Vec2, Vec2) {
        let s = Self::view_scale(width, height);
        (Vec2::new(width - 248.0 * s, height - 36.0 * s), Vec2::new(240.0, 28.0) * s)
    }

    fn pause_rect(width: f32, height: f32) -> (Vec2, Vec2) {
        let s = Self::view_scale(width, height);
        (Vec2::new(width - 40.0 * s, 5.0 * s), Vec2::splat(28.0 * s))
    }

    pub(super) fn is_battlefield_point(&self, point: Vec2, width: f32, height: f32) -> bool {
        let s = Self::view_scale(width, height);
        if point.x < 0.0 || point.x > width || point.y <= 38.0 * s || point.y >= height {
            return false;
        }
        let mut rects = vec![Self::minimap_rect(width, height)];
        rects.extend((0..CATEGORIES.len()).map(|i| Self::sidebar_tab_rect(i, width, height)));
        if self.clan.panel_open {
            if self.clan.tab == CommandTab::Hero {
                rects.extend((0..self.sidebar_cards().len()).map(|i| self.command_rect(i, width, height)));
            } else {
                let (p, mut z) = Self::command_panel_rect(width, height);
                z.y = (28.0 + self.sidebar_cards().len() as f32 * 58.0) * s;
                rects.push((p, z));
            }
        }
        if !self.player_recruit_queue.is_empty() {
            rects.push((Vec2::new(296.0 * s, height - 35.0 * s), Vec2::new(300.0, 27.0) * s));
        }
        !rects.into_iter().any(|(p, z)| Self::point_in_rect(point, p, z))
    }

    fn command_locked(&self, command: &Command) -> bool {
        match command {
            Command::Recruit(kind) => !self.unit_unlocked(*kind, Faction::Player),
            Command::Spell(Spell::Meteor) => self.clan.experience[0] < 500.0,
            Command::Summon => self.clan.experience[0] < HERO_UNLOCK_XP,
            Command::Advance | Command::Retreat | Command::Fury => !self.hero.alive(),
            _ => false,
        }
    }

    fn draw_lock_icon(&self, ctx: &mut dyn GameContext<Action>, center: Vec2, size: f32) {
        if let Some(icons) = self.ui_icons {
            ctx.draw_sprite(center - Vec2::splat(size * 0.5), Vec2::splat(size), icons.lock);
        }
    }

    fn sidebar_cards(&self) -> Vec<Card> {
        match self.clan.tab {
            CommandTab::Army => UnitKind::ALL
                .into_iter()
                .enumerate()
                .map(|(i, kind)| {
                    let unlocked = self.unit_unlocked(kind, Faction::Player);
                    Card {
                        title: kind.name().into(),
                        status: if unlocked {
                            format!("{} OURO", kind.stats().cost as u32)
                        } else {
                            format!("{} XP", kind.unlock_xp() as u32)
                        },
                        detail: format!(
                            "{} VIDA   {} DANO   {} POPULACAO",
                            kind.stats().health as u32,
                            kind.stats().damage as u32,
                            kind.population()
                        ),
                        key: ["1", "2", "3", "4"][i],
                        enabled: unlocked
                            && self.player_coins >= kind.stats().cost
                            && self.population(Faction::Player) + kind.population()
                                <= self.population_cap(Faction::Player),
                        portrait: Some((kind, false)),
                        command: Command::Recruit(kind),
                    }
                })
                .collect(),
            CommandTab::Research => {
                let cards: Vec<_> = Research::ALL
                    .into_iter()
                    .map(|research| Card {
                        title: research.name().into(),
                        status: if self.clan.research[research.index()] >= 3 {
                            "MAXIMO".into()
                        } else {
                            format!(
                                "{} OURO - NV {}",
                                self.research_cost(research) as u32,
                                self.clan.research[research.index()] + 1
                            )
                        },
                        detail: research.description().into(),
                        key: ["I", "II", "+"][research.index()],
                        enabled: self.clan.research[research.index()] < 3
                            && self.player_coins >= self.research_cost(research),
                        portrait: None,
                        command: Command::Research(research),
                    })
                    .collect();
                cards
            }
            CommandTab::Tower => TowerUpgrade::ALL
                .into_iter()
                .map(|upgrade| {
                    let level = self.tower_level(Faction::Player, upgrade);
                    let cost = self.tower_upgrade_cost(Faction::Player, upgrade);
                    Card {
                        title: upgrade.name().into(),
                        status: if level >= 3 {
                            "MAXIMO".into()
                        } else {
                            format!("{} OURO - NV {}", cost as u32, level + 1)
                        },
                        detail: upgrade.description().into(),
                        key: ["M", "V", "A", "R", "+"][upgrade.index()],
                        enabled: level < 3 && self.player_coins >= cost,
                        portrait: None,
                        command: Command::Tower(upgrade),
                    }
                })
                .collect(),
            CommandTab::Magic => [
                (
                    "RAIO",
                    "Q",
                    self.spell_cooldown,
                    true,
                    Command::Spell(Spell::Lightning),
                    "DANO EM AREA. SELECIONE E CLIQUE NO CAMPO.",
                ),
                (
                    "METEORO",
                    "E",
                    self.clan.meteor_cooldown,
                    self.clan.experience[0] >= 500.0,
                    Command::Spell(Spell::Meteor),
                    "EXPLOSAO EM AREA. TAMBEM ATINGE A TORRE.",
                ),
            ]
            .into_iter()
            .enumerate()
            .map(|(i, (title, key, cooldown, unlocked, command, detail))| Card {
                title: title.into(),
                status: if !unlocked {
                    if i == 1 { "500 XP".into() } else { "REQUER CHEFE".into() }
                } else if cooldown > 0.0 {
                    format!("{} S", cooldown.ceil() as u32)
                } else {
                    "PRONTO".into()
                },
                detail: detail.into(),
                key,
                enabled: unlocked && cooldown <= 0.0,
                portrait: None,
                command,
            })
            .collect(),
            CommandTab::Hero => vec![
                Card {
                    title: "CONVOCAR CHEFE".into(),
                    status: if self.clan.experience[0] < HERO_UNLOCK_XP {
                        "250 XP".into()
                    } else if self.hero.alive() {
                        "EM CAMPO".into()
                    } else if self.hero.summoned {
                        format!("{} S", (HERO_RETURN_COOLDOWN - self.clan.hero_respawn).ceil() as u32)
                    } else {
                        "PRONTO".into()
                    },
                    detail: "O CHEFE NAO OCUPA POPULACAO.".into(),
                    key: "5",
                    enabled: self.clan.experience[0] >= HERO_UNLOCK_XP && !self.hero.summoned,
                    portrait: Some((UnitKind::Archer, true)),
                    command: Command::Summon,
                },
                Card {
                    title: "AVANCAR".into(),
                    status: "RUMO AO COMBATE".into(),
                    detail: "O CHEFE AVANCA ATE ENCONTRAR UM ALVO.".into(),
                    key: "",
                    enabled: self.hero.alive(),
                    portrait: None,
                    command: Command::Advance,
                },
                Card {
                    title: "RECUAR".into(),
                    status: "VOLTAR A BASE".into(),
                    detail: "ENTRA NO CASTELO. NOVA CONVOCACAO EM 30 S.".into(),
                    key: "",
                    enabled: self.hero.alive(),
                    portrait: None,
                    command: Command::Retreat,
                },
                Card {
                    title: "FURIA".into(),
                    status: if !self.hero.alive() {
                        "REQUER CHEFE".into()
                    } else if self.clan.fury_cooldown > 0.0 {
                        format!("{} S", self.clan.fury_cooldown.ceil() as u32)
                    } else {
                        "PRONTO".into()
                    },
                    detail: "DOBRA O DANO DO CHEFE POR 8 SEGUNDOS.".into(),
                    key: "W",
                    enabled: self.hero.alive() && self.clan.fury_cooldown <= 0.0,
                    portrait: None,
                    command: Command::Fury,
                },
            ],
        }
    }

    pub(super) fn handle_war_click(&mut self, mouse: Vec2, width: f32, height: f32) {
        if self.state != BattleState::Playing {
            self.reset();
            return;
        }
        let (p, z) = Self::pause_rect(width, height);
        if Self::point_in_rect(mouse, p, z) {
            self.clan.paused = !self.clan.paused;
            return;
        }
        let (p, z) = Self::minimap_rect(width, height);
        if Self::point_in_rect(mouse, p, z) {
            let s = Self::view_scale(width, height);
            self.clan.camera = (mouse.x - p.x - 4.0 * s) / (z.x - 8.0 * s) * WORLD_WIDTH - width / s * 0.5;
            self.move_camera(0.0, width, height);
            return;
        }
        for (i, (tab, _, _)) in CATEGORIES.into_iter().enumerate() {
            let (p, z) = Self::sidebar_tab_rect(i, width, height);
            if Self::point_in_rect(mouse, p, z) {
                self.toggle_sidebar(tab);
                return;
            }
        }
        if self.clan.paused {
            return;
        }
        if self.clan.panel_open {
            for (i, card) in self.sidebar_cards().into_iter().enumerate() {
                let (p, z) = self.command_rect(i, width, height);
                if !Self::point_in_rect(mouse, p, z) {
                    continue;
                }
                if self.command_locked(&card.command) {
                    self.announce(format!("{} - {}", card.title, card.status));
                    return;
                }
                match card.command {
                    Command::Recruit(kind) => {
                        self.enqueue_player_recruit(kind);
                    }
                    Command::Research(research) => {
                        if !self.buy_research(research) {
                            self.announce("OURO INSUFICIENTE OU NIVEL MAXIMO");
                        }
                    }
                    Command::Tower(upgrade) => {
                        if !self.buy_tower_upgrade(Faction::Player, upgrade) {
                            self.announce("OURO INSUFICIENTE OU NIVEL MAXIMO");
                        }
                    }
                    Command::Summon => {
                        self.summon_hero(WORLD_WIDTH, WORLD_HEIGHT);
                    }
                    Command::Advance => {
                        self.set_hero_direction(1.0);
                    }
                    Command::Retreat => {
                        self.set_hero_direction(-1.0);
                    }
                    Command::Spell(spell) => {
                        self.select_spell(spell);
                        if self.clan.selected_spell.is_some() {
                            self.clan.panel_open = false;
                        }
                    }
                    Command::Fury => {
                        self.cast_fury();
                    }
                }
                return;
            }
        }
        if self.is_battlefield_point(mouse, width, height) {
            self.clan.panel_open = false;
            self.cast_selected_spell(self.screen_to_world(mouse, width, height));
        }
    }

    pub(super) fn draw_war_hud(&self, ctx: &mut dyn GameContext<Action>, width: f32, height: f32) {
        let s = Self::view_scale(width, height);
        let mouse = ctx.input().mouse_position();
        ctx.draw_rect(Vec2::ZERO, Vec2::new(width, 38.0 * s), Color::from_rgba8(25, 31, 25, 225));
        for (i, value) in [
            format!("OURO  {}", self.player_coins as u32),
            format!("XP  {}", self.clan.experience[0] as u32),
            format!(
                "POP  {}/{}",
                self.population(Faction::Player),
                self.population_cap(Faction::Player)
            ),
            format!("REP  {}", self.clan.reputation),
        ]
        .into_iter()
        .enumerate()
        {
            Self::draw_text(
                ctx,
                &value,
                Vec2::new(16.0 + i as f32 * 145.0, 14.0) * s,
                1.25 * s,
                if i == 0 { gold() } else { text_color() },
            );
        }
        Self::draw_text(
            ctx,
            &format!("{}:{:02}", self.clan.elapsed as u32 / 60, self.clan.elapsed as u32 % 60),
            Vec2::new(width - 100.0 * s, 14.0 * s),
            1.15 * s,
            text_color(),
        );
        let (p, z) = Self::pause_rect(width, height);
        Self::wood_panel(ctx, p, z, s, self.clan.paused);
        if self.clan.paused {
            ctx.draw_triangle(
                p + Vec2::new(10.0, 7.0) * s,
                p + Vec2::new(21.0, 14.0) * s,
                p + Vec2::new(10.0, 21.0) * s,
                gold(),
            );
        } else {
            for x in [9.0, 16.0] {
                ctx.draw_rect(p + Vec2::new(x, 8.0) * s, Vec2::new(3.0, 12.0) * s, gold());
            }
        }
        let mut tooltip = None;
        for (i, (tab, label, detail)) in CATEGORIES.into_iter().enumerate() {
            let (p, z) = Self::sidebar_tab_rect(i, width, height);
            let hovered = mouse.is_some_and(|m| Self::point_in_rect(m, p, z));
            Self::wood_panel(ctx, p, z, s, hovered || (self.clan.panel_open && self.clan.tab == tab));
            match tab {
                CommandTab::Army | CommandTab::Hero => {
                    let hero = tab == CommandTab::Hero;
                    let kind = if hero { UnitKind::Archer } else { UnitKind::Runner };
                    let foot = p + Vec2::new(23.0, 34.0) * s;
                    if !self.draw_unit_portrait(ctx, foot, 26.0 * s, kind, hero) {
                        Self::draw_goblin(ctx, foot, 0.55 * s, kind, Faction::Player, Vec2::ZERO, false, hero);
                    }
                }
                CommandTab::Magic => Self::draw_spell_icon(ctx, p + Vec2::new(23.0, 21.0) * s, 0.85 * s, true),
                CommandTab::Tower => Self::draw_tower_icon(ctx, p + Vec2::new(23.0, 21.0) * s, 0.85 * s),
                CommandTab::Research => {
                    Self::draw_text_centered(ctx, "+", p.x + z.x * 0.5, p.y + 9.0 * s, 3.0 * s, gold())
                }
            }
            if tab == CommandTab::Hero && self.clan.experience[0] < HERO_UNLOCK_XP {
                self.draw_lock_icon(ctx, p + Vec2::new(34.0, 13.0) * s, 18.0 * s);
            }
            Self::draw_text_centered(ctx, label, p.x + z.x * 0.5, p.y + 38.0 * s, 0.65 * s, text_color());
            Self::draw_text(ctx, ["T", "M", "G", "C", "U"][i], p + Vec2::new(4.0, 4.0) * s, 0.7 * s, gold());
            if hovered {
                tooltip = Some((label.to_owned(), detail.to_owned()));
            }
        }
        if self.clan.panel_open && self.clan.tab == CommandTab::Hero {
            for (i, card) in self.sidebar_cards().into_iter().enumerate() {
                let (p, z) = self.command_rect(i, width, height);
                let hovered = mouse.is_some_and(|m| Self::point_in_rect(m, p, z));
                Self::wood_panel(ctx, p, z, s, hovered);
                let color = if card.enabled {
                    gold()
                } else {
                    Color::from_rgba8(112, 113, 101, 255)
                };
                let center = p + z * 0.5;
                match card.command {
                    Command::Summon => {
                        self.draw_unit_portrait(ctx, p + Vec2::new(18.0, 30.0) * s, 28.0 * s, UnitKind::Archer, true);
                    }
                    Command::Advance | Command::Retreat => {
                        let d = if matches!(card.command, Command::Advance) {
                            1.0
                        } else {
                            -1.0
                        };
                        ctx.draw_triangle(
                            center + Vec2::new(10.0 * d, 0.0) * s,
                            center + Vec2::new(-7.0 * d, -8.0) * s,
                            center + Vec2::new(-7.0 * d, 8.0) * s,
                            color,
                        );
                    }
                    Command::Fury => Self::draw_spell_icon(ctx, center, 0.65 * s, true),
                    _ => {}
                }
                if !card.enabled {
                    ctx.draw_rect(p, z, Color::from_rgba8(24, 24, 22, 115));
                }
                if self.command_locked(&card.command) {
                    self.draw_lock_icon(ctx, center, 27.0 * s);
                }
                if card.status.ends_with(" S") {
                    Self::draw_text_centered(ctx, &card.status, center.x, p.y + 28.0 * s, 0.8 * s, gold());
                }
                if hovered {
                    tooltip = Some((card.title, format!("{} - {}", card.status, card.detail)));
                }
            }
        } else if self.clan.panel_open {
            let (p, _) = Self::command_panel_rect(width, height);
            let title = CATEGORIES.iter().find(|(tab, _, _)| *tab == self.clan.tab).unwrap().1;
            Self::wood_panel(ctx, p, Vec2::new(180.0, 24.0) * s, s, false);
            Self::draw_text(ctx, title, p + Vec2::new(8.0, 8.0) * s, 1.1 * s, gold());
            Self::draw_text(ctx, "ESC", p + Vec2::new(151.0, 9.0) * s, 0.75 * s, text_color());
            for (i, card) in self.sidebar_cards().into_iter().enumerate() {
                let (p, z) = self.command_rect(i, width, height);
                let hovered = mouse.is_some_and(|m| Self::point_in_rect(m, p, z));
                Self::wood_panel(ctx, p, z, s, hovered);
                let color = if card.enabled {
                    gold()
                } else {
                    Color::from_rgba8(139, 145, 122, 255)
                };
                if let Some((kind, hero)) = card.portrait {
                    let foot = p + Vec2::new(27.0, 46.0) * s;
                    if !self.draw_unit_portrait(ctx, foot, 37.0 * s, kind, hero) {
                        Self::draw_goblin(ctx, foot, 0.75 * s, kind, Faction::Player, Vec2::ZERO, false, hero);
                    }
                    Self::draw_text(ctx, card.key, p + Vec2::new(5.0, 5.0) * s, 0.8 * s, color);
                } else if let (Some(icons), Command::Research(research)) = (self.ui_icons, &card.command)
                    && matches!(research, Research::Weapons | Research::Armor)
                {
                    let texture = if matches!(research, Research::Weapons) {
                        icons.sword
                    } else {
                        icons.shield
                    };
                    ctx.draw_sprite_tinted(
                        p + Vec2::new(8.0, 8.0) * s,
                        Vec2::splat(36.0 * s),
                        texture,
                        if card.enabled {
                            Color::WHITE
                        } else {
                            Color::from_rgba8(155, 155, 155, 255)
                        },
                    );
                } else if matches!(card.command, Command::Advance | Command::Retreat) {
                    let d = if matches!(card.command, Command::Advance) {
                        1.0
                    } else {
                        -1.0
                    };
                    let center = p + Vec2::new(26.0, 26.0) * s;
                    ctx.draw_triangle(
                        center + Vec2::new(9.0 * d, 0.0) * s,
                        center + Vec2::new(-7.0 * d, -10.0) * s,
                        center + Vec2::new(-7.0 * d, 10.0) * s,
                        color,
                    );
                } else {
                    Self::draw_text_centered(ctx, card.key, p.x + 26.0 * s, p.y + 16.0 * s, 2.0 * s, color);
                }
                if self.command_locked(&card.command) {
                    ctx.draw_rect(
                        p + Vec2::new(4.0, 4.0) * s,
                        Vec2::new(44.0, 44.0) * s,
                        Color::from_rgba8(24, 24, 22, 105),
                    );
                    self.draw_lock_icon(ctx, p + Vec2::new(26.0, 26.0) * s, 28.0 * s);
                }
                Self::draw_text(ctx, &card.title, p + Vec2::new(55.0, 13.0) * s, 0.98 * s, color);
                Self::draw_text(ctx, &card.status, p + Vec2::new(55.0, 32.0) * s, 0.82 * s, text_color());
                if hovered {
                    tooltip = Some((card.title, card.detail));
                }
            }
        }
        let (p, z) = Self::minimap_rect(width, height);
        Self::wood_panel(ctx, p, z, s, false);
        let map_x = |x: f32| p.x + 4.0 * s + x / WORLD_WIDTH * (z.x - 8.0 * s);
        Self::draw_line(
            ctx,
            Vec2::new(map_x(0.0), p.y + 20.0 * s),
            Vec2::new(map_x(WORLD_WIDTH), p.y + 20.0 * s),
            s,
            Color::from_rgba8(105, 122, 77, 255),
        );
        for unit in self.units.iter().filter(|unit| unit.alive()) {
            ctx.draw_rect(
                Vec2::new(map_x(unit.pos.x), p.y + 13.0 * s),
                Vec2::new(2.0, 7.0) * s,
                if unit.faction == Faction::Player {
                    Color::from_rgba8(164, 204, 95, 255)
                } else {
                    Color::from_rgba8(225, 116, 81, 255)
                },
            );
        }
        for x in [
            Self::castle_x(Faction::Player, WORLD_WIDTH),
            Self::castle_x(Faction::Enemy, WORLD_WIDTH),
        ] {
            ctx.draw_rect(Vec2::new(map_x(x), p.y + 10.0 * s), Vec2::new(3.0, 11.0) * s, gold());
        }
        let viewport = (width / s / WORLD_WIDTH * (z.x - 8.0 * s)).min(z.x - 8.0 * s);
        let view = Vec2::new(map_x(self.clan.camera), p.y + 4.0 * s);
        ctx.draw_rect(view, Vec2::new(viewport, 20.0 * s), Color::from_rgba8(231, 218, 158, 30));
        Self::draw_line(ctx, view, view + Vec2::new(viewport, 0.0), s, gold());
        if self.hero.alive() {
            Self::ellipse(
                ctx,
                Vec2::new(map_x(self.hero.pos.x), p.y + 15.0 * s),
                Vec2::splat(2.3 * s),
                gold(),
            );
        }
        if let Some(kind) = self.player_recruit_queue.front() {
            let p = Vec2::new(296.0 * s, height - 35.0 * s);
            Self::wood_panel(ctx, p, Vec2::new(300.0, 27.0) * s, s, false);
            Self::draw_text(
                ctx,
                &format!("TREINANDO {}  -  FILA {}", kind.name(), self.player_recruit_queue.len()),
                p + Vec2::new(8.0, 7.0) * s,
                0.95 * s,
                text_color(),
            );
            Self::draw_bar(
                ctx,
                p + Vec2::new(8.0, 20.0) * s,
                Vec2::new(284.0, 3.0) * s,
                1.0 - self.player_recruit_timer / PLAYER_RECRUIT_INTERVAL,
                gold(),
            );
        }
        if let Some((title, detail)) = tooltip {
            let mut lines = vec![String::new()];
            for word in detail.split_whitespace() {
                let line = lines.last_mut().unwrap();
                let candidate = if line.is_empty() {
                    word.to_owned()
                } else {
                    format!("{line} {word}")
                };
                if Self::text_width(&candidate, 0.9 * s) > 280.0 * s && !line.is_empty() {
                    lines.push(word.to_owned());
                } else {
                    *line = candidate;
                }
            }
            let box_height = (38.0 + lines.len() as f32 * 12.0) * s;
            let p = Vec2::new(52.0 * s, height - 64.0 * s - box_height);
            Self::wood_panel(ctx, p, Vec2::new(300.0 * s, box_height), s, false);
            Self::draw_text(ctx, &title, p + Vec2::new(10.0, 10.0) * s, 1.25 * s, gold());
            for (i, line) in lines.iter().enumerate() {
                Self::draw_text(
                    ctx,
                    line,
                    p + Vec2::new(10.0, 32.0 + i as f32 * 12.0) * s,
                    0.9 * s,
                    text_color(),
                );
            }
        } else if self.clan.notice_time > 0.0 || self.clan.selected_spell.is_some() {
            let text = if self.clan.selected_spell.is_some() {
                "CLIQUE NO CAMPO PARA CONJURAR - ESC CANCELA"
            } else {
                &self.clan.notice
            };
            let w = Self::text_width(text, s) + 24.0 * s;
            let p = Vec2::new(((width - w) * 0.5).max(248.0 * s), 46.0 * s);
            Self::wood_panel(ctx, p, Vec2::new(w, 25.0 * s), s, false);
            Self::draw_text(ctx, text, p + Vec2::new(12.0, 8.0) * s, s, gold());
        }
        if self.clan.paused {
            Self::wood_panel(
                ctx,
                Vec2::new(width * 0.5 - 160.0 * s, height * 0.38),
                Vec2::new(320.0, 60.0) * s,
                s,
                false,
            );
            Self::draw_text_centered(ctx, "BATALHA PAUSADA", width * 0.5, height * 0.38 + 12.0 * s, 2.0 * s, gold());
            Self::draw_text_centered(
                ctx,
                "ESPACO PARA CONTINUAR",
                width * 0.5,
                height * 0.38 + 40.0 * s,
                s,
                text_color(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn click_tab(game: &mut UnitesWar, index: usize) {
        let (p, z) = UnitesWar::sidebar_tab_rect(index, 960.0, 540.0);
        game.handle_war_click(p + z * 0.5, 960.0, 540.0);
    }

    fn click_card(game: &mut UnitesWar, index: usize) {
        let (p, z) = game.command_rect(index, 960.0, 540.0);
        game.handle_war_click(p + z * 0.5, 960.0, 540.0);
    }

    #[test]
    fn category_buttons_switch_and_collapse_without_spending_gold() {
        let mut game = UnitesWar::new();
        game.start_battle();
        assert!(!game.clan.panel_open);
        for (i, (tab, _, _)) in CATEGORIES.into_iter().enumerate() {
            click_tab(&mut game, i);
            assert!(game.clan.panel_open);
            assert_eq!(game.clan.tab, tab);
            assert_eq!(game.player_coins, STARTING_COINS);
        }
        click_tab(&mut game, 4);
        assert!(!game.clan.panel_open);
        assert!(game.player_recruit_queue.is_empty());
    }

    #[test]
    fn hidden_cards_cannot_recruit_and_visible_cards_can() {
        let mut game = UnitesWar::new();
        game.start_battle();
        click_card(&mut game, 0);
        assert!(game.player_recruit_queue.is_empty());
        click_tab(&mut game, 0);
        click_card(&mut game, 0);
        assert_eq!(game.player_recruit_queue.len(), 1);
        assert_eq!(game.player_coins, STARTING_COINS - UnitKind::Runner.stats().cost);
        click_tab(&mut game, 0);
        click_card(&mut game, 0);
        assert_eq!(game.player_recruit_queue.len(), 1);
    }

    #[test]
    fn selecting_magic_closes_the_panel_and_waits_for_a_field_click() {
        let mut game = UnitesWar::new();
        game.start_battle();
        click_tab(&mut game, 2);
        click_card(&mut game, 0);
        assert!(!game.clan.panel_open);
        assert_eq!(game.clan.selected_spell, Some(Spell::Lightning));
        assert_eq!(game.spell_cooldown, 0.0);
        game.handle_war_click(Vec2::new(200.0, 20.0), 960.0, 540.0);
        assert_eq!(game.spell_cooldown, 0.0);
        game.handle_war_click(Vec2::new(400.0, 320.0), 960.0, 540.0);
        assert_eq!(game.spell_cooldown, SPELL_COOLDOWN);
        assert!(game.clan.selected_spell.is_none());
    }

    #[test]
    fn paused_panels_allow_browsing_but_do_not_spend_gold() {
        let mut game = UnitesWar::new();
        game.start_battle();
        game.clan.paused = true;
        click_tab(&mut game, 0);
        assert!(game.clan.panel_open);
        click_card(&mut game, 0);
        assert!(game.player_recruit_queue.is_empty());
        assert_eq!(game.player_coins, STARTING_COINS);
        game.clan.paused = false;
        click_card(&mut game, 0);
        assert_eq!(game.player_recruit_queue.len(), 1);
    }

    #[test]
    fn hero_orders_and_tower_upgrade_work_in_their_own_panels() {
        let mut game = UnitesWar::new();
        game.start_battle();
        game.clan.experience[0] = 1000.0;
        game.player_coins = 1000.0;
        click_tab(&mut game, 3);
        click_card(&mut game, 0);
        assert!(game.hero.alive());
        click_card(&mut game, 2);
        assert_eq!(game.hero.move_direction, -1.0);
        click_card(&mut game, 1);
        assert_eq!(game.hero.move_direction, 1.0);
        click_tab(&mut game, 4);
        click_card(&mut game, 4);
        assert_eq!(game.player_castle.upgrades[TowerUpgrade::Health.index()], 1);
        assert_eq!(game.player_coins, 880.0);
    }

    #[test]
    fn hero_buttons_show_recall_countdown_and_only_allow_summon_when_ready() {
        let mut game = UnitesWar::new();
        game.start_battle();
        game.clan.experience[0] = HERO_UNLOCK_XP;
        click_tab(&mut game, 3);
        click_card(&mut game, 0);
        click_card(&mut game, 2);
        assert!(game.sidebar_cards()[1].key.is_empty());
        assert!(game.sidebar_cards()[2].key.is_empty());
        game.update_hero_movement(10.0, WORLD_WIDTH, WORLD_HEIGHT);
        assert_eq!(game.sidebar_cards()[0].status, "30 S");
        assert!(game.sidebar_cards().iter().all(|card| !card.enabled));
        click_card(&mut game, 0);
        assert!(game.hero.in_castle);
        game.update_clan(29.0);
        assert_eq!(game.sidebar_cards()[0].status, "1 S");
        click_card(&mut game, 0);
        assert!(!game.hero.alive());
        game.update_clan(1.0);
        assert_eq!(game.sidebar_cards()[0].status, "PRONTO");
        click_card(&mut game, 0);
        assert!(game.hero.alive());
        assert!(!game.hero.in_castle);
        assert_eq!(game.hero.health, HERO_MAX_HEALTH);
        assert_eq!(game.hero.move_direction, 1.0);
    }

    #[test]
    fn compact_hero_icons_have_matching_hit_regions_and_activate_fury() {
        let mut game = UnitesWar::new();
        game.start_army_test();
        click_tab(&mut game, 3);
        for (width, height) in [(960.0, 540.0), (1920.0, 1080.0), (1024.0, 768.0)] {
            for i in 0..4 {
                let (p, z) = game.command_rect(i, width, height);
                assert!(!game.is_battlefield_point(p + z * 0.5, width, height));
                assert!(p.y > 38.0 * UnitesWar::view_scale(width, height));
                let (nav, _) = UnitesWar::sidebar_tab_rect(0, width, height);
                assert!(p.y + z.y < nav.y);
            }
            let (old, z) = UnitesWar::button_rect(0, width, height);
            assert!(game.is_battlefield_point(old + z * 0.5, width, height));
        }
        click_card(&mut game, 3);
        assert_eq!(game.clan.fury_time, 8.0);
        assert!(!game.sidebar_cards()[3].enabled);
        assert_eq!(game.sidebar_cards()[3].status, "28 S");
        game.update_clan(1.0);
        click_card(&mut game, 3);
        assert_eq!(game.clan.fury_time, 7.0);
    }

    #[test]
    fn locks_follow_unlock_requirements_not_gold_or_cooldowns() {
        let mut game = UnitesWar::new();
        game.start_battle();
        game.player_coins = 0.0;
        assert!(!game.command_locked(&Command::Recruit(UnitKind::Guard)));
        assert!(!game.command_locked(&Command::Research(Research::Weapons)));
        for (command, xp) in [
            (Command::Recruit(UnitKind::Archer), 120.0),
            (Command::Recruit(UnitKind::Brute), 360.0),
            (Command::Summon, 250.0),
            (Command::Spell(Spell::Meteor), 500.0),
        ] {
            game.clan.experience[0] = xp - 1.0;
            assert!(game.command_locked(&command));
            game.clan.experience[0] = xp;
            assert!(!game.command_locked(&command));
        }
        assert!(game.command_locked(&Command::Fury));
        assert!(game.summon_hero(WORLD_WIDTH, WORLD_HEIGHT));
        assert!(!game.command_locked(&Command::Fury));
        game.cast_fury();
        assert!(!game.command_locked(&Command::Fury));
        assert!(game.clan.fury_cooldown > 0.0);
        game.clan.meteor_cooldown = 20.0;
        assert!(!game.command_locked(&Command::Spell(Spell::Meteor)));
    }

    #[test]
    fn locked_troop_card_stays_unpurchased_until_xp_unlock() {
        let mut game = UnitesWar::new();
        game.start_battle();
        game.player_coins = 1000.0;
        click_tab(&mut game, 0);
        click_card(&mut game, 2);
        assert!(game.player_recruit_queue.is_empty());
        assert_eq!(game.player_coins, 1000.0);
        game.clan.experience[0] = 120.0;
        click_card(&mut game, 2);
        assert_eq!(game.player_recruit_queue.front(), Some(&UnitKind::Archer));
        assert_eq!(game.player_coins, 935.0);
    }

    #[test]
    fn tower_panel_has_five_independent_upgrades_and_troop_panel_only_two() {
        let mut game = UnitesWar::new();
        game.start_battle();
        game.player_coins = 2000.0;
        click_tab(&mut game, 1);
        assert_eq!(game.sidebar_cards().len(), 2);
        click_tab(&mut game, 4);
        assert_eq!(game.sidebar_cards().len(), 5);
        for i in 0..5 {
            click_card(&mut game, i);
        }
        assert_eq!(game.player_castle.upgrades, [1; 5]);
        assert_eq!(game.clan.research, [0; 2]);
        assert_eq!(game.player_coins, 1400.0);
        for (width, height) in [(960.0, 540.0), (1280.0, 720.0), (1024.0, 768.0)] {
            for i in 0..5 {
                let (p, z) = game.command_rect(i, width, height);
                assert!(!game.is_battlefield_point(p + z * 0.5, width, height));
                let (nav, _) = UnitesWar::sidebar_tab_rect(4, width, height);
                assert!(p.y + z.y < nav.y);
            }
            let s = UnitesWar::view_scale(width, height);
            let (nav, z) = UnitesWar::sidebar_tab_rect(4, width, height);
            assert!(nav.x + z.x < 296.0 * s);
        }
    }

    #[test]
    fn sidebar_blocks_world_input_only_where_the_hud_is_visible() {
        for (width, height) in [(960.0, 540.0), (1280.0, 720.0), (1920.0, 1080.0), (1024.0, 768.0)] {
            let mut game = UnitesWar::new();
            game.start_battle();
            let (p, z) = UnitesWar::button_rect(0, width, height);
            assert!(game.is_battlefield_point(p + z * 0.5, width, height));
            game.toggle_sidebar(CommandTab::Army);
            assert!(!game.is_battlefield_point(p + z * 0.5, width, height));
            let (last, size) = UnitesWar::button_rect(3, width, height);
            assert!(last.x + size.x < width && last.y + size.y < height);
            let s = UnitesWar::view_scale(width, height);
            assert!(game.is_battlefield_point(Vec2::new(500.0 * s, height - 80.0 * s), width, height));
            assert!(!game.is_battlefield_point(Vec2::new(10.0, 10.0), width, height));
            let (map, size) = UnitesWar::minimap_rect(width, height);
            assert!(!game.is_battlefield_point(map + size * 0.5, width, height));
            game.handle_war_click(map + size * 0.5, width, height);
            assert!((game.clan.camera - (WORLD_WIDTH - width / s) * 0.5).abs() < 0.001);
        }
    }
}

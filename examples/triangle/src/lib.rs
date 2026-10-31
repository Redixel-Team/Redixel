use redixel::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Action {
    Exit,
}

struct Triangle;

impl Game for Triangle {
    type Action = Action;

    fn on_start(&mut self, ctx: &mut dyn GameContext<Self::Action>) {
        ctx.input_mut().bind(Action::Exit, KeyCode::Escape.into());
        log::info!("triangle::on_start");
    }

    fn on_update(&mut self, ctx: &mut dyn GameContext<Self::Action>) {
        if ctx.input().just_pressed(Action::Exit) {
            ctx.exit();
        }
    }

    fn on_render(&mut self, ctx: &mut dyn GameContext<Self::Action>) {
        ctx.clear_color(Color::rgb(0.1, 0.1, 0.12));

        let h: f32 = ctx.surface_height() as f32;
        let w: f32 = ctx.surface_width() as f32;

        let center: Vec2 = Vec2::new(w / 2.0, h / 2.0);
        let min_dimension: f32 = w.min(h);

        let scale_factor: f32 = 0.20;
        let half_size: f32 = (min_dimension * scale_factor) / 2.0;

        let p1: Vec2 = center - Vec2::new(0.0, half_size);
        let p2: Vec2 = center + Vec2::new(-half_size, half_size);
        let p3: Vec2 = center + Vec2::splat(half_size);

        ctx.draw_triangle(p1, p2, p3, Color::rgb(1.0, 0.5, 0.0));
    }
}

redixel::entry_point!(Triangle);

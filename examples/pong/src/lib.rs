use redixel::prelude::*;

const PADDLE_W: f32 = 15.0;
const PADDLE_H: f32 = 90.0;
const PADDLE_SPEED: f32 = 400.0;
const PADDLE_MARGIN: f32 = 20.0;
const BALL_SIZE: f32 = 14.0;
const BALL_SPEED: f32 = 450.0;
const SCROLL_IMPULSE: f32 = 250.0;
const SCROLL_FRICTION: f32 = 25.0;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Action {
    P1Up,
    P1Down,
    Exit,
    ToggleInput,
}

struct Paddle {
    pos:   Vec2,
    vel_y: f32,
}

impl Paddle {
    fn new() -> Self {
        Self {
            pos:   Vec2::ZERO,
            vel_y: 0.0,
        }
    }

    fn clamp(&mut self, h: f32) {
        if self.pos.y <= 0.0 || self.pos.y >= h - PADDLE_H {
            self.vel_y = 0.0;
        }

        self.pos.y = self.pos.y.clamp(0.0, h - PADDLE_H);
    }
}

struct Ball {
    pos: Vec2,
    vel: Vec2,
}

impl Ball {
    fn new() -> Self {
        Self {
            pos: Vec2::ZERO,
            vel: Vec2::new(BALL_SPEED, BALL_SPEED * 0.65),
        }
    }

    fn reset(&mut self, dir: f32, w: f32, h: f32) {
        self.pos = Vec2::new(w / 2.0 - BALL_SIZE / 2.0, h / 2.0 - BALL_SIZE / 2.0);
        self.vel = Vec2::new(BALL_SPEED * dir, BALL_SPEED * 0.65);
    }
}

struct Pong {
    left:             Paddle,
    right:            Paddle,
    ball:             Ball,
    score:            (u32, u32),
    initialized:      bool,
    use_mouse_scroll: bool,
}

impl Pong {
    fn new() -> Self {
        Self {
            left:             Paddle::new(),
            right:            Paddle::new(),
            ball:             Ball::new(),
            score:            (0, 0),
            initialized:      false,
            use_mouse_scroll: false,
        }
    }

    fn init_layout(&mut self, w: f32, h: f32) {
        self.left.pos = Vec2::new(PADDLE_MARGIN, h / 2.0 - PADDLE_H / 2.0);
        self.left.vel_y = 0.0;
        self.right.pos = Vec2::new(w - PADDLE_MARGIN - PADDLE_W, h / 2.0 - PADDLE_H / 2.0);
        self.ball.reset(1.0, w, h);
        self.initialized = true;
    }

    fn update_ai(&mut self, dt: f32, h: f32) {
        let target: f32 = self.ball.pos.y + BALL_SIZE / 2.0 - PADDLE_H / 2.0;
        let step: f32 = PADDLE_SPEED * 0.85 * dt;
        let diff: f32 = target - self.right.pos.y;
        self.right.pos.y += diff.clamp(-step, step);
        self.right.clamp(h);
    }

    fn update_ball(&mut self, w: f32, h: f32) {
        if self.ball.pos.y <= 0.0 {
            self.ball.pos.y = 0.0;
            self.ball.vel.y = self.ball.vel.y.abs();
        }

        if self.ball.pos.y + BALL_SIZE >= h {
            self.ball.pos.y = h - BALL_SIZE;
            self.ball.vel.y = -self.ball.vel.y.abs();
        }

        if self.ball.vel.x < 0.0
            && self.ball.pos.x <= self.left.pos.x + PADDLE_W
            && self.ball.pos.x >= self.left.pos.x
            && self.ball.pos.y + BALL_SIZE >= self.left.pos.y
            && self.ball.pos.y <= self.left.pos.y + PADDLE_H
        {
            self.ball.pos.x = self.left.pos.x + PADDLE_W;
            self.ball.vel.x = self.ball.vel.x.abs();
        }

        if self.ball.vel.x > 0.0
            && self.ball.pos.x + BALL_SIZE >= self.right.pos.x
            && self.ball.pos.x + BALL_SIZE <= self.right.pos.x + PADDLE_W
            && self.ball.pos.y + BALL_SIZE >= self.right.pos.y
            && self.ball.pos.y <= self.right.pos.y + PADDLE_H
        {
            self.ball.pos.x = self.right.pos.x - BALL_SIZE;
            self.ball.vel.x = -self.ball.vel.x.abs();
        }

        if self.ball.pos.x + BALL_SIZE < 0.0 {
            self.score.1 += 1;
            log::info!("Score - P1: {}  AI: {}", self.score.0, self.score.1);
            self.ball.reset(1.0, w, h);
        }

        if self.ball.pos.x > w {
            self.score.0 += 1;
            log::info!("Score - P1: {}  AI: {}", self.score.0, self.score.1);
            self.ball.reset(-1.0, w, h);
        }
    }
}

impl Game for Pong {
    type Action = Action;

    fn on_start(&mut self, ctx: &mut dyn GameContext<Self::Action>) {
        ctx.input_mut().bind(Action::P1Up, KeyCode::KeyW.into());
        ctx.input_mut().bind(Action::P1Up, KeyCode::ArrowUp.into());
        ctx.input_mut().bind(Action::P1Down, KeyCode::KeyS.into());
        ctx.input_mut().bind(Action::P1Down, KeyCode::ArrowDown.into());
        ctx.input_mut().bind(Action::ToggleInput, KeyCode::KeyM.into());
        ctx.input_mut().bind(Action::Exit, KeyCode::Escape.into());
        log::info!("W/S or Arrows to move. 'M' to toggle Mouse Scroll. ESC to quit.");
    }

    fn on_update(&mut self, ctx: &mut dyn GameContext<Self::Action>) {
        let w: f32 = ctx.surface_width() as f32;
        let h: f32 = ctx.surface_height() as f32;
        let dt: f32 = ctx.delta_time() as f32;

        if !self.initialized {
            self.init_layout(w, h);
        }

        self.right.pos.x = w - PADDLE_MARGIN - PADDLE_W;

        if ctx.input().just_pressed(Action::ToggleInput) {
            self.use_mouse_scroll = !self.use_mouse_scroll;
            log::info!("CONTROL_MODE: {}", if self.use_mouse_scroll { "SCROLL" } else { "KEYBOARD" });
        }

        if self.use_mouse_scroll {
            let scroll_y: f32 = ctx.input().scroll_delta().y;

            if scroll_y != 0.0 {
                self.left.vel_y -= scroll_y * SCROLL_IMPULSE;
            }

            self.left.pos.y += self.left.vel_y * dt;
            self.left.vel_y -= self.left.vel_y * SCROLL_FRICTION * dt;

            if self.left.vel_y.abs() < 1.0 {
                self.left.vel_y = 0.0;
            }
        } else {
            self.left.vel_y = 0.0;

            if ctx.input().held(Action::P1Up) {
                self.left.pos.y -= PADDLE_SPEED * dt;
            }

            if ctx.input().held(Action::P1Down) {
                self.left.pos.y += PADDLE_SPEED * dt;
            }
        }

        if ctx.input().held(Action::Exit) {
            ctx.exit();
        }

        self.left.clamp(h);
        self.update_ai(dt, h);

        self.ball.pos += self.ball.vel * dt;
        self.update_ball(w, h);
    }

    fn on_render(&mut self, ctx: &mut dyn GameContext<Self::Action>) {
        let w: f32 = ctx.surface_width() as f32;
        let h: f32 = ctx.surface_height() as f32;
        ctx.clear_color(Color::BLACK);

        let mut y: f32 = 0.0_f32;
        while y < h {
            ctx.draw_rect(
                Vec2::new(w / 2.0 - 2.0, y),
                Vec2::new(4.0, 10.0),
                Color::from_rgba8(143, 143, 143, 255),
            );

            y += 18.0;
        }

        ctx.draw_rect(self.left.pos, Vec2::new(PADDLE_W, PADDLE_H), Color::WHITE);

        ctx.draw_rect(
            self.right.pos,
            Vec2::new(PADDLE_W, PADDLE_H),
            Color::from_rgba8(239, 168, 168, 255),
        );

        ctx.draw_rect(self.ball.pos, Vec2::new(BALL_SIZE, BALL_SIZE), Color::YELLOW);
    }
}

redixel::entry_point!(Pong::new());

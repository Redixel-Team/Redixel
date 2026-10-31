/// Implementation detail of [`entry_point!`](crate::entry_point). Not public API.
#[doc(hidden)]
pub mod __private {
    #[cfg(target_os = "android")]
    use android_logger::{Config, init_once};
    #[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
    use env_logger::{Builder, Env};
    pub use log;
    #[cfg(target_arch = "wasm32")]
    use log::Level;
    #[cfg(target_os = "android")]
    use log::LevelFilter;
    #[cfg(target_arch = "wasm32")]
    use redixel_core::RedixelError;
    /// `#[wasm_bindgen]` writes its runtime paths as `wasm_bindgen::…`, so the
    /// crate must be in scope under that exact name where the macro expands; the
    /// attribute takes a second name so both can be imported there.
    #[cfg(target_arch = "wasm32")]
    pub use wasm_bindgen;
    #[cfg(target_arch = "wasm32")]
    pub use wasm_bindgen::prelude::wasm_bindgen as wasm_bindgen_start;
    #[cfg(target_os = "android")]
    pub use winit::platform::android::activity::AndroidApp;
    #[cfg(target_os = "android")]
    use winit::platform::android::activity::WindowManagerFlags;

    /// `try_init`, since a logger already being installed is the host's
    /// preference rather than a startup failure.
    #[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
    pub fn init_desktop_logger() {
        let _ = Builder::from_env(Env::default().default_filter_or("info")).try_init();
    }

    #[cfg(target_os = "android")]
    pub fn init_android_logger(tag: &str) {
        init_once(Config::default().with_max_level(LevelFilter::Info).with_tag(tag));
    }

    /// Keeps the display awake: a player can go a long while without touching
    /// the screen while still very much playing.
    #[cfg(target_os = "android")]
    pub fn prepare_android_window(app: &AndroidApp) {
        app.set_window_flags(WindowManagerFlags::KEEP_SCREEN_ON, WindowManagerFlags::empty());
    }

    #[cfg(target_arch = "wasm32")]
    pub fn init_web_logger() -> Result<(), RedixelError> {
        console_error_panic_hook::set_once();
        console_log::init_with_level(Level::Info)?;

        Ok(())
    }
}

/// Generates every platform entry point for a [`Game`](crate::prelude::Game),
/// plus the `fn main()` the native binary calls.
///
/// One invocation at the root of a game's `lib.rs` replaces the four
/// `#[cfg]`-gated functions the platform loaders expect — `desktop_main`, the
/// unmangled `android_main` the native-activity glue resolves by symbol, the
/// `extern "C" ios_main` an Xcode target is pointed at, and the
/// `#[wasm_bindgen(start)]` function the browser runs on module instantiation —
/// along with the logger each installs first. The binary target shrinks to
/// `fn main() { my_game::main(); }`.
///
/// # Short form
///
/// ```rust,ignore
/// redixel::entry_point!(Pong::new());
/// ```
///
/// # Named form
///
/// `game:` comes first; the rest are optional and may appear in any order.
///
/// ```rust,ignore
/// redixel::entry_point! {
///     game: preset::client(),
///     config: preset::config(),
///     desktop: native::run,
///     log_tag: "SHOOTER_MP",
/// }
/// ```
///
/// - `config:` — the [`RuntimeConfig`](crate::prelude::RuntimeConfig) to run
///   with, defaulting to [`build_config()`](crate::build_config). It and `game:`
///   are expanded *inside* each entry point rather than evaluated once, so both
///   may name items that exist only on the platform compiling them.
/// - `desktop:` — path to a `fn() -> Result<(), RedixelError>` replacing the
///   desktop run step, called with the logger already installed; `game:` and
///   `config:` then become that function's business. This is for anything
///   argv-dependent, such as choosing between hosting a server and joining one.
/// - `log_tag:` — the Android logcat tag, defaulting to `"REDIXEL_ENGINE"`.
///
/// # Generated items
///
/// `desktop_main` (desktop), `ios_main` (iOS), `android_main` (Android),
/// `wasm_main` (browser), and `main` on every target. `main` exits with status
/// `1` when `desktop_main` fails and does nothing elsewhere, since Android, iOS
/// and the browser enter through their exported symbol instead.
///
/// Invoke this **once per crate** — a second invocation collides on `main`,
/// `desktop_main` and the two unmangled symbols.
#[macro_export]
macro_rules! entry_point {
    (@opts { $game:expr } { $config:expr } { $($desktop:path)? } { $tag:expr } config: $new:expr $(, $($rest:tt)*)?) => {
        $crate::entry_point!(@opts { $game } { $new } { $($desktop)? } { $tag } $($($rest)*)?);
    };

    (@opts { $game:expr } { $config:expr } { $($desktop:path)? } { $tag:expr } desktop: $new:path $(, $($rest:tt)*)?) => {
        $crate::entry_point!(@opts { $game } { $config } { $new } { $tag } $($($rest)*)?);
    };

    (@opts { $game:expr } { $config:expr } { $($desktop:path)? } { $tag:expr } log_tag: $new:expr $(, $($rest:tt)*)?) => {
        $crate::entry_point!(@opts { $game } { $config } { $($desktop)? } { $new } $($($rest)*)?);
    };

    (@opts { $game:expr } { $config:expr } { $($desktop:path)? } { $tag:expr } $(,)?) => {
        $crate::entry_point!(@desktop { $game } { $config } { $($desktop)? });
        $crate::entry_point!(@entries { $game } { $config } { $tag });
    };

    (@opts { $($game:tt)* } { $($config:tt)* } { $($desktop:tt)* } { $($tag:tt)* } $($unexpected:tt)+) => {
        ::core::compile_error!(::core::concat!(
            "unexpected `entry_point!` parameter: `",
            ::core::stringify!($($unexpected)+),
            "`; expected `config: <expr>`, `desktop: <path>` or `log_tag: <expr>`",
        ));
    };

    (@desktop { $game:expr } { $config:expr } { }) => {
        /// Desktop entry point. Generated by `redixel::entry_point!`.
        #[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
        pub fn desktop_main() -> ::core::result::Result<(), $crate::prelude::RedixelError> {
            $crate::__private::init_desktop_logger();

            $crate::run_desktop_with($game, $config)
        }
    };

    (@desktop { $game:expr } { $config:expr } { $desktop:path }) => {
        /// Desktop entry point. Generated by `redixel::entry_point!`.
        #[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
        pub fn desktop_main() -> ::core::result::Result<(), $crate::prelude::RedixelError> {
            $crate::__private::init_desktop_logger();

            $desktop()
        }
    };

    (@entries { $game:expr } { $config:expr } { $tag:expr }) => {
        /// iOS entry point, exported unmangled for an Xcode target to call in
        /// place of `main`. Generated by `redixel::entry_point!`.
        #[cfg(target_os = "ios")]
        #[unsafe(no_mangle)]
        pub extern "C" fn ios_main() {
            if let ::core::result::Result::Err(e) = $crate::run_ios_with($game, $config) {
                $crate::__private::log::error!("Engine error: {e:?}");
            }
        }

        /// Android entry point, exported unmangled for the native-activity glue
        /// to resolve by symbol. Generated by `redixel::entry_point!`.
        #[cfg(target_os = "android")]
        #[unsafe(no_mangle)]
        pub fn android_main(app: $crate::__private::AndroidApp) {
            $crate::__private::init_android_logger($tag);
            $crate::__private::prepare_android_window(&app);

            if let ::core::result::Result::Err(e) = $crate::run_android_with($game, app, $config) {
                $crate::__private::log::error!("Engine error: {e:?}");
            }
        }

        /// Browser entry point. It sits in its own module so that the names
        /// `#[wasm_bindgen]` expands against stay out of the game's namespace.
        /// Generated by `redixel::entry_point!`.
        #[cfg(target_arch = "wasm32")]
        mod __redixel_wasm_entry {
            #[allow(unused_imports)]
            use super::*;

            use $crate::__private::{wasm_bindgen, wasm_bindgen_start};

            #[wasm_bindgen_start(start)]
            pub fn wasm_main() -> ::core::result::Result<(), $crate::prelude::RedixelError> {
                $crate::__private::init_web_logger()?;

                $crate::run_wasm_with($game, $config)
            }
        }

        /// Process entry point — call it from the binary target's `main.rs`.
        /// Generated by `redixel::entry_point!`.
        pub fn main() {
            #[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
            if let ::core::result::Result::Err(e) = desktop_main() {
                ::std::eprintln!("Engine error: {e:?}");
                ::std::process::exit(1);
            }
        }
    };

    (game: $game:expr $(, $($opts:tt)*)?) => {
        $crate::entry_point!(@opts { $game } { $crate::build_config() } { } { "REDIXEL_ENGINE" } $($($opts)*)?);
    };

    ($game:expr $(,)?) => {
        $crate::entry_point!(@opts { $game } { $crate::build_config() } { } { "REDIXEL_ENGINE" });
    };
}

#[cfg(test)]
#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use redixel_core::{Game, GameContext, RedixelError};

    struct SmokeGame;

    impl Game for SmokeGame {
        type Action = ();

        fn on_start(&mut self, _ctx: &mut dyn GameContext<Self::Action>) {}

        fn on_update(&mut self, _ctx: &mut dyn GameContext<Self::Action>) {}

        fn on_render(&mut self, _ctx: &mut dyn GameContext<Self::Action>) {}
    }

    static OVERRIDE_RAN: AtomicBool = AtomicBool::new(false);

    fn run_desktop_override() -> Result<(), RedixelError> {
        OVERRIDE_RAN.store(true, Ordering::SeqCst);

        Ok(())
    }

    mod short_form {
        use super::SmokeGame;

        crate::entry_point!(SmokeGame);
    }

    mod named_form {
        use super::SmokeGame;

        crate::entry_point! {
            game: SmokeGame,
            log_tag: "SMOKE_TEST",
            config: crate::build_config(),
        }
    }

    mod overridden_desktop {
        crate::entry_point! {
            game: super::SmokeGame,
            desktop: super::run_desktop_override,
        }
    }

    #[test]
    fn every_invocation_form_generates_the_same_entry_points() {
        let _: fn() -> Result<(), RedixelError> = short_form::desktop_main;
        let _: fn() -> Result<(), RedixelError> = named_form::desktop_main;
        let _: fn() -> Result<(), RedixelError> = overridden_desktop::desktop_main;

        let _: fn() = short_form::main;
        let _: fn() = named_form::main;
    }

    #[test]
    fn generated_main_dispatches_to_the_desktop_override() {
        overridden_desktop::main();

        assert!(
            OVERRIDE_RAN.load(Ordering::SeqCst),
            "`main()` must call the `desktop:` override"
        );
    }
}

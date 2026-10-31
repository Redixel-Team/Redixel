# Running on iOS

Redixel supports iOS through the same `Game`/`GameContext` API as every other platform, exposed via an `ios_main` entry point — the same idea as Android's JNI-loaded `android_main`, just embedded into an Xcode project instead of loaded by the OS directly.

> **Requires a Mac with Xcode.** Building for iOS needs Apple's proprietary SDK and frameworks (Foundation, UIKit, Metal), which only ship with Xcode — there is no cross-compilation path from Linux or Windows.

1. **Add the target:**

   ```sh
   rustup target add aarch64-apple-ios
   ```

2. **Build the library:**

   ```sh
   cargo build --release --manifest-path examples/shooter/Cargo.toml --target aarch64-apple-ios --lib
   ```

3. **Embed it in an Xcode project:** link the static library (`libshooter.a`, in `examples/shooter/target/aarch64-apple-ios/release/`) into an Xcode app target with no competing `main.swift`/`AppDelegate`, set `ios_main` as that target's entry point, then build/run from Xcode as usual.

> **Note:** Redixel doesn't generate the `.xcodeproj`/`.ipa` itself — that project lives in Xcode.

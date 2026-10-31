# Running on Android

Redixel provides native support for Android, leveraging JNI to maintain high performance and hardware access.

1. **Install Android prerequisites:**
   Ensure you have the [Android SDK](https://developer.android.com/studio) and `cargo-apk` installed:

   ```sh
   cargo install cargo-apk
   ```

<details>
<summary><b>Show SDK & Environment Setup Details</b></summary>
   
To successfully build the APK, `cargo-apk` requires specific SDK packages and environment variables to be explicitly set.
   
**1. Android Studio SDK Manager**
Open Android Studio, navigate to **Tools > SDK Manager**, and install:

- **SDK Platforms:** Android API 33 or 34 (Android 13 or 14).
- **SDK Tools:**
  - Android SDK Build-Tools.
  - NDK (Side by side) - e.g., version `xx.x.xxxxxxx`.
  - Android SDK Command-line Tools (latest).

**2. Environment Variables & PATH**
You must set the SDK paths and ensure `adb` and `keytool` (Java) are accessible in your terminal.

_Linux/macOS_ (Add to `~/.bashrc` or `~/.zshrc`):

```sh
export ANDROID_HOME=$HOME/Android/Sdk
export NDK_HOME=$ANDROID_HOME/ndk/xx.x.xxxxxxx # Match your downloaded version
export PATH=$PATH:$ANDROID_HOME/platform-tools
```

> **Note:** Ensure you have a JDK installed (e.g., sudo apt install default-jdk).

_Windows_ (Add to System Environment Variables):

- `ANDROID_HOME`: `C:\Users\YOUR_USER\AppData\Local\Android\Sdk`
- `NDK_HOME`: `%ANDROID_HOME%\ndk\xx.x.xxxxxxx`
- Edit your `Path` variable and append these two directories:
- `%ANDROID_HOME%\platform-tools`
- `C:\Program Files\Android\Android Studio\jbr\bin` (Required for `keytool`)

**3. Accept SDK Licenses**
Make sure you have accepted all licenses (you can do this in Android Studio or by running `sdkmanager --licenses` in your terminal).

</details>

2. **Add the target:**

   ```sh
   rustup target add aarch64-linux-android
   ```

3. **Run on your device:**
   Before executing the command, ensure your phone has "Developer Options" enabled, turn on "USB Debugging", and accept the RSA key fingerprint prompt on your screen when connected via USB.

   ```sh
   cargo apk run --manifest-path examples/shooter/Cargo.toml --lib
   ```

> **Note:** Logs can be inspected in real-time using `adb logcat -s REDIXEL_ENGINE`.

> **Note:** the `Shooter` example plays audio, which needs Android 8.0 (API level 26) or newer on the device.

## Manual Installation

If you prefer to install the compiled APK manually without using USB Debugging, you can build it and find the output in the `target` directory:

```sh
cargo apk build --manifest-path examples/shooter/Cargo.toml --lib
```

_The generated `.apk` will be located at `examples/shooter/target/debug/apk/shooter.apk`. You can transfer this file to your device and install it directly._

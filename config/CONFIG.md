# Redixel Configuration Guide

This document details the configuration options available for the **Redixel** engine. The configuration is stored in a JSON file (typically `config.json` or `settings.json`) and is loaded at runtime.

## Structure Overview

The configuration is divided into four main sections:

1.  **[app](#app)**: General application metadata.
2.  **[window](#window)**: Display and windowing settings.
3.  **[renderer](#renderer)**: Graphics API and presentation logic.
4.  **[audio](#audio)**: Master and per-channel volume.

---

### `app`

General application settings.

| Field  | Type   | Description                                                                                            |
| :----- | :----- | :----------------------------------------------------------------------------------------------------- |
| `name` | String | The name of the application. This typically appears in the operating system's taskbar or process list. |

**Example:**

```json
"app": {
    "name": "Redixel"
}
```

---

### `window`

Settings controlling the application window dimensions and behavior.

| Field        | Type    | Description                                                    |
| :----------- | :------ | :------------------------------------------------------------- |
| `width`      | Integer | The initial width of the window in pixels.                     |
| `height`     | Integer | The initial height of the window in pixels.                    |
| `fullscreen` | Boolean | `true` to start in fullscreen mode, `false` for windowed mode. |
| `target_fps` | Float   | The target frames-per-second cap for the update loop.          |

**Example:**

```json
"window": {
    "width": 1280,
    "height": 720,
    "fullscreen": false,
    "target_fps": 60.0
}
```

---

### `renderer`

Technical settings for the graphics backend.

**Note:** The values in this section are **integer mappings** that correspond directly to the underlying graphics library's enums (e.g., `wgpu::Backends` and `wgpu::PresentMode`).

#### `backend`

Determines which Graphics API (driver) the engine uses to render.

| Value | Backend API    | Description                                                                |
| :---- | :------------- | :------------------------------------------------------------------------- |
| **0** | **Auto / All** | Automatically selects the best supported backend for the system. (Default) |
| **1** | **Vulkan**     | Forces the Vulkan backend.                                                 |
| **2** | **OpenGL**     | Forces the OpenGL backend.                                                 |
| **3** | **Metal**      | Forces Apple Metal (macOS/iOS only).                                       |
| **4** | **DirectX 12** | Forces DirectX 12 (Windows only).                                          |
| **5** | **WebGPU**     | Forces the WebGPU backend (Browser/Wasm).                                  |
| **6** | **Primary**    | Selects the primary backend for the platform.                              |
| **7** | **Secondary**  | Selects the secondary/fallback backend.                                    |

#### `present_mode`

Determines how the engine synchronizes with the display (VSync behavior).

| Value | Mode            | Description                                                                                                                         |
| :---- | :-------------- | :---------------------------------------------------------------------------------------------------------------------------------- |
| **0** | **AutoVsync**   | Use VSync if supported, otherwise fallback. Best for tearing-free mobile/low-power usage.                                           |
| **1** | **AutoNoVsync** | Try to disable VSync for lowest latency.                                                                                            |
| **2** | **Fifo**        | **(VSync)** hard cap. The presentation queue is First-In-First-Out. Tearing is impossible.                                          |
| **3** | **FifoRelaxed** | Similar to Fifo, but if the app is late, it presents immediately (might tear).                                                      |
| **4** | **Immediate**   | **(No VSync)**. Images are presented immediately. Lowest latency, but screen tearing will occur.                                    |
| **5** | **Mailbox**     | **(Fast VSync)**. The system waits for VSync but constantly replaces the queued image with the newest one. Low latency, no tearing. |

**Example:**

```json
"renderer": {
    "present_mode": 0,
    "backend": 0
}
```

---

### `audio`

Master and per-channel volume, each from `0.0` (silent) to `1.0` (unattenuated); values outside that range are clamped.

Unlike the other sections, the engine also writes this one back: a volume the game changed with `GameContext::set_master_volume` or `set_channel_volume` is saved to `config.json` when the app exits cleanly, leaving every other key and its order untouched. Nothing is written if the file failed to load, and the web build has no file to write to.

| Field           | Type  | Description                                          |
| :-------------- | :---- | :--------------------------------------------------- |
| `master_volume` | Float | Overall volume, multiplied into both channels below. |
| `sfx_volume`    | Float | Volume of sound effects played with `play_sound`.    |
| `music_volume`  | Float | Volume of music played with `play_music`.            |

**Example:**

```json
"audio": {
    "master_volume": 1.0,
    "sfx_volume": 1.0,
    "music_volume": 1.0
}
```

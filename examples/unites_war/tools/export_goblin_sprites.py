"""Render the Blender Run and Attack actions for Unites War's side-view renderer.

Run with Blender in background mode; see ../README.md for the command.
The source .blend is only read. Cameras and lighting are changed in memory.
"""

import argparse
import hashlib
import json
import sys
from pathlib import Path

import bpy
from bpy_extras.object_utils import world_to_camera_view
from mathutils import Vector


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--samples", type=int, default=24)
    args = parser.parse_args(sys.argv[sys.argv.index("--") + 1 :])
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)

    scene = bpy.data.scenes["Goblin Saqueador - Corrida"]
    bpy.context.window.scene = scene
    rig = bpy.data.objects["Rig Saqueador"]
    action = bpy.data.actions["Run"]
    attack = bpy.data.actions["Attack"]

    def activate(clip):
        rig.animation_data.action = clip
        rig.animation_data.action_slot = clip.slots[0]

    activate(action)
    first, last = map(int, action.frame_range)
    # The last frame duplicates the first; never hold it twice in the loop.
    frames = list(range(first, last))
    fps = scene.render.fps / scene.render.fps_base
    duration = len(frames) / fps
    assert len(frames) == 24 and abs(duration - 0.8) < 0.0001
    attack_first, attack_last = map(int, attack.frame_range)
    # Include the recovery endpoint. Attack clamps instead of looping.
    attack_frames = list(range(attack_first, attack_last + 1))
    attack_duration = (attack_last - attack_first) / fps
    assert len(attack_frames) >= 2 and attack_duration > 0
    hit_frame = next(marker.frame for marker in attack.pose_markers if marker.name == "Corte")
    assert attack_first <= hit_frame <= attack_last
    hit_fraction = (hit_frame - attack_first) / (attack_last - attack_first)

    for obj in bpy.data.collections["SAQUEADOR - Estudio"].objects:
        if obj.type == "MESH":
            obj.hide_render = True
    camera = scene.camera
    camera.location = (-8, -4, 1.60)
    camera.rotation_euler = (
        Vector((0, 0, 1.60)) - camera.location
    ).to_track_quat("-Z", "Y").to_euler()
    camera.data.type = "ORTHO"
    camera.data.ortho_scale = 3.50
    scene.render.resolution_x = 256
    scene.render.resolution_y = 256
    scene.render.resolution_percentage = 100
    scene.render.film_transparent = True
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
    scene.render.image_settings.color_depth = "8"
    scene.render.image_settings.compression = 70
    scene.render.engine = "CYCLES"
    scene.cycles.samples = args.samples
    scene.cycles.use_denoising = True
    scene.view_settings.exposure = 0.2
    scene.world.node_tree.nodes["Background"].inputs["Color"].default_value = (
        0.32, 0.38, 0.27, 1
    )
    scene.world.node_tree.nodes["Background"].inputs["Strength"].default_value = 0.5
    lighting = [
        ("Saqueador - Luz principal", (-5, -6, 6), 750, (1, 0.95, 0.8)),
        ("Saqueador - Preenchimento", (5, -1, 4), 550, (0.75, 0.85, 1)),
        ("Saqueador - Recorte", (2, 5, 5), 750, (0.9, 1, 0.72)),
    ]
    for name, position, energy, color in lighting:
        obj = bpy.data.objects[name]
        obj.location = position
        obj.rotation_euler = (
            Vector((0, 0, 1.5)) - obj.location
        ).to_track_quat("-Z", "Y").to_euler()
        obj.data.energy = energy
        obj.data.color = color
        obj.data.size = 4

    # Use OptiX if available, otherwise keep a portable CPU fallback.
    scene.cycles.device = "CPU"
    prefs = bpy.context.preferences.addons["cycles"].preferences
    try:
        prefs.compute_device_type = "OPTIX"
        prefs.refresh_devices()
        devices = [d for d in prefs.devices if d.type == "OPTIX"]
        if devices:
            for device in prefs.devices:
                device.use = device in devices
            scene.cycles.device = "GPU"
    except (TypeError, RuntimeError):
        pass

    bpy.context.view_layer.update()
    anchor = world_to_camera_view(scene, camera, Vector((0, 0, 0)))
    # Measure the planted foot's screen travel to synchronize playback to the
    # troop's movement speed without changing combat or collision dimensions.
    foot_x = []
    rig.data.pose_position = "POSE"
    for frame in (first, first + 8):
        scene.frame_set(frame)
        bpy.context.view_layer.update()
        foot = rig.matrix_world @ rig.pose.bones["Foot.R"].head
        foot_x.append(world_to_camera_view(scene, camera, foot).x * 256)
    ground_speed = abs(foot_x[1] - foot_x[0]) / (8 / fps)

    rig.data.pose_position = "REST"
    bpy.context.view_layer.update()
    scene.render.filepath = str(output / "idle.png")
    bpy.ops.render.render(write_still=True)
    rig.data.pose_position = "POSE"
    for index, frame in enumerate(frames):
        scene.frame_set(frame)
        scene.render.filepath = str(output / f"run_{index:02}.png")
        bpy.ops.render.render(write_still=True)
        print(f"RUN_FRAME {index + 1}/{len(frames)}", flush=True)

    activate(attack)
    for index, frame in enumerate(attack_frames):
        scene.frame_set(frame)
        scene.render.filepath = str(output / f"attack_{index:02}.png")
        bpy.ops.render.render(write_still=True)
        print(f"ATTACK_FRAME {index + 1}/{len(attack_frames)}", flush=True)

    metadata = {
        "source": "source.blend",
        "source_sha256": hashlib.sha256(Path(bpy.data.filepath).read_bytes()).hexdigest(),
        "blender_version": bpy.app.version_string,
        "action": action.name,
        "frames": len(frames),
        "fps": fps,
        "duration_seconds": duration,
        "size": [256, 256],
        "anchor_uv": [anchor.x, 1 - anchor.y],
        "facing": "right",
        "ground_speed_pixels_per_second": ground_speed,
        "idle": "rest pose",
        "attack": {
            "action": attack.name,
            "frames": len(attack_frames),
            "duration_seconds": attack_duration,
            "hit_frame": hit_frame,
            "hit_fraction": hit_fraction,
            "loop": False,
        },
    }
    (output / "manifest.json").write_text(json.dumps(metadata, indent=2) + "\n")
    lines = [
        "// Generated by tools/export_goblin_sprites.py; do not edit by hand.",
        "const FRAME_PIXELS: f32 = 256.0;",
        f"const RUN_CYCLE_SECONDS: f32 = {duration:.7g};",
        f"pub(super) const ATTACK_SECONDS: f32 = {attack_duration:.7g};",
        f"pub(super) const ATTACK_HIT_FRACTION: f32 = {hit_fraction:.7g};",
        f"const GROUND_SPEED_PIXELS: f32 = {ground_speed:.7g};",
        f"const ANCHOR_UV: [f32; 2] = [{anchor.x:.7g}, {1 - anchor.y:.7g}];",
        'const IDLE_PNG: &[u8] = include_bytes!("idle.png");',
        f"const RUN_PNG: [&[u8]; {len(frames)}] = [",
        *[f'    include_bytes!("run_{i:02}.png"),' for i in range(len(frames))],
        "];",
        f"const ATTACK_PNG: [&[u8]; {len(attack_frames)}] = [",
        *[f'    include_bytes!("attack_{i:02}.png"),' for i in range(len(attack_frames))],
        "];",
    ]
    (output / "frames.rs").write_text("\n".join(lines) + "\n")
    print("SPRITES_READY", json.dumps(metadata), flush=True)


if __name__ == "__main__":
    main()

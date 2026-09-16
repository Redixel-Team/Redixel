"""Export saved troop/hero actions as transparent, ground-anchored game frames.

blender -b <source.blend> --python-exit-code 1 --python <this file> -- --group troops
Use --group hero for the hooded hero. Source files are never saved or rebuilt.
"""

import argparse
import hashlib
import json
import math
import sys
from pathlib import Path

import bpy
from mathutils import Vector

HERE = Path(__file__).resolve().parent
V = Vector
MODELS = {
    'troops': [
        ('Goblin Arqueiro - Goblin Saqueador', 'Arqueiro', 'goblin_arqueiro', 3.2, 0),
        ('Goblin Guardiao - Goblin Saqueador', 'Guardiao', 'goblin_guardiao', 3.2, 0),
        ('Ogro - Corpo novo', 'Ogro', 'ogro', 5.26, 0),
    ],
    'hero': [('Heroi Arqueiro', 'Heroi', 'arqueiro_heroi', 3.42, -math.pi / 2)],
}


def activate(scene, rig, action, frame):
    rig.animation_data.action = action
    rig.animation_data.action_slot = action.slots[0]
    scene.frame_set(int(frame), subframe=frame % 1)
    bpy.context.view_layer.update()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--group', choices=MODELS, required=True)
    parser.add_argument('--output', type=Path, default=HERE.parent / 'assets')
    parser.add_argument('--samples', type=int, default=16)
    parser.add_argument('--size', type=int, default=192)
    parser.add_argument('--clips', nargs='+', choices=['idle', 'run', 'attack'], default=['idle', 'run', 'attack'],
                        help='Render only changed clips; other frames must already exist with the same geometry, camera and size.')
    args = parser.parse_args(sys.argv[sys.argv.index('--') + 1:])
    source = Path(bpy.data.filepath)
    source_hash = hashlib.sha256(source.read_bytes()).hexdigest()
    scene = bpy.data.scenes['Unites War - Tropas animadas' if args.group == 'troops' else 'Heroi Arqueiro - Vigia Sombrio']
    bpy.context.window.scene = scene
    camera = bpy.data.objects.new('Export - Camera', bpy.data.cameras.new('Export - Camera'))
    scene.collection.objects.link(camera)
    scene.camera = camera
    camera.data.type = 'ORTHO'
    camera.rotation_euler = V((8, 4, 0)).to_track_quat('-Z', 'Y').to_euler()
    right = camera.rotation_euler.to_matrix() @ V((1, 0, 0))
    for ob in scene.objects:
        ob.hide_render = True
    camera.hide_render = False
    scene.render.engine = 'CYCLES'
    scene.cycles.samples = args.samples
    scene.cycles.use_denoising = True
    scene.cycles.device = 'CPU'
    prefs = bpy.context.preferences.addons['cycles'].preferences
    try:
        prefs.compute_device_type = 'OPTIX'
        prefs.refresh_devices()
        devices = [d for d in prefs.devices if d.type == 'OPTIX']
        if devices:
            for d in prefs.devices:
                d.use = d in devices
            scene.cycles.device = 'GPU'
    except (TypeError, RuntimeError):
        pass
    scene.render.resolution_x = scene.render.resolution_y = args.size
    scene.render.resolution_percentage = 100
    scene.render.film_transparent = True
    scene.render.image_settings.file_format = 'PNG'
    scene.render.image_settings.color_mode = 'RGBA'
    scene.render.image_settings.color_depth = '8'
    scene.render.image_settings.compression = 80
    scene.view_settings.exposure = .45
    scene.world.use_nodes = True
    bg = scene.world.node_tree.nodes['Background']
    bg.inputs['Color'].default_value = (.32, .38, .30, 1)
    bg.inputs['Strength'].default_value = .6
    for i, (position, energy, color) in enumerate([
        ((-5, -6, 6), 850, (1, .95, .84)), ((5, -1, 4), 600, (.78, .88, 1)), ((2, 5, 6), 950, (.92, 1, .8)),
    ]):
        data = bpy.data.lights.new('Export - Light ' + str(i), 'AREA')
        data.energy, data.color, data.size = energy, color, 4
        ob = bpy.data.objects.new(data.name, data)
        scene.collection.objects.link(ob)
        ob.location = position
        ob.rotation_euler = (V((0, 0, 2)) - ob.location).to_track_quat('-Z', 'Y').to_euler()

    for root_name, prefix, directory, body_height, yaw in MODELS[args.group]:
        root = scene.objects[root_name]
        root.location = (0, 0, 0)
        root.rotation_euler.z = yaw
        if args.group == 'hero':
            # Present the front of the hood and chest while the sprite faces
            # right. This reflection exists only in the export scene.
            root.scale.x = -abs(root.scale.x)
            scene.view_settings.exposure = .85
        objects = [o for o in root.children_recursive if o.type == 'MESH']
        rig = next(o for o in root.children_recursive if o.type == 'ARMATURE')
        rig.hide_set(False)
        rig.data.pose_position = 'POSE'
        for track in rig.animation_data.nla_tracks:
            track.mute = True
        for ob in objects:
            ob.hide_render = False
            ob.hide_set(False)
        run, attack = [bpy.data.actions[prefix + '_' + c] for c in ('Run', 'Attack')]
        idle = bpy.data.actions.get(prefix + '_Idle')
        first, last = map(int, run.frame_range)
        end = int(attack.frame_range[1])
        hit = next(m.frame for m in attack.pose_markers if m.name in ('Disparo', 'Impacto'))
        # The game draws the flying projectile from the release event. Keep
        # only the nocked arrow in the character frames, avoiding two arrows.
        arrows = [o for o in objects if o.name == 'Arqueiro - Flecha do disparo' or o.name.startswith('Flecha armada')]
        def set_pose(action, frame):
            activate(scene, rig, action, frame)
            for ob in arrows:
                ob.hide_render = action == attack and frame >= hit

        bounds = []
        for action, frames in [(run, range(first, last, 3)), (attack, range(1, end + 1)), (idle, [1, 16, 31, 46] if idle else [])]:
            for frame in frames:
                set_pose(action, frame)
                graph = bpy.context.evaluated_depsgraph_get()
                for ob in objects:
                    if ob.hide_render:
                        continue
                    evaluated = ob.evaluated_get(graph)
                    bounds.extend((p.dot(right), p.z) for corner in evaluated.bound_box for p in [evaluated.matrix_world @ V(corner)])
        lo = [min(p[i] for p in bounds) for i in (0, 1)]
        hi = [max(p[i] for p in bounds) for i in (0, 1)]
        canvas = max(hi[i] - lo[i] for i in (0, 1)) * 1.12
        cx, cz = [(lo[i] + hi[i]) / 2 for i in (0, 1)]
        camera.location = V((-8, -4, cz)) + right * cx
        camera.data.ortho_scale = canvas
        anchor = [.5 - cx / canvas, .5 + cz / canvas]
        feet = []
        for frame in (first, first + 8):
            set_pose(run, frame)
            feet.append((rig.matrix_world @ rig.pose.bones['Foot.R'].head).dot(right))
        stride = abs(feet[1] - feet[0]) / (8 / 30) * ((last - first) / 30) / body_height
        stride = float(run.get('stride_height_ratio', stride))
        assert stride > .01
        out = args.output / directory
        out.mkdir(parents=True, exist_ok=True)
        if set(args.clips) != {'idle', 'run', 'attack'}:
            previous = json.loads((out / 'manifest.json').read_text())
            assert previous['size'] == args.size
            assert max(abs(a - b) for a, b in zip(previous['anchor_uv'], anchor)) < 1e-6, 'Framing changed; render all clips.'
            assert abs(previous['canvas_height_ratio'] - canvas / body_height) < 1e-6, 'Canvas changed; render all clips.'
        clips = {'idle': (idle or attack, list(range(1, int(idle.frame_range[1]), 5)) if idle else [1]),
                 'run': (run, list(range(first, last, 2))),
                 'attack': (attack, sorted(set([*range(1, end + 1, 2), hit, end])))}
        files = {}
        for label, (action, frames) in clips.items():
            files[label] = []
            for i, frame in enumerate(frames):
                set_pose(action, frame)
                path = out / f'{label}_{i:02}.png'
                scene.render.filepath = str(path)
                if label in args.clips:
                    bpy.ops.render.render(write_still=True)
                else:
                    assert path.is_file(), ('Missing reusable frame', str(path))
                files[label].append(path.name)
            print('CHARACTER_CLIP', directory, label, len(frames), flush=True)
        # Explicit normalized sample times also cover inserted impact frames.
        attack_times = [(f - 1) / (end - 1) for f in clips['attack'][1]]
        metadata = {'source_sha256': source_hash, 'blender_version': bpy.app.version_string,
                    'size': args.size, 'anchor_uv': anchor, 'canvas_height_ratio': canvas / body_height,
                    'stride_height_ratio': stride, 'run_seconds': (last - first) / 30,
                    'attack_seconds': (end - 1) / 30, 'hit_fraction': (hit - 1) / (end - 1),
                    'idle_seconds': (idle.frame_range[1] - idle.frame_range[0]) / 30 if idle else 1.0,
                    'attack_times': attack_times, 'files': files,
                    'facing': 'right', 'root_motion': False}
        (out / 'manifest.json').write_text(json.dumps(metadata, indent=2) + '\n')
        def rust_files(label):
            return '&[' + ', '.join('include_bytes!("' + f + '")' for f in files[label]) + ']'
        def number(v):
            value = f'{v:.7g}'
            return value if '.' in value or 'e' in value else value + '.0'
        rust = '// Generated by tools/export_character_sprites.py.\n'
        rust += 'pub(super) const DATA: SpriteData = SpriteData {\n'
        for label in files:
            rust += f'    {label}: {rust_files(label)},\n'
        for key in ('canvas_height_ratio', 'stride_height_ratio', 'run_seconds', 'attack_seconds', 'hit_fraction', 'idle_seconds'):
            rust += f'    {key}: {number(metadata[key])},\n'
        rust += f'    pixels: {args.size},\n    anchor: [{number(anchor[0])}, {number(anchor[1])}],\n'
        rust += '    attack_times: &[' + ', '.join(map(number, attack_times)) + '],\n};\n'
        (out / 'frames.rs').write_text(rust)
        for ob in objects:
            ob.hide_render = True
        print('CHARACTER_EXPORTED', directory, json.dumps({k: v for k, v in metadata.items() if k != 'files'}), flush=True)
    assert hashlib.sha256(source.read_bytes()).hexdigest() == source_hash


if __name__ == '__main__':
    main()

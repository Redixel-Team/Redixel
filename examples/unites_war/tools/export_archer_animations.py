"""Export the saved hero's three clips and a compact Blender demo video."""

import argparse
import importlib.util
import subprocess
import tempfile
import sys
from pathlib import Path

import bpy
from mathutils import Vector

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('troop_exports', HERE / 'export_goblin_animations.py')
exports = importlib.util.module_from_spec(spec)
spec.loader.exec_module(exports)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--stills', action='store_true')
    parser.add_argument('--video', action='store_true')
    parser.add_argument('--glb', action='store_true')
    args = parser.parse_args(sys.argv[sys.argv.index('--') + 1:] if '--' in sys.argv else [])
    default = not (args.stills or args.video or args.glb)
    scene = bpy.data.scenes['Heroi Arqueiro - Vigia Sombrio']
    bpy.context.window.scene = scene
    output = Path(bpy.data.filepath).parent
    stem = Path(bpy.data.filepath).stem
    if stem == 'source':
        stem = 'arqueiro_heroi_animado'
    exports.MODELS = [('Heroi Arqueiro', 'Heroi', stem + '.glb')]
    if args.glb or default:
        exports.export_glbs(scene, output, stem + '_glb.json')
    if not (args.stills or args.video or default):
        return
    exports.configure_preview(scene)
    scene.camera = scene.objects['Camera - Arqueiro corpo inteiro']
    scene.camera.location = (5, -8, 3.0)
    scene.camera.rotation_euler = (Vector((-.4, 0, 1.9)) - scene.camera.location).to_track_quat('-Z', 'Y').to_euler()
    scene.camera.data.type = 'ORTHO'
    scene.camera.data.ortho_scale = 8.4 if scene.objects['Heroi Arqueiro'].get('recurve_revision') else 7.6
    scene.view_settings.exposure = .5
    if args.stills:
        attack_start = next(m.frame for m in scene.timeline_markers if m.name == 'DISPARO')
        for frame, label in [(74, 'corrida'), (attack_start + 18, 'tensao'), (attack_start + 23, 'soltura')]:
            scene.frame_set(frame)
            scene.render.filepath = str(output / (stem + '_' + label + '.png'))
            bpy.ops.render.render(write_still=True)
    if not (args.video or default):
        return
    # Keep the reusable stills at full size; video is a lightweight preview.
    scene.render.resolution_percentage = 75
    frames = list(range(scene.frame_start, scene.frame_end + 1, 2))
    with tempfile.TemporaryDirectory(prefix='unites-war-hero-preview-') as temporary:
        for index, frame in enumerate(frames):
            scene.frame_set(frame)
            scene.render.filepath = str(Path(temporary) / f'{index:04}.png')
            bpy.ops.render.render(write_still=True)
            if index % 15 == 0:
                print('HERO_PREVIEW', index + 1, len(frames), flush=True)
        subprocess.run(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-y', '-framerate', '15', '-i',
                        str(Path(temporary) / '%04d.png'), '-c:v', 'libx264', '-crf', '20', '-pix_fmt',
                        'yuv420p', '-movflags', '+faststart', str(output / (stem + '.mp4'))], check=True)
    print('HERO_EXPORTED', str(output / (stem + '.mp4')), flush=True)


if __name__ == '__main__':
    main()

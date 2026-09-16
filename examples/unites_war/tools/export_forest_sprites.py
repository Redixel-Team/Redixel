"""Export the saved Blender forest into seamless, game-aligned background tiles.

Run with Blender --background <source.blend> --python-exit-code 1 --python
<this script> -- --output examples/unites_war/assets/forest
The source file is never modified. No third-party Python packages are needed.
"""

import argparse
import hashlib
import json
import math
import struct
import sys
import tempfile
import zlib
from pathlib import Path

import bpy
import numpy as np
from bpy_extras.object_utils import world_to_camera_view
from mathutils import Vector

WORLD_WIDTH = 2400
WORLD_HEIGHT = 540
TILE_WIDTH = 800
BLEED = 1
PIXELS_PER_UNIT = 2
TILT = math.radians(6)


def write_png(path, pixels):
    """Store already color-managed RGB pixels without a second color transform."""
    height, width, channels = pixels.shape
    assert channels == 3

    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))

    rows = b''.join(b'\0' + row.tobytes() for row in pixels)
    path.write_bytes(
        b'\x89PNG\r\n\x1a\n'
        + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0))
        + chunk(b'sRGB', b'\0')
        + chunk(b'IDAT', zlib.compress(rows, 9))
        + chunk(b'IEND', b'')
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--samples', type=int, default=32)
    args = parser.parse_args(sys.argv[sys.argv.index('--') + 1:])
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    scene = bpy.data.scenes['Unites War - Floresta 3D']
    bpy.context.window.scene = scene
    # Artillery is rendered separately at runtime so it can track its target.
    for obj in scene.objects:
        if obj.name.endswith(' - canhao'):
            obj.hide_render = True

    # point(x,y,depth) in build_forest_scene.py already compensates for depth.
    # Non-square render pixels cancel the remaining cos(tilt) vertical squeeze.
    # Once baked, the PNGs use square game pixels and exactly match ground_at.
    width = WORLD_WIDTH + 2 * BLEED
    camera_data = bpy.data.cameras.new('Export - game coordinates')
    camera = bpy.data.objects.new('Export - game coordinates', camera_data)
    scene.collection.objects.link(camera)
    target = Vector((WORLD_WIDTH / 80, 0, (430 - WORLD_HEIGHT / 2) / 40))
    camera.location = target + Vector((0, -60, 60 * math.tan(TILT)))
    camera.rotation_euler = (target - camera.location).to_track_quat('-Z', 'Y').to_euler()
    camera_data.type = 'ORTHO'
    camera_data.ortho_scale = width / 40
    camera_data.clip_end = 300
    scene.camera = camera
    scene.render.resolution_x = width * PIXELS_PER_UNIT
    scene.render.resolution_y = WORLD_HEIGHT * PIXELS_PER_UNIT
    scene.render.resolution_percentage = 100
    scene.render.pixel_aspect_x = 1 / math.cos(TILT)
    scene.render.pixel_aspect_y = 1
    scene.render.use_border = False
    scene.render.use_compositing = False
    scene.render.use_sequencer = False
    scene.render.film_transparent = False
    scene.render.image_settings.file_format = 'PNG'
    scene.render.image_settings.color_mode = 'RGB'
    scene.render.image_settings.color_depth = '8'
    scene.render.engine = 'CYCLES'
    scene.cycles.samples = args.samples
    scene.cycles.use_denoising = True
    scene.cycles.seed = 1847
    scene.cycles.device = 'CPU'
    prefs = bpy.context.preferences.addons['cycles'].preferences
    try:
        prefs.compute_device_type = 'OPTIX'
        prefs.refresh_devices()
        devices = [device for device in prefs.devices if device.type == 'OPTIX']
        if devices:
            for device in prefs.devices:
                device.use = device in devices
            scene.cycles.device = 'GPU'
    except (TypeError, RuntimeError):
        pass

    # Check both ends, the tile seams and the valley at multiple scene depths.
    bpy.context.view_layer.update()
    max_error = 0.0
    for x in [0, 70, 800, 1200, 1600, 2330, 2400]:
        y = 337 - 45 * math.cos(x / WORLD_WIDTH * math.tau)
        for depth in [-1.5, 0.3, 17]:
            point = Vector((x / 40, depth, (430 - y) / 40 - depth * math.tan(TILT)))
            uv = world_to_camera_view(scene, camera, point)
            error = max(abs(uv.x * width - BLEED - x), abs((1 - uv.y) * WORLD_HEIGHT - y))
            max_error = max(max_error, error)
    assert max_error < 0.01, f'Camera does not match the game terrain: {max_error}'

    # Render once so lighting, denoising and edge pixels are identical at seams.
    with tempfile.TemporaryDirectory(prefix='unites-war-forest-') as temp:
        scene.render.filepath = str(Path(temp) / 'panorama.png')
        bpy.ops.render.render(write_still=True)
        image = bpy.data.images.load(scene.render.filepath, check_existing=False)
        image.colorspace_settings.name = 'Non-Color'
        pixels = np.empty(len(image.pixels), dtype=np.float32)
        image.pixels.foreach_get(pixels)
        pixels = pixels.reshape(image.size[1], image.size[0], 4)[::-1, :, :3]
        pixels = np.clip(np.rint(pixels * 255), 0, 255).astype(np.uint8)
        tile_pixels = (TILE_WIDTH + 2 * BLEED) * PIXELS_PER_UNIT
        tiles = []
        for index in range(WORLD_WIDTH // TILE_WIDTH):
            start = index * TILE_WIDTH * PIXELS_PER_UNIT
            filename = f'tile_{index:02}.png'
            write_png(output / filename, pixels[:, start:start + tile_pixels])
            tiles.append({
                'file': filename,
                'world_x': index * TILE_WIDTH - BLEED,
                'world_width': TILE_WIDTH + 2 * BLEED,
                'pixels': [tile_pixels, WORLD_HEIGHT * PIXELS_PER_UNIT],
                'sha256': hashlib.sha256((output / filename).read_bytes()).hexdigest(),
            })
        bpy.data.images.remove(image)

    manifest = {
        'source': Path(bpy.data.filepath).name,
        'source_sha256': hashlib.sha256(Path(bpy.data.filepath).read_bytes()).hexdigest(),
        'scene': scene.name,
        'blender_version': bpy.app.version_string,
        'world_size': [WORLD_WIDTH, WORLD_HEIGHT],
        'tile_width': TILE_WIDTH,
        'bleed': BLEED,
        'pixels_per_unit': PIXELS_PER_UNIT,
        'max_ground_projection_error': max_error,
        'samples': args.samples,
        'dynamic_cannons': True,
        'tiles': tiles,
    }
    (output / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    rust = [
        '// Generated by tools/export_forest_sprites.py; do not edit by hand.',
        f'const MAP_WIDTH: f32 = {WORLD_WIDTH}.0;',
        f'const MAP_HEIGHT: f32 = {WORLD_HEIGHT}.0;',
        f'const TILE_WIDTH: f32 = {TILE_WIDTH}.0;',
        f'const BLEED: f32 = {BLEED}.0;',
        f'const TILE_PNG: [&[u8]; {len(tiles)}] = [',
        *[f'    include_bytes!("{tile["file"]}"),' for tile in tiles],
        '];',
    ]
    (output / 'tiles.rs').write_text('\n'.join(rust) + '\n')
    print('FOREST_EXPORTED', json.dumps(manifest), flush=True)


if __name__ == '__main__':
    main()

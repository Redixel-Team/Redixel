"""Render the three forest cameras from a saved Blender file.

blender -b unites_war_floresta.blend --python-exit-code 1 \
  --python examples/unites_war/tools/render_forest_scene.py
"""

from pathlib import Path

import bpy


def render_forest():
    scene = bpy.data.scenes['Unites War - Floresta 3D']
    bpy.context.window.scene = scene
    scene.render.engine = 'CYCLES'
    scene.cycles.samples = 32
    scene.cycles.use_denoising = True
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
    output = Path(bpy.data.filepath).parent
    cameras = [
        ('Camera - Inicio do jogo', 1920, 1080, 'unites_war_floresta.png'),
        ('Camera - Vale e rio', 1920, 1080, 'unites_war_floresta_vale.png'),
        ('Camera - Panorama completo', 3000, 750, 'unites_war_floresta_panorama.png'),
    ]
    for name, width, height, filename in cameras:
        scene.camera = scene.objects[name]
        scene.render.resolution_x = width
        scene.render.resolution_y = height
        scene.render.resolution_percentage = 100
        scene.render.filepath = str(output / filename)
        bpy.ops.render.render(write_still=True)
        print('FOREST_RENDERED', filename, width, height, flush=True)


if __name__ == '__main__':
    render_forest()

"""Render the archer and export only the character/equipment as a static GLB."""

import contextlib
import io
import json
import struct
from pathlib import Path

import bpy


def export_archer():
    scene=bpy.data.scenes['Heroi Arqueiro - Vigia Sombrio']
    bpy.context.window.scene=scene
    output=Path(bpy.data.filepath).parent
    prefs=bpy.context.preferences.addons['cycles'].preferences
    scene.cycles.device='CPU'
    try:
        prefs.compute_device_type='OPTIX'
        prefs.refresh_devices()
        devices=[device for device in prefs.devices if device.type=='OPTIX']
        if devices:
            for device in prefs.devices:device.use=device in devices
            scene.cycles.device='GPU'
    except (TypeError,RuntimeError):
        pass
    scene.cycles.samples=48
    scene.cycles.use_denoising=True
    for camera,filename in [('Camera - Arqueiro corpo inteiro','arqueiro_heroi_dark.png'),
                            ('Camera - Arqueiro retrato','arqueiro_heroi_retrato.png')]:
        scene.camera=scene.objects[camera]
        scene.render.filepath=str(output/filename)
        bpy.ops.render.render(write_still=True)
        print('ARCHER_RENDERED',filename,flush=True)

    # Apply the procedural materials' base colors to glTF; fine shader noise
    # remains editable in Blender. Bake modifiers/curves and join the static
    # model here to avoid hundreds of mesh nodes in a game engine.
    model=bpy.data.collections['ARQUEIRO - Modelo']
    bpy.ops.object.select_all(action='DESELECT')
    for obj in model.all_objects:
        if obj.type in {'CURVE','MESH'}:obj.select_set(True)
    selected=list(bpy.context.selected_objects)
    if selected:
        bpy.context.view_layer.objects.active=selected[0]
        bpy.ops.object.convert(target='MESH')
        bpy.context.view_layer.objects.active=bpy.data.objects['Tunica acolchoada']
        bpy.ops.object.join()
        bpy.context.object.name='Arqueiro - Malha para o jogo'
    bpy.ops.object.select_all(action='DESELECT')
    for obj in model.all_objects:obj.select_set(True)
    bpy.context.view_layer.objects.active=bpy.data.objects['Heroi Arqueiro']
    filepath=output/'arqueiro_heroi_dark.glb'
    with contextlib.redirect_stdout(io.StringIO()):
        bpy.ops.export_scene.gltf(filepath=str(filepath),export_format='GLB',use_selection=True,
            use_active_scene=True,export_apply=True,export_animations=False,export_cameras=False,
            export_lights=False,export_extras=True)
    raw=filepath.read_bytes()
    length=struct.unpack_from('<I',raw,12)[0]
    data=json.loads(raw[20:20+length])
    assert data.get('meshes') and not data.get('cameras') and not data.get('animations')
    assert len(data['meshes'])==1
    assert not any('Estudio' in node.get('name','') or 'Tronco de fundo' in node.get('name','') for node in data['nodes'])
    print('ARCHER_EXPORTED',filepath,len(data['meshes']),'meshes',flush=True)


if __name__=='__main__':
    export_archer()

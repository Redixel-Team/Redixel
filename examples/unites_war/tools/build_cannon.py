"""Build editable artillery and bake transparent sprites for the 2D runtime.
Run in a fresh background Blender; existing character/forest sources stay intact.
"""
import bpy
import math
import json
from pathlib import Path
from mathutils import Vector

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'assets/cannon'
OUT.mkdir(parents=True, exist_ok=True)
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=False)
scene = bpy.context.scene
scene.name = 'Unites War - Artilharia'

def material(name, color, metallic, roughness):
    m = bpy.data.materials.new(name)
    m.diffuse_color = (*color, 1)
    m.use_nodes = True
    bs = m.node_tree.nodes.get('Principled BSDF')
    bs.inputs['Base Color'].default_value = (*color, 1)
    bs.inputs['Metallic'].default_value = metallic
    bs.inputs['Roughness'].default_value = roughness
    return m

iron = material('Ferro negro', (.055, .065, .07), .8, .36)
edge = material('Aros de ferro envelhecido', (.16, .14, .095), .72, .42)
bore = material('Interior escuro', (.004, .005, .006), .1, .95)
root = bpy.data.objects.new('Canhao - eixo de mira', None)
scene.collection.objects.link(root)

def cylinder(name, x, radius, depth, mat):
    bpy.ops.mesh.primitive_cylinder_add(vertices=32, radius=radius, depth=depth,
                                      location=(x, 0, 0), rotation=(0, math.pi/2, 0))
    obj = bpy.context.object
    obj.name = name
    obj.parent = root
    obj.data.materials.append(mat)
    mod = obj.modifiers.new('Bordas gastas', 'BEVEL')
    mod.width = .016
    mod.segments = 2
    obj.modifiers.new('Normais', 'WEIGHTED_NORMAL')
    return obj

cylinder('Tubo de ferro', .49, .17, 1.30, iron)
cylinder('Culatra', -.17, .21, .16, iron)
for x in [-.06, .25, .88]:
    cylinder('Aro reforcado', x, .195, .075, edge)
# Open muzzle with a recessed dark disk instead of a solid end cap.
bpy.ops.mesh.primitive_torus_add(major_radius=.166, minor_radius=.039,
                                location=(1.16,0,0), rotation=(0,math.pi/2,0))
muzzle = bpy.context.object
muzzle.name = 'Boca do canhao'
muzzle.parent = root
muzzle.data.materials.append(edge)
cylinder('Alma escura', 1.145, .132, .009, bore)
bpy.ops.mesh.primitive_uv_sphere_add(segments=32, ring_count=16, radius=.12)
ball = bpy.context.object
ball.name = 'Bola de canhao - ferro macico'
ball.data.materials.append(iron)
for p in ball.data.polygons:
    p.use_smooth = True
ball.hide_render = True
ball.hide_viewport = True
bpy.ops.object.camera_add(location=(0,-8,0))
camera = bpy.context.object
camera.rotation_euler = (Vector((0,0,0))-camera.location).to_track_quat('-Z','Y').to_euler()
camera.data.type = 'ORTHO'
camera.data.ortho_scale = 3.2
scene.camera = camera
for name, location, energy, size in [('Key',(-2,-4,5),700,4),('Rim',(2,2,3),900,3)]:
    bpy.ops.object.light_add(type='AREA', location=location)
    lamp = bpy.context.object
    lamp.name = name
    lamp.data.energy = energy
    lamp.data.shape = 'DISK'
    lamp.data.size = size
    lamp.rotation_euler = (-lamp.location).to_track_quat('-Z','Y').to_euler()
scene.render.engine = 'CYCLES'
scene.cycles.samples = 16
scene.cycles.use_denoising = True
scene.render.film_transparent = True
scene.render.resolution_x = scene.render.resolution_y = 192
scene.render.resolution_percentage = 100
scene.render.image_settings.file_format = 'PNG'
scene.render.image_settings.color_mode = 'RGBA'
scene.world.color = (.22,.22,.22)
# Store a presentation pose with both independently editable models visible.
ball.hide_viewport = False
ball.location = (-.5,0,-.45)
ball.hide_render = False
bpy.context.preferences.filepaths.save_version = 0
bpy.ops.object.select_all(action='DESELECT')
for obj in [root, *root.children]:
    obj.select_set(True)
bpy.ops.export_scene.gltf(filepath=str(OUT/'cannon.glb'), export_format='GLB',
                          use_selection=True, export_animations=False)
bpy.ops.object.select_all(action='DESELECT')
ball.select_set(True)
ball.location = (0,0,0)
bpy.ops.export_scene.gltf(filepath=str(OUT/'ball.glb'), export_format='GLB',
                          use_selection=True, export_animations=False)
ball.location = (-.5,0,-.45)
bpy.ops.wm.save_as_mainfile(filepath=str(OUT/'source.blend'))
ball.location = (0,0,0)
ball.hide_render = True
files = []
for angle in range(-10, 91, 5):
    root.rotation_euler.y = math.radians(angle)
    name = f'cannon_{angle+10:03}.png'
    scene.render.filepath = str(OUT/name)
    bpy.ops.render.render(write_still=True)
    files.append(name)
for obj in root.children:
    obj.hide_render = True
ball.hide_render = False
camera.data.ortho_scale = .3
scene.render.resolution_x = scene.render.resolution_y = 64
scene.render.filepath = str(OUT/'ball.png')
bpy.ops.render.render(write_still=True)
(OUT/'frames.rs').write_text('const CANNON_PNG: [&[u8]; 21] = [\n' + ''.join(f'    include_bytes!("{f}"),\n' for f in files) + '];\n')
(OUT/'manifest.json').write_text(json.dumps({'angle_min':-10,'angle_step':5,'canvas_world_size':128,'pivot':[.5,.5],'muzzle_world_distance':46.4,'ball_diameter':10,'source':'source.blend','sprites':files},indent=2)+'\n')
print('ARTILLERY_EXPORTED', flush=True)

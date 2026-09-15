"""Export the saved static troop models, preserving hand edits and omitting animation.

blender -b ~/Documents/Blender/unites_war_tropas_dark.blend \
  --python-exit-code 1 --python <this file>
Only reads the saved scene; exports GLBs, a preview and a validation manifest.
"""

import hashlib
import json
from pathlib import Path

import bpy
from mathutils import Vector


def export_units(render=True):
    scene=bpy.data.scenes['Unites War - Tropas sem animacao']
    bpy.context.window.scene=scene
    output=Path(bpy.data.filepath).parent
    models=[('Goblin Arqueiro - Goblin Saqueador','goblin_arqueiro_dark.glb'),
            ('Goblin Guardiao - Goblin Saqueador','goblin_guardiao_dark.glb'),
            ('Ogro - Corpo novo','ogro_dark.glb')]
    report=[]
    for name,filename in models:
        root=scene.objects[name]
        objects=[root,*root.children_recursive]
        meshes=[o for o in objects if o.type=='MESH']
        assert not any(o.animation_data for o in objects),name+' has animation data'
        assert not any(getattr(o.data,'animation_data',None) for o in objects if o.data)
        assert all(m.object in objects for o in meshes for m in o.modifiers if m.type=='ARMATURE')
        bpy.context.view_layer.update()
        depsgraph=bpy.context.evaluated_depsgraph_get()
        points=[o.evaluated_get(depsgraph).matrix_world@Vector(corner) for o in meshes for corner in o.evaluated_get(depsgraph).bound_box]
        low=[min(p[i] for p in points) for i in range(3)];high=[max(p[i] for p in points) for i in range(3)]
        assert -.06<low[2]<.15,(name,low)
        report.append({'name':name,'file':filename,'dimensions':[round(high[i]-low[i],3) for i in range(3)],
                       'min_z':round(low[2],4),'meshes':len(meshes),'rigs':sum(o.type=='ARMATURE' for o in objects),
                       'polygons':sum(len(o.data.polygons) for o in meshes),'animated_objects':0})
        # Center each standalone GLB at the origin; the presentation stays spaced.
        location=root.location.copy()
        hidden={o:o.hide_get() for o in objects}
        try:
            for ob in scene.objects:ob.select_set(False)
            root.location=(0,0,0)
            for ob in objects:
                ob.hide_set(False);ob.select_set(True)
            bpy.context.view_layer.objects.active=root
            bpy.context.view_layer.update()
            bpy.ops.export_scene.gltf(filepath=str(output/filename),export_format='GLB',use_selection=True,
                                      export_animations=False,export_skins=True,export_yup=True,
                                      export_cameras=False,export_lights=False,export_extras=True)
        finally:
            root.location=location
            for ob,state in hidden.items():ob.hide_set(state);ob.select_set(False)
            bpy.context.view_layer.update()
    assert report[2]['dimensions'][2]>report[0]['dimensions'][2]*1.6
    result={'source_blend':Path(bpy.data.filepath).name,'source_sha256':hashlib.sha256(Path(bpy.data.filepath).read_bytes()).hexdigest(),
            'scene':scene.name,'models':report,'note':'Sem animacao. Os dois goblins reutilizam a base e os pesos do saqueador; o ogro tem anatomia nova.'}
    (output/'unites_war_tropas_dark.json').write_text(json.dumps(result,indent=2,ensure_ascii=False)+'\n')
    if render:
        scene.render.filepath=str(output/'unites_war_tropas_dark.png')
        bpy.ops.render.render(write_still=True)
    print('STATIC_TROOPS_EXPORTED',json.dumps(result,ensure_ascii=False),flush=True)


if __name__=='__main__':
    export_units()

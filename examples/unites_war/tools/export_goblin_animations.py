"""Export each troop's Run/Attack GLB and an optional compact animation preview.

blender -b ~/Documents/Blender/unites_war_tropas_animadas.blend \
  --python-exit-code 1 --python <this file> -- --stills
Options: --stills, --video, --glb. Source scene is never saved or rebuilt.
"""

import argparse
import json
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

import bpy
from mathutils import Vector

MODELS=[('Goblin Arqueiro - Goblin Saqueador','Arqueiro','goblin_arqueiro_animado.glb'),
        ('Goblin Guardiao - Goblin Saqueador','Guardiao','goblin_guardiao_animado.glb'),
        ('Ogro - Corpo novo','Ogro','ogro_animado.glb')]


def export_glbs(scene,output,manifest='unites_war_tropas_glb.json'):
    report=[]
    for name,prefix,filename in MODELS:
        root=scene.objects[name];objects=[root,*root.children_recursive]
        rig=next(ob for ob in objects if ob.type=='ARMATURE')
        location=root.location.copy();hidden={o:o.hide_get() for o in objects}
        # Existing demo combines two actions on one timeline. Temporary tracks
        # export them as two independent clips without gaps or repetitions.
        tracks=[]
        for track in rig.animation_data.nla_tracks:
            strips=[(s.name,s.frame_start,s.action,s.repeat,s.extrapolation,s.blend_in,s.blend_out) for s in track.strips]
            tracks.append((track.name,track.mute,strips))
        for track in list(rig.animation_data.nla_tracks):rig.animation_data.nla_tracks.remove(track)
        rig.animation_data.action=None
        kinds=['Idle','Run','Attack'] if prefix+'_Idle' in bpy.data.actions else ['Run','Attack']
        for kind in kinds:
            track=rig.animation_data.nla_tracks.new();track.name=kind
            strip=track.strips.new(kind,1,bpy.data.actions[prefix+'_'+kind]);strip.extrapolation='NOTHING'
        try:
            for ob in scene.objects:ob.select_set(False)
            root.location=(0,0,0)
            for ob in objects:ob.hide_set(False);ob.select_set(True)
            bpy.context.view_layer.objects.active=rig
            scene.frame_set(1);bpy.context.view_layer.update()
            bpy.ops.export_scene.gltf(filepath=str(output/filename),export_format='GLB',use_selection=True,
                export_animations=True,export_animation_mode='NLA_TRACKS',export_frame_range=False,
                export_force_sampling=True,export_skins=True,export_yup=True,
                export_cameras=False,export_lights=False,export_extras=True)
            data=(output/filename).read_bytes()
            length,kind=struct.unpack_from('<II',data,12);assert kind==0x4e4f534a
            gltf=json.loads(data[20:20+length])
            animations=gltf.get('animations',[])
            assert {a['name'] for a in animations}==set(kinds),(filename,[a['name'] for a in animations])
            assert gltf.get('skins') and gltf.get('meshes')
            assert all('translation' not in n or n['translation']==[0,0,0] for n in gltf['nodes'] if n.get('name')==name)
            durations={a['name']:max(gltf['accessors'][s['input']]['max'][0] for s in a['samplers'])-min(gltf['accessors'][s['input']]['min'][0] for s in a['samplers']) for a in animations}
            for clip,seconds in durations.items():
                expected=bpy.data.actions[prefix+'_'+clip]['duration_seconds']
                assert abs(seconds-expected)<.0001,(filename,clip,seconds,expected)
            report.append({'file':filename,'bytes':len(data),'clips':durations,'skins':len(gltf['skins'])})
        finally:
            root.location=location
            for ob,value in hidden.items():ob.hide_set(value);ob.select_set(False)
            for track in list(rig.animation_data.nla_tracks):rig.animation_data.nla_tracks.remove(track)
            for name,mute,strips in tracks:
                track=rig.animation_data.nla_tracks.new();track.name=name;track.mute=mute
                for name,start,action,repeat,extrapolation,blend_in,blend_out in strips:
                    strip=track.strips.new(name,int(start),action);strip.repeat=repeat;strip.extrapolation=extrapolation
                    strip.blend_in=blend_in;strip.blend_out=blend_out
            scene.frame_set(1);bpy.context.view_layer.update()
    (output/manifest).write_text(json.dumps(report,indent=2)+'\n')
    print('ANIMATED_GLBS',json.dumps(report),flush=True)


def configure_preview(scene):
    scene.render.engine='CYCLES';scene.cycles.samples=12;scene.cycles.use_denoising=True;scene.cycles.device='CPU'
    prefs=bpy.context.preferences.addons['cycles'].preferences
    try:
        prefs.compute_device_type='OPTIX';prefs.refresh_devices();devices=[d for d in prefs.devices if d.type=='OPTIX']
        if devices:
            for device in prefs.devices:device.use=device in devices
            scene.cycles.device='GPU'
    except (TypeError,RuntimeError):pass
    scene.render.resolution_x=960;scene.render.resolution_y=576;scene.render.resolution_percentage=100
    scene.render.image_settings.file_format='PNG';scene.render.image_settings.color_mode='RGB'
    scene.camera.data.ortho_scale=15.8
    scene.camera.rotation_euler=(Vector((1,0,2.6))-scene.camera.location).to_track_quat('-Z','Y').to_euler()


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--stills',action='store_true');parser.add_argument('--video',action='store_true');parser.add_argument('--glb',action='store_true')
    args=parser.parse_args(sys.argv[sys.argv.index('--')+1:] if '--' in sys.argv else [])
    scene=bpy.data.scenes['Unites War - Tropas animadas'];bpy.context.window.scene=scene
    output=Path(bpy.data.filepath).parent
    if args.glb:export_glbs(scene,output)
    if args.stills or args.video:configure_preview(scene)
    if args.stills:
        for frame,name in [(7,'corrida'),(97,'preparacao'),(104,'impacto')]:
            scene.frame_set(frame);scene.render.filepath=str(output/('tropas_animadas_'+name+'.png'))
            bpy.ops.render.render(write_still=True)
    if args.video:
        with tempfile.TemporaryDirectory(prefix='unites-war-troop-preview-') as temporary:
            for index,frame in enumerate(range(1,171,2)):
                scene.frame_set(frame);scene.render.filepath=str(Path(temporary)/f'{index:04}.png')
                bpy.ops.render.render(write_still=True)
                if index%15==0:print('PREVIEW_PROGRESS',index+1,85,flush=True)
            subprocess.run(['ffmpeg','-hide_banner','-loglevel','error','-y','-framerate','15','-i',str(Path(temporary)/'%04d.png'),
                            '-c:v','libx264','-crf','20','-pix_fmt','yuv420p','-movflags','+faststart',str(output/'unites_war_tropas_animadas.mp4')],check=True)
        print('ANIMATION_PREVIEW',str(output/'unites_war_tropas_animadas.mp4'),flush=True)


if __name__=='__main__':main()

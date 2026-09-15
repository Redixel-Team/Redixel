"""Add Run and Attack clips to the saved troop models without rebuilding them.

blender -b ~/Documents/Blender/unites_war_tropas_dark.blend \
  --python-exit-code 1 --python <this file>
Writes a new unites_war_tropas_animadas.blend; keeps the static source intact.
"""

import hashlib
import json
import math
import runpy
from pathlib import Path

import bpy
from mathutils import Matrix, Vector

HERE=Path(__file__).resolve().parent
V=Vector
IDENTITY=Matrix.Identity(3)
H=runpy.run_path(str(HERE/'build_forest_scene.py'),run_name='mesh_helpers')
Mesh=H['Mesh']
MODEL_NAMES=['Goblin Arqueiro - Goblin Saqueador','Goblin Guardiao - Goblin Saqueador','Ogro - Corpo novo']


def rotation(x=0,y=0,z=0):
    return Matrix.Rotation(z,3,'Z')@Matrix.Rotation(y,3,'Y')@Matrix.Rotation(x,3,'X')


def smooth(t):
    t=max(0,min(1,t));return t*t*(3-2*t)


def interpolate(keys,frame):
    for a,b in zip(keys,keys[1:]):
        if a[0]<=frame<=b[0]:
            t=smooth((frame-a[0])/(b[0]-a[0]))
            return [u+(v-u)*t for u,v in zip(a[1:],b[1:])]
    return list(keys[-1][1:])


def add_bones(rig,definitions):
    rig.hide_set(False)
    for ob in bpy.context.scene.objects:ob.select_set(False)
    rig.select_set(True);bpy.context.view_layer.objects.active=rig
    bpy.ops.object.mode_set(mode='EDIT')
    for name,head,tail,parent in definitions:
        bone=rig.data.edit_bones.new(name);bone.head=head;bone.tail=tail
        if parent:bone.parent=rig.data.edit_bones[parent]
    bpy.ops.object.mode_set(mode='OBJECT');rig.select_set(False)


def bind_mesh(obj,rig,weights):
    obj.vertex_groups.clear()
    groups={}
    for vertex in obj.data.vertices:
        entries=weights(vertex.co)
        total=sum(entries.values())
        assert total>0,(obj.name,vertex.index)
        for name,value in entries.items():
            if value<=1e-6:continue
            if name not in groups:groups[name]=obj.vertex_groups.new(name=name)
            groups[name].add([vertex.index],value/total,'REPLACE')
    for mod in list(obj.modifiers):
        if mod.type=='ARMATURE':obj.modifiers.remove(mod)
    mod=obj.modifiers.new('Deformacao das tropas','ARMATURE');mod.object=rig


def segment_distance(point,bone):
    delta=bone.tail_local-bone.head_local
    t=max(0,min(1,(point-bone.head_local).dot(delta)/delta.length_squared))
    return (point-(bone.head_local+delta*t)).length


def ogre_weights(point,rig):
    x,y,z=point
    side='R' if x>=0 else 'L'
    if z>3.82 and abs(x)<.91:return {'Head':1}
    # The ogre's hands hang below its waist. Only the central lower body
    # belongs to the legs; include forearms at the inner elbow as well.
    def nearest_weights(names):
        nearest=sorted((segment_distance(point,rig.data.bones[name]),name) for name in names)[:3]
        weights={name:1/(distance+.10)**5 for distance,name in nearest}
        total=sum(weights.values())
        return {name:value/total for name,value in weights.items()}
    if z<1.8 and abs(x)<1.1:
        return nearest_weights(['Pelvis','Thigh.'+side,'Shin.'+side,'Foot.'+side])
    # Prevent the nearby elbow from pulling the belly into a web when raised.
    shoulder=smooth((z-2.65)/.65)
    inner=.80-.22*shoulder;outer=.96+.09*shoulder
    arm=smooth((abs(x)-inner)/(outer-inner))
    torso=nearest_weights(['Pelvis','Spine','Chest','Neck','Head'])
    limb=nearest_weights(['UpperArm.'+side,'Forearm.'+side,'Hand.'+side])
    return {**{name:weight*(1-arm) for name,weight in torso.items()},**{name:weight*arm for name,weight in limb.items()}}


def rig_ogre(scene,root):
    data=bpy.data.armatures.new('Esqueleto do ogro')
    rig=bpy.data.objects.new('Ogro - Rig',data);scene.collection.objects.link(rig);rig.parent=root
    defs=[('Root',(0,0,0),(0,0,.3),None),
          ('Pelvis',(0,.08,1.95),(0,.08,2.45),'Root'),
          ('Spine',(0,.08,2.45),(0,.12,3.2),'Pelvis'),
          ('Chest',(0,.12,3.2),(0,.03,3.65),'Spine'),
          ('Neck',(0,.03,3.65),(0,-.02,4.05),'Chest'),
          ('Head',(0,-.02,4.05),(0,-.02,4.88),'Neck')]
    for side,sign in [('L',-1),('R',1)]:
        defs.extend([
            ('Clavicle.'+side,(sign*.20,.12,3.44),(sign*.94,.08,3.44),'Chest'),
            ('UpperArm.'+side,(sign*.94,.08,3.44),(sign*1.42,0,2.69),'Clavicle.'+side),
            ('Forearm.'+side,(sign*1.42,0,2.69),(sign*1.76,-.19,1.90),'UpperArm.'+side),
            ('Hand.'+side,(sign*1.76,-.19,1.90),(sign*1.76,-.35,1.56),'Forearm.'+side),
            ('Thigh.'+side,(sign*.46,.08,1.95),(sign*.55,.02,1.05),'Pelvis'),
            ('Shin.'+side,(sign*.55,.02,1.05),(sign*.63,-.10,.38),'Thigh.'+side),
            ('Foot.'+side,(sign*.63,-.10,.38),(sign*.63,-.57,.20),'Shin.'+side)])
    add_bones(rig,defs)
    for ob in root.children_recursive:
        if ob.type!='MESH':continue
        name=ob.name.removeprefix('Ogro - ')
        side='L' if '-1' in name else 'R'
        if name=='Anatomia macica' or name.startswith(('Correia peitoral','Costura da correia','Cicatriz do peito','Saia de couro')):
            bind_mesh(ob,rig,lambda p:ogre_weights(p,rig));continue
        if 'porrete' in name or name.startswith('Empunhadura'):bone='Hand.R'
        elif name.startswith('Unha do pe'):bone='Foot.'+side
        elif name.startswith(('Dedo','Unha')):bone='Hand.'+side
        elif name.startswith(('Bracadeira','Lamina do bracelete')):bone='Forearm.'+side
        elif name.startswith(('Joelheira','Faixa da perna')):bone='Shin.'+side
        elif 'ombreira' in name:bone='UpperArm.L'
        elif name.startswith(('Cinta','Fivela','Marca da fivela')):bone='Pelvis'
        else:bone='Head'
        bind_mesh(ob,rig,lambda p,b=bone:{b:1})
    rig.show_in_front=True;rig.hide_render=True
    root['origem']='Anatomia propria com rig e pesos; corrida pesada e ataque com porrete.'
    return rig


def prepare_bow(scene,root,rig):
    defs=[('BowTop',(.64,-.34,2.40),(.64,-.34,2.65),'Hand.R'),
          ('BowBottom',(.64,-.34,.08),(.64,-.34,.30),'Hand.R'),
          ('BowString',(.64,-.34,1.24),(.64,-.34,1.44),'Hand.R'),
          ('Arrow',(0,0,0),(0,-.25,0),'Root')]
    add_bones(rig,defs)
    for name in ['Arqueiro - Arco recurvo','Arqueiro - Laminacao do arco']:
        def weights(co):
            weight=min(1,abs(co.z-1.24)/1.16)**2
            return {'Hand.R':1-weight,'BowTop' if co.z>1.24 else 'BowBottom':weight}
        bind_mesh(scene.objects[name],rig,weights)
    original=scene.objects['Arqueiro - Corda do arco']
    collection=original.users_collection[0];mat=original.data.materials[0]
    # Two skin-weighted halves allow the midpoint to follow the drawing hand.
    mesh=Mesh()
    for start,end in [(V((.64,-.34,.08)),V((.64,-.34,1.24))),(V((.64,-.34,1.24)),V((.64,-.34,2.4)))]:
        for i in range(8):mesh.branch(start.lerp(end,i/8),start.lerp(end,(i+1)/8),.009,.009,sides=6)
    bpy.data.objects.remove(original,do_unlink=True)
    string=mesh.object('Arqueiro - Corda do arco',collection,[mat],smooth=True);string.parent=root
    def string_weights(co):
        t=min(1,abs(co.z-1.24)/1.16)
        return {'BowString':1-t,'BowTop' if co.z>1.24 else 'BowBottom':t}
    bind_mesh(string,rig,string_weights)
    mesh=Mesh();mesh.branch((0,0,0),(0,-1.03,0),.012,.012,sides=8)
    mesh.branch((0,-1.00,0),(0,-1.18,0),.055,.003,material=1,sides=4)
    for side in [-1,1]:mesh.polygon([(0,-.02,0),(side*.065,-.08,0),(side*.065,-.23,0),(0,-.30,0)],2)
    materials=[bpy.data.materials['Tropas | Madeira envelhecida'],bpy.data.materials['Saqueador | Bordas de ferro gastas'],bpy.data.materials['Saqueador | Presas marfim']]
    arrow=mesh.object('Arqueiro - Flecha do disparo',collection,materials);arrow.parent=root
    bind_mesh(arrow,rig,lambda p:{'Arrow':1})


class Pose:
    def __init__(self,rig,seed=None):
        self.rig=rig;self.bones=rig.data.bones
        self.targets={b.name:(seed[b.name].copy() if seed and b.name in seed else b.matrix_local.copy()) for b in self.bones}
        self.heads={n:m.translation.copy() for n,m in self.targets.items()}
        self.rots={b.name:self.targets[b.name].to_3x3()@b.matrix_local.to_3x3().inverted() for b in self.bones}

    def place(self,name,head,rot):
        head=V(head);self.heads[name]=head;self.rots[name]=rot
        matrix=(rot@self.bones[name].matrix_local.to_3x3()).to_4x4();matrix.translation=head;self.targets[name]=matrix

    def child(self,name,rot=None):
        b=self.bones[name];parent=b.parent.name
        head=self.heads[parent]+self.rots[parent]@(b.head_local-self.bones[parent].head_local)
        self.place(name,head,self.rots[parent] if rot is None else rot)
        return head

    def aim(self,name,start,end):
        b=self.bones[name]
        rot=(b.tail_local-b.head_local).normalized().rotation_difference((end-start).normalized()).to_matrix()
        self.place(name,start,rot)

    def limb(self,upper,lower,effector,start,end,pole,hand_rot,reach_fraction=.998):
        start,end,pole=V(start),V(end),V(pole)
        a,b=self.bones[upper].length,self.bones[lower].length
        delta=end-start;distance=min(delta.length,(a+b)*reach_fraction)
        end=start+delta.normalized()*distance
        direction=(end-start).normalized();pole=(pole-direction*pole.dot(direction)).normalized()
        along=(a*a-b*b+distance*distance)/(2*distance)
        joint=start+direction*along+pole*math.sqrt(max(0,a*a-along*along))
        self.aim(upper,start,joint);self.aim(lower,joint,end);self.place(effector,end,hand_rot)
        return end

    def arm(self,side,end,hand_rot,pole=None):
        self.child('Clavicle.'+side)
        upper='UpperArm.'+side;clav='Clavicle.'+side
        shoulder=self.heads[clav]+self.rots[clav]@(self.bones[upper].head_local-self.bones[clav].head_local)
        return self.limb(upper,'Forearm.'+side,'Hand.'+side,shoulder,end,pole or ((-1 if side=='L' else 1),.25,-.15),hand_rot)

    def legs(self,feet=None):
        for side in ['L','R']:
            upper='Thigh.'+side
            hip=self.heads['Pelvis']+self.rots['Pelvis']@(self.bones[upper].head_local-self.bones['Pelvis'].head_local)
            ankle=self.bones['Foot.'+side].head_local.copy() if feet is None else feet[side]
            self.limb(upper,'Shin.'+side,'Foot.'+side,hip,ankle,(0,-1,.02),IDENTITY,.999999)

    def torso(self,drop=0,forward=0,lean=0,yaw=0,twist=0):
        self.place('Root',self.bones['Root'].head_local,IDENTITY)
        self.place('Pelvis',self.bones['Pelvis'].head_local+V((0,-forward,-drop)),rotation(z=yaw))
        self.child('Spine',rotation(x=lean*.45,z=yaw+twist*.4))
        self.child('Chest',rotation(x=lean,z=yaw+twist))
        self.child('Neck',rotation(x=lean*.65,z=(yaw+twist)*.4))
        self.child('Head',rotation(x=lean*.4,z=(yaw+twist)*.2))
        for name in ['Pouch','Loincloth','Bandana','Pendant']:
            if name in self.bones:self.child(name)

    def bow(self,draw=0,arrow=None):
        for name in ['BowTop','BowBottom','BowString']:self.child(name)
        hand_rot=self.rots['Hand.R']
        for name,z in [('BowTop',-.04),('BowBottom',.04)]:
            self.place(name,self.heads[name]+hand_rot@V((-.07*draw,0,z*draw)),hand_rot)
        self.place('BowString',self.heads['BowString']+hand_rot@V((-.42*draw,0,0)),hand_rot)
        if arrow is None:
            self.place('Arrow',(0,.5,1.8),IDENTITY*.0001)
        else:
            position,rot,scale=arrow;self.place('Arrow',position,rot*scale)

    def keyframe(self,frame,previous):
        for bone in self.bones:
            parent=self.targets[bone.parent.name] if bone.parent else Matrix.Identity(4)
            relative=bone.parent.matrix_local.inverted()@bone.matrix_local if bone.parent else bone.matrix_local
            pb=self.rig.pose.bones[bone.name];pb.rotation_mode='QUATERNION'
            pb.matrix_basis=(parent@relative).inverted()@self.targets[bone.name]
            pb.rotation_quaternion.normalize()
            if bone.name in previous and pb.rotation_quaternion.dot(previous[bone.name])<0:pb.rotation_quaternion.negate()
            previous[bone.name]=pb.rotation_quaternion.copy()
            for channel in ['location','rotation_quaternion','scale']:pb.keyframe_insert(data_path=channel,frame=frame,group=bone.name)


def activate(rig,action):
    rig.animation_data_create();rig.animation_data.action=action
    if action and len(action.slots):rig.animation_data.action_slot=action.slots[0]


def sample_clip(scene,rig,source,end,source_end):
    activate(rig,source)
    result=[]
    for frame in range(1,end+1):
        time=1+(frame-1)*(source_end-1)/(end-1)
        scene.frame_set(int(time),subframe=time-int(time));bpy.context.view_layer.update()
        result.append({pb.name:pb.matrix.copy() for pb in rig.pose.bones if pb.name not in ['BowTop','BowBottom','BowString','Arrow']})
    activate(rig,None)
    return result


def bake(scene,rig,name,end,pose_fn,markers):
    action=bpy.data.actions.new(name);action.use_fake_user=True;activate(rig,action)
    previous={}
    for frame in range(1,end+1):
        scene.frame_set(frame)
        pose_fn(frame).keyframe(frame,previous)
    for layer in action.layers:
        for strip in layer.strips:
            for bag in strip.channelbags:
                for curve in bag.fcurves:
                    for key in curve.keyframe_points:key.interpolation='LINEAR'
    for label,frame in markers:
        marker=action.pose_markers.new(label);marker.frame=frame
    action['duration_seconds']=(end-1)/30
    action['loop']=name.endswith('Run')
    action['rig']=rig.name
    return action


def animate_archer(scene,rig,run_source):
    seed=sample_clip(scene,rig,run_source,25,25)
    def run(frame):
        p=Pose(rig,seed[frame-1]);phase=(frame-1)/24*math.tau
        bob=p.heads['Chest'].z-rig.data.bones['Chest'].head_local.z
        p.arm('R',(.82,-.28+.10*math.sin(phase),1.43+bob),rotation(x=.035*math.sin(phase)))
        p.arm('L',(-.64,-.24-.16*math.sin(phase),1.37+bob),rotation(x=-.2))
        p.bow();return p
    run_action=bake(scene,rig,'Arqueiro_Run',25,run,[('Apoio direito',1),('Apoio esquerdo',13)])
    # Raise, nock, draw, loose, settle. Left hand draws a right-held bow.
    keys=[(1,0,0),(5,.22,0),(10,1,0),(17,1,1),(18,1,.15),(20,1,0),(25,.70,0),(37,0,0)]
    def attack(frame):
        aim,draw=interpolate(keys,frame)
        p=Pose(rig);p.torso(drop=.025*aim,lean=.04*aim,yaw=-.15*aim,twist=-.35*aim);p.legs()
        hand=V((.81,-.245,1.225)).lerp(V((.20,-.86,1.98)),aim)
        hr=rotation(z=-math.pi/2*aim)
        p.arm('R',hand,hr)
        p.bow(draw)
        nock=p.heads['BowString'].copy()
        if frame<=5:
            left=rig.data.bones['Hand.L'].head_local.lerp(V((.18,.31,2.25)),smooth((frame-1)/4))
        elif frame<10:
            left=V((.18,.31,2.25)).lerp(nock+V((-.02,.035,.07)),smooth((frame-5)/5))
        else:
            follow=V((-.02,.035,.07))
            if 18<=frame<=22:follow.y+=.10*math.sin((frame-18)/4*math.pi)
            left=rig.data.bones['Hand.L'].head_local.lerp(nock+follow,aim)
        p.arm('L',left,rotation(x=-.85*aim,z=.1*aim))
        if 8<=frame<=17:
            p.bow(draw,(nock,rotation(z=math.pi/2*(1-aim)),min(1,(frame-7)/2)))
        elif 18<=frame<=23:
            # A separate root bone carries the cosmetic arrow after release.
            position=V((.105,-.25-(frame-17)*.38,1.995))
            p.bow(draw,(position,IDENTITY,1 if frame<23 else .001))
        else:p.bow(draw)
        if frame in [1,37]:
            p=Pose(rig);p.bow()
        return p
    attack_action=bake(scene,rig,'Arqueiro_Attack',37,attack,[('Buscar flecha',5),('Mirar',10),('Corda tensionada',17),('Disparo',18),('Recuperar',25)])
    return run_action,attack_action


def animate_guard(scene,rig,run_source,attack_source):
    seed=sample_clip(scene,rig,run_source,25,25)
    def run(frame):
        p=Pose(rig,seed[frame-1]);phase=(frame-1)/24*math.tau
        bob=p.heads['Chest'].z-rig.data.bones['Chest'].head_local.z
        p.arm('L',(-.67,-.34,1.40+bob),rotation(x=-.06,z=-.06))
        p.arm('R',(.83,-.32+.1*math.sin(phase),1.37+bob),rotation(x=.08*math.sin(phase)))
        return p
    run_action=bake(scene,rig,'Guardiao_Run',25,run,[('Apoio direito',1),('Apoio esquerdo',13)])
    attacks=sample_clip(scene,rig,attack_source,25,10)
    def attack(frame):
        p=Pose(rig,attacks[frame-1]);guard=math.sin(math.pi*(frame-1)/24)**.6
        wrist=rig.data.bones['Hand.L'].head_local.lerp(V((-.67,-.34,1.35)),guard)
        p.arm('L',wrist,rotation(x=-.06*guard,z=-.07*guard))
        # Retiming a quaternion clip alone slightly drifts the planted feet.
        p.legs()
        if frame in [1,25]:p=Pose(rig)
        return p
    attack_action=bake(scene,rig,'Guardiao_Attack',25,attack,[('Preparar espada',7),('Impacto',12),('Cobrir com escudo',16),('Recuperar',22)])
    return run_action,attack_action


def animate_ogre(scene,rig):
    def run(frame):
        phase=(frame-1)/32*math.tau
        p=Pose(rig);p.torso(drop=.14+.04*math.cos(2*phase),forward=.045,lean=.12,yaw=.035*math.sin(phase),twist=-.06*math.sin(phase))
        feet={}
        for side,offset in [('R',0),('L',math.pi)]:
            t=phase+offset
            feet[side]=rig.data.bones['Foot.'+side].head_local+V((0,-.48*math.cos(t),.32*max(0,-math.sin(t))))
        p.legs(feet)
        p.arm('R',(1.78,-.24+.20*math.sin(phase),2.01+.04*math.cos(phase)),rotation(x=.10*math.sin(phase)))
        p.arm('L',(-1.66,-.23-.34*math.sin(phase),2.04-.06*math.cos(phase)),rotation(x=-.18))
        return p
    run_action=bake(scene,rig,'Ogro_Run',33,run,[('Passo pesado direito',1),('Passo pesado esquerdo',17)])
    keys=[(1,0,0,0,1.76,-.19,1.90,0),
          (6,.09,-.04,-.02,1.65,.05,2.32,-.20),
          (12,.12,-.06,-.08,1.30,.10,3.12,-.48),
          (18,.03,-.04,-.06,1.68,-.40,3.65,-.48),
          (23,.15,.12,.25,.85,-1.05,2.00,1.65),
          (26,.18,.14,.28,.60,-.90,1.60,2.20),
          (31,.10,.07,.13,1.18,-.60,1.88,1.0),
          (43,0,0,0,1.76,-.19,1.90,0)]
    def attack(frame):
        drop,forward,lean,x,y,z,blade=interpolate(keys,frame)
        p=Pose(rig);strength=math.sin(math.pi*(frame-1)/42)
        p.torso(drop,forward,lean,yaw=-.035*strength,twist=-.10*strength);p.legs()
        p.arm('R',(x,y,z),rotation(x=blade,z=-.05*strength))
        p.arm('L',rig.data.bones['Hand.L'].head_local.lerp(V((-1.17,-.64,2.73)),strength),rotation(x=-.20*strength))
        if frame in [1,43]:p=Pose(rig)
        return p
    attack_action=bake(scene,rig,'Ogro_Attack',43,attack,[('Firmar os pes',6),('Erguer porrete',18),('Impacto',23),('Peso do golpe',26),('Recuperar',36)])
    return run_action,attack_action


def validate(scene,rig,actions):
    info=[]
    for action in actions:
        activate(rig,action)
        first,last=map(int,action.frame_range)
        scene.frame_set(first);bpy.context.view_layer.update()
        initial={pb.name:pb.matrix.copy() for pb in rig.pose.bones}
        scene.frame_set(last);bpy.context.view_layer.update()
        seam=max(abs(rig.pose.bones[n].matrix[i][j]-m[i][j]) for n,m in initial.items() for i in range(4) for j in range(4))
        assert seam<.0002,(action.name,'loop/recovery',seam)
        feet=[]
        for frame in range(first,last+1):
            scene.frame_set(frame);bpy.context.view_layer.update()
            assert all(math.isfinite(value) for pb in rig.pose.bones for row in pb.matrix for value in row)
            assert rig.pose.bones['Root'].matrix.translation.length<.0001
            if action.name.endswith('Attack'):
                feet.append(max((rig.pose.bones['Foot.'+side].matrix.translation-rig.data.bones['Foot.'+side].head_local).length for side in ['L','R']))
        assert not feet or max(feet)<.0003,(action.name,'sliding planted feet',max(feet))
        info.append({'action':action.name,'frames':[first,last],'seconds':(last-first)/30,'loop':action.name.endswith('Run'),'endpoint_error':seam,'foot_drift':max(feet) if feet else None})
    return info


def main():
    source=Path(bpy.data.filepath);source_hash=hashlib.sha256(source.read_bytes()).hexdigest()
    scene=bpy.data.scenes['Unites War - Tropas sem animacao'];bpy.context.window.scene=scene
    scene.name='Unites War - Tropas animadas'
    roots=[scene.objects[name] for name in MODEL_NAMES]
    archer=next(o for o in roots[0].children_recursive if o.type=='ARMATURE')
    guard=next(o for o in roots[1].children_recursive if o.type=='ARMATURE')
    ogre=rig_ogre(scene,roots[2]);prepare_bow(scene,roots[0],archer)
    raider=HERE.parent/'assets/goblin_saqueador/source.blend'
    with bpy.data.libraries.load(str(raider),link=False) as (available,loaded):loaded.actions=['Run','Attack']
    run_source,attack_source=loaded.actions
    pairs=[animate_archer(scene,archer,run_source),animate_guard(scene,guard,run_source,attack_source),animate_ogre(scene,ogre)]
    bpy.data.actions.remove(run_source);bpy.data.actions.remove(attack_source)
    rigs=[archer,guard,ogre]
    report={rig.name:validate(scene,rig,pair) for rig,pair in zip(rigs,pairs)}
    # A single timeline previews every clip: two runs, then two attacks.
    for rig,(run,attack) in zip(rigs,pairs):
        activate(rig,None)
        track=rig.animation_data.nla_tracks.new();track.name='Demonstracao - corrida e ataque'
        strip=track.strips.new('Corrida - 2 ciclos',1,run);strip.repeat=2;strip.extrapolation='NOTHING'
        strip.blend_in=3;strip.blend_out=5
        strip=track.strips.new('Ataque - 2 ciclos',81,attack);strip.repeat=2;strip.extrapolation='NOTHING'
        rig.hide_set(True)
    scene.frame_start=1;scene.frame_end=170;scene.render.fps=30;scene.render.fps_base=1;scene.sync_mode='FRAME_DROP'
    scene.camera.data.ortho_scale=15.8
    scene.camera.rotation_euler=(V((1,0,2.6))-scene.camera.location).to_track_quat('-Z','Y').to_euler()
    scene.render.filepath=str(source.with_name('unites_war_tropas_animadas.png'))
    for name,frame in [('CORRIDA',1),('ATAQUES',81),('FIM',170)]:scene.timeline_markers.new(name,frame=frame)
    scene.frame_set(1)
    for root in roots:root['animacoes']='Run e Attack; movimento no lugar, sem root motion.'
    assert hashlib.sha256(source.read_bytes()).hexdigest()==source_hash
    output=source.with_name('unites_war_tropas_animadas.blend')
    bpy.ops.wm.save_as_mainfile(filepath=str(output),compress=True)
    result={'static_source':source.name,'static_source_sha256':source_hash,'scene':scene.name,'clips':report}
    output.with_suffix('.json').write_text(json.dumps(result,indent=2,ensure_ascii=False)+'\n')
    print('TROOP_ANIMATIONS_READY',json.dumps(result,ensure_ascii=False),flush=True)


if __name__=='__main__':main()

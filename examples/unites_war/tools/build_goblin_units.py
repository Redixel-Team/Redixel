"""Static dark-fantasy troop lineup. Reuses the saved raider for two variants.

blender -b --factory-startup --python-exit-code 1 --python <this file>
Creates a separate scene; does not modify the source goblin or game assets.
"""

import hashlib
import json
import math
import runpy
from pathlib import Path

import bpy
from mathutils import Matrix, Vector

HERE = Path(__file__).resolve().parent
SOURCE = HERE.parent / 'assets/goblin_saqueador/source.blend'
OUTPUT = Path.home() / 'Documents/Blender'
H = runpy.run_path(str(HERE / 'build_forest_scene.py'), run_name='forest_mesh_helpers')
Mesh, linear = H['Mesh'], H['linear']
V = Vector
TAU = math.tau


def material(name, rgb, roughness=.8, metallic=0):
    mat = bpy.data.materials.new('Tropas | ' + name)
    mat.diffuse_color = (*[c / 255 for c in rgb], 1)
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes.get('Principled BSDF')
    bsdf.inputs['Base Color'].default_value = (*[linear(c) for c in rgb], 1)
    bsdf.inputs['Roughness'].default_value = roughness
    bsdf.inputs['Metallic'].default_value = metallic
    return mat


def oval_mesh(mesh, center, scale, segments=20, rings=12, index=0):
    center = V(center)
    def p(i, j):
        a, t = i * TAU / segments, j * math.pi / rings
        return center + V((scale[0]*math.sin(t)*math.cos(a), scale[1]*math.sin(t)*math.sin(a), scale[2]*math.cos(t)))
    for j in range(rings):
        for i in range(segments):
            if j == 0:
                mesh.polygon([p(i,j),p(i,j+1),p(i+1,j+1)],index)
            elif j == rings-1:
                mesh.polygon([p(i,j),p(i+1,j+1),p(i+1,j)],index)
            else:
                mesh.polygon([p(i,j),p(i,j+1),p(i+1,j+1),p(i+1,j)],index)


class Parts:
    def __init__(self, name, scene, root=None, rig=None):
        self.name = name
        self.collection = bpy.data.collections.new(name + ' - Equipamento')
        scene.collection.children.link(self.collection)
        self.root, self.rig = root, rig

    def obj(self, mesh, name, mats, bone=None, smooth=False, bevel=0):
        obj = mesh.object(self.name + ' - ' + name, self.collection, mats, smooth=smooth, bevel=bevel)
        obj.parent = self.root
        if bone and self.rig:
            group = obj.vertex_groups.new(name=bone)
            group.add(list(range(len(obj.data.vertices))), 1, 'REPLACE')
            mod = obj.modifiers.new('Esqueleto reutilizado', 'ARMATURE')
            mod.object = self.rig
        return obj

    def oval(self, name, center, scale, mat, bone=None):
        mesh=Mesh();oval_mesh(mesh,center,scale)
        return self.obj(mesh,name,[mat],bone,smooth=True)

    def line(self, name, points, radius, mat, bone=None, sides=8):
        mesh=Mesh()
        for a,b in zip(points,points[1:]):
            mesh.branch(a,b,radius,radius,sides=sides)
        return self.obj(mesh,name,[mat],bone,smooth=True)

    def box(self, name, center, size, mat, bone=None, bevel=.015):
        mesh=Mesh();mesh.box(center,size)
        return self.obj(mesh,name,[mat],bone,bevel=bevel)

    def spike(self, name, start, end, radius, mat, bone=None):
        mesh=Mesh();mesh.branch(start,end,radius,.005,sides=9)
        return self.obj(mesh,name,[mat],bone,smooth=True)


def copy_goblin(source, scene, name, archer):
    collection=bpy.data.collections.new(name + ' - Corpo reutilizado')
    scene.collection.children.link(collection)
    omitted=['adaga','Enrolamento do cabo','Ponta da guarda','Risco do aco',
             'Bandana','bandana','Mecha selvagem']
    if archer:
        omitted += ['Ombreira de ferro','Aro gasto da ombreira','Rebites da armadura',
                    'Risco na ombreira','Espigao','Lamina da bracadeira','Talisma',
                    'Orbita do talisma','Nariz do talisma','Dente do talisma','Cordao do talisma']
    mapping={}
    for old in source.all_objects:
        if any(word in old.name for word in omitted):
            continue
        obj=old.copy()
        if old.data:
            obj.data=old.data.copy()
            if hasattr(obj.data,'animation_data_clear'):obj.data.animation_data_clear()
        obj.animation_data_clear()
        obj.name=name + ' - ' + old.name
        collection.objects.link(obj)
        mapping[old]=obj
    root=next(obj for old,obj in mapping.items() if old.name=='Goblin Saqueador')
    rig=next(obj for old,obj in mapping.items() if old.type=='ARMATURE')
    for old,obj in mapping.items():
        obj.parent=mapping.get(old.parent)
        for mod in obj.modifiers:
            if mod.type=='ARMATURE':mod.object=rig
        for constraint in obj.constraints:
            if hasattr(constraint,'target') and constraint.target in mapping:
                constraint.target=mapping[constraint.target]
    rig.data.pose_position='POSE'
    for pb in rig.pose.bones:
        pb.matrix_basis=Matrix.Identity(4)
        for constraint in list(pb.constraints):pb.constraints.remove(constraint)
    rig.hide_render=True
    rig.hide_set(True)
    root['origem']='Corpo, pesos e esqueleto reutilizados do saqueador; sem acoes.'
    return root,rig


def build_archer(source,scene,mats):
    root,rig=copy_goblin(source,scene,'Goblin Arqueiro',True)
    p=Parts('Arqueiro',scene,root,rig)
    hood,lining,leather,edge,iron,bone=mats['hood'],mats['lining'],mats['leather'],mats['edge'],mats['iron'],mats['bone']
    # Open-front hood. The long ears and muzzle keep the goblin silhouette.
    profiles=[(2.18,.48,.42),(2.43,.56,.47),(2.75,.55,.48),(2.96,.41,.39),(3.13,.17,.22),(3.20,.015,.03)]
    rings=[]
    for z,rx,ry in profiles:
        rings.append([V((rx*math.sin(a),.03-ry*math.cos(a),z)) for a in [math.radians(66)+i*math.radians(228)/24 for i in range(25)]])
    mesh=Mesh()
    for a,b in zip(rings,rings[1:]):
        for j in range(24):mesh.polygon([a[j],a[j+1],b[j+1],b[j]])
    ob=p.obj(mesh,'Capuz de cacador',[hood,lining],'Head',smooth=True)
    solid=ob.modifiers.new('Tecido com forro','SOLIDIFY');solid.thickness=.045;solid.material_offset=1
    for side in [0,24]:p.line('Costura do capuz '+str(side),[r[side] for r in rings],.012,edge,'Head')
    # Short, folded mantle across the back, clear of the legs.
    mesh=Mesh();rows=[]
    for level,(z,rx,depth) in enumerate([(2.08,.40,.33),(1.82,.48,.42),(1.38,.57,.47),(.79,.58,.45)]):
        row=[]
        for j in range(25):
            a=j*math.pi/24
            row.append(V((rx*math.cos(a),.10+depth*math.sin(a)+.02*math.sin(j*2.6),z-(.06*(j%3) if level==3 else 0))))
        rows.append(row)
    for a,b in zip(rows,rows[1:]):
        for j in range(24):mesh.polygon([a[j],b[j],b[j+1],a[j+1]])
    ob=p.obj(mesh,'Capa curta desfiada',[hood],'Chest',smooth=True)
    ob.modifiers.new('Espessura da capa','SOLIDIFY').thickness=.024
    p.line('Borda da capa',rows[-1],.012,edge,'Chest')
    # Carrying bow: the grip sits exactly where the old dagger grip was.
    bowpoints=[]
    for i in range(33):
        t=i/32
        bowpoints.append((.64+.20*math.sin(math.pi*t)+.09*math.sin(TAU*t)**2,-.34, .08+2.32*t))
    p.line('Arco recurvo',bowpoints,.038,mats['wood'],'Hand.R',12)
    p.line('Laminacao do arco',[(x,y-.027,z) for x,y,z in bowpoints],.012,edge,'Hand.R')
    p.line('Corda do arco',[bowpoints[0],bowpoints[-1]],.007,mats['thread'],'Hand.R',6)
    for i in range(9):
        z=1.06+i*.027
        p.line('Enrolamento do arco '+str(i),[(.80,-.377,z),(.87,-.377,z+.015)],.01,leather,'Hand.R')
    # Quiver and six arrows, angled away from the hood.
    bottom=V((.23,.40,1.48));top=V((.47,.43,2.40))
    mesh=Mesh();mesh.branch(bottom,top,.14,.18,sides=12)
    p.obj(mesh,'Aljava de couro',[leather],'Chest',smooth=True)
    p.oval('Interior escuro da aljava',top+V((0,0,.01)),(.16,.16,.025),lining,'Chest')
    p.line('Borda da aljava',[(top.x+.18*math.cos(i*TAU/24),top.y+.18*math.sin(i*TAU/24),top.z+.018) for i in range(25)],.025,edge,'Chest')
    for i in range(6):
        end=V((.63+(i%3)*.055,.36+(i//3)*.11,2.92+(i%3)*.10))
        start=bottom+V(((i%3-1)*.055,0,.10))
        p.line('Flecha '+str(i),[start,end],.010,mats['wood'],'Chest',6)
        feathers=Mesh()
        for sign in [-1,1]:
            feathers.polygon([end+V((0,0,-.25)),end+V((sign*.065,0,-.10)),end+V((sign*.06,0,-.025)),end])
        p.obj(feathers,'Penas da flecha '+str(i),[bone],'Chest')
    p.line('Correia da aljava',[(-.28,-.25,1.89),(.26,-.28,1.43),(.37,.06,1.52)],.035,leather,'Chest')
    return root


def build_guard(source,scene,mats):
    root,rig=copy_goblin(source,scene,'Goblin Guardiao',False)
    p=Parts('Guardiao',scene,root,rig)
    iron,edge,leather=mats['iron'],mats['metal_edge'],mats['leather']
    # Rounded iron cap with a reinforced brow and short nasal guard.
    mesh=Mesh();rings=[]
    for z,rx,ry in [(2.77,.49,.43),(2.96,.40,.37),(3.10,.20,.20),(3.14,.01,.01)]:
        rings.append([V((rx*math.cos(i*TAU/24),-.035+ry*math.sin(i*TAU/24),z)) for i in range(24)])
    for a,b in zip(rings,rings[1:]):
        for i in range(24):mesh.polygon([a[i],a[(i+1)%24],b[(i+1)%24],b[i]])
    ob=p.obj(mesh,'Elmo de ferro batido',[iron],'Head',smooth=True)
    ob.modifiers.new('Espessura do elmo','SOLIDIFY').thickness=.04
    p.line('Aro do elmo',rings[0]+[rings[0][0]],.032,edge,'Head')
    p.line('Crista do elmo',[(0,-.43,2.8),(0,-.3,3.01),(0,-.03,3.16),(0,.24,3.02)],.035,edge,'Head')
    p.box('Protecao nasal',(0,-.49,2.71),(.085,.065,.27),iron,'Head')
    for side in [-1,1]:
        p.box('Protecao temporal '+str(side),(side*.46,-.12,2.66),(.085,.23,.28),iron,'Head')
    # A worn wooden heater shield, with iron rim, boss and clan emblem.
    center=V((-.69,-.43,1.48))
    outline=[(0,.76),(.49,.48),(.45,-.27),(.22,-.56),(0,-.73),(-.22,-.56),(-.45,-.27),(-.49,.48)]
    front=[center+V((x,-.06,z)) for x,z in outline]
    rear=[v+V((0,.115,0)) for v in front]
    mesh=Mesh();mesh.polygon(list(reversed(front)));mesh.polygon(rear)
    for i in range(len(front)):mesh.polygon([front[i],rear[i],rear[(i+1)%8],front[(i+1)%8]])
    p.obj(mesh,'Escudo de madeira',[mats['wood']],'Hand.L',bevel=.018)
    p.line('Aro de ferro do escudo',front+[front[0]],.045,iron,'Hand.L',10)
    for x in [-.30,-.15,0,.15,.30]:
        p.line('Junta das tabuas '+str(x),[center+V((x,-.065,-.45+abs(x)*.3)),center+V((x,-.066,.64-abs(x)*.55))],.009,mats['lining'],'Hand.L')
    for i,point in enumerate(front):p.oval('Rebite do escudo '+str(i),point+V((0,-.04,0)),(.035,.018,.035),edge,'Hand.L')
    p.oval('Umbo do escudo',center+V((0,-.095,.03)),(.17,.11,.17),iron,'Hand.L')
    p.spike('Espigao do escudo',center+V((0,-.20,.03)),center+V((0,-.33,.03)),.065,edge,'Hand.L')
    p.line('Empunhadura traseira',[(-.80,-.22,1.17),(-.61,-.20,1.14),(-.48,-.27,1.20)],.037,leather,'Hand.L')
    for side in [-1,1]:
        p.line('Marca do cla '+str(side),[center+V((side*.28,-.075,-.20)),center+V((side*.14,-.075,-.43)),center+V((side*.11,-.075,-.26))],.019,mats['ochre'],'Hand.L')
    # Grip follows the original weighted right hand; blade has a chipped edge.
    p.line('Punho da espada',[(.83,-.33,1.00),(.83,-.33,1.34)],.062,leather,'Hand.R')
    p.oval('Pomo da espada',(.83,-.33,1.00),(.085,.07,.06),edge,'Hand.R')
    p.box('Guarda da espada',(.83,-.33,1.34),(.38,.095,.085),iron,'Hand.R')
    blade=Mesh()
    edgepoints=[(.72,1.38),(.71,1.93),(.75,1.99),(.71,2.04),(.75,2.29),(.86,2.45),(.98,2.25),(.96,1.38)]
    ridge=V((.85,-.405,1.85))
    for i,(x,z) in enumerate(edgepoints):
        xx,zz=edgepoints[(i+1)%len(edgepoints)]
        blade.polygon([V((x,-.35,z)),V((xx,-.35,zz)),ridge],i%2)
    ob=p.obj(blade,'Espada lascada',[iron,edge],'Hand.R')
    ob.modifiers.new('Espessura da espada','SOLIDIFY').thickness=.035
    p.box('Placa do peito',(0,-.238,1.70),(.52,.09,.41),iron,'Chest',.035)
    for side in [-1,1]:p.oval('Rebite peitoral '+str(side),(side*.20,-.302,1.84),(.03,.018,.03),edge,'Chest')
    return root


def build_ogre(scene,mats):
    root=bpy.data.objects.new('Ogro - Corpo novo',None);scene.collection.objects.link(root)
    p=Parts('Ogro',scene,root)
    skin=mats['ogre_skin'];iron=mats['iron'];edge=mats['metal_edge'];leather=mats['leather'];bone=mats['bone']
    # A separate broad, hunched anatomy. Voxel-union blends the muscle masses.
    body=Mesh()
    masses=[((0,.12,2.97),(.87,.60,.86)),((0,-.01,2.45),(.77,.56,.73)),
            ((0,.08,1.91),(.63,.45,.42)),((0,.15,3.61),(.58,.46,.53)),
            ((0,-.02,4.16),(.60,.51,.72)),((0,-.26,3.91),(.49,.43,.36)),
            ((0,-.56,4.09),(.39,.29,.25)),((0,-.69,4.26),(.25,.24,.18))]
    for sign in [-1,1]:
        masses += [((sign*.46,.08,1.45),(.41,.39,.65)),((sign*.57,-.02,.78),(.31,.33,.53)),
                   ((sign*.63,-.25,.23),(.38,.66,.23)),((sign*.93,.08,3.44),(.53,.48,.55)),
                   ((sign*1.30,.04,2.98),(.44,.40,.64)),((sign*1.57,-.04,2.43),(.39,.36,.57)),
                   ((sign*1.72,-.13,1.97),(.28,.28,.32)),((sign*1.76,-.19,1.75),(.34,.32,.37))]
    for center,scale in masses:oval_mesh(body,center,scale,24,14)
    ob=p.obj(body,'Anatomia macica',[skin],smooth=True)
    bpy.context.view_layer.objects.active=ob;ob.select_set(True)
    mod=ob.modifiers.new('Uniao dos volumes','REMESH');mod.mode='VOXEL';mod.voxel_size=.055;mod.use_smooth_shade=True
    bpy.ops.object.modifier_apply(modifier=mod.name)
    smooth=ob.modifiers.new('Suavizar anatomia','SMOOTH');smooth.factor=.7;smooth.iterations=3
    bpy.ops.object.modifier_apply(modifier=smooth.name)
    dec=ob.modifiers.new('Malha para jogo','DECIMATE');dec.ratio=.55
    bpy.ops.object.modifier_apply(modifier=dec.name)
    ob.select_set(False)
    # Heavy brow, small sunken eyes, broad nostrils and protruding lower tusks.
    for side in [-1,1]:
        p.oval('Olheira '+str(side),(side*.235,-.474,4.36),(.23,.12,.17),mats['skin_shadow'])
        p.oval('Olho '+str(side),(side*.235,-.574,4.35),(.124,.043,.081),mats['amber'])
        p.oval('Pupila '+str(side),(side*.235,-.614,4.35),(.034,.013,.055),mats['lining'])
        p.oval('Reflexo '+str(side),(side*.217,-.626,4.378),(.012,.005,.012),bone)
        brow=p.oval('Testa pesada '+str(side),(side*.25,-.525,4.48),(.29,.13,.115),skin)
        p.oval('Narina '+str(side),(side*.13,-.884,4.235),(.07,.017,.041),mats['lining'])
        p.line('Presa curva '+str(side),[(side*.29,-.67,3.93),(side*.36,-.76,4.10),(side*.35,-.76,4.29)],.06,bone)
        p.spike('Ponta da presa '+str(side),(side*.35,-.76,4.25),(side*.32,-.74,4.38),.045,bone)
        ear=Mesh();ear.polygon([(side*.48,-.02,4.39),(side*.90,.04,4.50),(side*.64,-.08,4.07)])
        e=p.obj(ear,'Orelha curta '+str(side),[skin]);e.modifiers.new('Espessura','SOLIDIFY').thickness=.11
        for finger in range(4):
            x=side*(1.55+finger*.13)
            p.oval('Dedo '+str(side)+' '+str(finger),(x,-.365,1.65),(.075,.15,.18),skin)
            p.oval('Unha '+str(side)+' '+str(finger),(x,-.49,1.60),(.055,.027,.068),mats['nail'])
        for toe in range(3):p.oval('Unha do pe '+str(side)+' '+str(toe),(side*.63+(toe-1)*.18,-.817,.15),(.085,.06,.045),mats['nail'])
    p.line('Boca cerrada',[(-.27,-.776,4.035),(0,-.817,4.008),(.27,-.776,4.035)],.022,mats['lining'])
    p.line('Cicatriz no rosto',[(-.43,-.40,4.54),(-.33,-.55,4.34),(-.39,-.55,4.19)],.014,mats['scar'])
    # Belt and irregular overlapping leather skirt panels.
    for i in range(14):
        a=i*TAU/14;b=(i+1)*TAU/14
        zbottom=1.31+.17*math.sin(i*2.4)
        mesh=Mesh()
        mesh.polygon([(.66*math.cos(a),.08+.47*math.sin(a),2.15),(.66*math.cos(b),.08+.47*math.sin(b),2.15),
                      (.76*math.cos(b),.06+.54*math.sin(b),zbottom+.08),(.78*math.cos((a+b)/2),.06+.56*math.sin((a+b)/2),zbottom-.09),
                      (.76*math.cos(a),.06+.54*math.sin(a),zbottom)])
        ob=p.obj(mesh,'Saia de couro '+str(i),[leather if i%2 else mats['cloth']])
        ob.modifiers.new('Couro espesso','SOLIDIFY').thickness=.035
    for z in [2.14,2.29]:p.line('Cinta '+str(z),[(.70*math.cos(i*TAU/40),.06+.51*math.sin(i*TAU/40),z) for i in range(41)],.063,leather)
    p.box('Fivela',(0,-.516,2.2),(.43,.105,.28),iron,bevel=.03)
    p.box('Marca da fivela',(0,-.579,2.2),(.18,.025,.13),edge)
    # Asymmetric scavenged armor, stitches and worn edges.
    p.oval('Ombreira pesada',(-1.02,.01,3.68),(.67,.55,.29),iron)
    p.line('Borda da ombreira',[(-1.59,-.26,3.68),(-1.3,-.49,3.60),(-.90,-.52,3.62),(-.52,-.30,3.67)],.043,edge)
    for i in range(4):
        x=-1.48+i*.27
        p.spike('Espigao da ombreira '+str(i),(x,.025,3.88),(x-.1,.04,4.25+(.15 if i==1 else 0)),.085,iron)
        p.oval('Rebite da ombreira '+str(i),(x,-.44,3.68),(.05,.025,.05),edge)
    p.line('Correia peitoral',[(-.61,-.39,3.72),(-.20,-.56,3.25),(.36,-.57,2.59)],.085,leather)
    for i in range(7):
        x=-.56+i*.13;z=3.67-i*.15
        p.line('Costura da correia '+str(i),[(x-.03,-.60,z),(x+.02,-.60,z-.04)],.009,bone)
    for side in [-1,1]:
        p.oval('Bracadeira '+str(side),(side*1.66,-.07,2.15),(.36,.35,.29),leather)
        for i in range(3):p.box('Lamina do bracelete '+str(side)+' '+str(i),(side*(1.46+i*.17),-.40,2.15),(.13,.09,.40),iron,bevel=.02)
        p.oval('Joelheira '+str(side),(side*.55,-.34,1.09),(.28,.12,.32),iron)
        for z in [.56,.8]:p.line('Faixa da perna '+str(side)+' '+str(z),[(side*.58+.30*math.cos(i*TAU/20),-.015+.32*math.sin(i*TAU/20),z) for i in range(21)],.035,leather)
    for i in range(3):
        p.line('Cicatriz do peito '+str(i),[(.16+i*.13,-.54,3.58),(.30+i*.13,-.60,3.35),(.34+i*.13,-.59,3.20)],.014,mats['scar'])
    # Massive bound-wood club in the right fist.
    p.line('Cabo do porrete',[(1.74,-.39,1.25),(1.79,-.39,1.82),(1.96,-.37,2.72),(2.06,-.35,3.15)],.105,mats['wood'],sides=12)
    for i in range(8):p.line('Empunhadura do porrete '+str(i),[(1.69,-.49,1.48+i*.056),(1.88,-.49,1.50+i*.056)],.023,leather)
    p.oval('Cabeca do porrete',(2.04,-.35,3.12),(.39,.36,.64),mats['wood'])
    for z in [2.72,3.12,3.54]:
        p.line('Aro do porrete '+str(z),[(2.04+.36*math.cos(i*TAU/20),-.35+.34*math.sin(i*TAU/20),z) for i in range(21)],.055,iron)
    for i in range(5):
        a=i*TAU/5
        start=V((2.04+.33*math.cos(a),-.35+.33*math.sin(a),3.15))
        p.spike('Cravo do porrete '+str(i),start,start+V((.25*math.cos(a),.25*math.sin(a),.08)),.075,edge)
    root['origem']='Anatomia nova: corpo largo e curvado, sem esqueleto ou animacao.'
    return root


def descendants(root):
    return [root,*root.children_recursive]


def setup_studio(scene,roots,mats):
    stage=Parts('Estudio',scene)
    stone=mats['stone']
    for index,(root,radius) in enumerate(zip(roots,[1.35,1.40,2.72])):
        mesh=Mesh();mesh.branch((root.location.x,0,-.19),(root.location.x,0,-.045),radius,radius,sides=64)
        stage.obj(mesh,'Base '+str(index),[stone],bevel=.025)
    stage.box('Chao',(0,0,-.28),(200,200,.1),mats['background'],bevel=0)
    for text,x in [('ARQUEIRO',-3.6),('GUARDIAO',0),('OGRO',4.4)]:
        data=bpy.data.curves.new('Legenda '+text,'FONT');data.body=text;data.align_x='CENTER';data.size=.23;data.extrude=.002
        data.materials.append(mats['label']);obj=bpy.data.objects.new('Legenda '+text,data);stage.collection.objects.link(obj)
        obj.location=(x,-1.22,-.008);obj.rotation_euler=(math.radians(65),0,0)
    world=bpy.data.worlds.new('Tropas - Ambiente');world.use_nodes=True
    world.node_tree.nodes['Background'].inputs['Color'].default_value=(.18,.23,.27,1)
    world.node_tree.nodes['Background'].inputs['Strength'].default_value=.5;scene.world=world
    for name,pos,energy,size,color in [('Principal',(-5,-7,9),1900,7,(.84,.93,1)),('Preenchimento',(7,-4,6),1550,6,(1,.81,.63)),('Recorte',(1,5,8),2300,5,(.62,.82,1))]:
        data=bpy.data.lights.new(name,'AREA');data.energy=energy;data.shape='DISK';data.size=size;data.color=color
        ob=bpy.data.objects.new('Estudio - '+name,data);stage.collection.objects.link(ob);ob.location=pos
        ob.rotation_euler=(V((.5,0,2))-ob.location).to_track_quat('-Z','Y').to_euler()
    data=bpy.data.cameras.new('Tropas - Camera');camera=bpy.data.objects.new('Tropas - Camera',data);stage.collection.objects.link(camera)
    camera.location=(8,-22,10);camera.rotation_euler=(V((1,0,2.1))-camera.location).to_track_quat('-Z','Y').to_euler()
    data.type='ORTHO';data.ortho_scale=14.8;scene.camera=camera
    scene.render.engine='CYCLES';scene.cycles.samples=24;scene.cycles.use_denoising=True;scene.cycles.device='CPU'
    prefs=bpy.context.preferences.addons['cycles'].preferences
    try:
        prefs.compute_device_type='OPTIX';prefs.refresh_devices();devices=[d for d in prefs.devices if d.type=='OPTIX']
        if devices:
            for d in prefs.devices:d.use=d in devices
            scene.cycles.device='GPU'
    except (TypeError,RuntimeError):pass
    scene.render.resolution_x=1600;scene.render.resolution_y=960;scene.render.resolution_percentage=100
    scene.render.image_settings.file_format='PNG';scene.render.image_settings.color_mode='RGB'
    scene.view_settings.view_transform='AgX'
    scene.render.filepath=str(OUTPUT/'unites_war_tropas_dark.png')
    scene.frame_start=1;scene.frame_end=1;scene.frame_set(1)
    for screen in bpy.data.screens:
        for area in screen.areas:
            if area.type=='VIEW_3D':
                space=area.spaces.active;space.shading.type='SOLID';space.shading.color_type='MATERIAL'
                space.shading.show_shadows=False;space.shading.show_cavity=False;space.shading.show_specular_highlight=False
                space.overlay.show_overlays=False;space.clip_end=400;space.region_3d.view_perspective='CAMERA';space.region_3d.view_camera_zoom=7


def build_units():
    source_hash=hashlib.sha256(SOURCE.read_bytes()).hexdigest()
    scene=bpy.data.scenes.new('Unites War - Tropas sem animacao');bpy.context.window.scene=scene
    # Append only the model collection, not the old studio or unrelated scenes.
    with bpy.data.libraries.load(str(SOURCE),link=False) as (available,loaded):
        loaded.collections=['SAQUEADOR - Modelo']
    source=loaded.collections[0]
    def reused(suffix):return next(m for m in bpy.data.materials if m.name=='Saqueador | '+suffix)
    mats={
        'iron':reused('Ferro negro'),'metal_edge':reused('Bordas de ferro gastas'),
        'leather':reused('Couro castanho'),'bone':reused('Presas marfim'),
        'thread':reused('Faixas de linho'),'amber':reused('Olhos ambar'),
        'hood':material('Linho verde escuro',(36,47,39)),
        'lining':material('Forro e cavidades',(11,14,12)),
        'wood':material('Madeira envelhecida',(65,48,31)),
        'edge':material('Couro nas bordas',(97,84,55)),
        'ochre':material('Insignia ocre',(154,129,68)),
        'ogre_skin':material('Pele cinza musgo do ogro',(89,104,78)),
        'skin_shadow':material('Cavidades do ogro',(37,48,35)),
        'nail':material('Unhas do ogro',(57,52,40)),
        'scar':material('Cicatrizes do ogro',(133,110,85)),
        'cloth':material('Pano castanho escuro',(52,36,29)),
        'stone':material('Pedra das bases',(33,38,41)),
        'background':material('Fundo do estudio',(20,25,28)),
        'label':material('Legendas',(169,171,147)),
    }
    roots=[build_archer(source,scene,mats),build_guard(source,scene,mats),build_ogre(scene,mats)]
    roots[2].scale=(1.08,1.08,1.08)
    for root,x in zip(roots,[-3.6,0,4.4]):root.location.x=x
    setup_studio(scene,roots,mats)
    bpy.context.view_layer.update()
    report=[]
    for root in roots:
        objects=descendants(root);meshes=[o for o in objects if o.type=='MESH']
        depsgraph=bpy.context.evaluated_depsgraph_get()
        points=[o.evaluated_get(depsgraph).matrix_world@V(corner) for o in meshes for corner in o.evaluated_get(depsgraph).bound_box]
        low=[min(v[i] for v in points) for i in range(3)];high=[max(v[i] for v in points) for i in range(3)]
        assert all(o.animation_data is None for o in objects)
        assert all(not getattr(o.data,'animation_data',None) for o in objects if o.data)
        assert low[2]>-.06 and low[2]<.15,(root.name,low)
        report.append({'name':root.name,'mesh_count':len(meshes),'dimensions':[round(high[i]-low[i],3) for i in range(3)],'min_z':round(low[2],4),'polygons':sum(len(o.data.polygons) for o in meshes),'rigs':sum(o.type=='ARMATURE' for o in objects),'animated_objects':0})
    assert report[2]['dimensions'][2]>report[0]['dimensions'][2]*1.5
    assert hashlib.sha256(SOURCE.read_bytes()).hexdigest()==source_hash
    OUTPUT.mkdir(parents=True,exist_ok=True)
    report={'source_sha256':source_hash,'models':report,'scene':scene.name,'note':'Modelos estaticos. Arqueiro e guardiao preservam os pesos e rigs do saqueador; ogro tem anatomia nova.'}
    (OUTPUT/'unites_war_tropas_dark.json').write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n')
    # Save a clean library containing only this scene and its dependencies.
    # Static variants have no reference to the imported Run/Attack actions.
    bpy.data.libraries.write(str(OUTPUT/'unites_war_tropas_dark.blend'),{scene},fake_user=True,compress=True)
    print('TROOPS_READY',json.dumps(report,ensure_ascii=False),flush=True)
    return scene,roots


if __name__=='__main__':
    scene,roots=build_units()
    bpy.ops.render.render(write_still=True)

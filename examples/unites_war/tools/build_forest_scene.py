"""Build an editable 3D recreation of the current Unites War forest.

Run inside Blender with build_scene(), or in background mode with --python.
Coordinates, palette and landmarks follow game/forest.rs and clan.rs.
Creates a new scene without deleting any existing scenes or objects.
"""

import math
import random
from pathlib import Path

import bpy
from mathutils import Vector

ROOT = Path.home() / 'Documents' / 'Blender'
SCALE = 1 / 40
TILT = math.radians(6)
RNG = random.Random(1847)


def linear(c):
    c = c / 255
    return c / 12.92 if c < .04045 else ((c + .055) / 1.055) ** 2.4


def ground(x):
    return 337 - 45 * math.cos(x / 2400 * math.tau)


def point(x, y, depth=0):
    # A shallow downward camera reveals the real depth of the terrain. Shift
    # the distant layers down to retain the original screen-space composition.
    return Vector((x * SCALE, depth, (430 - y) * SCALE - depth * math.tan(TILT)))


class Mesh:
    def __init__(self):
        self.vertices, self.faces, self.indices = [], [], []

    def polygon(self, vertices, material=0):
        n = len(self.vertices)
        self.vertices.extend(vertices)
        self.faces.append(tuple(range(n, n + len(vertices))))
        self.indices.append(material)

    def box(self, center, size, material=0):
        x, y, z = center
        a, b, c = (s * .5 for s in size)
        points = [(x-a,y-b,z-c),(x+a,y-b,z-c),(x+a,y+b,z-c),(x-a,y+b,z-c),
                  (x-a,y-b,z+c),(x+a,y-b,z+c),(x+a,y+b,z+c),(x-a,y+b,z+c)]
        for face in [(3,2,1,0),(0,1,5,4),(1,2,6,5),(2,3,7,6),(3,0,4,7),(4,5,6,7)]:
            self.polygon([points[i] for i in face], material)

    def branch(self, start, end, radius, tip, material=0, sides=9):
        start, end = Vector(start), Vector(end)
        direction = (end-start).normalized()
        u = direction.cross(Vector((0,1,0)))
        if u.length < .01:
            u = direction.cross(Vector((1,0,0)))
        u.normalize()
        v = direction.cross(u).normalized()
        ring = [u*math.cos(i*math.tau/sides)+v*math.sin(i*math.tau/sides) for i in range(sides)]
        for i in range(sides):
            j = (i+1) % sides
            self.polygon([start+ring[i]*radius,start+ring[j]*radius,
                          end+ring[j]*tip,end+ring[i]*tip], material)
        self.polygon([end+d*tip for d in ring], material)
        self.polygon([start+d*radius for d in reversed(ring)], material)

    def object(self, name, collection, materials, bevel=0, smooth=False):
        data = bpy.data.meshes.new(name)
        # Shared vertices give smooth foliage normals and proper solid bevels.
        unique, lookup, remap = [], {}, []
        for vertex in self.vertices:
            key = tuple(round(float(value), 7) for value in vertex)
            if key not in lookup:
                lookup[key] = len(unique)
                unique.append(vertex)
            remap.append(lookup[key])
        data.from_pydata(unique, [], [tuple(remap[i] for i in face) for face in self.faces])
        data.update()
        for mat in materials:
            data.materials.append(mat)
        for face, index in zip(data.polygons, self.indices):
            face.material_index = index
            face.use_smooth = smooth
        obj = bpy.data.objects.new(name, data)
        collection.objects.link(obj)
        if bevel:
            modifier = obj.modifiers.new('Cantos gastos', 'BEVEL')
            modifier.width = bevel
            modifier.segments = 1
        return obj


def build_scene():
    RNG.seed(1847)
    scene = bpy.data.scenes.new('Unites War - Floresta 3D')
    bpy.context.window.scene = scene
    groups = {}
    for name in ['01 - Ceu e nuvens', '02 - Montanhas', '03 - Colinas', '04 - Rio',
                 '05 - Floresta distante', '06 - Arvores proximas', '07 - Terreno e trilha',
                 '08 - Torre do cla', '09 - Torre rival', '10 - Cameras e luzes']:
        col = bpy.data.collections.new('FLORESTA - ' + name)
        scene.collection.children.link(col)
        groups[name[:2]] = col

    def material(name, color, roughness=.85, emission=False):
        mat = bpy.data.materials.new('Floresta - ' + name)
        mat.diffuse_color = (*[c/255 for c in color], 1)
        mat.use_nodes = True
        nodes = mat.node_tree.nodes
        bsdf = nodes.get('Principled BSDF')
        rgba = (*[linear(c) for c in color], 1)
        bsdf.inputs['Base Color'].default_value = rgba
        bsdf.inputs['Roughness'].default_value = roughness
        if emission:
            bsdf.inputs['Emission Color'].default_value = rgba
            bsdf.inputs['Emission Strength'].default_value = 1
        return mat

    bark = material('Casca marrom', (77,68,42))
    bark_light = material('Casca iluminada', (105,91,52))
    foliage = [material('Copa sombra', (55,94,41)), material('Copa verde', (83,130,49)),
               material('Copa luz', (124,159,62)), material('Copa oliva', (103,143,53))]
    far_foliage = [material('Copa distante sombra', (83,124,70)),
                   material('Copa distante verde', (109,149,83)),
                   material('Copa distante luz', (137,167,94)),
                   material('Copa distante oliva', (120,156,85))]
    for palette in [foliage, far_foliage]:
        mat = palette[1]
        nodes, links = mat.node_tree.nodes, mat.node_tree.links
        tex = nodes.new('ShaderNodeTexCoord')
        separate = nodes.new('ShaderNodeSeparateXYZ')
        ramp = nodes.new('ShaderNodeValToRGB')
        ramp.color_ramp.elements[0].position = .05
        ramp.color_ramp.elements[0].color = palette[0].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value
        ramp.color_ramp.elements[1].position = .95
        ramp.color_ramp.elements[1].color = palette[2].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value
        links.new(tex.outputs['Generated'], separate.inputs[0])
        links.new(separate.outputs['Z'], ramp.inputs[0])
        links.new(ramp.outputs[0], nodes['Principled BSDF'].inputs['Base Color'])
    grass = [material('Grama escura',(68,106,42)), material('Grama verde',(114,146,51)),
             material('Grama borda',(163,179,79))]
    dirt = [material('Terra profunda',(88,76,46)), material('Terra superior',(118,94,54)),
            material('Trilha batida',(140,117,70)), material('Pedrinhas',(144,132,88))]
    stones = [material('Pedra cinza oliva',(122,123,102)), material('Pedra clara',(151,150,126)),
              material('Pedra sombra',(94,100,83)), material('Argamassa',(60,67,56))]
    iron = material('Ferro velho',(50,57,48), .46)
    wood = material('Madeira do portao',(85,60,35))
    banner_green = material('Bandeira verde',(89,129,58))
    banner_red = material('Bandeira vermelha',(158,61,45))
    emblem = material('Emblema dourado',(220,204,147))
    snow = material('Neve distante',(221,232,214))
    mountain_mats = [material('Montanha luz',(135,170,172)), material('Montanha sombra',(104,146,153)),
                     material('Montanha lateral',(120,158,164))]
    water = material('Agua verde azulada',(88,163,163), .27)
    foam = material('Brilho do rio',(184,214,189), .5)
    water_nodes = water.node_tree.nodes
    noise = water_nodes.new('ShaderNodeTexNoise')
    noise.inputs['Scale'].default_value = 4
    noise.inputs['Detail'].default_value = 2
    bump = water_nodes.new('ShaderNodeBump')
    bump.inputs['Strength'].default_value = .12
    bump.inputs['Distance'].default_value = .025
    water.node_tree.links.new(noise.outputs['Fac'], bump.inputs['Height'])
    water.node_tree.links.new(bump.outputs['Normal'], water_nodes['Principled BSDF'].inputs['Normal'])

    # Shared leaf meshes keep the forest light in the viewport.
    sphere_data = {}
    def sphere(name, center, radius, collection, mats, variant=0, leafy=False):
        key = (tuple(m.name for m in mats), variant, leafy)
        if key not in sphere_data:
            mesh = Mesh()
            segments, rings = (14, 9) if leafy else (20, 12)
            def vertex(i,j):
                theta = math.pi*j/rings
                phi = math.tau*i/segments
                variation = 1 + (.035*math.sin(phi*5+variant)*math.sin(theta*4) if leafy else 0)
                return Vector((math.sin(theta)*math.cos(phi),math.sin(theta)*math.sin(phi),math.cos(theta)))*variation
            for j in range(rings):
                for i in range(segments):
                    if j==0:
                        verts=[vertex(i,j),vertex(i,j+1),vertex(i+1,j+1)]
                    elif j==rings-1:
                        verts=[vertex(i,j),vertex(i,j+1),vertex(i+1,j)]
                    else:
                        verts=[vertex(i,j),vertex(i,j+1),vertex(i+1,j+1),vertex(i+1,j)]
                    index = 1 if leafy else 0
                    mesh.polygon(verts,index)
            obj = mesh.object(name, collection, mats, smooth=True)
            sphere_data[key] = obj.data
        else:
            obj = bpy.data.objects.new(name,sphere_data[key])
            collection.objects.link(obj)
        obj.location=center
        obj.scale=radius
        return obj

    # The sky is an actual backdrop with a vertical color gradient.
    sky = bpy.data.materials.new('Floresta - Ceu em degrade')
    sky.diffuse_color=(.48,.72,.80,1)
    sky.use_nodes=True
    nodes=sky.node_tree.nodes
    nodes.clear()
    tex=nodes.new('ShaderNodeTexCoord')
    separate=nodes.new('ShaderNodeSeparateXYZ')
    ramp=nodes.new('ShaderNodeValToRGB')
    ramp.color_ramp.elements[0].position=.22
    ramp.color_ramp.elements[0].color=(*[linear(c) for c in (220,227,183)],1)
    ramp.color_ramp.elements[1].position=.86
    ramp.color_ramp.elements[1].color=(*[linear(c) for c in (105,173,200)],1)
    emit=nodes.new('ShaderNodeEmission')
    output=nodes.new('ShaderNodeOutputMaterial')
    for a,b in [(tex.outputs['Generated'],separate.inputs[0]),(separate.outputs['Z'],ramp.inputs[0]),
                (ramp.outputs[0],emit.inputs[0]),(emit.outputs[0],output.inputs['Surface'])]:
        sky.node_tree.links.new(a,b)
    mesh=Mesh()
    mesh.polygon([point(-1500,850,48),point(3900,850,48),point(3900,-320,48),point(-1500,-320,48)])
    mesh.object('Ceu - horizonte azul e creme',groups['01'],[sky])
    sunmat=material('Sol creme',(255,239,172),emission=True)
    sphere('Sol',point(720,109,38),(.78,.18,.78),groups['01'],[sunmat])
    cloudmat=material('Nuvens marfim',(237,240,216))
    for i in range(14):
        x=i*220-95
        y=93+(i%3)*25
        center=point(x,y,34)
        for j,(dx,dz,rx,rz) in enumerate([(0,0,1.34,.23),(-.28,.22,.68,.43),(.46,.1,.64,.28)]):
            sphere(f'Nuvem {i+1:02} - volume {j+1}',center+Vector((dx,0,dz)),(rx,.35,rz),groups['01'],[cloudmat])

    for i in range(12):
        x=i*235-100
        peak=132+(i%3)*19
        depth=25+(i%2)*2
        a,b,c=point(x-165,280,depth),point(x+185,280,depth),point(x+25,280,depth+5)
        top=point(x,peak,depth+1.2)
        ridge=point(x+37,253,depth-.6)
        mesh=Mesh()
        for face,index in [([a,top,ridge],0),([ridge,top,b],1),([b,top,c],2),([c,top,a],0)]:
            j=face.index(top)
            left,right=face[(j+1)%3],face[(j+2)%3]
            cap_left,cap_right=top+(left-top)*.23,top+(right-top)*.23
            mesh.polygon([cap_left,left,right,cap_right],index)
            mesh.polygon([top,cap_left,cap_right],3)
        mesh.object(f'Montanha {i+1:02} - rocha e neve',groups['02'],mountain_mats+[snow])

    # Solid rolling ridges, with a shallow opening for the winding river.
    for layer in range(3):
        depth=17-layer*3.6
        color=[(120,157,108),(104,146,81),(127,161,74)][layer]
        mat=material(f'Colina camada {layer+1}',color)
        mesh=Mesh()
        for i in range(164):
            x0=-120+i*17
            x1=x0+17
            def crest(x,d):
                y=252+layer*30+math.sin(x/185)*28
                center=1120+(17-d)*12
                channel=max(0,1-abs(x-center)/(40+layer*22))
                y+=channel*(48+layer*13)
                return point(x,y,d)
            a,b=crest(x0,depth),crest(x1,depth)
            c,d=crest(x1,depth+3.5),crest(x0,depth+3.5)
            c.z+=.25
            d.z+=.25
            mesh.polygon([a,b,c,d])
            mesh.polygon([Vector((a.x,a.y,-6)),Vector((b.x,b.y,-6)),b,a])
            mesh.polygon([d,c,Vector((c.x,c.y,-6)),Vector((d.x,d.y,-6))])
        mesh.object(f'Colina {layer+1} - relevo ondulado',groups['03'],[mat],smooth=True)

    # A ribbon of water and pale banks descending into the central valley.
    river, banks, glints=Mesh(),Mesh(),Mesh()
    river_points=[]
    for i in range(49):
        t=i/48
        x=1122+158*t+math.sin(t*math.pi*2)*12
        y=260+145*t
        depth=17-13.7*t
        half=(8+71*t**1.4)*SCALE
        center=point(x,y,depth)
        river_points.append((center,half))
    for i in range(48):
        a,wa=river_points[i]
        b,wb=river_points[i+1]
        river.polygon([a+Vector((-wa,0,.015)),b+Vector((-wb,0,.015)),b+Vector((wb,0,.015)),a+Vector((wa,0,.015))])
        for side in [-1,1]:
            banks.polygon([a+Vector((side*wa,0,0)),a+Vector((side*(wa+.1),0,-.025)),
                           b+Vector((side*(wb+.1),0,-.025)),b+Vector((side*wb,0,0))])
        if i%5==0:
            offset=wa*.35*math.sin(i)
            near,far=a.lerp(b,.48),a.lerp(b,.25)
            glints.polygon([near+Vector((offset-wa*.32,0,.035)),near+Vector((offset+wa*.32,0,.035)),
                            far+Vector((offset+wa*.27,0,.035)),far+Vector((offset-wa*.27,0,.035))])
    river.object('Rio - agua no vale',groups['04'],[water],smooth=True)
    banks.object('Rio - margens claras',groups['04'],[dirt[3]])
    glints.object('Rio - reflexos suaves',groups['04'],[foam]).visible_shadow=False

    # The army's near bank uses exactly the game's cosine terrain profile.
    soil, turf, path=Mesh(),Mesh(),Mesh()
    for i in range(256):
        x=-80+i*10
        x1=x+10
        a,b=point(x,ground(x),-1.5),point(x1,ground(x1),-1.5)
        c,d=point(x1,ground(x1)-8,2.9),point(x,ground(x)-8,2.9)
        soil.polygon([Vector((a.x,a.y,-6)),Vector((b.x,b.y,-6)),b,a],0)
        soil.polygon([a,b,c,d],1)
        soil.polygon([d,c,Vector((c.x,c.y,-6)),Vector((d.x,d.y,-6))],0)
        # A narrow grassy lip and a broad walkable dirt top.
        turf.polygon([a+Vector((0,-.012,.08)),b+Vector((0,-.012,.08)),
                      b+Vector((0,-.015,-.10)),a+Vector((0,-.015,-.10))],0)
        turf.polygon([a+Vector((0,-.01,.09)),b+Vector((0,-.01,.09)),
                      b+Vector((0,.22,.10)),a+Vector((0,.22,.10))],2)
        near_a,near_b=point(x,ground(x)-1,-1.23),point(x1,ground(x1)-1,-1.23)
        far_a,far_b=point(x,ground(x)-5,1.05),point(x1,ground(x1)-5,1.05)
        path.polygon([near_a,near_b,far_b,far_a],0)
        turf.polygon([far_a+Vector((0,0,.01)),far_b+Vector((0,0,.01)),c+Vector((0,0,.04)),d+Vector((0,0,.04))],1)
    soil.object('Terreno - barranco de terra',groups['07'],dirt[:2])
    path.object('Terreno - caminho dos exercitos',groups['07'],[dirt[2]])
    turf.object('Terreno - borda de grama',groups['07'],grass)

    # Connect the near bank to the hills so the distant trunks have an actual
    # surface beneath them, including when the scene is viewed from the side.
    forest_floor=Mesh()
    def floor_vertex(x,depth):
        y=ground(x)-9-(depth-2.9)*6.4
        vertex=point(x,y,depth)
        t=max(0,min(1,(17-depth)/13.7))
        river_x=1122+158*t+math.sin(t*math.tau)*12
        width=8+71*t**1.4
        channel=max(0,min(1,(width+45-abs(x-river_x))/15))
        water_z=point(river_x,260+145*t,depth).z-.055
        vertex.z=vertex.z*(1-channel)+min(vertex.z,water_z)*channel
        return vertex
    for i in range(257):
        x=-80+i*10
        for j in range(10):
            a,b=2.9+j*.65,2.9+(j+1)*.65
            forest_floor.polygon([floor_vertex(x,a),floor_vertex(x+10,a),
                                  floor_vertex(x+10,b),floor_vertex(x,b)])
    forest_floor.object('Floresta - solo sob as arvores',groups['05'],
                        [material('Chao da floresta',(121,157,76))],smooth=True)

    def tree(index, x, size, depth, distant):
        col=groups['05' if distant else '06']
        base=point(x,ground(x)-(25 if distant else 9),depth)
        def local(x,z,y=0):
            return base+Vector((x*SCALE*size,y,z*SCALE*size))
        mesh=Mesh()
        mesh.branch(local(0,0),local(-1,40),.14*size,.115*size)
        mesh.branch(local(-1,40),local(-3,82),.115*size,.05*size)
        mesh.branch(local(-1,38),local(-22,70),.067*size,.023*size,1)
        mesh.branch(local(-1,49),local(25,86),.063*size,.02*size,1)
        mesh.object(f'Arvore {index:02} - tronco e galhos',col,[bark,bark_light])
        mats=far_foliage if distant else foliage
        for j,(dx,z,r) in enumerate([(-24,73,24),(22,85,27),(-3,103,31),(-5,69,27)]):
            obj=sphere(f'Arvore {index:02} - copa {j+1}',local(dx,z,(j%2)*.12),
                       (r*SCALE*size,r*.72*SCALE*size,r*.82*SCALE*size),col,mats,
                       variant=index%3,leafy=True)
            obj.rotation_euler.z=(index*.47+j*.33)%math.tau

    for i in range(35):
        x=i*72+23
        if 1090<x<1355 or x>2450:
            continue
        tree(i+1,x,.48+(i*17%9)*.075,5.4+(i%3)*.42,True)
    for i,x in enumerate([225,426,1870,2180]):
        tree(80+i,x,1.05,2.9,False)

    # Small rocks and grass are linked or batched, rather than thousands of objects.
    grass_mesh=Mesh()
    for i in range(150):
        x=RNG.uniform(-30,2430)
        depth=RNG.choice([-1.32,1.3,2.4])
        base=point(x,ground(x)-4,depth)
        for j in range(3):
            offset=Vector(((j-1)*.045,0,0))
            height=RNG.uniform(.06,.17)
            grass_mesh.polygon([base+offset+Vector((-.016,0,0)),base+offset+Vector((.016,0,0)),
                                base+offset+Vector((RNG.uniform(-.06,.06),.03,height))],i%3)
    grass_mesh.object('Grama - tufos nas bordas',groups['07'],grass)
    for i in range(56):
        x=i*43+8
        depth=RNG.uniform(-1.2,1.15)
        base=point(x,ground(x)-2,depth)
        radius=RNG.uniform(.025,.09)
        sphere(f'Pedrinha {i+1:02}',base,(radius*1.5,radius,radius*.55),groups['07'],[dirt[3]],variant=i%2)
    exposed=Mesh()
    for i in range(110):
        x=i*23+8
        y=ground(x)+18+(i*11%23)
        center=point(x,y,-1.525)
        rx,rz=(3+i%4)*SCALE,1.8*SCALE
        exposed.polygon([center+Vector((rx*math.cos(j*math.tau/10),0,rz*math.sin(j*math.tau/10)))
                         for j in range(10)])
    exposed.object('Terreno - pedras expostas no barranco',groups['07'],[material('Pedras na terra',(116,100,59))])

    def tower(x, enemy=False):
        col=groups['09' if enemy else '08']
        base=point(x,ground(x),.3)
        label='Torre rival' if enemy else 'Torre do cla'
        sign=-1 if enemy else 1
        def p(dx,z,depth=0):
            return base+Vector((dx*SCALE,depth,z*SCALE))
        core=Mesh()
        core.box(p(0,68,.15),(2.0,1.95,3.4),3)
        core.object(label+' - estrutura',col,stones,bevel=.025)
        masonry=Mesh()
        for row in range(8):
            z=8+row*16
            cuts=[-40,-27,0,27,40] if row%2 else [-40,-13.5,13.5,40]
            for a,b in zip(cuts,cuts[1:]):
                # Leave the arched gate and the cannon opening readable.
                center=(a+b)/2
                if (z<54 and abs(center)<17) or (74<z<106 and abs(center)<15):
                    continue
                masonry.box(p(center,z,-.87),((b-a-.8)*SCALE,.18,15.1*SCALE),RNG.randrange(3))
            for side in [-1,1]:
                for j in range(3):
                    masonry.box(p(side*40,z,-.57+j*.57),(.18,.55,15.1*SCALE),RNG.randrange(3))
        masonry.object(label+' - blocos de pedra',col,stones,bevel=.017)
        rim=Mesh()
        rim.box(p(0,137,.1),(2.42,2.22,.27),1)
        rim.box(p(0,126,.1),(2.21,2.09,.17),2)
        for i in range(5):
            dx=-42+i*21
            for y in [-.88,1.10]:
                rim.box(p(dx,150,y),(.34,.35,.43),1)
        for y in [-.36,.20,.74]:
            for dx in [-42,42]:
                rim.box(p(dx,150,y),(.34,.35,.43),1)
        rim.object(label+' - ameias',col,stones,bevel=.022)
        # An arched oak door, surrounded by separate wedge-shaped stones.
        door=Mesh()
        outline=[p(-17,0,-.99),p(17,0,-.99),p(17,30,-.99)]
        for i in range(1,13):
            angle=i*math.pi/12
            outline.append(p(math.cos(angle)*17,30+math.sin(angle)*19,-.99))
        door.polygon(outline)
        door.object(label+' - portao de madeira',col,[wood])
        arch=Mesh()
        for i in range(11):
            a=i*math.pi/11
            b=(i+1)*math.pi/11-.022
            arch.polygon([p(math.cos(a)*18,30+math.sin(a)*20,-1.025),
                          p(math.cos(a)*24,30+math.sin(a)*26,-1.025),
                          p(math.cos(b)*24,30+math.sin(b)*26,-1.025),
                          p(math.cos(b)*18,30+math.sin(b)*20,-1.025)],i%3)
        for side in [-1,1]:
            for z in [5,15,25]:
                arch.box(p(side*21,z,-.985),(.15,.15,.23),1)
        arch.object(label+' - arco do portao',col,stones,bevel=.008)
        fittings=Mesh()
        for dx in [-9,0,9]:
            fittings.branch(p(dx,1,-1.005),p(dx,36,-1.005),.014,.014)
        for z in [10,27]:
            fittings.box(p(0,z,-1.025),(.72,.035,.06))
        fittings.object(label+' - ferragens',col,[iron])
        cannon=Mesh()
        cannon.box(p(0,87,-.96),(.65,.07,.6))
        start,end=p(sign*8,88,-1.03),p(sign*58,98,-1.1)
        cannon.branch(start,end,.15,.145,sides=12)
        cannon.branch(end,end+(end-start).normalized()*.04,.17,.17,sides=12)
        cannon.object(label+' - canhao',col,[iron],bevel=.007)
        pole=Mesh()
        pole.branch(p(-13,148,.1),p(-13,213,.1),.035,.024)
        pole.object(label+' - mastro',col,[bark])
        flag=Mesh()
        for i in range(12):
            u0,u1=i/12,(i+1)/12
            def fp(u,v):
                return p(-12+38*u,211-7*u-27*v,.08+.12*math.sin(u*8-v))
            flag.polygon([fp(u0,0),fp(u0,1),fp(u1,1),fp(u1,0)])
        obj=flag.object(label+' - bandeira',col,[banner_red if enemy else banner_green],smooth=True)
        thick=obj.modifiers.new('Tecido', 'SOLIDIFY')
        thick.thickness=.015
        sphere(label+' - selo do cla',p(5,194,-.09),(.17,.025,.15),col,[emblem])

    tower(70)
    tower(2330,True)

    # Orthographic cameras match the game's side view and show the whole map.
    def camera(name, x, width, center_z=4):
        data=bpy.data.cameras.new(name)
        obj=bpy.data.objects.new(name,data)
        groups['10'].objects.link(obj)
        target=Vector((x,0,center_z))
        obj.location=target+Vector((0,-60,60*math.tan(TILT)))
        obj.rotation_euler=(target-obj.location).to_track_quat('-Z','Y').to_euler()
        data.type='ORTHO'
        data.ortho_scale=width
        data.clip_end=300
        data.passepartout_alpha=1
        return obj
    start_camera=camera('Camera - Inicio do jogo',12,24)
    camera('Camera - Vale e rio',30,24)
    camera('Camera - Panorama completo',30,63)
    scene.camera=start_camera

    world=bpy.data.worlds.new('Floresta - Ambiente de dia')
    world.use_nodes=True
    world.node_tree.nodes['Background'].inputs['Color'].default_value=(.57,.70,.8,1)
    world.node_tree.nodes['Background'].inputs['Strength'].default_value=.55
    scene.world=world
    sun_data=bpy.data.lights.new('Floresta - Luz do sol','SUN')
    sun_data.energy=2.2
    sun_data.angle=math.radians(14)
    sun_obj=bpy.data.objects.new('Floresta - Luz do sol',sun_data)
    groups['10'].objects.link(sun_obj)
    sun_obj.rotation_euler=(math.radians(28),math.radians(-32),math.radians(-25))
    fill_data=bpy.data.lights.new('Floresta - Luz suave frontal','AREA')
    fill_data.energy=3500
    fill_data.shape='DISK'
    fill_data.size=35
    fill_obj=bpy.data.objects.new('Floresta - Luz suave frontal',fill_data)
    groups['10'].objects.link(fill_obj)
    fill_obj.location=(30,-12,18)
    fill_obj.rotation_euler=(Vector((30,4,3))-fill_obj.location).to_track_quat('-Z','Y').to_euler()
    scene.render.engine='CYCLES'
    scene.cycles.samples=32
    scene.cycles.use_denoising=True
    scene.cycles.preview_samples=8
    scene.render.resolution_x=1920
    scene.render.resolution_y=1080
    scene.render.resolution_percentage=100
    scene.render.image_settings.file_format='PNG'
    scene.render.image_settings.color_mode='RGBA'
    scene.render.film_transparent=False
    scene.view_settings.view_transform='Standard'
    scene.view_settings.look='None'
    scene.view_settings.exposure=0
    scene.render.filepath=str(ROOT/'unites_war_floresta.png')
    scene['origem']='Recriacao 3D de examples/unites_war/src/game/forest.rs e clan.rs'
    scene['escala']='40 unidades do jogo = 1 unidade Blender; largura do mapa = 60'
    scene['cameras']='Inicio e Vale: 1920x1080. Panorama completo: 3000x750.'
    scene['observacao']='Elementos modelados em 3D, organizados em colecoes editaveis.'
    bpy.context.view_layer.update()
    for area in bpy.context.screen.areas:
        if area.type=='VIEW_3D':
            space=area.spaces.active
            space.shading.type='SOLID'
            space.shading.color_type='MATERIAL'
            space.shading.show_shadows=False
            space.shading.show_cavity=False
            space.shading.show_specular_highlight=False
            space.overlay.show_overlays=False
            space.clip_end=400
            space.region_3d.view_perspective='CAMERA'
            space.region_3d.view_camera_zoom=22
    print('FOREST_READY',scene.name,len(scene.objects),sum(len(o.data.polygons) for o in scene.objects if o.type=='MESH'))
    return scene


if __name__=='__main__':
    build_scene()
    ROOT.mkdir(parents=True,exist_ok=True)
    bpy.ops.wm.save_as_mainfile(filepath=str(ROOT/'unites_war_floresta.blend'))

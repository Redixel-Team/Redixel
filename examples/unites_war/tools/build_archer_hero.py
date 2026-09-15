"""Editable dark-fantasy archer, based on Alan's hooded archer reference.

Creates a separate scene and saves a new .blend. The pose is a drawn longbow.
The legs and boots complete the unseen lower half of the reference.
"""

import math
import random
import runpy
from pathlib import Path

import bpy
from mathutils import Vector

helpers = runpy.run_path(str(Path(__file__).with_name('build_forest_scene.py')), run_name='forest_mesh_helpers')
Mesh, linear = helpers['Mesh'], helpers['linear']
ROOT = Path.home() / 'Documents' / 'Blender'
V = Vector
Z = V((0, 0, 1))
RNG = random.Random(731)


def build_archer():
    scene = bpy.data.scenes.new('Heroi Arqueiro - Vigia Sombrio')
    bpy.context.window.scene = scene
    model = bpy.data.collections.new('ARQUEIRO - Modelo')
    scene.collection.children.link(model)
    groups = {}
    for key, name in [('body','Corpo e tunica'),('hood','Capuz e mascara'),('cape','Capa e gola'),
                      ('arms','Bracos e luvas'),('legs','Pernas e botas'),('armor','Couro e armadura'),
                      ('bow','Arco e flecha'),('gear','Aljava e equipamento')]:
        col = bpy.data.collections.new('ARQUEIRO - ' + name)
        model.children.link(col)
        groups[key] = col
    stage = bpy.data.collections.new('ARQUEIRO - Estudio')
    scene.collection.children.link(stage)
    root = bpy.data.objects.new('Heroi Arqueiro',None)
    model.objects.link(root)

    def material(name, color, roughness=.8, metallic=0, texture=False):
        mat=bpy.data.materials.new('Arqueiro - '+name)
        mat.diffuse_color=(*[c/255 for c in color],1)
        mat.use_nodes=True
        bsdf=mat.node_tree.nodes['Principled BSDF']
        bsdf.inputs['Base Color'].default_value=(*[linear(c) for c in color],1)
        bsdf.inputs['Roughness'].default_value=roughness
        bsdf.inputs['Metallic'].default_value=metallic
        bsdf.inputs['Specular IOR Level'].default_value=.28 if metallic else .12
        if texture:
            nodes,links=mat.node_tree.nodes,mat.node_tree.links
            noise=nodes.new('ShaderNodeTexNoise')
            noise.inputs['Scale'].default_value=145 if metallic==0 else 32
            noise.inputs['Detail'].default_value=2
            bump=nodes.new('ShaderNodeBump')
            bump.inputs['Strength'].default_value=.22
            bump.inputs['Distance'].default_value=.008 if metallic==0 else .003
            links.new(noise.outputs['Fac'],bump.inputs['Height'])
            links.new(bump.outputs['Normal'],bsdf.inputs['Normal'])
        return mat

    cloth=material('Tecido carvao',(28,36,35),.94,texture=True)
    hoodmat=material('Capuz verde acinzentado',(34,44,43),.93,texture=True)
    lining=material('Forro profundo',(9,13,13),1)
    maskmat=material('Mascara escura',(8,11,10),1,texture=True)
    cape_mat=material('Capa gasta',(30,39,36),.96,texture=True)
    edge_mat=material('Bordas gastas',(56,65,58),.94)
    leather=material('Couro envelhecido',(47,35,25),.73,texture=True)
    leather_dark=material('Couro negro',(26,27,23),.8,texture=True)
    thread=material('Linha encerada',(104,90,60),.88)
    iron=material('Aco escurecido',(42,49,48),.72,.65,True)
    iron_edge=material('Aco das bordas',(97,109,106),.42,.72)
    bronze=material('Bronze antigo',(85,64,36),.65,.65)
    bow_wood=material('Madeira do arco',(40,30,20),.68,texture=True)
    bow_edge=material('Laminacao do arco',(85,75,50),.66)
    string_mat=material('Corda encerada',(126,132,109),.88)
    feather=material('Penas escuras',(44,51,44),.98)
    feather_edge=material('Pontas das penas',(55,61,50),.98)

    def obj(mesh,name,key,mats,smooth=True,bevel=0,solid=0):
        ob=mesh.object(name,groups[key] if isinstance(key,str) else key,mats,bevel=bevel,smooth=smooth)
        if isinstance(key,str) and key in groups:
            ob.parent=root
        if solid:
            mod=ob.modifiers.new('Espessura do tecido','SOLIDIFY')
            mod.thickness=solid
            mod.material_offset=1 if len(mats)>1 else 0
        return ob

    def tube(name, points, radii, key, mat, sides=12, ellipse=1, caps=True):
        points=[V(p) for p in points]
        rings=[]
        for i,p in enumerate(points):
            tangent=(points[min(i+1,len(points)-1)]-points[max(i-1,0)]).normalized()
            u=V((0,1,0))
            if abs(tangent.dot(u))>.96:u=V((1,0,0))
            u=(u-tangent*u.dot(tangent)).normalized()
            v=tangent.cross(u).normalized()
            r=radii[i] if isinstance(radii,(list,tuple)) else radii
            rings.append([p+u*(math.cos(j*math.tau/sides)*r*ellipse)+v*(math.sin(j*math.tau/sides)*r)
                          for j in range(sides)])
        mesh=Mesh()
        for a,b in zip(rings,rings[1:]):
            for j in range(sides):
                k=(j+1)%sides
                mesh.polygon([a[j],a[k],b[k],b[j]])
        if caps:
            mesh.polygon(list(reversed(rings[0])))
            mesh.polygon(rings[-1])
        return obj(mesh,name,key,[mat])

    def curve(name, points, radius, key, mat, cyclic=False):
        data=bpy.data.curves.new(name,'CURVE')
        data.dimensions='3D'
        data.resolution_u=2
        data.bevel_depth=radius
        data.bevel_resolution=2
        spline=data.splines.new('POLY')
        spline.points.add(len(points)-1)
        for p,co in zip(spline.points,points):p.co=(*co,1)
        spline.use_cyclic_u=cyclic
        data.materials.append(mat)
        ob=bpy.data.objects.new(name,data)
        (groups[key] if isinstance(key,str) else key).objects.link(ob)
        if isinstance(key,str):ob.parent=root
        return ob

    def oval(name, center, scale, key, mat, segments=24, rings=14):
        center=V(center)
        mesh=Mesh()
        def p(i,j):
            a=i*math.tau/segments
            t=j*math.pi/rings
            return center+V((scale[0]*math.sin(t)*math.cos(a),scale[1]*math.sin(t)*math.sin(a),scale[2]*math.cos(t)))
        for j in range(rings):
            for i in range(segments):
                if j==0:mesh.polygon([p(i,j),p(i,j+1),p(i+1,j+1)])
                elif j==rings-1:mesh.polygon([p(i,j),p(i,j+1),p(i+1,j)])
                else:mesh.polygon([p(i,j),p(i,j+1),p(i+1,j+1),p(i+1,j)])
        return obj(mesh,name,key,[mat])

    def body_loft(name, profiles, key, mat, segments=40, folds=0):
        rings=[]
        for i,(z,rx,ry,cx,cy) in enumerate(profiles):
            ring=[]
            for j in range(segments):
                a=j*math.tau/segments
                wr=1+folds*(math.sin(a*9+i*.7)+.35*math.sin(a*15-i*.8))
                ring.append(V((cx+rx*math.cos(a)*wr,cy+ry*math.sin(a)*wr,z)))
            rings.append(ring)
        mesh=Mesh()
        for a,b in zip(rings,rings[1:]):
            for j in range(segments):
                k=(j+1)%segments
                mesh.polygon([a[j],a[k],b[k],b[j]])
        mesh.polygon(list(reversed(rings[0])))
        mesh.polygon(rings[-1])
        return obj(mesh,name,key,[mat])

    def ribbon(name,points,width,key,mat):
        points=[V(p) for p in points]
        mesh=Mesh()
        for i in range(len(points)-1):
            tangent=(points[i+1]-points[i]).normalized()
            side=V((tangent.z,0,-tangent.x)).normalized()*width*.5
            mesh.polygon([points[i]-side,points[i+1]-side,points[i+1]+side,points[i]+side])
        return obj(mesh,name,key,[mat],solid=.018)

    def buckle(name,center,width,height,key='armor'):
        center=V(center)
        mesh=Mesh()
        for dx in [-width/2,width/2]:mesh.box(center+V((dx,0,0)),(.017,.025,height))
        for dz in [-height/2,height/2]:mesh.box(center+V((0,0,dz)),(width+.015,.025,.017))
        mesh.box(center,(.012,.035,height*.82))
        return obj(mesh,name,key,[bronze],False,.004)

    # A fitted brigandine silhouette, rather than visible primitive joints.
    body_loft('Tunica acolchoada',[(1.20,.32,.21,0,0),(1.39,.34,.22,0,0),(1.57,.29,.205,0,0),
                                  (1.79,.285,.205,0,0),(2.05,.365,.235,0,0),(2.25,.415,.23,0,0),
                                  (2.41,.405,.19,0,0),(2.50,.18,.145,0,0)],'body',cloth,folds=.022)
    for side in [-1,1]:
        # Tapered skirts part around the wide shooting stance.
        mesh=Mesh()
        for i in range(8):
            t0,t1=i/8,(i+1)/8
            def flap(t,u):
                return V((side*(.025+.25*u+.03*t),-.23-.025*math.sin(t*math.pi)+.02*math.sin(u*8),1.54-.48*t+.08*u*t))
            for j in range(8):
                u0,u1=j/8,(j+1)/8
                mesh.polygon([flap(t0,u0),flap(t1,u0),flap(t1,u1),flap(t0,u1)])
        obj(mesh,f'Aba da tunica {side}','body',[leather_dark,lining],solid=.014)
        curve(f'Costura da aba {side}',[flap(i/24,1)+V((0,-.008,0)) for i in range(25)],.005,'body',edge_mat)

    # Arms are shaped along the anatomical bends, with cloth folds at elbows.
    left=[(-.34,0,2.42),(-.49,-.04,2.44),(-.76,-.10,2.44),(-.94,-.14,2.44),
          (-1.08,-.17,2.46),(-1.28,-.21,2.48),(-1.49,-.25,2.49)]
    right=[(.34,0,2.42),(.50,.0,2.49),(.73,.01,2.57),(.91,.01,2.60),
           (.78,-.055,2.62),(.53,-.155,2.66),(.29,-.25,2.68)]
    tube('Manga do braco do arco',left,[.15,.158,.14,.118,.122,.10,.076],'arms',cloth,20)
    tube('Manga do braco da corda',right,[.15,.155,.14,.12,.125,.105,.076],'arms',cloth,20)
    for side,points in [('L',left),('R',right)]:
        elbow,wrist=V(points[3]),V(points[-1])
        a=elbow.lerp(wrist,.19)
        b=elbow.lerp(wrist,.86)
        tube(f'Bracadeira de couro {side}',[a,a.lerp(b,.15),a.lerp(b,.85),b],[.135,.131,.104,.098],'armor',leather,20)
        axis=(b-a).normalized()
        for i,t in enumerate([.08,.38,.70,.92]):
            p=a.lerp(b,t)
            radius=.137*(1-t)+.105*t
            tube(f'Tira da bracadeira {side} {i}',[p-axis*.025,p+axis*.025],[radius,radius],'armor',leather_dark,20)
            oval(f'Rebite da bracadeira {side} {i}',p+V((0,-radius,0)),(.018,.01,.018),'armor',bronze,12,8)
        # A slim plate protects the forearm, held down by the leather straps.
        pa=a+V((0,-.13,0))
        pb=b+V((0,-.10,0))
        ribbon(f'Placa de antebraco {side}',[pa,pa.lerp(pb,.25),pa.lerp(pb,.8),pb],.14,'armor',iron)
        for j in range(3):
            p=elbow.lerp(V(points[2]),.20+j*.15)
            tangent=(V(points[2])-elbow).normalized()
            tube(f'Dobra do cotovelo {side} {j}',[p-tangent*.008,p+tangent*.008],[.125,.125],'arms',hoodmat,20)

    # Gloves: four curled fingers around the grip and three fingers at the nock.
    oval('Luva - palma do arco',(-1.555,-.255,2.475),(.13,.085,.115),'arms',leather_dark)
    for i,z in enumerate([2.55,2.495,2.445,2.395]):
        curve(f'Luva do arco - dedo {i+1}',[(-1.56,-.30,z),(-1.66,-.365,z-.006),(-1.74,-.31,z-.015),(-1.71,-.235,z-.01)],
              .024 if i<3 else .021,'arms',leather_dark)
    curve('Luva do arco - polegar',[(-1.51,-.235,2.54),(-1.59,-.185,2.57),(-1.69,-.205,2.565)],.03,'arms',leather)
    oval('Luva - palma da corda',(.235,-.255,2.67),(.115,.08,.08),'arms',leather_dark)
    for i,z in enumerate([2.71,2.662,2.615]):
        curve(f'Luva da corda - dedo {i+1}',[(.24,-.295,z),(.13,-.32,z),(.085,-.285,z-.018),(.15,-.245,z-.022)],.022,'arms',leather_dark)
    curve('Luva da corda - polegar',[(.27,-.22,2.69),(.20,-.20,2.73),(.135,-.25,2.715)],.025,'arms',leather)

    # Hood: an open shell with a projecting brow and a real dark lining.
    forward=V((-.92,-.392,0)).normalized()
    lateral=V((-.392,.92,0)).normalized()
    head_center=V((.0,-.005,3.01))
    hood_profiles=[(.235,.30,.365),(.17,.342,.39),(.08,.365,.40),(-.045,.357,.405),
                   (-.17,.31,.36),(-.27,.235,.29),(-.335,.12,.18),(-.36,.018,.055)]
    def hood_point(i,a):
        depth,rx,rz=hood_profiles[i]
        fold=.006*math.sin(a*13+i*.65)
        brow=(.075*max(0,math.sin(a)) if i<3 else .012*math.sin(a))
        return head_center+forward*(depth+brow)+lateral*((rx+fold)*math.cos(a))+Z*((rz+fold)*math.sin(a))
    mesh=Mesh()
    for i in range(len(hood_profiles)-1):
        for j in range(64):
            a,b=j*math.tau/64,(j+1)*math.tau/64
            mesh.polygon([hood_point(i,a),hood_point(i+1,a),hood_point(i+1,b),hood_point(i,b)])
    mesh.polygon([hood_point(len(hood_profiles)-1,j*math.tau/64) for j in reversed(range(64))])
    obj(mesh,'Capuz - tecido com abertura profunda','hood',[hoodmat,lining],solid=.022)
    curve('Capuz - borda espessa',[hood_point(0,j*math.tau/96) for j in range(96)],.011,'hood',edge_mat,True)
    for j,a in enumerate([math.pi*.23,math.pi*.50,math.pi*.77,math.pi*1.25,math.pi*1.75]):
        path=[hood_point(i,a)+(hood_point(i,a)-head_center).normalized()*.008 for i in range(len(hood_profiles)-1)]
        curve(f'Capuz - costura de painel {j}',path,.0045,'hood',edge_mat)
    # A dark cloth mask sits well behind the brow; no exposed face or glowing eyes.
    face=oval('Rosto coberto pela mascara',(0,0,0),(.205,.175,.285),'hood',maskmat,32,20)
    face.location=head_center-forward*.008-Z*.015
    face.rotation_euler.z=math.atan2(forward.y,forward.x)+math.pi/2
    for z in [2.90,2.945]:
        curve(f'Dobra da mascara {z}',[head_center+forward*.135+lateral*u+Z*(z-3.01-.015*math.cos(u*12))
                                      for u in [-.17,-.11,-.05,0,.05,.11,.17]],.008,'hood',lining)

    # Layered cowl and shoulder mantle flow into a long folded cape.
    for layer in range(3):
        mesh=Mesh()
        for i in range(5):
            t0,t1=i/5,(i+1)/5
            def mantle(t,a):
                rx=.18+t*(.36-layer*.06)
                ry=.15+t*(.18-layer*.022)
                wave=.025*math.sin(a*7+t*3)
                return V((rx*math.cos(a),ry*math.sin(a),2.81-layer*.075-t*.26+wave-.055*max(0,-math.sin(a))))
            for j in range(56):
                a,b=j*math.tau/56,(j+1)*math.tau/56
                mesh.polygon([mantle(t0,a),mantle(t1,a),mantle(t1,b),mantle(t0,b)])
        obj(mesh,f'Gola drapeada - camada {layer+1}','cape',[cape_mat,lining],solid=.016)
        curve(f'Gola - dobra de borda {layer+1}',[mantle(1,j*math.tau/80) for j in range(80)],.008,'cape',hoodmat,True)
    cape_mesh=Mesh()
    def cape_point(t,u):
        x=(u-.5)*(1.0+.30*t)+.19*t*t
        y=.25+.19*t+.065*math.sin(u*math.pi*10+.25*t)*(.3+.7*t)+.10*math.sin(t*math.pi)
        hem=.055*(math.sin(u*49)+.6*math.sin(u*83))*t**10
        return V((x,y,2.52-2.16*t+hem+.06*math.cos(u*math.pi*2)*t))
    for i in range(22):
        for j in range(36):
            t0,t1=i/22,(i+1)/22
            u0,u1=j/36,(j+1)/36
            cape_mesh.polygon([cape_point(t0,u0),cape_point(t1,u0),cape_point(t1,u1),cape_point(t0,u1)])
    obj(cape_mesh,'Capa longa - dobras e barra desgastada','cape',[cape_mat,lining],solid=.018)
    for side in [.015,.985]:
        curve(f'Capa - bainha lateral {side}',[cape_point(i/48,side)+V((0,-.01,0)) for i in range(49)],.005,'cape',edge_mat)
    curve('Capa - barra desfiada',[cape_point(1,j/96)+V((0,-.01,0)) for j in range(97)],.006,'cape',edge_mat)

    # Legs, gathered cloth, knee guards and tall leather boots.
    for side,hip,knee,ankle in [('L',(-.18,0,1.47),(-.32,-.12,.82),(-.46,-.17,.22)),
                                ('R',(.18,.02,1.47),(.32,.12,.80),(.48,.15,.22))]:
        hip,knee,ankle=V(hip),V(knee),V(ankle)
        tube(f'Calca {side}',[hip,hip.lerp(knee,.28),hip.lerp(knee,.7),knee,knee.lerp(ankle,.18),knee.lerp(ankle,.6),ankle],
             [.19,.195,.155,.137,.143,.115,.09],'legs',cloth,24)
        start=knee.lerp(ankle,.16)
        tube(f'Bota - cano {side}',[ankle,ankle.lerp(start,.4),start],[.115,.135,.16],'legs',leather_dark,24)
        for i,t in enumerate([.19,.69,.94]):
            p=ankle.lerp(start,t)
            radius=.12+.045*t
            tube(f'Bota - tira {side} {i}',[p-Z*.025,p+Z*.025],[radius,radius],'armor',leather,24)
            buckle(f'Bota - fivela {side} {i}',p+V((-.02,-radius-.01,0)),.08,.052)
        # Feet point slightly outwards, with a distinct flat sole.
        direction=V((-.48,-.877,0)) if side=='L' else V((.16,-.987,0))
        center=ankle+direction*.09-Z*.075
        sole=oval(f'Bota - sola {side}',(0,0,0),(.145,.27,.0275),'legs',leather_dark)
        sole.location=V((center.x,center.y,.0125))
        sole.rotation_euler.z=math.atan2(direction.y,direction.x)+math.pi/2
        boot=oval(f'Bota - pe {side}',(0,0,0),(.139,.25,.115),'legs',leather)
        boot.location=V((center.x,center.y,.14))
        boot.rotation_euler.z=sole.rotation_euler.z
        knee_center=knee+V((0,-.115,0))
        oval(f'Joelheira de couro {side}',knee_center,(.13,.07,.16),'armor',leather_dark)
        oval(f'Joelheira - placa {side}',knee_center+V((0,-.048,0)),(.10,.034,.12),'armor',iron)

    # Belts, overlapping chest panels and hand-sewn shoulder strap.
    for i,z in enumerate([1.62,1.45]):
        points=[(.31*math.cos(j*math.tau/64),.225*math.sin(j*math.tau/64),z+.025*math.cos(j*math.tau/64)) for j in range(65)]
        for k in range(64):
            a,b=V(points[k]),V(points[k+1])
            mesh=Mesh()
            mesh.polygon([a-Z*.055,b-Z*.055,b+Z*.055,a+Z*.055])
            obj(mesh,f'Cinto {i+1} - segmento {k:02}','armor',[leather],solid=.012)
        buckle(f'Cinto - fivela principal {i}',(.03,-.246,z),.16,.11)
    for i in range(4):
        z=1.78+i*.115
        ribbon(f'Peitoral - placa {i}',[(-.24,-.222,z),(-.08,-.247,z+.02),(.10,-.244,z+.018),(.27,-.217,z-.01)],.105,'armor',leather_dark)
        for x in [-.215,.24]:oval(f'Rebite peitoral {i} {x}',(x,-.24,z),(.012,.008,.012),'armor',bronze,12,8)
    strap=[(-.26,-.237,1.54),(-.20,-.247,1.76),(-.04,-.266,1.97),(.12,-.267,2.18),(.31,-.215,2.41)]
    ribbon('Correia diagonal da aljava',strap,.105,'gear',leather)
    buckle('Correia - fivela no peito',(.015,-.281,2.04),.115,.13,'gear')
    for a,b in zip(strap,strap[1:]):
        a,b=V(a),V(b)
        direction=(b-a).normalized()
        side=V((direction.z,0,-direction.x))*.038
        for t in [.12,.25,.38,.51,.64,.77,.90]:
            for sign in [-1,1]:
                p=a.lerp(b,t)+side*sign+V((0,-.012,0))
                curve('Ponto de costura da correia',[p-direction*.008,p+direction*.008],.0024,'gear',thread)
    oval('Bolsa do cinto',(-.34,-.04,1.46),(.14,.12,.19),'gear',leather)
    oval('Aba da bolsa',(-.35,-.126,1.52),(.135,.045,.10),'gear',leather_dark)
    oval('Fecho da bolsa',(-.35,-.169,1.47),(.016,.009,.018),'gear',bronze,12,8)
    tube('Bainha lateral',[(.37,-.06,1.49),(.47,-.09,1.02)],[.055,.036],'gear',leather_dark,12,ellipse=.65)
    tube('Punho da faca reserva',[(.37,-.06,1.48),(.335,-.05,1.66)],.034,'gear',leather,12)

    def sample_curve(points,steps=10):
        points=[V(p) for p in points]
        result=[]
        for i in range(len(points)-1):
            p0,p1=points[max(0,i-1)],points[i]
            p2,p3=points[i+1],points[min(len(points)-1,i+2)]
            for j in range(steps):
                t=j/steps
                result.append(.5*((2*p1)+(-p0+p2)*t+(2*p0-5*p1+4*p2-p3)*t*t+(-p0+3*p1-3*p2+p3)*t*t*t))
        result.append(points[-1])
        return result

    # A bent laminated longbow, a taut V-shaped string and a nocked arrow.
    bow_points=[(-1.42,-.285,1.08),(-1.45,-.285,1.21),(-1.61,-.285,1.66),
                (-1.69,-.285,2.11),(-1.67,-.285,2.47),(-1.69,-.285,2.83),
                (-1.61,-.285,3.29),(-1.43,-.285,3.75),(-1.415,-.285,3.88)]
    bow_path=sample_curve(bow_points)
    bow_radii=[.026+.029*(1-min(1,abs(p.z-2.48)/1.4)) for p in bow_path]
    tube('Arco longo - madeira curvada',bow_path,bow_radii,'bow',bow_wood,12,ellipse=.72)
    curve('Arco - laminacao externa',[p+V((-.028,-.007,0)) for p in bow_path],.009,'bow',bow_edge)
    curve('Arco - filete interno',[p+V((.024,-.019,0)) for p in bow_path],.005,'bow',leather_dark)
    grip=[]
    for i in range(181):
        t=i/180
        a=t*math.tau*8
        grip.append((-1.672+.062*math.cos(a),-.285+.046*math.sin(a),2.335+t*.28))
    curve('Arco - empunhadura enrolada',grip,.009,'bow',leather)
    nock=V((.12,-.285,2.663))
    top,bottom=V(bow_points[-1]),V(bow_points[0])
    curve('Corda do arco - ramo superior',[top,nock],.0038,'bow',string_mat)
    curve('Corda do arco - ramo inferior',[nock,bottom],.0038,'bow',string_mat)
    for name,p in [('alto',top),('baixo',bottom)]:
        curve(f'Corda - laco {name}',[p+V((.012,.026,0)),p+V((-.028,0,.015)),p+V((.012,-.026,0))],.004,'bow',string_mat)

    def arrow(name,start,end,key='gear',quiver=False):
        start,end=V(start),V(end)
        direction=(end-start).normalized()
        shaft_end=end-direction*.11
        tube(name+' - haste',[start,shaft_end],[.008,.007],key,bow_wood,8)
        u=direction.cross(Z)
        if u.length<.1:u=direction.cross(V((0,1,0)))
        u.normalize()
        v=direction.cross(u).normalized()
        tip=Mesh()
        base=end-direction*.115
        middle=end-direction*.075
        for a in range(4):
            r0=u*math.cos(a*math.pi/2)+v*math.sin(a*math.pi/2)
            r1=u*math.cos((a+1)*math.pi/2)+v*math.sin((a+1)*math.pi/2)
            tip.polygon([end,middle+r0*.046,middle+r1*.046])
            tip.polygon([middle+r1*.046,middle+r0*.046,base])
        obj(tip,name+' - ponta de aco',key,[iron_edge],False)
        feathers=Mesh()
        for i in range(3):
            radial=u*math.cos(i*math.tau/3)+v*math.sin(i*math.tau/3)
            a=start+direction*.055
            b=start+direction*.25
            feathers.polygon([a,b,b+radial*.018,a+direction*.045+radial*.062],i%2)
            curve(name+f' - nervura {i}',[a,b+radial*.018],.002,key,feather_edge)
            for j in range(5):
                t=.1+j*.16
                p=a.lerp(b,t)
                curve(name+f' - barba {i}-{j}',[p,p-direction*.025+radial*(.045*(1-t)+.01)],.0015,key,feather_edge)
        obj(feathers,name+' - penas',key,[feather,feather_edge],False,solid=.002)
        tube(name+' - encaixe',[start-direction*.012,start+direction*.035],.012,key,leather_dark,8)

    arrow('Flecha armada',nock,(-2.10,-.285,2.663),'bow')

    # The open quiver is behind the shoulder, with distinct feathered arrows.
    qa,qb=V((.29,.32,1.80)),V((.45,.39,2.72))
    qdir=(qb-qa).normalized()
    tube('Aljava - corpo de couro',[qa,qa.lerp(qb,.1),qa.lerp(qb,.9),qb],[.12,.15,.158,.16],'gear',leather,28,caps=False)
    tube('Aljava - interior escuro',[qb-qdir*.13,qb-qdir*.015],[.144,.145],'gear',lining,28,caps=False)
    for i,t in enumerate([.03,.15,.86,.99]):
        p=qa.lerp(qb,t)
        tube(f'Aljava - aro {i}',[p-qdir*.025,p+qdir*.025],[.164,.164],'gear',leather_dark,28,caps=False)
    for i in range(6):
        a=i*2.399
        offset=V((math.cos(a)*.075,math.sin(a)*.07,0))
        tip=qa+qdir*.18+offset
        feathers_at=qb+qdir*(.52+(i%3)*.08)+offset+V(((i-2.5)*.036,0,0))
        arrow(f'Flecha na aljava {i+1}',feathers_at,tip)
    curve('Aljava - costura longitudinal',[qa+V((.13,-.025,0)),qb+V((.14,-.025,0))],.004,'gear',thread)

    # Raised leather seams and small worn details stay readable at hero scale.
    for side in [-1,1]:
        curve(f'Tunica - costura lateral {side}',[(side*.28,-.13,1.58),(side*.28,-.155,1.8),
                                               (side*.34,-.18,2.1),(side*.38,-.13,2.32)],.005,'body',edge_mat)
    clasp=V((-.26,-.248,2.57))
    oval('Fecho da capa - bronze',clasp,(.044,.015,.045),'armor',bronze,20,12)
    curve('Fecho da capa - presilha',[clasp+V((-.05,0,0)),clasp+V((0,-.015,-.025)),clasp+V((.05,0,0))],.005,'armor',iron_edge)

    # Display stage: damp stone, moss and a softly lit forest behind the model.
    earth=material('Terra umida',(24,29,26),.98,texture=True)
    rock=material('Pedra da base',(43,50,48),.92,texture=True)
    moss=material('Musgo discreto',(38,52,32),.96)
    bark=material('Troncos ao fundo',(29,42,41),.96)
    tree_far=material('Troncos na neblina',(49,70,69),.98)
    base_mesh=Mesh()
    base_mesh.branch((0,0,-.14),(0,0,-.015),1.14,1.19,sides=64)
    obj(base_mesh,'Base - pedra oval',stage,[rock],False,.035)
    ground_mesh=Mesh()
    ground_mesh.box((0,0,-.26),(200,200,.12))
    obj(ground_mesh,'Solo do estudio',stage,[earth],False)
    for i in range(16):
        a=RNG.random()*math.tau
        r=RNG.uniform(.83,1.15)
        x,y=r*math.cos(a),r*math.sin(a)
        oval(f'Musgo da base {i}',(x,y,-.005),(RNG.uniform(.08,.18),RNG.uniform(.04,.10),.018),stage,moss,12,8)
    for i in range(15):
        x=-6+i*.85
        y=3.4+(i%3)*1.6
        trunk=Mesh()
        start=V((x,y,-.22))
        top=V((x+RNG.uniform(-.45,.45),y+.35,6.0))
        trunk.branch(start,top,RNG.uniform(.09,.17),.055,sides=9)
        trunk.branch(start.lerp(top,.58),top+V((.9,-.1,.2)),.075,.028,sides=7)
        obj(trunk,f'Tronco de fundo {i}',stage,[tree_far if i%3 else bark])

    world=bpy.data.worlds.new('Arqueiro - Floresta noturna')
    world.use_nodes=True
    world.node_tree.nodes['Background'].inputs['Color'].default_value=(.09,.14,.15,1)
    world.node_tree.nodes['Background'].inputs['Strength'].default_value=.22
    scene.world=world
    def light(name,location,target,energy,color,size):
        data=bpy.data.lights.new(name,'AREA')
        data.energy=energy
        data.color=color
        data.shape='DISK'
        data.size=size
        ob=bpy.data.objects.new(name,data)
        stage.objects.link(ob)
        ob.location=location
        ob.rotation_euler=(V(target)-ob.location).to_track_quat('-Z','Y').to_euler()
    light('Arqueiro - Luz principal',(-3.5,-4.5,6),(-.3,0,2.2),650,(.72,.84,.88),3.8)
    light('Arqueiro - Preenchimento',(3,-3,3.5),(0,0,2),180,(.77,.82,.78),3)
    light('Arqueiro - Recorte',(2,3,5),(0,0,2),950,(.57,.75,.78),2.7)
    light('Arqueiro - Reflexo quente',(-2,2,2.8),(0,0,1.8),180,(1,.48,.23),2)
    light('Arqueiro - Floresta ao fundo',(-2,2,5),(0,6,2.5),900,(.54,.71,.73),5)

    def camera(name,location,target,scale,portrait=False):
        data=bpy.data.cameras.new(name)
        ob=bpy.data.objects.new(name,data)
        stage.objects.link(ob)
        ob.location=location
        ob.rotation_euler=(V(target)-ob.location).to_track_quat('-Z','Y').to_euler()
        data.type='ORTHO'
        data.ortho_scale=scale
        data.passepartout_alpha=1
        data.clip_end=150
        data.dof.use_dof=True
        data.dof.focus_distance=(V(target)-ob.location).length
        data.dof.aperture_fstop=12 if not portrait else 9
        return ob
    full=camera('Camera - Arqueiro corpo inteiro',(-4.7,-8.5,3.7),(-.40,0,1.91),4.45)
    camera('Camera - Arqueiro retrato',(-4.7,-8.5,4.0),(-.37,0,2.69),2.9,True)
    scene.camera=full
    scene.render.engine='CYCLES'
    scene.cycles.samples=48
    scene.cycles.use_denoising=True
    scene.cycles.preview_samples=8
    scene.render.resolution_x=1000
    scene.render.resolution_y=1200
    scene.render.resolution_percentage=100
    scene.render.image_settings.file_format='PNG'
    scene.render.image_settings.color_mode='RGBA'
    scene.render.film_transparent=False
    scene.view_settings.view_transform='AgX'
    scene.view_settings.look='AgX - Medium High Contrast'
    scene.view_settings.exposure=.15
    scene.render.filepath=str(ROOT/'arqueiro_heroi_dark.png')
    scene['referencia']='Captura_de_tela_20260908_231012.png: arqueiro encapuzado com arco tensionado.'
    scene['modelo']='Vigia Sombrio - personagem completo em pose de disparo, com equipamento separado.'
    scene['nota']='Pernas e botas completam a parte inferior nao visivel na referencia.'
    for area in bpy.context.screen.areas:
        if area.type=='VIEW_3D':
            space=area.spaces.active
            space.shading.type='SOLID'
            space.shading.color_type='MATERIAL'
            space.shading.show_shadows=False
            space.shading.show_cavity=False
            space.shading.show_specular_highlight=False
            space.overlay.show_overlays=False
            space.region_3d.view_perspective='CAMERA'
            space.region_3d.view_camera_zoom=12
    bpy.context.view_layer.objects.active=root
    root.select_set(True)
    bpy.context.view_layer.update()
    print('ARCHER_READY',scene.name,len(model.all_objects),sum(len(o.data.polygons) for o in model.all_objects if o.type=='MESH'))
    return locals()


if __name__=='__main__':
    build_archer()
    ROOT.mkdir(parents=True,exist_ok=True)
    bpy.ops.wm.save_as_mainfile(filepath=str(ROOT/'arqueiro_heroi_dark.blend'))

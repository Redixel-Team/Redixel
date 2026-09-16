"""Revise the saved animated hero: larger black recurve bow and a clearer gait/shot.

blender -b <saved animated hero.blend> --python-exit-code 1 --python <this file>
Keeps the existing body, rig and source file; writes arqueiro_heroi_recurvo.blend.
"""

import hashlib
import json
import math
import runpy
from pathlib import Path

import bpy
from mathutils import Matrix, Vector

HERE = Path(__file__).resolve().parent
H = runpy.run_path(str(HERE / 'animate_goblin_units.py'), run_name='animation_helpers')
Pose, bake, activate, bind_mesh = [H[n] for n in ('Pose', 'bake', 'activate', 'bind_mesh')]
rotation, smooth, interpolate = [H[n] for n in ('rotation', 'smooth', 'interpolate')]
V = Vector
I = Matrix.Identity(3)
GRIP = V((-1.555, -.255, 2.475))
NOCK = V((.12, -.285, 2.663))
BOW_CENTER = V((-1.67, -.285, 2.48))
BOW_SCALE = 1.24
SPAN = 1.40 * BOW_SCALE
STRIDE = .68
STANCE = .48


def profile(u):
    # Target is -X. The tips remain behind the grip, with a small forward
    # hook at the ends. The old deep hook put the string through the limbs.
    return interpolate([(0, -1.67), (.16, -1.69), (.38, -1.56), (.62, -1.31),
                        (.77, -1.11), (.90, -1.05), (1.02, -1.15)], u)[0]


def restring_bow(scene, rig):
    """Route the string along the recurved ends, on the archer's side."""
    wood = scene.objects['Arco longo - madeira curvada']
    for side, sign, name in [('BowTop', 1, 'Corda do arco - ramo superior'),
                             ('BowBottom', -1, 'Corda do arco - ramo inferior')]:
        ob = scene.objects[name]
        # Support points follow the existing belly surface, including its
        # small sculpted irregularities. Bone weights match the limb below.
        points = []
        for i in range(13):
            u = .82 + .18 * i / 12
            z = BOW_CENTER.z + sign * SPAN * u
            nearby = [v.co.x for v in wood.data.vertices if abs(v.co.z - z) < .035]
            points.append(V((max(nearby) + .009, BOW_CENTER.y, z)))
        mesh = H['Mesh']()
        vertices_weights = []
        contact_weight = .82 ** 2
        def segment(a, b, wa, wb):
            before = len(mesh.vertices)
            mesh.branch(a, b, .005, .005, sides=6)
            for v in mesh.vertices[before:]:
                vertices_weights.append(wa if (v - a).length < (v - b).length else wb)
        start_weights = {'BowString': 1}
        contact_weights = {'Hand.L': 1 - contact_weight, side: contact_weight}
        segment(NOCK, points[0], start_weights, contact_weights)
        for i, (a, b) in enumerate(zip(points, points[1:])):
            wa = (.82 + .18 * i / 12) ** 2
            wb = (.82 + .18 * (i + 1) / 12) ** 2
            segment(a, b, {'Hand.L': 1 - wa, side: wa}, {'Hand.L': 1 - wb, side: wb})
        data = bpy.data.meshes.new(name + ' - curva corrigida')
        data.from_pydata(mesh.vertices, [], mesh.faces)
        for mat in ob.data.materials:
            data.materials.append(mat)
        ob.data = data
        ob.vertex_groups.clear()
        groups = {n: ob.vertex_groups.new(name=n) for n in ['Hand.L', side, 'BowString']}
        for index, weights in enumerate(vertices_weights):
            for bone, weight in weights.items():
                if weight > 1e-6:
                    groups[bone].add([index], weight, 'REPLACE')
        for polygon in data.polygons:
            polygon.use_smooth = True


def original_profile(t):
    return interpolate([(0, -1.67), (.257, -1.69), (.579, -1.61), (.907, -1.43), (1.01, -1.415)], abs(t))[0]


def warp_bow(point):
    p = V(point)
    t = (p.z - BOW_CENTER.z) / 1.40
    p.x += profile(abs(t)) - original_profile(t)
    p.y = BOW_CENTER.y + (p.y - BOW_CENTER.y) * 1.18
    p.z = BOW_CENTER.z + (p.z - BOW_CENTER.z) * BOW_SCALE
    return p


def revise_bow(scene, rig):
    limb_names = ['Arco longo - madeira curvada', 'Arco - laminacao externa', 'Arco - filete interno']
    original = scene.objects[limb_names[0]]
    old_height = max(v.co.z for v in original.data.vertices) - min(v.co.z for v in original.data.vertices)
    old_tips = {name: rig.data.bones[name].head_local.copy() for name in ['BowTop', 'BowBottom']}
    tips = {name: warp_bow(point) for name, point in old_tips.items()}
    for name in limb_names:
        ob = scene.objects[name]
        ob.data = ob.data.copy()
        for vertex in ob.data.vertices:
            vertex.co = warp_bow(vertex.co)
        ob.data.update()
        def weights(p):
            weight = min(1, abs(p.z - BOW_CENTER.z) / SPAN) ** 2
            return {'Hand.L': 1 - weight, 'BowTop' if p.z > BOW_CENTER.z else 'BowBottom': weight}
        bind_mesh(ob, rig, weights)
    for side, name in [('BowTop', 'Corda do arco - ramo superior'), ('BowBottom', 'Corda do arco - ramo inferior')]:
        ob = scene.objects[name]
        ob.data = ob.data.copy()
        axis = old_tips[side] - NOCK
        for vertex in ob.data.vertices:
            t = max(0, min(1, (vertex.co - NOCK).dot(axis) / axis.length_squared))
            vertex.co += (tips[side] - old_tips[side]) * t
        bind_mesh(ob, rig, lambda p, side=side: {
            side: max(0, min(1, (p - NOCK).dot(tips[side] - NOCK) / (tips[side] - NOCK).length_squared)),
            'BowString': 1 - max(0, min(1, (p - NOCK).dot(tips[side] - NOCK) / (tips[side] - NOCK).length_squared)),
        })
    for name in ['Corda - laco alto', 'Corda - laco baixo']:
        ob = scene.objects[name]
        ob.data = ob.data.copy()
        for vertex in ob.data.vertices:
            vertex.co = warp_bow(vertex.co)
    rig.hide_set(False)
    for ob in scene.objects:
        ob.select_set(False)
    rig.select_set(True)
    bpy.context.view_layer.objects.active = rig
    bpy.ops.object.mode_set(mode='EDIT')
    for name, tip in tips.items():
        bone = rig.data.edit_bones[name]
        delta = tip - bone.head
        bone.head += delta
        bone.tail += delta
    bpy.ops.object.mode_set(mode='OBJECT')
    restring_bow(scene, rig)

    # Share copies only among bow parts; quiver, arrows and costume keep their materials.
    colors = {'wood': (10, 11, 14), 'edge': (24, 26, 30), 'grip': (13, 12, 14), 'string': (61, 59, 53)}
    copies = {}
    for name, role in [(limb_names[0], 'wood'), (limb_names[1], 'edge'), (limb_names[2], 'grip'),
                       ('Arco - empunhadura enrolada', 'grip'), ('Corda do arco - ramo superior', 'string'),
                       ('Corda do arco - ramo inferior', 'string'), ('Corda - laco alto', 'string'), ('Corda - laco baixo', 'string')]:
        ob = scene.objects[name]
        if role not in copies:
            mat = ob.data.materials[0].copy()
            mat.name = 'Heroi - Arco negro - ' + role
            rgb = [c / 255 for c in colors[role]]
            mat.diffuse_color = (*rgb, 1)
            bsdf = mat.node_tree.nodes.get('Principled BSDF')
            linear = lambda c: c / 12.92 if c <= .04045 else ((c + .055) / 1.055) ** 2.4
            bsdf.inputs['Base Color'].default_value = (*map(linear, rgb), 1)
            bsdf.inputs['Roughness'].default_value = .62 if role == 'wood' else .75
            bsdf.inputs['Metallic'].default_value = .08 if role == 'edge' else 0
            bsdf.inputs['Specular IOR Level'].default_value = .32 if role == 'wood' else .2
            copies[role] = mat
        ob.data.materials[0] = copies[role]
    new_height = max(v.co.z for v in original.data.vertices) - min(v.co.z for v in original.data.vertices)
    assert new_height / old_height > 1.23
    return {'old_height': old_height, 'new_height': new_height, 'scale': new_height / old_height,
            'tips': {name: list(point) for name, point in tips.items()}}


def make_pose(rig, aim=0, draw=0, breath=0, phase=None, shot_frame=None):
    running = phase is not None
    phase = phase or 0
    p = Pose(rig)
    cycle = phase * math.tau
    sway = math.sin(cycle)
    body_yaw = -.76 + .06 * sway if running else .055 * draw
    # Lower on contact, rise over the supporting leg at mid-stance.
    drop = -.22 - .045 * math.cos(cycle * 2) if running else -.032 * aim + .008 * breath
    offset = V((-.09 if running else -.02 * aim + .025 * draw, .017 * sway if running else 0, drop))
    p.place('Pelvis', rig.data.bones['Pelvis'].head_local + offset, rotation(z=body_yaw))
    p.child('Spine', rotation(y=-.12 if running else -.018 * aim, z=body_yaw - (.03 * sway if running else 0)))
    p.child('Chest', rotation(y=-.16 if running else -.028 * aim + .004 * breath,
                              z=body_yaw - (.085 * sway if running else .035 * draw)))
    p.child('Neck', rotation(y=-.06 if running else -.012 * aim, z=-.13 if running else 0))
    p.child('Head', rotation(y=.018 * math.sin(cycle - .5) if running else .007 * breath,
                             z=-.04 + .018 * math.sin(cycle - .7) if running else -.012 * draw))
    for side, off in [('L', 0), ('R', .5)]:
        b = rig.data.bones['Thigh.' + side]
        hip = p.heads['Pelvis'] + p.rots['Pelvis'] @ (b.head_local - rig.data.bones['Pelvis'].head_local)
        foot = rig.data.bones['Foot.' + side].head_local.copy()
        foot_rot = I
        if running:
            t = (phase + off) % 1
            if t < STANCE:
                travel, lift, pitch = -STRIDE + 2 * STRIDE * t / STANCE, 0, 0
            else:
                swing = (t - STANCE) / (1 - STANCE)
                travel = STRIDE - 2 * STRIDE * smooth(swing)
                lift = .43 * math.sin(math.pi * swing) ** 1.2
                pitch = .22 * math.sin(math.tau * swing)
            hip_x = -.18 * math.cos(.76) if side == 'L' else .18 * math.cos(.76)
            foot = V((travel + hip_x, .20 if side == 'L' else -.20, .22 + lift))
            foot_rot = rotation(y=pitch) @ rotation(z=-1.07 if side == 'L' else -1.72)
        actual = p.limb('Thigh.' + side, 'Shin.' + side, 'Foot.' + side, hip, foot, (-1, 0, .02), foot_rot, .999999)
        if running and t < STANCE:
            assert (actual - foot).length < .001, ('Stance leg cannot reach the ground', side, phase, tuple(actual - foot))

    wrist_idle = V((-.86, -.46, 2.00 + drop))
    wrist = wrist_idle.lerp(GRIP + V((0, 0, drop)), aim)
    hand_rot = rotation(y=-.20 * (1 - aim))
    if running:
        wrist = V((-.89 + .045 * sway, -.49, 2.27 + drop + .025 * math.sin(cycle - .5)))
        hand_rot = rotation(y=-.39 + .025 * sway, z=-.06)
    p.arm('L', wrist, hand_rot, (-.1, -.8, -.20))
    for name in ['BowTop', 'BowBottom', 'BowString']:
        p.child(name)
    for name, dz in [('BowTop', .045), ('BowBottom', -.045)]:
        p.place(name, p.heads[name] + hand_rot @ V((-.12 * (1 - draw), 0, dz * (1 - draw))), hand_rot)
    nock = p.heads['BowString'] + hand_rot @ V((-1.08 * (1 - draw), 0, 0))
    p.place('BowString', nock, hand_rot)
    right_idle = V((.33, -.37, 1.99 + drop))
    right = right_idle.copy()
    right_rot = rotation(y=.25)
    if running:
        right = V((.05 - .38 * math.cos(cycle), -.42, 2.08 + drop + .07 * math.sin(cycle)))
        right_rot = rotation(y=.2, z=-.25)
    elif shot_frame is not None:
        f = shot_frame
        quiver = V((.48, .38, 2.96))
        grip_offset = hand_rot @ (rig.data.bones['Hand.R'].head_local - NOCK)
        if f <= 5:
            right = right_idle.lerp(quiver, smooth((f - 1) / 4))
        elif f < 10:
            right = quiver.lerp(nock + grip_offset, smooth((f - 5) / 5))
        elif f <= 21:
            right = nock + grip_offset
        else:
            # The hand continues backwards after release. Only the string
            # snaps forward; the old motion dragged the hand along with it.
            right = V((.235, -.255, 2.67)) + V((.12, .06, .018)) * math.sin(min(1, (f - 21) / 7) * math.pi / 2)
            right = right.lerp(right_idle, smooth((f - 27) / 10))
        right_rot = rotation(y=.25 * (1 - aim), z=-.04 * aim)
    p.arm('R', right, right_rot, (.8, -.18, .15))

    if running:
        p.child('CapeTop', rotation(y=-.10 + .025 * math.sin(cycle - .5), x=.10, z=body_yaw))
        p.child('CapeHem', rotation(y=-.27 + .07 * math.sin(cycle - 1.0), x=.16,
                                    z=body_yaw + .06 * math.sin(cycle - .9)))
    else:
        p.child('CapeTop', rotation(y=-.025 * aim + .012 * breath, x=.02 * breath))
        p.child('CapeHem', rotation(y=-.06 * aim + .022 * breath, x=.025 * breath, z=.025 * draw))
    arrow_pos, arrow_rot = NOCK, I * .0001
    if shot_frame is not None and 5 <= shot_frame <= 21:
        if shot_frame < 10:
            arrow_pos = p.heads['Hand.R'] - (rig.data.bones['Hand.R'].head_local - NOCK)
            arrow_rot = rotation(y=-1.2 * (1 - smooth((shot_frame - 5) / 5)))
        else:
            arrow_pos, arrow_rot = nock, hand_rot
    p.place('Arrow', arrow_pos, arrow_rot)
    return p


def main():
    source = Path(bpy.data.filepath)
    source_hash = hashlib.sha256(source.read_bytes()).hexdigest()
    scene = bpy.data.scenes['Heroi Arqueiro - Vigia Sombrio']
    bpy.context.window.scene = scene
    rig, root = scene.objects['Heroi - Rig'], scene.objects['Heroi Arqueiro']
    assert not root.get('recurve_revision'), 'This is a one-time migration; use the saved refined file for later edits.'
    for track in list(rig.animation_data.nla_tracks):
        rig.animation_data.nla_tracks.remove(track)
    activate(rig, None)
    for name in ['Heroi_Idle', 'Heroi_Run', 'Heroi_Attack']:
        bpy.data.actions.remove(bpy.data.actions[name])
    bow_report = revise_bow(scene, rig)
    idle = bake(scene, rig, 'Heroi_Idle', 61,
                lambda f: make_pose(rig, breath=math.sin((f - 1) / 60 * math.tau)), [])
    idle['loop'] = True
    run = bake(scene, rig, 'Heroi_Run', 33, lambda f: make_pose(rig, phase=(f - 1) / 32),
               [('Contato esquerdo', 1), ('Contato direito', 17)])
    # Used by the sprite exporter instead of inferring a stance from two
    # arbitrary frames which may span the airborne part of the gait.
    run['stride_height_ratio'] = (2 * STRIDE / STANCE) / 3.42
    keys = [(1, 0, 0), (5, .18, 0), (10, .88, 0), (13, 1, .35),
            (19, 1, 1), (21, 1, 1), (22, 1, .07), (23, 1, -.02),
            (25, 1, .025), (28, .88, 0), (32, .32, 0), (37, 0, 0)]
    attack = bake(scene, rig, 'Heroi_Attack', 37,
                  lambda f: make_pose(rig, *interpolate(keys, f), shot_frame=f),
                  [('Buscar flecha', 5), ('Encaixar flecha', 10), ('Tensao maxima', 19),
                   ('Disparo', 22), ('Recuperar', 28)])
    clips = H['validate'](scene, rig, [idle, run, attack])
    clips[0]['loop'] = True
    # All the body's original meshes and weights survive this focused edit.
    activate(rig, None)
    track = rig.animation_data.nla_tracks.new()
    track.name = 'Demonstracao - arco recurvo'
    scene.timeline_markers.clear()
    for name, frame, action, repeat in [('ESPERA', 1, idle, 1), ('CORRIDA', 66, run, 2), ('DISPARO', 141, attack, 2)]:
        strip = track.strips.new(name, frame, action)
        strip.repeat, strip.extrapolation = repeat, 'HOLD_FORWARD'
        scene.timeline_markers.new(name, frame=frame)
    scene.frame_start, scene.frame_end = 1, 213
    scene.render.fps = 30
    scene.sync_mode = 'FRAME_DROP'
    scene.camera = scene.objects['Camera - Arqueiro corpo inteiro']
    scene.camera.data.ortho_scale = 4.2
    scene.frame_set(1)
    rig.hide_set(True)
    root['recurve_revision'] = 1
    root['bow_orientation_revision'] = 1
    root['animacoes'] = 'Idle, Run, Attack; arco negro 24% maior, pontas recurvas; espera 1, corrida 66, disparos 141.'
    output = source.with_name('arqueiro_heroi_recurvo.blend')
    bpy.ops.wm.save_as_mainfile(filepath=str(output))
    report = {'input_sha256': source_hash, 'bow': bow_report, 'clips': clips}
    output.with_suffix('.json').write_text(json.dumps(report, indent=2) + '\n')
    assert hashlib.sha256(source.read_bytes()).hexdigest() == source_hash
    print('HERO_REFINED', json.dumps(report), flush=True)


if __name__ == '__main__':
    main()

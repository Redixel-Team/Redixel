"""Rig the saved hooded hero and author Idle, Run and Attack without rebuilding it.

blender -b ~/Documents/Blender/arqueiro_heroi_dark.blend --python-exit-code 1 --python <this file>
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
Pose, add_bones, bind_mesh, bake, activate = [H[n] for n in ('Pose', 'add_bones', 'bind_mesh', 'bake', 'activate')]
rotation, smooth, interpolate = [H[n] for n in ('rotation', 'smooth', 'interpolate')]
V = Vector
I = Matrix.Identity(3)
GRIP = V((-1.555, -.255, 2.475))
NOCK = V((.12, -.285, 2.663))


def main():
    source = Path(bpy.data.filepath)
    digest = hashlib.sha256(source.read_bytes()).hexdigest()
    scene = bpy.data.scenes['Heroi Arqueiro - Vigia Sombrio']
    bpy.context.window.scene = scene
    model = bpy.data.collections['ARQUEIRO - Modelo']
    root = bpy.data.objects['Heroi Arqueiro']
    assert not any(o.type == 'ARMATURE' for o in model.all_objects), 'Use the static source once; export saved edits afterwards.'
    # Bake curves and garment thickness in the new file before skinning.
    objects = [o for o in model.all_objects if o.type in {'MESH', 'CURVE'}]
    for ob in scene.objects:
        ob.select_set(False)
    for ob in objects:
        ob.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]
    bpy.ops.object.convert(target='MESH')
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    rig = bpy.data.objects.new('Heroi - Rig', bpy.data.armatures.new('Esqueleto do Vigia Sombrio'))
    model.objects.link(rig)
    rig.parent = root
    definitions = [
        ('Root', (0, 0, 0), (0, 0, .25), None),
        ('Pelvis', (0, 0, 1.47), (0, 0, 1.78), 'Root'),
        ('Spine', (0, 0, 1.78), (0, 0, 2.19), 'Pelvis'),
        ('Chest', (0, 0, 2.19), (0, 0, 2.57), 'Spine'),
        ('Neck', (0, 0, 2.57), (0, 0, 2.84), 'Chest'),
        ('Head', (0, 0, 2.84), (0, 0, 3.35), 'Neck'),
        ('CapeTop', (0, .30, 2.48), (0, .43, 1.45), 'Chest'),
        ('CapeHem', (0, .43, 1.45), (.16, .48, .38), 'CapeTop'),
        ('BowTop', (-1.415, -.285, 3.88), (-1.415, -.285, 4.08), 'Hand.L'),
        ('BowBottom', (-1.42, -.285, 1.08), (-1.42, -.285, 1.28), 'Hand.L'),
        ('BowString', NOCK, NOCK + V((0, 0, .2)), 'Hand.L'),
        ('Arrow', NOCK, NOCK + V((-.3, 0, 0)), 'Root'),
    ]
    for side, shoulder, elbow, wrist, hip, knee, ankle in [
        ('L', (-.34, 0, 2.42), (-.94, -.14, 2.44), GRIP,
         (-.18, 0, 1.47), (-.32, -.12, .82), (-.46, -.17, .22)),
        ('R', (.34, 0, 2.42), (.91, .01, 2.60), (.235, -.255, 2.67),
         (.18, .02, 1.47), (.32, .12, .80), (.48, .15, .22)),
    ]:
        definitions.extend([
            ('Clavicle.' + side, (0, 0, 2.42), shoulder, 'Chest'),
            ('UpperArm.' + side, shoulder, elbow, 'Clavicle.' + side),
            ('Forearm.' + side, elbow, wrist, 'UpperArm.' + side),
            ('Hand.' + side, wrist, V(wrist) + V((-.16, 0, 0)), 'Forearm.' + side),
            ('Thigh.' + side, hip, knee, 'Pelvis'),
            ('Shin.' + side, knee, ankle, 'Thigh.' + side),
            ('Foot.' + side, ankle, V(ankle) + V((-.05, -.20, -.08)), 'Shin.' + side),
        ])
    add_bones(rig, [d for d in definitions if not d[0].startswith('Bow')])
    add_bones(rig, [d for d in definitions if d[0].startswith('Bow')])

    def axial(point):
        z = point.z
        if z < 1.60:
            return {'Pelvis': 1}
        if z < 2.08:
            t = smooth((z - 1.60) / .48)
            return {'Pelvis': 1 - t, 'Spine': t}
        t = smooth((z - 2.08) / .35)
        return {'Spine': 1 - t, 'Chest': t}

    def limb_weights(p, upper, lower):
        b = rig.data.bones[upper]
        t = (p - b.tail_local).dot((b.tail_local - b.head_local).normalized())
        blend = smooth((t + .12) / .24)
        return {upper: 1 - blend, lower: blend}

    for ob in objects:
        name = ob.name
        side = 'L' if ' L' in name else 'R'
        bone = None
        if name.startswith('Flecha armada'):
            bone = 'Arrow'
        elif name.startswith('Corda do arco'):
            top = 'superior' in name
            tip = V((-1.415, -.285, 3.88) if top else (-1.42, -.285, 1.08))
            def weights(p, tip=tip, top=top):
                t = max(0, min(1, (p - NOCK).dot(tip - NOCK) / (tip - NOCK).length_squared))
                return {'BowString': 1 - t, 'BowTop' if top else 'BowBottom': t}
        elif name.startswith('Corda - laco'):
            bone = 'BowTop' if 'alto' in name else 'BowBottom'
        elif name.startswith(('Arco longo', 'Arco - laminacao', 'Arco - filete')):
            def weights(p):
                t = min(1, abs(p.z - GRIP.z) / 1.4) ** 2
                return {'Hand.L': 1 - t, 'BowTop' if p.z > GRIP.z else 'BowBottom': t}
        elif name.startswith('Arco - empunhadura'):
            bone = 'Hand.L'
        elif name.startswith(('Capuz', 'Rosto', 'Dobra da mascara')):
            bone = 'Head'
        elif name.startswith(('Capa longa', 'Capa -')):
            def weights(p):
                t = smooth((2.08 - p.z) / 1.30)
                return {'CapeTop': 1 - t, 'CapeHem': t}
        elif name.startswith(('Gola', 'Fecho da capa')):
            bone = 'Chest'
        elif name.startswith('Manga do braco'):
            side = 'L' if 'do arco' in name else 'R'
            weights = lambda p, s=side: limb_weights(p, 'UpperArm.' + s, 'Forearm.' + s)
        elif name.startswith('Luva'):
            bone = 'Hand.L' if 'arco' in name else 'Hand.R'
        elif name.startswith(('Bracadeira', 'Tira da bracadeira', 'Rebite da bracadeira', 'Placa de antebraco')):
            bone = 'Forearm.' + side
        elif name.startswith('Dobra do cotovelo'):
            weights = lambda p, s=side: limb_weights(p, 'UpperArm.' + s, 'Forearm.' + s)
        elif name.startswith('Calca'):
            weights = lambda p, s=side: limb_weights(p, 'Thigh.' + s, 'Shin.' + s)
        elif name.startswith(('Bota - sola', 'Bota - pe')):
            bone = 'Foot.' + side
        elif name.startswith(('Bota', 'Joelheira')):
            bone = 'Shin.' + side
        elif name.startswith(('Aljava', 'Flecha da aljava')):
            bone = 'Chest'
        elif name.startswith(('Bolsa', 'Aba da bolsa', 'Fecho da bolsa', 'Bainha', 'Punhal')):
            bone = 'Pelvis'
        else:
            weights = axial
        bind_mesh(ob, rig, (lambda p, b=bone: {b: 1}) if bone else weights)

    def pose(aim=0, draw=0, phase=0, running=False, breath=0, arrow=False, recoil=0):
        p = Pose(rig)
        bob = (-.12 + .045 * math.cos(phase * 2)) if running else -.012 * aim + .008 * breath
        p.place('Pelvis', rig.data.bones['Pelvis'].head_local + V((-.045 if running else 0, 0, bob)), I)
        for name in ['Spine', 'Chest', 'Neck', 'Head']:
            p.child(name, rotation(y=-.065 if running else -.018 * aim))
        for side, offset in [('L', 0), ('R', math.pi)]:
            b = rig.data.bones['Thigh.' + side]
            hip = p.heads['Pelvis'] + b.head_local - rig.data.bones['Pelvis'].head_local
            foot = rig.data.bones['Foot.' + side].head_local.copy()
            if running:
                foot.x += -.36 * math.cos(phase + offset)
                foot.z += .26 * max(0, -math.sin(phase + offset))
            p.limb('Thigh.' + side, 'Shin.' + side, 'Foot.' + side, hip, foot, (-1, 0, .02), I, .999999)
        # The source is sculpted at full draw. Idle lowers the complete bow;
        # the draw control relaxes its string and limbs relative to that pose.
        wrist = V((-.83, -.46, 1.82 + bob)).lerp(GRIP + V((0, 0, bob)), aim)
        if running:
            wrist.x += .055 * math.sin(phase)
        hand_rot = rotation(y=-.19 * (1 - aim))
        p.arm('L', wrist, hand_rot, (-.1, -.7, -.25))
        for name in ['BowTop', 'BowBottom', 'BowString']:
            p.child(name)
        for name, dz in [('BowTop', .065), ('BowBottom', -.065)]:
            p.place(name, p.heads[name] + hand_rot @ V((-.10 * (1 - draw), 0, dz * (1 - draw))), hand_rot)
        nock = p.heads['BowString'] + hand_rot @ V((-1.53 * (1 - draw), 0, 0))
        p.place('BowString', nock, hand_rot)
        right_idle = V((.34, -.38, 1.91 + bob))
        if running:
            right_idle.x += -.23 * math.sin(phase)
            right_idle.z += .07 * math.cos(phase)
        right = right_idle.lerp(nock + hand_rot @ (rig.data.bones['Hand.R'].head_local - NOCK) + V((.10 * recoil, 0, 0)), aim)
        p.arm('R', right, rotation(y=.28 * (1 - aim)), (.8, -.2, .16))
        p.child('CapeTop', rotation(y=.12 if running else .012 * breath, x=.13 if running else .015 * breath))
        p.child('CapeHem', rotation(y=.20 + .07 * math.sin(phase + .6) if running else .025 * breath,
                                   x=.25 + .045 * math.sin(phase) if running else .025 * breath))
        p.place('Arrow', nock if arrow else NOCK, hand_rot if arrow else I * .0001)
        return p

    idle = bake(scene, rig, 'Heroi_Idle', 61, lambda f: pose(breath=math.sin((f - 1) / 60 * math.tau)), [])
    idle['loop'] = True
    run = bake(scene, rig, 'Heroi_Run', 25, lambda f: pose(phase=(f - 1) / 24 * math.tau, running=True), [('Passo esquerdo', 1), ('Passo direito', 13)])
    keys = [(1, 0, 0), (5, .55, 0), (9, 1, .12), (16, 1, 1), (17, 1, .1), (19, 1, 0), (23, .72, 0), (31, 0, 0)]
    def attack_pose(frame):
        aim, draw = interpolate(keys, frame)
        return pose(aim, draw, arrow=6 <= frame <= 16,
                    recoil=math.sin((frame - 17) / 5 * math.pi) if 17 <= frame <= 22 else 0)
    attack = bake(scene, rig, 'Heroi_Attack', 31, attack_pose, [('Encaixar flecha', 6), ('Corda tensionada', 16), ('Disparo', 17), ('Recuperar', 23)])
    report = H['validate'](scene, rig, [idle, run, attack])
    report[0]['loop'] = True
    activate(rig, None)
    track = rig.animation_data.nla_tracks.new()
    track.name = 'Demonstracao - espera, corrida e disparo'
    for name, first, action, repeat in [('ESPERA', 1, idle, 1), ('CORRIDA', 66, run, 2), ('DISPARO', 121, attack, 2)]:
        strip = track.strips.new(name, first, action)
        strip.repeat = repeat
        strip.extrapolation = 'HOLD_FORWARD'
        scene.timeline_markers.new(name, frame=first)
    scene.frame_start = 1
    scene.frame_end = 181
    scene.render.fps = 30
    scene.sync_mode = 'FRAME_DROP'
    scene.frame_set(1)
    rig.hide_set(True)
    root['animacoes'] = 'Idle, Run, Attack; espera 1, corrida 66, disparo 121.'
    output = source.with_name('arqueiro_heroi_animado.blend')
    bpy.ops.wm.save_as_mainfile(filepath=str(output))
    asset = HERE.parent / 'assets/arqueiro_heroi/source.blend'
    asset.parent.mkdir(parents=True, exist_ok=True)
    bpy.ops.wm.save_as_mainfile(filepath=str(asset), copy=True)
    assert hashlib.sha256(source.read_bytes()).hexdigest() == digest
    output.with_suffix('.json').write_text(json.dumps({'source_sha256': digest, 'clips': report}, indent=2) + '\n')
    print('HERO_ANIMATED', json.dumps(report), flush=True)


if __name__ == '__main__':
    main()

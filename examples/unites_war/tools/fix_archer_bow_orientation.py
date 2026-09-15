"""Correct the saved enlarged bow without rebuilding the hero or locomotion.

blender -b arqueiro_heroi_recurvo.blend --python-exit-code 1 --python <this file>
"""

import hashlib
import json
import runpy
from pathlib import Path

import bpy

HERE = Path(__file__).resolve().parent
R = runpy.run_path(str(HERE / 'refine_archer_hero.py'), run_name='hero_refinement')
H = R['H']


def legacy_profile(u):
    return H['interpolate']([(0, -1.67), (.16, -1.69), (.38, -1.56), (.62, -1.33),
                             (.77, -1.29), (.90, -1.52), (1.02, -1.94)], u)[0]


def corrected(point):
    p = point.copy()
    u = abs(p.z - R['BOW_CENTER'].z) / R['SPAN']
    p.x += R['profile'](u) - legacy_profile(u)
    return p


def key_selected(pose, names, frame):
    for name in names:
        bone = pose.bones[name]
        parent = pose.targets[bone.parent.name]
        rest = bone.parent.matrix_local.inverted() @ bone.matrix_local
        pb = pose.rig.pose.bones[name]
        old_quaternion = pb.rotation_quaternion.copy()
        pb.matrix_basis = (parent @ rest).inverted() @ pose.targets[name]
        pb.rotation_quaternion.normalize()
        if pb.rotation_quaternion.dot(old_quaternion) < 0:
            pb.rotation_quaternion.negate()
        for channel in ['location', 'rotation_quaternion', 'scale']:
            pb.keyframe_insert(data_path=channel, frame=frame, group=name)


def main():
    source = Path(bpy.data.filepath)
    source_hash = hashlib.sha256(source.read_bytes()).hexdigest()
    scene = bpy.data.scenes['Heroi Arqueiro - Vigia Sombrio']
    bpy.context.window.scene = scene
    root, rig = scene.objects['Heroi Arqueiro'], scene.objects['Heroi - Rig']
    assert root.get('recurve_revision') == 1 and not root.get('bow_orientation_revision')
    tracks = [(track, track.mute) for track in rig.animation_data.nla_tracks]
    for track, _ in tracks:
        track.mute = True
    H['activate'](rig, None)
    for name in ['Arco longo - madeira curvada', 'Arco - laminacao externa',
                 'Arco - filete interno', 'Corda - laco alto', 'Corda - laco baixo']:
        ob = scene.objects[name]
        ob.data = ob.data.copy()
        for vertex in ob.data.vertices:
            vertex.co = corrected(vertex.co)
        ob.data.update()
    rig.hide_set(False)
    for ob in scene.objects:
        ob.select_set(False)
    rig.select_set(True)
    bpy.context.view_layer.objects.active = rig
    bpy.ops.object.mode_set(mode='EDIT')
    for name in ['BowTop', 'BowBottom']:
        bone = rig.data.edit_bones[name]
        delta = corrected(bone.head) - bone.head
        bone.head += delta
        bone.tail += delta
    bpy.ops.object.mode_set(mode='OBJECT')
    R['restring_bow'](scene, rig)

    # Retain every existing action, key time, marker and body channel.
    # Only the string, drawing arm and nocked arrow need a new brace position.
    actions = [bpy.data.actions['Heroi_' + n] for n in ['Idle', 'Run', 'Attack']]
    for action in actions:
        H['activate'](rig, action)
        samples = []
        for frame in range(1, int(action.frame_range.y) + 1):
            scene.frame_set(frame)
            samples.append({pb.name: pb.matrix.copy() for pb in rig.pose.bones})
        for frame, sample in enumerate(samples, 1):
            scene.frame_set(frame)
            pose = H['Pose'](rig, sample)
            base = pose.heads['Hand.L'] + pose.rots['Hand.L'] @ (
                rig.data.bones['BowString'].head_local - rig.data.bones['Hand.L'].head_local)
            delta = (base - pose.heads['BowString']) * .25
            pose.place('BowString', pose.heads['BowString'] + delta, pose.rots['BowString'])
            changed = ['BowString']
            if action.name == 'Heroi_Attack' and 5 < frame <= 21:
                weight = H['smooth']((frame - 5) / 5) if frame < 10 else 1
                pose.arm('R', pose.heads['Hand.R'] + delta * weight,
                         pose.rots['Hand.R'], (.8, -.18, .15))
                changed += ['UpperArm.R', 'Forearm.R', 'Hand.R']
                pose.place('Arrow', pose.heads['Arrow'] + delta * weight, pose.rots['Arrow'])
                changed.append('Arrow')
            key_selected(pose, changed, frame)

    clips = H['validate'](scene, rig, actions)
    # The world-facing direction is unchanged: only the bow's shape is fixed.
    assert rig.data.bones['BowTop'].head_local.x > R['BOW_CENTER'].x
    assert rig.data.bones['BowBottom'].head_local.x > R['BOW_CENTER'].x
    H['activate'](rig, None)
    for track, muted in tracks:
        track.mute = muted
    rig.hide_set(True)
    root['bow_orientation_revision'] = 1
    scene.frame_set(159)
    output = source.with_name('arqueiro_heroi_arco_corrigido.blend')
    bpy.ops.wm.save_as_mainfile(filepath=str(output))
    assert hashlib.sha256(source.read_bytes()).hexdigest() == source_hash
    report = {'source_sha256': source_hash, 'clips': clips, 'output': str(output)}
    output.with_suffix('.json').write_text(json.dumps(report, indent=2) + '\n')
    print('BOW_CORRECTED', json.dumps(report), flush=True)


if __name__ == '__main__':
    main()

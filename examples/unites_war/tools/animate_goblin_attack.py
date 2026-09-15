"""Author the Attack action on the existing goblin rig. Run inside Blender.

This only adds/replaces Attack; it does not save files or change Run.
The action is a 0.3-second, planted-foot diagonal knife slash at 30 fps.
"""

import math

import bpy
from mathutils import Matrix, Vector


def animate_attack():
    scene = bpy.context.scene
    rig = bpy.data.objects["Rig Saqueador"]
    rig.animation_data_create()
    run = bpy.data.actions["Run"]
    run.use_fake_user = True
    old = bpy.data.actions.get("Attack")
    if old:
        bpy.data.actions.remove(old)
    action = bpy.data.actions.new("Attack")
    action.use_fake_user = True
    rig.animation_data.action = action
    rig.data.pose_position = "POSE"
    bones = rig.data.bones
    identity = Matrix.Identity(3)

    def rotation(x=0, y=0, z=0):
        return (Matrix.Rotation(z, 3, "Z") @ Matrix.Rotation(y, 3, "Y")
                @ Matrix.Rotation(x, 3, "X"))

    def solve_joint(start, end, upper, lower, pole):
        delta = end - start
        distance = delta.length
        direction = delta.normalized()
        pole = (pole - direction * pole.dot(direction)).normalized()
        along = (upper * upper - lower * lower + distance * distance) / (2 * distance)
        height = math.sqrt(max(0, upper * upper - along * along))
        return start + direction * along + pole * height

    # Frame, pelvis yaw, chest yaw, lean, crouch, forward shift,
    # wrist position, blade deformation Euler angles, off-hand guard, cloth lag.
    keys = [
        (1, 0, 0, 0, 0, 0, (.81, -.245, 1.225), (0, 0, 0), 0, 0),
        (2, .07, .12, -.025, .025, -.015, (.88, -.02, 1.64), (-.25, .15, .1), .3, -.04),
        (3, .13, .24, -.05, .05, -.025, (.84, .10, 1.94), (-.48, .18, .18), .65, -.1),
        (4, -.08, -.16, .15, .065, .07, (.70, -.57, 1.87), (.85, .06, -.2), 1, .02),
        (5, -.18, -.30, .23, .08, .13, (.39, -.79, 1.61), (1.58, -.12, -.23), 1, .18),
        (6, -.20, -.33, .22, .08, .12, (.10, -.64, 1.30), (2.05, -.35, -.3), .8, .24),
        (7, -.13, -.20, .14, .06, .08, (.26, -.49, 1.21), (1.45, -.25, -.2), .6, .15),
        (8, -.05, -.08, .055, .025, .03, (.63, -.34, 1.21), (.65, -.08, -.08), .3, -.04),
        (9, -.012, -.02, .01, .006, .005, (.79, -.26, 1.225), (.12, 0, 0), .05, -.025),
        (10, 0, 0, 0, 0, 0, (.81, -.245, 1.225), (0, 0, 0), 0, 0),
    ]
    previous_quaternions = {}
    for frame, yaw, twist, lean, drop, forward, wrist, blade, guard, lag in keys:
        scene.frame_set(frame)
        targets, heads, rotations = {}, {}, {}

        def place(name, head, deform):
            bone = bones[name]
            heads[name], rotations[name] = head.copy(), deform.copy()
            matrix = (deform @ bone.matrix_local.to_3x3()).to_4x4()
            matrix.translation = head
            targets[name] = matrix

        def child(name, deform=None):
            bone = bones[name]
            parent = bone.parent.name
            head = heads[parent] + rotations[parent] @ (bone.head_local - bones[parent].head_local)
            place(name, head, rotations[parent] if deform is None else deform)
            return head

        def aim(name, start, end):
            rest = (bones[name].tail_local - bones[name].head_local).normalized()
            place(name, start, rest.rotation_difference((end - start).normalized()).to_matrix())

        place("Root", bones["Root"].head_local, identity)
        place("Pelvis", bones["Pelvis"].head_local + Vector((0, -forward, -drop)), rotation(z=yaw))
        child("Spine", rotation(x=lean * .45, z=yaw + twist * .4))
        child("Chest", rotation(x=lean, z=yaw + twist))
        child("Neck", rotation(x=lean * .5, z=(yaw + twist) * .4))
        child("Head", rotation(x=lean * .2, z=(yaw + twist) * .18))
        for name, parent, factor in [("Pouch", "Pelvis", 1), ("Loincloth", "Pelvis", -.65),
                                     ("Bandana", "Head", 1.1), ("Pendant", "Chest", .8)]:
            child(name, rotations[parent] @ rotation(x=lag * factor, z=lag * .3))

        for side in ("L", "R"):
            clavicle = "Clavicle." + side
            child(clavicle)
            upper, fore, hand = (prefix + side for prefix in ("UpperArm.", "Forearm.", "Hand."))
            shoulder = heads[clavicle] + rotations[clavicle] @ (bones[upper].head_local - bones[clavicle].head_local)
            if side == "R":
                end = Vector(wrist)
                pole = Vector((1, .25, -.3))
            else:
                end = bones[hand].head_local.lerp(Vector((-.75, -.40, 1.62)), guard)
                pole = Vector((-1, .15, -.4))
            # Keep the arm lengths exact, even at the strongest extension.
            reach = bones[upper].length + bones[fore].length
            if (end - shoulder).length > reach * .995:
                end = shoulder + (end - shoulder).normalized() * reach * .995
            elbow = solve_joint(shoulder, end, bones[upper].length, bones[fore].length, pole)
            aim(upper, shoulder, elbow)
            aim(fore, elbow, end)
            hand_rotation = rotation(*blade) if side == "R" else rotations[fore] @ rotation(z=-guard * .2)
            place(hand, end, hand_rotation)

            thigh, shin, foot = (prefix + side for prefix in ("Thigh.", "Shin.", "Foot."))
            hip = heads["Pelvis"] + rotations["Pelvis"] @ (bones[thigh].head_local - bones["Pelvis"].head_local)
            ankle = bones[foot].head_local.copy()
            rest_dir = (ankle - bones[thigh].head_local).normalized()
            rest_pole = bones[shin].head_local - bones[thigh].head_local
            rest_pole = (rest_pole - rest_dir * rest_pole.dot(rest_dir)).normalized()
            knee = solve_joint(hip, ankle, bones[thigh].length, bones[shin].length, rest_pole)
            aim(thigh, hip, knee)
            aim(shin, knee, ankle)
            place(foot, ankle, identity)

        # Start and finish exactly at the idle rest pose used in the game.
        if frame in (1, 10):
            targets = {bone.name: bone.matrix_local.copy() for bone in bones}
        for bone in bones:
            parent_pose = targets[bone.parent.name] if bone.parent else Matrix.Identity(4)
            relative = bone.parent.matrix_local.inverted() @ bone.matrix_local if bone.parent else bone.matrix_local
            pb = rig.pose.bones[bone.name]
            pb.rotation_mode = "QUATERNION"
            pb.matrix_basis = (parent_pose @ relative).inverted() @ targets[bone.name]
            pb.rotation_quaternion.normalize()
            previous = previous_quaternions.get(bone.name)
            if previous and pb.rotation_quaternion.dot(previous) < 0:
                pb.rotation_quaternion.negate()
            previous_quaternions[bone.name] = pb.rotation_quaternion.copy()
            for channel in ("location", "rotation_quaternion", "scale"):
                pb.keyframe_insert(data_path=channel, frame=frame, group=bone.name)

    for layer in action.layers:
        for strip in layer.strips:
            for bag in strip.channelbags:
                for curve in bag.fcurves:
                    for key in curve.keyframe_points:
                        key.interpolation = "LINEAR"
    action["description"] = "Corte diagonal de faca: preparar, cortar, recuperar. Pes fixos; sem root motion."
    action["duration_seconds"] = .3
    action["contact_frame"] = 5
    action.pose_markers.new("Preparar").frame = 3
    action.pose_markers.new("Corte").frame = 5
    action.pose_markers.new("Recuperar").frame = 8
    scene.render.fps = 30
    scene.render.fps_base = 1
    scene.frame_start, scene.frame_end = 1, 10
    scene.sync_mode = "FRAME_DROP"
    scene.frame_set(5)
    bpy.context.view_layer.update()
    print("ATTACK_READY", action.name, tuple(action.frame_range), len(bones))


if __name__ == "__main__":
    animate_attack()

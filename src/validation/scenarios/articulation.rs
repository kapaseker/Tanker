//! 验证既有多发姿态与真实 WASD 控制、驾驶联动、上下限及炮口世界方向。

use super::*;
use crate::{
    gameplay::{
        combat::firing::{self, Bullet},
        player::Tank,
    },
    tank::articulation::TankAim,
};

/// 记录组合输入阶段的起始姿态和回绕前的相机位置。
#[derive(Resource)]
struct FollowSample {
    /// 阶段开始时车体世界姿态。
    body: Transform,
    /// 阶段开始时炮塔相对偏航，单位弧度。
    yaw: f32,
    /// 世界坐标系中相机相对车体的位置，单位米。
    camera_offset: Vec3,
}

/// 上一帧姿态，用于检查真实关节的逐帧速度。
#[derive(Resource)]
struct PreviousAim(TankAim);
/// 前一帧玩家视角，用于区分松键与必要的视差修正。
#[derive(Resource)]
struct PreviousView(TankAim);
/// 前三次实际发射方向。
#[derive(Resource, Default)]
struct FiredDirections(Vec<Vec3>);

/// 离屏失焦验证使用临时窗口组件，不创建可见窗口。
#[derive(Resource)]
struct UnfocusedWindow(Entity);

/// 仅模拟焦点状态的 ECS 组件；离屏入口禁用 Winit，不创建系统窗口。
#[derive(Component)]
pub(in crate::validation) struct SimulatedFocus;

/// 前三帧验证独立弹丸，后段使用真实键盘控制；开火帧只解除测试冷却。
pub(super) fn prepare(world: &mut World, frame: u32) -> Result<(), String> {
    if frame == 872 {
        world
            .query_filtered::<&mut TankAim, With<Tank>>()
            .single_mut(world)
            .map_err(|e| e.to_string())?
            .pitch = 0.0;
    }
    if frame == 872 {
        world
            .query_filtered::<&mut crate::gameplay::camera::PlayerView, With<Tank>>()
            .single_mut(world)
            .map_err(|e| e.to_string())?
            .0
            .pitch = 0.0;
    }
    if frame == 884 {
        let entity = world
            .spawn((
                Window {
                    focused: false,
                    ..default()
                },
                SimulatedFocus,
            ))
            .id();
        world.insert_resource(UnfocusedWindow(entity));
    }
    if frame == 885 {
        let entity = world
            .remove_resource::<UnfocusedWindow>()
            .ok_or("缺少失焦窗口")?
            .0;
        world.despawn(entity);
    }
    if frame == 886 {
        world
            .resource_mut::<crate::app::transition::Fade>()
            .input_armed = false;
    }
    if frame == 887 {
        world
            .resource_mut::<crate::app::transition::Fade>()
            .input_armed = true;
    }
    if matches!(frame, 750 | 810 | 871) {
        let (aim, body) = world
            .query_filtered::<(&crate::gameplay::camera::PlayerView, &Transform), With<Tank>>()
            .single(world)
            .map(|(a, b)| (a.0, *b))
            .map_err(|e| e.to_string())?;
        let camera = world
            .query_filtered::<&Transform, With<crate::gameplay::camera::FollowCamera>>()
            .single(world)
            .map_err(|e| e.to_string())?;
        world.insert_resource(FollowSample {
            body,
            yaw: aim.yaw,
            camera_offset: camera.translation - body.translation,
        });
    }
    if frame == 870 {
        world
            .query_filtered::<&mut TankAim, With<Tank>>()
            .single_mut(world)
            .map_err(|e| e.to_string())?
            .yaw = 359.99_f32.to_radians();
    }
    if frame == 870 {
        world
            .query_filtered::<&mut crate::gameplay::camera::PlayerView, With<Tank>>()
            .single_mut(world)
            .map_err(|e| e.to_string())?
            .0
            .yaw = 359.99_f32.to_radians();
    }
    let angles = match frame {
        1 => Some((90.0_f32, 20.0_f32)),
        2 => Some((-90.0, 5.0)),
        3 => Some((45.0, 0.0)),
        _ => None,
    };
    if let Some((yaw, pitch)) = angles {
        let mut aims = world.query_filtered::<&mut TankAim, With<Tank>>();
        *aims
            .single_mut(world)
            .map_err(|e| format!("炮塔状态异常：{e}"))? = TankAim {
            yaw: yaw.to_radians(),
            pitch: pitch.to_radians(),
        };
        world
            .query_filtered::<&mut crate::gameplay::camera::PlayerView, With<Tank>>()
            .single_mut(world)
            .map_err(|e| e.to_string())?
            .0 = TankAim {
            yaw: yaw.to_radians(),
            pitch: pitch.to_radians(),
        };
        firing::reset_session(world);
    }
    if matches!(frame, 211 | 421) {
        firing::reset_session(world);
    }
    Ok(())
}

/// 预期方向来自独立的已知角度；检查弹丸仍沿发射时的方向飞行而非追随当前炮塔。
pub(super) fn check(world: &mut World, frame: u32, report: &mut String) -> Result<(), String> {
    let (aim, body) = world
        .query_filtered::<(&TankAim, &Transform), With<Tank>>()
        .single(world)
        .map(|(a, b)| (*a, *b))
        .map_err(|e| e.to_string())?;
    if frame > 3 && frame != 870 && frame != 872 {
        if let Some(previous) = world.get_resource::<PreviousAim>() {
            let yaw_delta = (aim.yaw - previous.0.yaw + std::f32::consts::PI)
                .rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            ensure(
                yaw_delta.abs() <= 30.0_f32.to_radians() / 60.0 + 1e-5,
                "水平关节超速",
            )?;
            ensure(
                (aim.pitch - previous.0.pitch).abs() <= 9.0_f32.to_radians() / 60.0 + 1e-5,
                "俯仰关节超速",
            )?;
        }
        let barrel = world
            .query_filtered::<&GlobalTransform, With<firing::PlayerBarrel>>()
            .single(world)
            .map_err(|e| e.to_string())?;
        let expected = body.rotation
            * Quat::from_rotation_y(aim.yaw)
            * Quat::from_rotation_z(-aim.pitch)
            * Vec3::NEG_X;
        ensure(
            (barrel.rotation() * Vec3::NEG_X).abs_diff_eq(expected, 1e-4),
            "真实炮管未跟随输入姿态",
        )?;
    }
    let view = world
        .query_filtered::<&crate::gameplay::camera::PlayerView, With<Tank>>()
        .single(world)
        .map_err(|e| e.to_string())?
        .0;
    if (878..=887).contains(&frame) {
        let previous = world.resource::<PreviousView>().0;
        let aim = view;
        let yaw_delta = (aim.yaw - previous.yaw + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        let pitch_delta = aim.pitch - previous.pitch;
        match frame {
            879..=880 => ensure(
                yaw_delta.abs() < 1e-6 && pitch_delta > 0.0,
                "单轴松键未立即停止或另一轴停止",
            )?,
            878 | 881 => ensure(
                yaw_delta < 0.0 && yaw_delta.abs() < 0.1_f32.to_radians(),
                "反向未从低速起步",
            )?,
            882 => ensure(
                yaw_delta.abs() < 1e-6 && pitch_delta > 0.0,
                "相反输入未抵消",
            )?,
            884 | 886 => ensure(
                yaw_delta.abs() < 1e-6 && pitch_delta.abs() < 1e-6,
                "失焦或锁定仍在调炮",
            )?,
            885 | 887 => ensure(
                yaw_delta > 0.0
                    && yaw_delta < 0.1_f32.to_radians()
                    && pitch_delta > 0.0
                    && pitch_delta < 0.03_f32.to_radians(),
                "恢复后未从低速起步",
            )?,
            _ => {}
        }
        if frame == 887 {
            report.push_str("断言通过：单轴松键、反向、抵消、失焦及锁定后低速恢复\n");
        }
    }
    let prior_view = world
        .get_resource::<PreviousView>()
        .map(|previous| previous.0);
    if (211..=211).contains(&frame) || (421..=450).contains(&frame) {
        ensure(
            prior_view.is_some_and(|previous| (view.pitch - previous.pitch).abs() < 1e-6),
            "松键后视角仍有俯仰输入",
        )?;
    }
    world.insert_resource(PreviousAim(aim));
    world.insert_resource(PreviousView(view));
    let aim = view;
    if frame == 31 {
        ensure(
            (aim.yaw.to_degrees() - 45.02778).abs() < 0.001,
            "水平短按角度异常",
        )?;
        ensure(
            (aim.pitch.to_degrees() - 0.008333).abs() < 0.001,
            "俯仰短按角度异常",
        )?;
        report.push_str("断言通过：双轴首帧微调小于恒速，真实炮管方向与关节一致\n");
    }
    if matches!(frame, 210 | 211 | 420 | 421 | 450) {
        let (aim, body) = world
            .query_filtered::<(&crate::gameplay::camera::PlayerView, &Transform), With<Tank>>()
            .single(world)
            .map(|(aim, body)| (aim.0, *body))
            .map_err(|e| e.to_string())?;
        let expected_yaw: f32 = if frame <= 211 { 132.75 } else { 30.5 };
        ensure(
            (aim.yaw.to_degrees() - expected_yaw).abs() < 0.01,
            &format!(
                "WASD 水平角速度或松键停止异常：帧{frame}，实际{}°，预期{expected_yaw}°",
                aim.yaw.to_degrees()
            ),
        )?;
        let actual_pitch = world.resource::<PreviousAim>().0.pitch.to_degrees();
        let limit_ok = match frame {
            210 => {
                (actual_pitch - 20.0).abs() < 0.01 || (aim.pitch.to_degrees() - 20.0).abs() < 0.01
            }
            420 => (actual_pitch + 9.0).abs() < 0.01 || (aim.pitch.to_degrees() + 9.0).abs() < 0.01,
            _ => (-9.01..=20.01).contains(&actual_pitch),
        };
        ensure(
            limit_ok,
            &format!(
                "WASD 俯仰限位异常：视角 {}°，实际 {}°",
                aim.pitch.to_degrees(),
                actual_pitch
            ),
        )?;
        ensure(
            (body.rotation * Vec3::NEG_X - Vec3::NEG_X).length() > 0.5,
            "驾驶输入未转动车体",
        )?;
        let expected_position = body.translation
            + body.rotation * Quat::from_rotation_y(expected_yaw.to_radians()) * Vec3::X * 16.0
            + Vec3::Y * 5.0;
        let camera = world
            .query_filtered::<&Transform, With<crate::gameplay::camera::FollowCamera>>()
            .single(world)
            .map_err(|e| e.to_string())?;
        ensure(
            camera.translation.distance(expected_position) < 0.001,
            "相机未在当帧跟随炮塔",
        )?;
        if matches!(frame, 211 | 421) {
            let barrel = world
                .query_filtered::<&GlobalTransform, With<firing::PlayerBarrel>>()
                .single(world)
                .map_err(|e| e.to_string())?;
            let direction = barrel.rotation() * Vec3::NEG_X;
            ensure(
                world
                    .query::<&Bullet>()
                    .iter(world)
                    .any(|b| b.direction.abs_diff_eq(direction, 1e-4)),
                "按键和车体转向后的炮口世界发射方向不符",
            )?;
        }
        report.push_str(&format!("断言通过：WASD 帧{frame}，视角偏航{expected_yaw}°、实际俯仰{actual_pitch:.2}°，车体转向后保持相对角度，炮口发射方向正确\n"));
    }
    if (750..=869).contains(&frame) || frame == 871 {
        let (aim, body) = world
            .query_filtered::<(&crate::gameplay::camera::PlayerView, &Transform), With<Tank>>()
            .single(world)
            .map(|(a, b)| (a.0, *b))
            .map_err(|e| e.to_string())?;
        let camera = *world
            .query_filtered::<&Transform, With<crate::gameplay::camera::FollowCamera>>()
            .single(world)
            .map_err(|e| e.to_string())?;
        let sample = world.resource::<FollowSample>();
        let rear = body.rotation * Quat::from_rotation_y(aim.yaw) * Vec3::X;
        let horizontal = Vec3::new(rear.x, 0.0, rear.z).normalize();
        ensure(
            camera
                .translation
                .distance(body.translation + horizontal * 16.0 + Vec3::Y * 5.0)
                < 0.001,
            "组合输入后相机未跟随当帧姿态",
        )?;
        if frame == 871 {
            ensure(sample.yaw > 6.0 && aim.yaw < 0.1, "炮塔未跨越角度回绕边界")?;
            ensure(
                (body.rotation.inverse() * (camera.translation - body.translation))
                    .distance(sample.body.rotation.inverse() * sample.camera_offset)
                    < 0.2,
                "回绕时相机跳变",
            )?;
        } else if matches!(frame, 809 | 869) {
            let displacement = body.translation - sample.body.translation;
            let forward = sample.body.rotation * Vec3::NEG_X;
            let sign = if frame == 809 { 1.0 } else { -1.0 };
            ensure(
                displacement.dot(forward) * sign > 0.2,
                "组合输入未产生预期方向的驾驶位移",
            )?;
            ensure(
                body.rotation.angle_between(sample.body.rotation) > 0.1,
                "组合输入未转动车体",
            )?;
            ensure((aim.yaw - sample.yaw).abs() > 0.1, "组合输入未转动炮塔")?;
        }
        if matches!(frame, 809 | 869 | 871) {
            report.push_str(&format!("断言通过：组合驾驶与相机/回绕帧 {frame}\n"));
        }
    }
    if frame == 749 {
        let aim = &world
            .query_filtered::<&crate::gameplay::camera::PlayerView, With<Tank>>()
            .single(world)
            .map_err(|e| e.to_string())?
            .0;
        ensure(
            (aim.yaw.to_degrees() - 177.75).abs() < 0.02,
            "真实输入未转到朝后",
        )?;
        report.push_str("断言通过：真实调炮朝后，跟随视角帧 749\n");
    }
    if !(1..=3).contains(&frame) {
        return Ok(());
    }
    let barrel = *world
        .query_filtered::<&GlobalTransform, With<firing::PlayerBarrel>>()
        .single(world)
        .map_err(|e| e.to_string())?;
    let direction = barrel.rotation() * Vec3::NEG_X;
    if frame == 1 {
        world.insert_resource(FiredDirections::default());
    }
    world.resource_mut::<FiredDirections>().0.push(direction);
    let expected = world.resource::<FiredDirections>().0.clone();
    let bullets: Vec<_> = world
        .query::<(&Bullet, &Transform, Option<&ChildOf>)>()
        .iter(world)
        .map(|(bullet, transform, parent)| (bullet.direction, *transform, parent.is_some()))
        .collect();
    ensure(
        bullets.len() == frame as usize,
        "三次开火应生成同时存在的独立弹丸",
    )?;
    for direction in &expected {
        ensure(
            bullets.iter().any(|(actual, transform, parent)| {
                !parent
                    && actual.abs_diff_eq(*direction, 1e-4)
                    && (transform.rotation * Vec3::NEG_X).abs_diff_eq(*direction, 1e-4)
            }),
            "弹丸方向未保持发射时炮口方向",
        )?;
    }
    report.push_str(&format!(
        "断言通过：姿态帧{frame}，{}颗独立弹丸方向与视觉朝向保持正确\n",
        bullets.len()
    ));
    Ok(())
}

/// 以真实键盘链路比较不同模拟步长的同一秒调炮结果。
pub(super) fn check_step_sizes(app: &mut App, report: &mut String) -> Result<(), String> {
    let mut angles = Vec::new();
    for (steps, dt) in [(30, 1.0 / 30.0), (120, 1.0 / 120.0)] {
        update(app, STEP, Some(888))?;
        *app.world_mut()
            .query_filtered::<&mut TankAim, With<Tank>>()
            .single_mut(app.world_mut())
            .map_err(|e| e.to_string())? = TankAim::default();
        app.world_mut()
            .query_filtered::<&mut crate::gameplay::camera::PlayerView, With<Tank>>()
            .single_mut(app.world_mut())
            .map_err(|e| e.to_string())?
            .0 = TankAim::default();
        for _ in 0..steps {
            update(app, std::time::Duration::from_secs_f64(dt), Some(872))?;
        }
        let aim = app
            .world_mut()
            .query_filtered::<&crate::gameplay::camera::PlayerView, With<Tank>>()
            .single(app.world())
            .map_err(|e| e.to_string())?
            .0;
        angles.push(aim);
        update(app, STEP, Some(888))?;
        let stopped = app
            .world_mut()
            .query_filtered::<&crate::gameplay::camera::PlayerView, With<Tank>>()
            .single(app.world())
            .map_err(|e| e.to_string())?
            .0;
        ensure(
            (stopped.yaw - aim.yaw).abs() < 1e-6 && (stopped.pitch - aim.pitch).abs() < 1e-6,
            "松键后仍有角度尾巴",
        )?;
    }
    ensure(
        (angles[0].yaw - angles[1].yaw).abs() < 0.01_f32.to_radians()
            && (angles[0].pitch - angles[1].pitch).abs() < 0.01_f32.to_radians(),
        "不同步长调炮偏差超过 0.01°",
    )?;
    report.push_str("断言通过：1/30 与 1/120 秒步长的双轴角度偏差小于 0.01°，松键立即停止\n");
    Ok(())
}

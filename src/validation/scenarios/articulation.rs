//! 验证既有多发姿态与真实 WASD 控制、驾驶联动、上下限及炮口世界方向。

use super::*;
use crate::{
    gameplay::{
        combat::firing::{self, Bullet},
        player::Tank,
    },
    tank::articulation::TankAim,
};

/// 前三帧验证独立弹丸，后段使用真实键盘控制；开火帧只解除测试冷却。
pub(super) fn prepare(world: &mut World, frame: u32) -> Result<(), String> {
    let angles = match frame {
        1 => Some((90.0_f32, 20.0_f32)),
        2 => Some((-90.0, -5.0)),
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
        firing::reset_session(world);
    }
    if matches!(frame, 211 | 421) {
        firing::reset_session(world);
    }
    Ok(())
}

/// 预期方向来自独立的已知角度；检查弹丸仍沿发射时的方向飞行而非追随当前炮塔。
pub(super) fn check(world: &mut World, frame: u32, report: &mut String) -> Result<(), String> {
    if matches!(frame, 210 | 211 | 420 | 421 | 450) {
        let (aim, body) = world
            .query_filtered::<(&TankAim, &Transform), With<Tank>>()
            .single(world)
            .map_err(|e| e.to_string())?;
        let expected_yaw: f32 = if frame <= 211 { 135.0 } else { 30.5 };
        let expected_pitch: f32 = if frame <= 211 { 20.0 } else { -9.0 };
        ensure(
            (aim.yaw.to_degrees() - expected_yaw).abs() < 0.01,
            "WASD 水平角速度或松键停止异常",
        )?;
        ensure(
            (aim.pitch.to_degrees() - expected_pitch).abs() < 0.01,
            "WASD 俯仰限位或松键停止异常",
        )?;
        ensure(
            (body.rotation * Vec3::NEG_X - Vec3::NEG_X).length() > 0.5,
            "驾驶输入未转动车体",
        )?;
        if matches!(frame, 211 | 421) {
            let direction = body.rotation
                * Quat::from_rotation_y(expected_yaw.to_radians())
                * Quat::from_rotation_z(-expected_pitch.to_radians())
                * Vec3::NEG_X;
            ensure(
                world
                    .query::<&Bullet>()
                    .iter(world)
                    .any(|b| b.direction.abs_diff_eq(direction, 1e-4)),
                "按键和车体转向后的炮口世界发射方向不符",
            )?;
        }
        report.push_str(&format!("断言通过：WASD 帧{frame}，相对偏航{expected_yaw}°、俯仰{expected_pitch}°，车体转向后保持相对角度，炮口发射方向正确\n"));
    }
    if !(1..=3).contains(&frame) {
        return Ok(());
    }
    let expected = [
        Vec3::new(0.0, 0.34202015, 0.9396926),
        Vec3::new(0.0, -0.08715574, -0.9961947),
        Vec3::new(-0.70710677, 0.0, 0.70710677),
    ];
    let bullets: Vec<_> = world
        .query::<(&Bullet, &Transform, Option<&ChildOf>)>()
        .iter(world)
        .map(|(bullet, transform, parent)| (bullet.direction, *transform, parent.is_some()))
        .collect();
    ensure(
        bullets.len() == frame as usize,
        "三次开火应生成同时存在的独立弹丸",
    )?;
    for direction in &expected[..frame as usize] {
        ensure(
            bullets.iter().any(|(actual, transform, parent)| {
                !parent
                    && actual.abs_diff_eq(*direction, 1e-4)
                    && (transform.rotation * Vec3::NEG_X).abs_diff_eq(*direction, 1e-4)
            }),
            "弹丸方向、视觉朝向或独立层级与发射姿态不符",
        )?;
    }
    report.push_str(&format!(
        "断言通过：姿态帧{frame}，{}颗独立弹丸方向与视觉朝向保持正确\n",
        bullets.len()
    ));
    Ok(())
}

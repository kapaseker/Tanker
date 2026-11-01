//! 通过可设置姿态驱动真实炮塔与炮管，并在连续三帧验证独立弹丸。

use super::*;
use crate::{
    gameplay::{
        combat::firing::{self, Bullet},
        player::Tank,
    },
    tank::articulation::TankAim,
};

/// 不增加玩家输入；连续三帧强制首发可用，以观察同时存在的不同方向弹丸。
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
    Ok(())
}

/// 预期方向来自独立的已知角度；检查弹丸仍沿发射时的方向飞行而非追随当前炮塔。
pub(super) fn check(world: &mut World, frame: u32, report: &mut String) -> Result<(), String> {
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

//! 炮镜分划抽查断言（工单 04）：显隐翻转、读数格式与 WASD 调炮后的实时值。
//! 完整 gunner_sight 预设（相机位姿、视口换算、弹道方向等）由工单 05 落地。

use super::*;
use crate::gameplay::sight::{PitchReadout, SightRoot, TurretReadout};

/// 帧边界检查：帧 0 为第三人称主槽基准（分划整体隐藏，无残影），
/// Shift 切换后分划随炮镜主槽显示，读数格式与原型一致；
/// 帧 91 再按 Shift 退回第三人称，帧 120 断言整体隐藏无残影。
pub(super) fn check(
    app: &mut App,
    frame: u32,
    report: &mut String,
    failures: &mut Vec<String>,
) -> Result<(), String> {
    if !matches!(frame, 0 | 30 | 60 | 90 | 120) {
        return Ok(());
    }
    let world = app.world_mut();
    let visibility = *world
        .query_filtered::<&Visibility, With<SightRoot>>()
        .single(world)
        .map_err(|error| format!("分划根节点缺失：{error}"))?;
    let expected = if matches!(frame, 0 | 120) {
        Visibility::Hidden
    } else {
        Visibility::Visible
    };
    record_check(
        report,
        failures,
        visibility == expected,
        &format!(
            "帧 {frame} 分划显隐 {}（期望 {}）",
            describe(visibility),
            describe(expected)
        ),
    );
    if !matches!(frame, 30 | 60 | 90) {
        return Ok(());
    }
    // 读数格式：+02.4°（6 字符）/ +016.0°（7 字符），符号 + 定宽整数 + 1 位小数。
    let pitch = readout::<PitchReadout>(world, "俯仰角")?;
    let turret = readout::<TurretReadout>(world, "炮塔角")?;
    record_check(
        report,
        failures,
        is_angle_format(&pitch, 6),
        &format!("帧 {frame} 俯仰角读数格式：{pitch}"),
    );
    record_check(
        report,
        failures,
        is_angle_format(&turret, 7),
        &format!("帧 {frame} 炮塔角读数格式：{turret}"),
    );
    if frame == 30 {
        // KeyW 已抬炮 0.5 秒：俯仰读数必须离开水平零位，证明读数由真实状态驱动。
        record_check(
            report,
            failures,
            pitch != "+00.0°",
            &format!("帧 {frame} 抬炮后俯仰读数离开零位：{pitch}"),
        );
    }
    report.push_str(&format!("帧 {frame} 读数：俯仰 {pitch}，炮塔 {turret}\n"));
    Ok(())
}

fn describe(visibility: Visibility) -> &'static str {
    match visibility {
        Visibility::Visible => "可见",
        Visibility::Hidden | Visibility::Inherited => "隐藏",
    }
}

/// 读取单个读数节点的当前文本。
fn readout<M: Component>(world: &mut World, name: &str) -> Result<String, String> {
    world
        .query_filtered::<&Text, With<M>>()
        .single(world)
        .map(|text| text.to_string())
        .map_err(|error| format!("{name} 读数缺失：{error}"))
}

/// 定宽角度文本校验：按字符数比对（° 为 2 字节 UTF-8，不能按字节算）、
/// 首字符为符号、数值部分可解析、以度符号结尾。
fn is_angle_format(text: &str, length: usize) -> bool {
    let Some(digits) = text.strip_suffix('°') else {
        return false;
    };
    digits.chars().count() + 1 == length
        && matches!(digits.as_bytes().first(), Some(b'+') | Some(b'-'))
        && digits.parse::<f32>().is_ok()
}

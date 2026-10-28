//! 战斗 HUD 装填与生命显示断言：节点宽度必须跟随真实冷却与血量。

use super::*;
use crate::gameplay::combat::firing::Gun;
use crate::gameplay::hud::{HealthFill, HealthTrack, HudRoot, ReloadFill, ReloadTrack};

/// 在公共模拟循环的原有帧边界检查 HUD 状态；帧 60 看装填中段，330 看满格常驻。
pub(super) fn check(
    app: &mut App,
    frame: u32,
    report: &mut String,
    failures: &mut Vec<String>,
) -> Result<(), String> {
    if frame != 60 && frame != 330 {
        return Ok(());
    }
    let world = app.world_mut();
    let hud_exists = world
        .query_filtered::<Entity, With<HudRoot>>()
        .iter(world)
        .count()
        == 1;
    record_check(report, failures, hud_exists, "HUD 状态簇随对局生成");
    if !hud_exists {
        return Ok(());
    }
    let track_width = world
        .query_filtered::<&ComputedNode, With<ReloadTrack>>()
        .single(world)
        .map_err(|error| format!("装填轨道缺失：{error}"))?
        .size
        .x;
    let fill_width = world
        .query_filtered::<&ComputedNode, With<ReloadFill>>()
        .single(world)
        .map_err(|error| format!("装填填充缺失：{error}"))?
        .size
        .x;
    let progress = if track_width > 0.0 {
        fill_width / track_width
    } else {
        0.0
    };
    // HUD 宽度必须与 Gun 资源的真实冷却一致，不能出现表现层自走计时。
    let expected = world.resource::<Gun>().reload_progress();
    record_check(
        report,
        failures,
        (progress - expected).abs() < 0.02,
        &format!("帧 {frame} 装填进度与冷却资源一致（{progress:.3} / 期望 {expected:.3}）"),
    );
    if frame == 60 {
        // 开火 1 秒后冷却 5 秒：应处于装填中段而非空或满。
        record_check(
            report,
            failures,
            progress > 0.01 && progress < 0.99,
            &format!("装填条在开火后推进（帧 60：{progress:.3}）"),
        );
    } else {
        // 冷却 5 秒已过：进度必须恰好满格（钳制），而非继续增长或消失。
        record_check(
            report,
            failures,
            progress >= 0.999,
            &format!("冷却结束后装填条满格常驻（帧 330：{progress:.3}）"),
        );
    }
    // 生命条当前固定满血：轨道与填充等宽。
    let health_track = world
        .query_filtered::<&ComputedNode, With<HealthTrack>>()
        .single(world)
        .map_err(|error| format!("生命轨道缺失：{error}"))?
        .size
        .x;
    let health_fill = world
        .query_filtered::<&ComputedNode, With<HealthFill>>()
        .single(world)
        .map_err(|error| format!("生命填充缺失：{error}"))?
        .size
        .x;
    record_check(
        report,
        failures,
        health_track > 0.0 && (health_fill - health_track).abs() < 1.0,
        "生命条显示满血",
    );
    Ok(())
}

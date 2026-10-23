//! 直行、转向与履带里程断言。

use super::*;

/// 在公共模拟循环的原有帧边界检查真实游戏状态。
pub(super) fn check(
    app: &mut App,
    frame: u32,
    start: Transform,
    start_tracks: [f32; 2],
    report: &mut String,
    failures: &mut Vec<String>,
) -> Result<(), String> {
    if frame == 60 {
        let (state, tracks) = subject_state(app.world_mut())?;
        report.push_str(&format!(
            "直行检查：位置={:?}，履带={tracks:?}，游戏时间={:?}，固定时间={:?}\n",
            state.translation,
            app.world().resource::<Time<Virtual>>().elapsed(),
            app.world().resource::<Time<Fixed>>().elapsed()
        ));
        // 出生平地内 1 秒应前进约 6 米；幽灵接触阻塞时只有约 0.15 米，
        // 4 米阈值保持强判别力，同时不依赖平地之外的地形起伏。
        record_check(
            report,
            failures,
            (state.translation - start.translation).dot(start.rotation * Vec3::NEG_X) > 4.0,
            "直行前进至少 4 米",
        );
        record_check(
            report,
            failures,
            tracks
                .iter()
                .zip(start_tracks)
                .all(|(now, before)| (now - before).abs() > 4.0),
            "直行履带累计里程大于 4 米",
        );
    }
    if frame == 180 {
        let (state, _) = subject_state(app.world_mut())?;
        record_check(
            report,
            failures,
            state.rotation.angle_between(start.rotation) > 1.0,
            "转弯偏航变化大于 1 弧度",
        );
    }
    Ok(())
}

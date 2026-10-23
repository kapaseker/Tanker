//! 轮组方向与停车断言。

use super::*;

/// 在公共模拟循环的原有帧边界检查真实游戏状态。
pub(super) fn check(
    app: &mut App,
    frame: u32,
    previous_tracks: &mut [f32; 2],
    report: &mut String,
    failures: &mut Vec<String>,
) -> Result<(), String> {
    if [30, 60, 90, 120, 240, 250].contains(&frame) {
        let (_, tracks) = subject_state(app.world_mut())?;
        let delta = [
            tracks[0] - previous_tracks[0],
            tracks[1] - previous_tracks[1],
        ];
        let passed = match frame {
            30 => delta.iter().all(|d| *d > 2.9),
            60 => delta.iter().all(|d| *d < -1.9),
            90 => delta[0] > 0.5 && delta[1] < -0.5,
            120 => delta[0] < -0.3 && delta[1] > 0.3,
            250 => delta.iter().all(|d| d.abs() < 0.0001),
            _ => true,
        };
        record_check(
            report,
            failures,
            passed,
            &format!("轮组输入方向与停车：帧{frame}，左右里程变化{delta:?}"),
        );
        *previous_tracks = tracks;
    }
    Ok(())
}

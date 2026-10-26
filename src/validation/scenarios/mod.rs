//! 场景帧表、输入脚本与公共预热/计帧流程；各场景保留原有断言。

mod combat;
mod detail;
mod drive;
mod gallery;
mod index;
pub(super) mod scene;
mod wheels;

use super::*;
use super::{
    capture::{PixelCheck, capture, capture_checked, capture_side, capture_wheel_views},
    offscreen::{update, wait_render},
};
use avian3d::prelude::{LinearVelocity, Position, Rotation};
use combat::arrange_combat;
use gallery::run_gallery_scenario;
use index::run_index_scenario;
use scene::{subject_state, validate_scene};

/// 内置验证场景，不提供独立于游戏逻辑的测试实现。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Scenario {
    Scene,
    Drive,
    Combat,
    /// 所选车型轮组的侧面动画与三视点对照。
    Wheels,
    /// 首页：验证 UI 结构、键盘流转与到真实场景的过渡。
    Index,
    /// 模型画廊：真实预览、滚动、自转及反复往返清理。
    Gallery,
    /// 单模型详情：八项资源、真实输入、返回恢复与重复清理。
    Detail,
}

impl Scenario {
    /// 计帧从资源就绪并完成落地预热后开始；0 为动作前基准图。
    pub(super) fn frames(self) -> &'static [u32] {
        match self {
            Self::Scene => &[0],
            // 体素地形是有意障碍：直行只模拟 1 秒（60 帧），车头前缘
            // （半长 3.75 m + 行驶 6 m）仍留在半径 24 m 的出生平区内；
            // 平区外的台阶全部是可翻越的 1 档坎，不构成驾驶失效判据。
            Self::Drive => &[0, 60, 180],
            Self::Combat => &[0, 6, 24, 48, 60, 120],
            Self::Wheels => &[
                0, 5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55, 60, 75, 90, 105, 120, 130, 240, 250,
            ],
            // 菜单关键帧按真实阶段动态计帧，固定列表只声明基准图。
            Self::Index | Self::Gallery | Self::Detail => &[0],
        }
    }

    /// 按固定模拟帧注入按键，经过游戏现有输入系统，而非直接调用业务逻辑。
    pub(super) fn keys(self, frame: u32) -> &'static [KeyCode] {
        match (self, frame) {
            (Self::Drive, 1..=60) => &[KeyCode::ArrowUp],
            (Self::Drive, 61..=180) => &[KeyCode::ArrowUp, KeyCode::ArrowLeft],
            (Self::Combat, 1) => &[KeyCode::Space],
            (Self::Wheels, 1..=30) => &[KeyCode::ArrowUp],
            (Self::Wheels, 31..=60) => &[KeyCode::ArrowDown],
            (Self::Wheels, 61..=90) => &[KeyCode::ArrowLeft],
            (Self::Wheels, 91..=120) => &[KeyCode::ArrowRight],
            (Self::Index | Self::Gallery | Self::Detail, 1) => &[KeyCode::ArrowDown],
            (Self::Index | Self::Gallery | Self::Detail, 3) => &[KeyCode::Enter],
            (Self::Index | Self::Gallery | Self::Detail, 5) => &[KeyCode::Escape],
            (Self::Index | Self::Gallery | Self::Detail, 7) => &[KeyCode::Tab],
            (Self::Index | Self::Gallery | Self::Detail, 8) => &[KeyCode::Tab, KeyCode::ShiftLeft],
            // 首页退出确认弹框：← 在两按钮间切换选中。
            (Self::Index, 15) => &[KeyCode::ArrowLeft],
            (Self::Gallery | Self::Detail, 11) => &[KeyCode::End],
            (Self::Gallery | Self::Detail, 12) => &[KeyCode::Home],
            (Self::Gallery | Self::Detail, 13) => &[KeyCode::PageDown],
            (Self::Gallery | Self::Detail, 14) => &[KeyCode::ArrowDown],
            (Self::Detail, 16) => &[KeyCode::ArrowRight],
            (Self::Detail, 17) => &[KeyCode::ArrowUp],
            (Self::Detail, 18) => &[KeyCode::ArrowLeft],
            _ => &[],
        }
    }
}

/// 执行固定预热、模拟、状态断言及关键帧输出；截图等待不推进游戏时间。
pub(super) fn run_scenario(
    app: &mut App,
    options: &Options,
    report: &mut String,
) -> Result<(), String> {
    if options.scenario == Scenario::Detail {
        return detail::run_detail_scenario(app, options, report);
    }
    if options.scenario == Scenario::Gallery {
        return run_gallery_scenario(app, options, report);
    }
    if options.scenario == Scenario::Index {
        return run_index_scenario(app, options, report);
    }
    wait_render(app, true)?;
    if options.scenario == Scenario::Combat {
        arrange_combat(app.world_mut());
        report.push_str("战斗布局：与玩家同高的靶车在平地直接承受水平射击，无台座。\n");
    }
    if options.enemy_wheels {
        let world = app.world_mut();
        let enemy = world
            .query_filtered::<Entity, With<crate::gameplay::enemy::EnemyTank>>()
            .iter(world)
            .min_by_key(|e| e.to_bits())
            .ok_or("未找到敌人轮组目标")?;
        let position = Vec3::new(0.0, 3.0, 9.0);
        world.entity_mut(enemy).insert((
            Transform::from_translation(position),
            Position(position),
            Rotation::default(),
            LinearVelocity::ZERO,
        ));
        world.insert_resource(WheelTarget(enemy));
    }
    report.push_str(if options.enemy_wheels {
        "轮组目标：敌人4+2\n"
    } else {
        "轮组目标：玩家5+2\n"
    });
    for _ in 0..120 {
        update(app, STEP, None)?;
    }
    validate_scene(app.world_mut())?;
    let (start, start_tracks) = subject_state(app.world_mut())?;
    report.push_str(&format!(
        "预热：120 帧；断言通过：地形就绪、坦克位置、相机跟随\n起点：{:?}\n",
        start.translation
    ));
    let world = app.world_mut();
    let mut enemies: Vec<_> = world
        .query_filtered::<&Transform, With<crate::gameplay::enemy::EnemyTank>>()
        .iter(world)
        .map(|transform| transform.translation)
        .collect();
    enemies.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.z.total_cmp(&b.z)));
    report.push_str(&format!("敌人布局：{enemies:?}\n"));
    let mut failures = Vec::new();
    let mut previous_tracks = start_tracks;
    if options.scenario == Scenario::Wheels {
        capture_wheel_views(app, &options.output)?;
        capture_side(app, &options.image_path(0))?;
    } else {
        capture(app, &options.image_path(0))?;
    }
    report.push_str(&format!(
        "模拟帧：0；图片：{}\n",
        options.image_path(0).display()
    ));
    for frame in 1..=*options.scenario.frames().last().unwrap() {
        update(app, STEP, Some(frame))?;
        validate_scene(app.world_mut())?;
        if options.scenario == Scenario::Wheels {
            wheels::check(app, frame, &mut previous_tracks, report, &mut failures)?;
        }
        if options.scenario == Scenario::Drive {
            drive::check(app, frame, start, start_tracks, report, &mut failures)?;
        }
        if options.scenario == Scenario::Combat {
            combat::check(app, frame, report, &mut failures)?;
        }
        if options.scenario.frames().contains(&frame) {
            let (state, tracks) = subject_state(app.world_mut())?;
            report.push_str(&format!(
                "模拟帧：{frame}；位置：{:?}；旋转：{:?}；履带里程：{tracks:?}\n",
                state.translation, state.rotation
            ));
            if options.scenario == Scenario::Wheels {
                capture_side(app, &options.image_path(frame))?;
            } else {
                capture(app, &options.image_path(frame))?;
            }
            report.push_str(&format!("图片：{}\n", options.image_path(frame).display()));
        }
    }
    ensure(failures.is_empty(), &failures.join("；"))?;
    info!(
        "离屏验证通过：{:?}，输出：{}",
        options.scenario,
        options.output.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_inputs_stop_at_boundaries_and_fire_only_once() {
        assert!(Scenario::Drive.keys(0).is_empty());
        assert_eq!(Scenario::Drive.keys(60), &[KeyCode::ArrowUp]);
        assert_eq!(
            Scenario::Drive.keys(61),
            &[KeyCode::ArrowUp, KeyCode::ArrowLeft]
        );
        assert!(Scenario::Drive.keys(181).is_empty());
        assert_eq!(Scenario::Combat.keys(1), &[KeyCode::Space]);
        assert!(Scenario::Combat.keys(2).is_empty());
        assert!(Scenario::Combat.keys(120).is_empty());
        assert!(Scenario::Scene.keys(1).is_empty());
        assert_eq!(Scenario::Index.keys(1), &[KeyCode::ArrowDown]);
        assert_eq!(Scenario::Index.keys(3), &[KeyCode::Enter]);
        assert_eq!(Scenario::Index.keys(5), &[KeyCode::Escape]);
        assert!(Scenario::Index.keys(2).is_empty());
    }
}

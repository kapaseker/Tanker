//! 场景帧表、输入脚本与公共预热/计帧流程；各场景保留原有断言。

pub(super) mod articulation;
mod combat;
mod detail;
mod drive;
mod gallery;
mod hud;
mod index;
mod integrated;
mod pastoral;
pub(super) mod scene;
mod sight;
mod wheels;
mod window_resize;

use super::*;
use super::{
    capture::{
        PixelCheck, capture, capture_at, capture_bullet, capture_checked, capture_concrete,
        capture_side, capture_wheel_views,
    },
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
    /// 玩家炮塔、炮管与多发弹丸的世界姿态验证。
    Articulation,
    /// 驾驶、调炮、开火与退出重进的完整瞄准集成验收。
    Integrated,
    /// 田园视觉样板、自由相机与首页往返。
    Pastoral,
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
    /// 战斗 HUD：开火后装填条按真实冷却推进、满格常驻，生命条满血。
    Hud,
    /// 炮镜视角全量行为断言（工单 05）：Shift 槽位互换与长按过滤、炮镜相机
    /// 位姿逐帧同步、副窗视口矩形换算、第三人称无准星哨兵、弹丸沿炮口方向、
    /// 切换前后调炮角不变，以及分划显隐与读数抽查。
    GunnerSight,
    /// 真实渲染链路的模拟高 DPI 进入对局与连续调整尺寸。
    WindowResize,
}

impl Scenario {
    /// 计帧从资源就绪并完成落地预热后开始；0 为动作前基准图。
    pub(super) fn frames(self) -> &'static [u32] {
        match self {
            Self::Scene => &[0],
            Self::Articulation => &[
                0, 1, 2, 3, 30, 31, 36, 120, 210, 211, 420, 421, 450, 749, 750, 809, 869, 870, 871,
                877, 880, 887,
            ],
            Self::Integrated => &[0, 60, 100, 101, 150, 174, 194, 270, 330, 360, 397, 398],
            // 先在障碍前直行 1 秒，再持续前进左转以检查车体偏航。
            Self::Drive => &[0, 60, 180],
            Self::Combat => &[0, 1, 6, 24, 48, 60, 120],
            // 开火 1 秒后看装填中段；480 帧（8 秒）超过 7.5 秒冷却，看满格常驻。
            Self::Hud => &[0, 60, 480],
            // 帧 0 第三人称基准；帧 1..=5 按住 Shift 进炮镜主槽（验证长按不连发）；
            // 30/60/90 帧读分划显隐与调炮后的读数，帧 91 再按 Shift 退回第三人称，
            // 帧 101 开火验证弹道沿炮口，帧 120 断言分划整体隐藏且槽位复原。
            Self::GunnerSight => &[0, 30, 60, 90, 120],
            Self::Wheels => &[
                0, 5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55, 60, 75, 90, 105, 120, 130, 240, 250,
            ],
            // 菜单关键帧按真实阶段动态计帧，固定列表只声明基准图。
            Self::Index | Self::Gallery | Self::Detail | Self::Pastoral | Self::WindowResize => &[0],
        }
    }

    /// 按固定模拟帧注入按键，经过游戏现有输入系统，而非直接调用业务逻辑。
    pub(super) fn keys(self, frame: u32) -> &'static [KeyCode] {
        match (self, frame) {
            (Self::Pastoral, 20..=80) => &[KeyCode::KeyW],
            (Self::Drive, 1..=60) => &[KeyCode::ArrowUp],
            (Self::Drive, 61..=180) => &[KeyCode::ArrowUp, KeyCode::ArrowLeft],
            (Self::Combat, 1) | (Self::Articulation, 1..=3) => &[KeyCode::Space],
            (Self::Integrated, 1..=60) => &[KeyCode::ArrowUp],
            (Self::Integrated, 100) => &[
                KeyCode::ArrowUp,
                KeyCode::ArrowLeft,
                KeyCode::KeyA,
                KeyCode::KeyW,
                KeyCode::Space,
            ],
            // 上下各按 60 帧，经过相同加速后回到水平。
            (Self::Integrated, 61..=120) => &[
                KeyCode::ArrowUp,
                KeyCode::ArrowLeft,
                KeyCode::KeyA,
                KeyCode::KeyW,
            ],
            (Self::Integrated, 121..=150) => &[KeyCode::ArrowUp, KeyCode::ArrowLeft, KeyCode::KeyA],
            (Self::Integrated, 151..=170) => &[KeyCode::ArrowUp, KeyCode::ArrowLeft],
            (Self::Integrated, 174 | 194 | 336) => &[KeyCode::Space],
            (Self::Integrated, 211..=270) => &[KeyCode::KeyS],
            (Self::Integrated, 271..=330) => &[KeyCode::KeyW],
            (Self::Integrated, 337..=360) => &[KeyCode::KeyA],
            (Self::Integrated, 361) => &[KeyCode::KeyD, KeyCode::KeyW],
            (Self::Integrated, 362) => &[KeyCode::KeyW],
            (Self::Integrated, 364 | 366) => &[KeyCode::KeyA],
            (Self::Integrated, 380..=390) => &[KeyCode::KeyQ],
            (Self::Integrated, 398) => &[KeyCode::KeyA],
            (Self::Integrated, 400 | 506) => &[KeyCode::KeyA, KeyCode::KeyW],
            (Self::Integrated, 501) => &[KeyCode::Escape],
            (Self::Integrated, 502) => &[KeyCode::Enter],
            (Self::Integrated, 503) => &[KeyCode::ArrowDown],
            (Self::Integrated, 505) => &[KeyCode::Enter],
            (Self::Articulation, 31..=90) => &[KeyCode::KeyA, KeyCode::KeyW, KeyCode::ArrowLeft],
            (Self::Articulation, 91..=210) => &[KeyCode::KeyA, KeyCode::KeyW],
            (Self::Articulation, 211 | 421) => &[KeyCode::Space],
            (Self::Articulation, 750..=809) => {
                &[KeyCode::ArrowUp, KeyCode::ArrowLeft, KeyCode::KeyA]
            }
            (Self::Articulation, 810..=869) => {
                &[KeyCode::ArrowDown, KeyCode::ArrowRight, KeyCode::KeyD]
            }
            (Self::Articulation, 871) => &[KeyCode::KeyA],
            (Self::Articulation, 872..=877 | 883..=887) => &[KeyCode::KeyA, KeyCode::KeyW],
            (Self::Articulation, 879..=880) => &[KeyCode::KeyW],
            (Self::Articulation, 878 | 881) => &[KeyCode::KeyD, KeyCode::KeyW],
            (Self::Articulation, 882) => &[KeyCode::KeyA, KeyCode::KeyD, KeyCode::KeyW],
            (Self::Articulation, 451..=749) => &[KeyCode::KeyA],
            (Self::Articulation, 212..=420) => &[KeyCode::KeyD, KeyCode::KeyS],
            (Self::Detail, 100..=279) => &[KeyCode::KeyA, KeyCode::KeyW],
            (Self::Detail, 280..=489) => &[KeyCode::KeyD, KeyCode::KeyS],
            (Self::Hud, 1) => &[KeyCode::Space],
            // 单按 Shift 进炮镜主槽；按住 5 帧验证长按不连发；随后左调炮抬
            // 炮管、再右调炮，供读数抽查；帧 91 单按退回，帧 101 开火。
            (Self::GunnerSight, 1..=5) => &[KeyCode::ShiftLeft],
            (Self::GunnerSight, 11..=40) => &[KeyCode::KeyA, KeyCode::KeyW, KeyCode::KeyQ],
            (Self::GunnerSight, 41..=60) => &[KeyCode::KeyD, KeyCode::KeyQ],
            (Self::GunnerSight, 61..=90) => &[KeyCode::KeyD, KeyCode::KeyE],
            (Self::GunnerSight, 91) => &[KeyCode::ShiftLeft],
            (Self::GunnerSight, 101) => &[KeyCode::Space],
            (Self::Wheels, 1..=30) => &[KeyCode::ArrowUp],
            (Self::Wheels, 31..=60) => &[KeyCode::ArrowDown],
            (Self::Wheels, 61..=90) => &[KeyCode::ArrowLeft],
            (Self::Wheels, 91..=120) => &[KeyCode::ArrowRight],
            (Self::Index | Self::Gallery | Self::Detail | Self::Pastoral | Self::WindowResize, 1) => {
                &[KeyCode::ArrowDown]
            }
            (Self::Index | Self::Gallery | Self::Detail | Self::Pastoral | Self::WindowResize, 3) => &[KeyCode::Enter],
            (Self::Index | Self::Gallery | Self::Detail | Self::Pastoral, 5) => &[KeyCode::Escape],
            (Self::Index | Self::Gallery | Self::Detail | Self::Pastoral, 7) => &[KeyCode::Tab],
            (Self::Index | Self::Gallery | Self::Detail | Self::Pastoral, 8) => {
                &[KeyCode::Tab, KeyCode::ShiftLeft]
            }
            // 首页退出确认弹框：← 在两按钮间切换选中。
            (Self::Index, 15) => &[KeyCode::ArrowLeft],
            (Self::Gallery | Self::Detail, 11) => &[KeyCode::End],
            (Self::Gallery | Self::Detail, 12) => &[KeyCode::Home],
            (Self::Gallery | Self::Detail, 13) => &[KeyCode::PageDown],
            (Self::Gallery | Self::Detail, 14) => &[KeyCode::ArrowDown],
            (Self::Gallery | Self::Detail, 16) => &[KeyCode::ArrowRight],
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
    if options.scenario == Scenario::WindowResize {
        return window_resize::run(app, options, report);
    }
    if options.scenario == Scenario::Pastoral {
        return pastoral::run(app, options, report);
    }
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
        "轮组目标：玩家7+2\n"
    });
    for _ in 0..120 {
        update(app, STEP, None)?;
    }
    validate_scene(app.world_mut())?;
    scene::validate_concrete(app.world_mut())?;
    report.push_str("断言通过：唯一混凝土墩静态刚体、VOX 实例与实际网格尺寸、贴地位置\n");
    if options.scenario == Scenario::Combat {
        let world = app.world_mut();
        let obstacles: Vec<_> = world.query_filtered::<Entity, With<crate::gameplay::obstacle::Concrete>>().iter(world).collect();
        for obstacle in obstacles { world.despawn(obstacle); }
    }
    if options.scenario == Scenario::Integrated {
        return integrated::run(app, options, report);
    }
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
    if options.scenario == Scenario::GunnerSight {
        // 帧 0 为动作前基准：槽位归属、准星哨兵与调炮角基线在进入循环前断言
        // （模拟循环从帧 1 开始，帧 0 断言无法在循环内覆盖）。
        sight::check(app, 0, report, &mut failures)?;
    }
    if options.scenario == Scenario::Scene && !options.single_image {
        capture_concrete(app, &options.output.join("concrete.png"))?;
        report.push_str("混凝土墩近景：concrete.png（截图等待不推进模拟时间）\n");
    }
    for frame in 1..=*options.scenario.frames().last().unwrap() {
        if options.scenario == Scenario::Articulation {
            articulation::prepare(app.world_mut(), frame)?;
        }
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
            if frame == 1 {
                capture_bullet(app, &options.output.join("bullet-close.png"))?;
                report.push_str("弹丸出生近景：bullet-close.png\n");
            }
        }
        if options.scenario == Scenario::Articulation {
            articulation::check(app.world_mut(), frame, report)?;
        }
        if options.scenario == Scenario::Hud {
            hud::check(app, frame, report, &mut failures)?;
        }
        if options.scenario == Scenario::GunnerSight {
            sight::check(app, frame, report, &mut failures)?;
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
                if options.scenario == Scenario::Articulation && frame <= 3 {
                    let (player, _) = subject_state(app.world_mut())?;
                    capture_at(
                        app,
                        &options.output.join(format!("joint-{frame}.png")),
                        player.translation + Vec3::Y * 0.9,
                        Vec3::new(-5.5, 3.0, 6.0),
                    )?;
                }
            }
            report.push_str(&format!("图片：{}\n", options.image_path(frame).display()));
        }
    }
    if options.scenario == Scenario::Articulation {
        articulation::check_step_sizes(app, report)?;
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
        assert_eq!(Scenario::Integrated.keys(60), &[KeyCode::ArrowUp]);
        assert_eq!(
            Scenario::Integrated.keys(100),
            &[KeyCode::ArrowUp, KeyCode::ArrowLeft, KeyCode::KeyA, KeyCode::KeyW, KeyCode::Space]
        );
        assert_eq!(
            Scenario::Integrated.keys(61),
            &[
                KeyCode::ArrowUp,
                KeyCode::ArrowLeft,
                KeyCode::KeyA,
                KeyCode::KeyW
            ]
        );
        assert_eq!(
            Scenario::Integrated.keys(121),
            &[KeyCode::ArrowUp, KeyCode::ArrowLeft, KeyCode::KeyA]
        );
        assert_eq!(Scenario::Integrated.keys(151), &[KeyCode::ArrowUp, KeyCode::ArrowLeft]);
        assert!(Scenario::Integrated.keys(171).is_empty());
        assert_eq!(Scenario::Integrated.keys(174), &[KeyCode::Space]);
        assert!(Scenario::Integrated.keys(397).is_empty());
        assert_eq!(Scenario::Integrated.keys(398), &[KeyCode::KeyA]);
        assert_eq!(Scenario::Integrated.keys(501), &[KeyCode::Escape]);
        assert_eq!(Scenario::Integrated.keys(505), &[KeyCode::Enter]);
        assert_eq!(Scenario::Index.keys(1), &[KeyCode::ArrowDown]);
        assert_eq!(Scenario::Index.keys(3), &[KeyCode::Enter]);
        assert_eq!(Scenario::Index.keys(5), &[KeyCode::Escape]);
        assert!(Scenario::Index.keys(2).is_empty());
        assert_eq!(Scenario::Gallery.keys(16), &[KeyCode::ArrowRight]);
    }
}

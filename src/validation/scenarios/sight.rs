//! 炮镜视角离屏断言（工单 05）：真实按键驱动下的外部行为验收。
//!
//! 覆盖 spec「测试决策」七项：
//! 1. 炮镜相机位姿逐帧等于炮管世界位姿（无滞后，含调炮与切换过程中）；
//! 2. 单按 Shift 主副槽互换（全屏/矩形归属 + 分划显隐翻转），按住多帧不连发；
//! 3. 第三人称主槽无准星组件（工单 01 删除的回归哨兵）；
//! 4. 开火弹丸方向 = 炮口朝向；
//! 5. 副窗视口矩形 = 设计像素 × UiScale × DPR（离屏基线 1280×720/DPR 1；
//!    resize 重算用例由 gunner.rs 单元测试覆盖——离屏的模拟窗口机制已随旧
//!    aiming 预设删除，重建成本大于收益）；
//! 6. 切换视角前后炮塔/炮管角度不变；
//! 7. 关键帧由 mod.rs 按帧表输出，按 AGENTS.md 人工读图验收。

use super::*;
use crate::gameplay::{
    combat::firing::{Bullet, PlayerBarrel},
    gunner::{GunnerCamera, GunnerOptics, PipCross, MAIN_ORDER, SUB_ORDER, ViewMode, gunner_camera_transform},
    sight::{PitchReadout, RangeReadout, SightRoot, TurretReadout, ZoomReadout},
};

/// 跨帧行为基线：视角切换前记录的炮塔/炮管角与逐帧位姿断言统计。
///
/// `check` 每模拟帧调用一次、无自身状态，故用资源在帧间携带比较基准。
#[derive(Resource, Default)]
struct Baseline {
    /// 帧 0 / 帧 90 记录的炮塔角；用于断言 Shift 切换不带动调炮。
    aim: Option<crate::tank::articulation::TankAim>,
    /// 已执行的逐帧位姿比对帧数。
    pose_checks: u32,
    /// 位姿偏离炮管世界位姿的帧数（0 即断言 1 通过）。
    pose_failures: u32,
}

/// 断言副窗视口矩形的设计基准（离屏无窗口实体，回退 1280×720、DPR 1、
/// UiScale 1）：320×180 设计像素、距右/上缘 24px。数值在此硬编码而非调用
/// `pip_viewport` 计算，避免断言与实现共用同一函数导致换算错误互相掩盖。
fn expected_pip() -> (UVec2, UVec2) {
    (UVec2::new(1280 - 320 - 24, 24), UVec2::new(320, 180))
}

/// 帧边界检查（工单 05 全量断言）。帧 0 由 mod.rs 在进入模拟循环前显式调用，
/// 其余帧在循环内逐帧调用；位姿比对覆盖全部帧，槽位/读数等只按关键帧断言。
pub(super) fn check(
    app: &mut App,
    frame: u32,
    report: &mut String,
    failures: &mut Vec<String>,
) -> Result<(), String> {
    // 关键帧之外的帧只做逐帧位姿比对（断言 1），不产生报告噪音。
    let tracked = matches!(frame, 0 | 1..=5 | 30 | 60 | 90 | 120 | 102);
    let world = app.world_mut();
    // 帧 0 即写入位姿统计，先确保跨帧基线资源存在。
    world.init_resource::<Baseline>();

    // ── 断言 1：炮镜相机位姿每帧严格取炮管世界位姿（无滞后）。
    // sync 系统在 PostUpdate 传播之后运行，此处读取的是同一帧最终状态；
    // 位姿按局部 Transform 比对（传播后 GlobalTransform 会滞后一帧），
    // "局部即世界"依赖相机为根实体，该前提在下方随关键帧显式断言。
    let barrel = *world
        .query_filtered::<&GlobalTransform, With<PlayerBarrel>>()
        .single(world)
        .map_err(|error| format!("炮管缺失：{error}"))?;
    // 立即取出需要的拷贝值（含"是否为根实体"），避免借引用阻塞后续可变访问。
    let (camera, camera_is_root) = {
        let (camera, camera_parent) = world
            .query_filtered::<(&Transform, Option<&ChildOf>), With<GunnerCamera>>()
            .single(world)
            .map_err(|error| format!("炮镜相机缺失：{error}"))?;
        (*camera, camera_parent.is_none())
    };
    let expected = gunner_camera_transform(&barrel);
    let pose_ok = camera.translation.abs_diff_eq(expected.translation, 1e-4)
        && camera.rotation.abs_diff_eq(expected.rotation, 1e-4);
    if !pose_ok {
        failures.push(format!(
            "帧 {frame} 炮镜相机位姿偏离炮管世界位姿：位置 {:?} / 期望 {:?}",
            camera.translation, expected.translation
        ));
    }
    {
        let mut baseline = world.resource_mut::<Baseline>();
        baseline.pose_checks += 1;
        baseline.pose_failures += u32::from(!pose_ok);
    }
    if !tracked {
        return Ok(());
    }

    // ── 当前模式、炮塔角与两台相机的槽位参数。
    let mode = *world.resource::<ViewMode>();
    let aim = *world
        .query_filtered::<&crate::tank::articulation::TankAim, With<crate::gameplay::player::Tank>>()
        .single(world)
        .map_err(|error| format!("玩家炮塔缺失：{error}"))?;
    let follow = world
        .query_filtered::<&Camera, With<crate::gameplay::camera::FollowCamera>>()
        .single(world)
        .map_err(|error| format!("跟随相机缺失：{error}"))?
        .clone();
    let gunner = world
        .query_filtered::<&Camera, With<GunnerCamera>>()
        .single(world)
        .map_err(|error| format!("炮镜相机参数缺失：{error}"))?
        .clone();

    // ── 断言 2/5：槽位归属（主槽全屏、副槽矩形）与副窗矩形换算。
    // "局部即世界"前提：炮镜相机必须是根实体，否则断言 1 的局部位姿比对
    // 会与实际世界位姿脱钩，故此不变量随关键帧显式钉死。
    record_check(
        report,
        failures,
        camera_is_root,
        &format!("帧 {frame} 炮镜相机为根实体（无父节点）"),
    );
    let gunner_main = matches!(mode, ViewMode::GunnerMain);
    let (pip_position, pip_size) = expected_pip();
    let slots_ok = if gunner_main {
        gunner.viewport.is_none()
            && gunner.order == MAIN_ORDER
            && follow.viewport.as_ref().is_some_and(|viewport| {
                viewport.physical_position == pip_position && viewport.physical_size == pip_size
            })
            && follow.order == SUB_ORDER
    } else {
        follow.viewport.is_none()
            && follow.order == MAIN_ORDER
            && gunner.viewport.as_ref().is_some_and(|viewport| {
                viewport.physical_position == pip_position && viewport.physical_size == pip_size
            })
            && gunner.order == SUB_ORDER
    };
    let slot_label = if gunner_main {
        "炮镜主/第三人称副"
    } else {
        "第三人称主/炮镜副"
    };
    record_check(
        report,
        failures,
        slots_ok,
        &format!(
            "帧 {frame} 槽位 {slot_label}：主槽全屏 order {MAIN_ORDER}、副槽矩形 {pip_position}×{pip_size} order {SUB_ORDER}"
        ),
    );

    // ── 长按不连发：帧 2..=5 Shift 仍按住，若实现退化成每帧重触发，
    // 模式会在两种视角间来回翻转；这里逐帧钉死必须仍处于炮镜主槽，
    // 而不是依赖下方按当前模式自适应的槽位断言。
    if (2..=5).contains(&frame) {
        record_check(
            report,
            failures,
            gunner_main,
            &format!("帧 {frame} 长按 Shift 不连发（仍为炮镜主槽）"),
        );
    }

    // ── 断言 3：第三人称主槽无准星组件（回归哨兵）。
    // 工单 01 已删除 reticle 组件与插件；若有人重新引入准星组件并生成实体，
    // 其组件类型会在本世界注册，按类型名扫描即可发现。
    if matches!(frame, 0 | 120) {
        let hits: Vec<String> = world
            .components()
            .iter_registered()
            .map(|(_, info)| info.name().to_string())
            .filter(|name| name.contains("Reticle") || name.contains("Crosshair"))
            .collect();
        record_check(
            report,
            failures,
            hits.is_empty(),
            &format!("帧 {frame} 第三人称主槽无准星组件（扫描命中：{hits:?}）"),
        );
    }

    // ── 分划 HUD 显隐翻转（断言 2 的 UI 部分）：帧 1 切换当帧可能因显隐系统
    // 与槽位系统同帧调度顺序而滞后，故从帧 2 起断言可见；退回后帧 120 断言隐藏。
    let expect_hidden = matches!(frame, 0 | 120);
    if expect_hidden || (2..=90).contains(&frame) {
        let visibility = *world
            .query_filtered::<&Visibility, With<SightRoot>>()
            .single(world)
            .map_err(|error| format!("分划根节点缺失：{error}"))?;
        let expected_visibility = if expect_hidden {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
        record_check(
            report,
            failures,
            visibility == expected_visibility,
            &format!(
                "帧 {frame} 分划显隐 {}（期望 {}）",
                describe(visibility),
                describe(expected_visibility)
            ),
        );
    }

    if matches!(frame, 0 | 30 | 120) {
        let crosses: Vec<Visibility> = world.query_filtered::<&Visibility, With<PipCross>>()
            .iter(world).copied().collect();
        let expected = if gunner_main { Visibility::Hidden } else { Visibility::Inherited };
        record_check(report, failures, crosses.len() == 4 && crosses.iter().all(|v| *v == expected),
            &format!("帧 {frame} 副窗四臂十字显隐正确"));

    }

    // 实际输入驱动倍率，并核对炮镜读数与运行时状态。
    if matches!(frame, 30 | 60 | 90 | 120) {
        let optics = world.resource::<GunnerOptics>();
        let zoom_value = optics.zoom;
        let range_value = optics.range;
        let zoom_text = world.query_filtered::<&Text, With<ZoomReadout>>().single(world)
            .map_err(|e| format!("倍率读数缺失：{e}"))?.as_str().to_string();
        let range_text = world.query_filtered::<&Text, With<RangeReadout>>().single(world)
            .map_err(|e| format!("距离读数缺失：{e}"))?.as_str().to_string();
        let expected_range = range_value.map_or_else(|| "--".to_string(), |d| format!("{:.0} m", d.round()));
        record_check(report, failures, zoom_text == format!("×{zoom_value:.1}") && range_text == expected_range,
            &format!("帧 {frame} 倍率与测距读数：{zoom_text} / {range_text}"));
        let expected_zoom = match frame { 30 => 1.0 + 20.0 * 11.0 / 180.0, 60 => 1.0 + 50.0 * 11.0 / 180.0,
            _ => 1.0 + 20.0 * 11.0 / 180.0 };
        record_check(report, failures, (zoom_value - expected_zoom).abs() < 0.03,
            &format!("帧 {frame} Q/E 缩放与视图切换保留：{zoom_value:.2}（期望 {expected_zoom:.2}）"));
    }

    // ── 断言 6：视角切换前后炮塔/炮管角度不变。
    if frame == 0 {
        world.resource_mut::<Baseline>().aim = Some(aim);
    }
    if (1..=5).contains(&frame) || frame == 120 {
        let baseline = world.resource::<Baseline>();
        let recorded = baseline.aim.expect("基准帧已记录炮塔角");
        record_check(
            report,
            failures,
            recorded.yaw == aim.yaw && recorded.pitch == aim.pitch,
            &format!(
                "帧 {frame} 切换视角后炮塔/炮管角不变：yaw {:.4} pitch {:.4}",
                aim.yaw, aim.pitch
            ),
        );
    }
    if frame == 90 {
        // 退回第三人称前最后一次记录：帧 120 与之比较（中间无调炮输入）。
        world.resource_mut::<Baseline>().aim = Some(aim);
    }

    // ── 断言 4：开火弹丸方向 = 炮口朝向（帧 101 注入空格，帧 102 检查出生弹）。
    if frame == 102 {
        let bullet = world
            .query::<&Bullet>()
            .single(world)
            .map_err(|error| format!("开火后未找到弹丸：{error}"))?;
        record_check(
            report,
            failures,
            bullet.direction.abs_diff_eq(barrel.rotation() * Vec3::NEG_X, 1e-4),
            &format!("帧 {frame} 弹丸方向沿炮口：{:?}", bullet.direction),
        );
    }

    // ── 读数格式与实时值（沿用工单 04 抽查断言）。
    if matches!(frame, 30 | 60 | 90) {
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
    }

    // ── 收尾：逐帧位姿断言汇总与关键帧说明。
    if frame == 120 {
        let baseline = world.resource::<Baseline>();
        record_check(
            report,
            failures,
            baseline.pose_failures == 0,
            &format!(
                "炮镜相机位姿逐帧同步：{} 帧比对、{} 帧失败",
                baseline.pose_checks, baseline.pose_failures
            ),
        );
        report.push_str("关键帧 0/30/60/90/120 已输出：第三人称基准、炮镜主槽与退回后画面\n");
    }
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

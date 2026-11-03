//! 炮镜分划 HUD：全屏 UI 层，仅在炮镜占主槽时显示（工单 04）。
//!
//! 形状/间距/配色逐项对照 web/gameplay.html 的 `.sight` 与 design.md
//! 「第一人称炮镜」：中央十字暖白、角度刻度装饰与读数浅绿。分划挂在现有
//! HUD 相机宿主（与状态簇同层，沿用 UiScale 体系），挂载归属复用
//! [`hud::attach_ui_target`](crate::gameplay::hud) 按主槽相机逐帧校对；
//! 角度读数由 [`TankAim`] 驱动；倍率与距离由炮镜状态驱动。
//!
//! 与原型的刻意差异：CSS 投影（text-shadow/drop-shadow）Bevy 无对应能力，
//! 暂不移植；字距 1px 无对应
//! 排版参数，不做。

use bevy::prelude::*;
use bevy::ui::{BackgroundGradient, ColorStop, RadialGradient, RadialGradientShape, UiPosition};

use crate::app::state::GameState;
use crate::gameplay::gunner::ViewMode;
use crate::gameplay::player;
use crate::gameplay::session::SessionEntity;
use crate::tank::articulation::TankAim;
use crate::ui::font::UiFont;
use crate::ui::resources::{colors, dimens};
use crate::ui::widgets::text_font;

/// 全屏炮镜分划根节点，分划定位在其中心锚点；
/// 显隐由 [`update`] 按 [`ViewMode`] 每帧校对，无残影。
#[derive(Component, Default, Clone)]
pub(crate) struct SightRoot;

/// 俯仰角读数节点标记；文本每帧由 [`update`] 写入。
#[derive(Component, Default, Clone)]
pub(crate) struct PitchReadout;

/// 炮塔旋转角读数节点标记；文本每帧由 [`update`] 写入。
#[derive(Component, Default, Clone)]
pub(crate) struct TurretReadout;

/// 倍率读数标记。
#[derive(Component)]
pub(crate) struct ZoomReadout;

/// 实时测距读数标记。
#[derive(Component)]
pub(crate) struct RangeReadout;

/// 线段端点 (x0, y0, x1, y1)，坐标为原型 SVG viewBox(440×460) 系；
/// 竖线 x0==x1，横线 y0==y1。屏幕映射：SVG 中心 (220,230) = 屏幕中心。
type Seg = (f32, f32, f32, f32);

/// 中央十字（四臂 12px 中央空隙）与下方四条弹道短分划，暖白 2px。
/// 弹道分划仅按原型移植形状，对应弹道值待校准。
const RETICLE: [Seg; 8] = [
    // 十字：左右臂各 34px、上臂 34px、下臂 20px，中心空隙 12px
    (180.0, 230.0, 214.0, 230.0),
    (226.0, 230.0, 260.0, 230.0),
    (220.0, 190.0, 220.0, 224.0),
    (220.0, 236.0, 220.0, 256.0),
    // 弹道短分划：18px 宽、间距 16px
    (211.0, 268.0, 229.0, 268.0),
    (211.0, 284.0, 229.0, 284.0),
    (211.0, 300.0, 229.0, 300.0),
    (211.0, 316.0, 229.0, 316.0),
];

/// 角度刻度静态装饰，浅绿 1.5px：左侧竖刻度对应俯仰角、上方横刻度对应
/// 炮塔旋转角；主线居中，中心长刻度 14px、短刻度 8px、间隔 20px，
/// 距中心约 170px 外移布局，避开中央瞄准视线。
const SCALE_DECOR: [Seg; 18] = [
    // 左侧竖刻度：主线 x=50、y 174..286
    (50.0, 174.0, 50.0, 286.0),
    (43.0, 230.0, 57.0, 230.0),
    (46.0, 174.0, 54.0, 174.0),
    (46.0, 194.0, 54.0, 194.0),
    (46.0, 214.0, 54.0, 214.0),
    (46.0, 246.0, 54.0, 246.0),
    (46.0, 266.0, 54.0, 266.0),
    (46.0, 286.0, 54.0, 286.0),
    // 上方横刻度：主线 y=60、x 126..314；短刻度为垂直短线、骑跨主线
    (126.0, 60.0, 314.0, 60.0),
    (220.0, 53.0, 220.0, 67.0),
    (140.0, 56.0, 140.0, 64.0),
    (160.0, 56.0, 160.0, 64.0),
    (180.0, 56.0, 180.0, 64.0),
    (200.0, 56.0, 200.0, 64.0),
    (240.0, 56.0, 240.0, 64.0),
    (260.0, 56.0, 260.0, 64.0),
    (280.0, 56.0, 280.0, 64.0),
    (300.0, 56.0, 300.0, 64.0),
];

/// SVG viewBox 中心的横/纵坐标；线段端点转中心相对偏移时的换算基准。
const SVG_CENTER_X: f32 = 220.0;
const SVG_CENTER_Y: f32 = 230.0;

/// 主视野和炮镜小窗共用的渐晕，半轴随承载节点尺寸等比缩放。
pub(crate) fn lens_vignette() -> BackgroundGradient {
    // 半轴取视口宽高的一半，100% 对应边缘中点，四角约为 141.4%。
    BackgroundGradient::from(RadialGradient::new(
        UiPosition::CENTER,
        RadialGradientShape::Ellipse(percent(50), percent(50)),
        vec![
            ColorStop::percent(colors::ON_PRIMARY.with_alpha(0.0), 72),
            ColorStop::percent(colors::ON_PRIMARY.with_alpha(0.23), 100),
            ColorStop::percent(colors::ON_PRIMARY.with_alpha(0.60), 125),
            ColorStop::percent(colors::ON_PRIMARY.with_alpha(0.90), 142),
        ],
    ))
}

/// 进入对局生成分划；退出清理由 SessionEntity 的统一回收系统完成。
///
/// UI 相机挂载不在此处绑定：复用 hud::attach_ui_target 按主槽归属逐帧补挂。
pub(crate) fn setup(mut commands: Commands, font: Res<UiFont>) {
    let root = commands
        .spawn((
            SightRoot,
            SessionEntity,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            // 默认隐藏，炮镜占主槽时显示分划。
            Visibility::Hidden,
        ))
        .id();
    // 保留以屏幕中心为原点的分划坐标。
    let anchor = commands.spawn((
        ChildOf(root),
        Node {
            position_type: PositionType::Absolute,
            left: percent(50),
            top: percent(50),
            width: px(0.0),
            height: px(0.0),
            ..default()
        },
    )).id();
    commands.entity(anchor).with_children(|parent| {
        for (x0, y0, x1, y1) in RETICLE {
            parent.spawn(seg_node(
                x0,
                y0,
                x1,
                y1,
                dimens::sight::RETICLE_STROKE,
                colors::sight::CROSS,
            ));
        }
        for (x0, y0, x1, y1) in SCALE_DECOR {
            parent.spawn(seg_node(
                x0,
                y0,
                x1,
                y1,
                dimens::sight::SCALE_STROKE,
                colors::sight::SCALE,
            ));
        }
        // 俯仰角读数：右缘锚定 center−184（父节点宽 0，right 即到中心距离），
        // 顶边 −半行高使行盒垂直居中对齐刻度中线（等效 CSS translateY(-50%)）。
        parent.spawn((
            PitchReadout,
            Node {
                position_type: PositionType::Absolute,
                right: px(dimens::sight::PITCH_RIGHT),
                top: px(-dimens::sight::READOUT_LINE_HEIGHT * 0.5),
                ..default()
            },
            Text::new("+00.0°"),
            text_font(&font, dimens::sight::READOUT_SIZE, 400.0),
            TextColor(colors::sight::SCALE),
        ));
        // 炮塔角读数：顶边 center−154，固定宽盒水平居中（等效 translateX(-50%)）。
        parent.spawn((
            TurretReadout,
            Node {
                position_type: PositionType::Absolute,
                left: px(-dimens::sight::TURRET_BOX_WIDTH * 0.5),
                top: px(-dimens::sight::TURRET_TOP),
                width: px(dimens::sight::TURRET_BOX_WIDTH),
                ..default()
            },
            Text::new("+000.0°"),
            TextLayout::justify(Justify::Center),
            text_font(&font, dimens::sight::READOUT_SIZE, 400.0),
            TextColor(colors::sight::SCALE),
        ));
        parent.spawn((ZoomReadout, Node {
            position_type: PositionType::Absolute,
            left: px(92.0), top: px(-dimens::sight::READOUT_LINE_HEIGHT * 0.5), ..default()
        }, Text::new("×1.0"), text_font(&font, dimens::sight::READOUT_SIZE, 400.0),
            TextColor(Color::srgb_u8(169, 201, 231))));
        parent.spawn((RangeReadout, Node {
            position_type: PositionType::Absolute,
            left: px(-60.0), top: px(92.0), width: px(120.0), ..default()
        }, Text::new("--"), TextLayout::justify(Justify::Center),
            text_font(&font, dimens::sight::READOUT_SIZE, 400.0),
            TextColor(Color::srgb_u8(231, 188, 120))));
    });
}

/// 倍率和距离文本由炮镜状态驱动，未命中时清除旧值。
fn update_optics_readouts(
    optics: Res<crate::gameplay::gunner::GunnerOptics>,
    mut zoom: Query<&mut Text, (With<ZoomReadout>, Without<RangeReadout>)>,
    mut range: Query<&mut Text, With<RangeReadout>>,
) {
    if let Ok(mut text) = zoom.single_mut() { *text = Text::new(format!("×{:.1}", optics.zoom)); }
    if let Ok(mut text) = range.single_mut() {
        *text = Text::new(optics.range.map_or_else(|| "--".to_string(), |distance| format!("{:.0} m", distance.round())));
    }
}

/// 一条分划线：SVG 端点转中心相对偏移的绝对定位矩形（等效 stroke，
/// 矩形中心线与线段重合，故单侧各让半个线宽）。
fn seg_node(x0: f32, y0: f32, x1: f32, y1: f32, stroke: f32, color: Color) -> impl Bundle {
    let (left, top, width, height) = if x0 == x1 {
        (
            x0 - SVG_CENTER_X - stroke * 0.5,
            y0.min(y1) - SVG_CENTER_Y,
            stroke,
            (y1 - y0).abs(),
        )
    } else {
        (
            x0.min(x1) - SVG_CENTER_X,
            y0 - SVG_CENTER_Y - stroke * 0.5,
            (x1 - x0).abs(),
            stroke,
        )
    };
    (
        Node {
            position_type: PositionType::Absolute,
            left: px(left),
            top: px(top),
            width: px(width),
            height: px(height),
            ..default()
        },
        BackgroundColor(color),
    )
}

/// 每帧：按 [`ViewMode`] 校对分划显隐（炮镜主槽显示、第三人称主槽隐藏），
/// 读玩家 [`TankAim`] 更新两个角度读数文本。
///
/// 角度语义按 design.md「第一人称炮镜」：俯仰以炮管水平为零、抬炮为正；
/// 炮塔角以车体正前方为零、向右为正。坦克未就绪时只跳过文本更新，
/// 显隐校对不受影响。
pub(crate) fn update(
    mode: Res<ViewMode>,
    mut roots: Query<&mut Visibility, With<SightRoot>>,
    aims: Query<&TankAim, With<player::Tank>>,
    mut pitch: Query<&mut Text, (With<PitchReadout>, Without<TurretReadout>)>,
    mut turret: Query<&mut Text, With<TurretReadout>>,
) {
    let target = if matches!(*mode, ViewMode::GunnerMain) {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    for mut visibility in &mut roots {
        if *visibility != target {
            *visibility = target;
        }
    }
    let Ok(aim) = aims.single() else {
        return;
    };
    if let Ok(mut text) = pitch.single_mut() {
        *text = Text::new(format_pitch(aim.pitch));
    }
    if let Ok(mut text) = turret.single_mut() {
        *text = Text::new(format_turret(aim.yaw));
    }
}

/// 俯仰角读数文本：抬炮为正，格式同原型 `+02.4°`（符号 + 两位整数 + 1 位小数）。
fn format_pitch(pitch: f32) -> String {
    format_signed(pitch.to_degrees(), 2)
}

/// 炮塔角读数文本：以车体正前方为零、向右为正。
///
/// 引擎 [`TankAim::yaw`] 正值向左且归一化到 [0,2π)，故取负后折算到 (-180,180]
/// 再排版；格式同原型 `+016.0°`（符号 + 三位整数 + 1 位小数）。
fn format_turret(yaw: f32) -> String {
    format_signed(wrap_degrees(-yaw.to_degrees()), 3)
}

/// 折算到 [-180,180)：yaw=0 显示 +000.0，yaw 越过 ±180° 时从另一侧续算。
fn wrap_degrees(degrees: f32) -> f32 {
    (degrees + 180.0).rem_euclid(360.0) - 180.0
}

/// 带符号定宽角度文本。先四舍五入到 0.1° 再取符号，四舍五入后为 0 时
/// 显示 `+00.0°` 而非 `-00.0°`；总宽 = 整数位 + "0." 两字符（零填充）。
fn format_signed(degrees: f32, integer_digits: usize) -> String {
    let rounded = (degrees * 10.0).round() / 10.0;
    let sign = if rounded < 0.0 { '-' } else { '+' };
    format!(
        "{sign}{:0width$.1}°",
        rounded.abs(),
        width = integer_digits + 2
    )
}

/// 注册分划生成与逐帧更新系统。
pub(crate) struct SightPlugin;

impl Plugin for SightPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::InGame), setup)
            .add_systems(Update, update.run_if(in_state(GameState::InGame)))
            .add_systems(PostUpdate, update_optics_readouts.after(crate::gameplay::gunner::measure_range).run_if(in_state(GameState::InGame)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::font::TextOptics;
    use bevy::ecs::system::RunSystemOnce;

    /// 倍率和距离按要求显示，丢失命中时立即清空旧距离。
    #[test]
    fn optics_readouts_show_zoom_and_range() {
        let mut app = App::new();
        app.insert_resource(crate::gameplay::gunner::GunnerOptics { zoom: 4.23, range: Some(420.6) })
            .add_systems(Update, update_optics_readouts);
        let zoom = app.world_mut().spawn((ZoomReadout, Text::default())).id();
        let range = app.world_mut().spawn((RangeReadout, Text::default())).id();
        app.update();
        assert_eq!(app.world().get::<Text>(zoom).unwrap().as_str(), "×4.2");
        assert_eq!(app.world().get::<Text>(range).unwrap().as_str(), "421 m");
        app.world_mut().resource_mut::<crate::gameplay::gunner::GunnerOptics>().range = None;
        app.update();
        assert_eq!(app.world().get::<Text>(range).unwrap().as_str(), "--");
    }

    /// 俯仰角读数格式与原型一致；四舍五入后为 0 时符号取 +。
    #[test]
    fn pitch_readout_matches_prototype_format() {
        assert_eq!(format_pitch(2.4_f32.to_radians()), "+02.4°");
        assert_eq!(format_pitch(0.0), "+00.0°");
        assert_eq!(format_pitch((-9.0_f32).to_radians()), "-09.0°");
        assert_eq!(format_pitch(20.0_f32.to_radians()), "+20.0°");
        // -0.04° 四舍五入到 0：不得出现 -00.0°。
        assert_eq!(format_pitch((-0.04_f32).to_radians()), "+00.0°");
    }

    /// 炮塔角读数：车体正前方为零、向右为正；正 yaw（向左）显示为负。
    #[test]
    fn turret_readout_wraps_and_matches_prototype_format() {
        assert_eq!(format_turret(0.0), "+000.0°");
        // yaw 350°（向左 350°）= 向右 10°。
        assert_eq!(format_turret(350.0_f32.to_radians()), "+010.0°");
        // yaw +16°（向左 16°）→ -016.0°。
        assert_eq!(format_turret(16.0_f32.to_radians()), "-016.0°");
        // yaw 180° 折算边界：显示 -180.0°（恰为车体正后方）。
        assert_eq!(format_turret(180.0_f32.to_radians()), "-180.0°");
    }

    /// 最小世界：setup 仅依赖 UiFont 资源（分划用 plain commands 生成）。
    fn spike_app() -> App {
        let mut app = App::new();
        app.insert_resource(UiFont(
            Handle::default(),
            TextOptics::default(),
        ));
        app
    }

    /// 分划结构锁定：根节点、十字/弹道线、角度刻度与四个读数。
    #[test]
    fn setup_spawns_reticle_scale_and_readouts() {
        let mut app = spike_app();
        app.world_mut().run_system_once(setup).unwrap();
        let world = app.world_mut();
        let (root, children) = world
            .query_filtered::<(Entity, Option<&Children>), With<SightRoot>>()
            .single(world)
            .unwrap();
        let anchors = children.expect("炮镜根节点应持有分划锚点");
        assert_eq!(anchors.len(), 1);
        let children = world.get::<Children>(anchors[0]).expect("分划锚点应持有子实体");
        assert_eq!(
            children.len(),
            RETICLE.len() + SCALE_DECOR.len() + 4,
            "应为线段表全量 + 四个读数节点"
        );
        // 初始隐藏（默认第三人称主槽），读数标记存在。
        assert_eq!(
            *world.get::<Visibility>(root).unwrap(),
            Visibility::Hidden
        );
        assert_eq!(
            world
                .query_filtered::<Entity, With<PitchReadout>>()
                .iter(world)
                .count(),
            1
        );
        assert_eq!(
            world
                .query_filtered::<Entity, With<TurretReadout>>()
                .iter(world)
                .count(),
            1
        );
    }

    /// 显隐随 ViewMode 翻转；无坦克时文本更新静默跳过，不影响显隐。
    #[test]
    fn update_toggles_visibility_by_view_mode() {
        let mut app = spike_app();
        app.init_resource::<ViewMode>().add_systems(Update, update);
        app.world_mut().spawn((SightRoot, Visibility::Hidden));
        app.update();
        assert_eq!(
            *app.world_mut()
                .query_filtered::<&Visibility, With<SightRoot>>()
                .single(app.world())
                .unwrap(),
            Visibility::Hidden,
            "第三人称主槽应隐藏分划"
        );
        *app.world_mut().resource_mut::<ViewMode>() = ViewMode::GunnerMain;
        app.update();
        assert_eq!(
            *app.world_mut()
                .query_filtered::<&Visibility, With<SightRoot>>()
                .single(app.world())
                .unwrap(),
            Visibility::Visible,
            "炮镜主槽应显示分划"
        );
    }

    /// 读数文本随 TankAim 实时更新（系统按标记写入，这里验证端到端格式）。
    #[test]
    fn update_writes_readouts_from_tank_aim() {
        let mut app = spike_app();
        app.init_resource::<ViewMode>().add_systems(Update, update);
        // 读数与生产路径一致：两个标记各自独立实体并持有 Text。
        app.world_mut().spawn((SightRoot, Visibility::Hidden, PitchReadout, Text::default()));
        app.world_mut().spawn((TurretReadout, Text::default()));
        app.world_mut().spawn((
            player::Tank,
            TankAim {
                yaw: 350.0_f32.to_radians(),
                pitch: 2.4_f32.to_radians(),
            },
        ));
        app.update();
        let pitch = app
            .world_mut()
            .query_filtered::<&Text, With<PitchReadout>>()
            .single(app.world())
            .unwrap();
        assert_eq!(pitch.as_str(), "+02.4°");
        let turret = app
            .world_mut()
            .query_filtered::<&Text, With<TurretReadout>>()
            .single(app.world())
            .unwrap();
        assert_eq!(turret.as_str(), "+010.0°");
    }
}

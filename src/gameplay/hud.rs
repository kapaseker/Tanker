//! 战斗 HUD：左下状态簇（生命条 + 五格装填条），随对局生成与回收。
//!
//! 表现层只读：生命读玩家的 [`Health`] 组件，装填读 [`Gun`] 资源的真实
//! 冷却计时，不自建进度；布局对照网页设计稿 web/style.css 的 `.status`
//!（1280×720 参考分辨率，游戏内固定 24px 边距，无需网页的超宽取景逻辑）。
//!
//! 静态结构使用 BSN（Bevy Scene Notation）的 `bsn!` 宏声明；逐帧更新与
//! 相机挂载仍是普通系统，BSN 只负责"出生时刻"的实体树。

use bevy::prelude::*;
use bevy::scene::{bsn, CommandsSceneExt, Scene, SceneList};

use crate::app::state::GameState;
use crate::gameplay::combat::firing::{FIRE_INTERVAL, Gun};
use crate::gameplay::health::Health;
use crate::gameplay::player;
use crate::gameplay::session::SessionEntity;
use crate::ui::resources::{colors, dimens};

/// 状态簇根节点；标记 SessionEntity 复用对局统一清理，无需单独 OnExit 系统。
#[derive(Component, Default, Clone)]
pub(crate) struct HudRoot;

/// 生命条填充节点标记；宽度每帧按 [`Health::ratio`] 更新。
#[derive(Component, Default, Clone)]
pub(crate) struct HealthFill;

/// 装填条填充节点标记；宽度每帧按 [`Gun::reload_progress`] 更新。
#[derive(Component, Default, Clone)]
pub(crate) struct ReloadFill;

/// 生命条轨道节点标记；供离屏验证读取轨道宽度换算填充比例。
#[derive(Component, Default, Clone)]
pub(crate) struct HealthTrack;

/// 装填条轨道节点标记；供离屏验证读取轨道宽度换算填充比例。
#[derive(Component, Default, Clone)]
pub(crate) struct ReloadTrack;

/// 进入对局时生成状态簇；退出清理由 SessionEntity 的统一回收系统完成。
///
/// UI 目标不在此处绑定：跟随相机由 session::setup 在同一状态过渡中经命令生成，
/// 当帧查询不到，改由 [`attach_ui_target`] 逐帧补挂，显式避免多相机歧义。
///
/// 使用 queue 模式：注册字符串资产路径后等图标加载完成，由 SpawnScene 调度
/// 整树生成（比手写 spawn 晚 1-2 帧，肉眼不可感知）。
pub(crate) fn setup(mut commands: Commands) {
    let plate_border = BorderColor::all(colors::hud::PLATE_BORDER);
    commands.queue_spawn_scene(bsn! {
        HudRoot
        SessionEntity
        Node {
            position_type: PositionType::Absolute,
            left: px(dimens::hud::INSET),
            bottom: px(dimens::hud::INSET),
            width: px(dimens::hud::WIDTH),
            padding: UiRect::all(px(dimens::hud::PADDING)),
            border: UiRect::all(px(dimens::BORDER_WIDTH)),
            flex_direction: FlexDirection::Column,
        }
        // 半透明只作用于底板；填充与图标保持完全不透明。
        BackgroundColor(colors::hud::PLATE)
        plate_border
        Children [
            @hud_row("image/hud/shield.png", true, health_track())
            --
            @hud_row("image/hud/bullet.png", false, reload_track())
        ]
    });
}

/// 状态行：左侧 16px 图标 + 撑满剩余宽度的轨道；轨道场景由调用方提供。
///
/// `gap_below` 为真时行底部留出 8px 间距（网页 gap 语义，仅首行使用）。
fn hud_row(icon_path: &'static str, gap_below: bool, track: impl Scene) -> impl Scene {
    let margin = if gap_below {
        UiRect::bottom(px(dimens::hud::ROW_GAP))
    } else {
        UiRect::default()
    };
    bsn! {
        Node {
            align_items: AlignItems::Center,
            margin: {margin},
        }
        Children [
            @row_icon(icon_path)
            --
            @track
        ]
    }
}

/// 状态行左侧的 16px 图标；PNG 为 8 倍分辨率白色描边，统一着薄荷色。
///
/// 字符串路径在解析期经 AssetServer 转为 Handle（隐式 into 到 HandleTemplate）。
fn row_icon(path: &'static str) -> impl Scene {
    bsn! {
        Node {
            width: px(dimens::hud::ICON_SIZE),
            height: px(dimens::hud::ICON_SIZE),
            margin: UiRect::right(px(dimens::hud::ROW_GAP)),
        }
        ImageNode {
            image: {path},
            color: colors::PRIMARY,
        }
    }
}

/// 生命条轨道：撑满行内剩余宽度，高 10px 形成主次；内嵌绝对定位的填充。
fn health_track() -> impl Scene {
    bsn! {
        HealthTrack
        Node {
            flex_grow: 1.0,
            height: px(dimens::hud::HEALTH_TRACK_HEIGHT),
        }
        BackgroundColor(colors::hud::TRACK)
        Children [@health_fill()]
    }
}

/// 装填条轨道：略矮于生命条；填充与分格线覆盖其上。
///
/// 分格线由 [`reload_ticks`] 按"一格一秒"从冷却间隔运行时推导后拼接进列表。
fn reload_track() -> impl Scene {
    bsn! {
        ReloadTrack
        Node {
            flex_grow: 1.0,
            height: px(dimens::hud::RELOAD_TRACK_HEIGHT),
        }
        BackgroundColor(colors::hud::TRACK)
        Children [
            @reload_fill()
            --
            { reload_ticks(FIRE_INTERVAL) }
        ]
    }
}

/// 进度条填充（生命条）：绝对定位于轨道左缘，初始满宽，宽度由 update 每帧写入。
fn health_fill() -> impl Scene {
    bsn! {
        HealthFill
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            top: px(0.0),
            bottom: px(0.0),
            width: Val::Percent(100.0),
        }
        // 装填填充沿用薄荷平色近似网页的渐变；满格常驻不隐藏、不降亮度。
        BackgroundColor(colors::PRIMARY)
    }
}

/// 进度条填充（装填条）：结构同 [`health_fill`]，仅标记不同；BSN 条目不支持
/// 泛型标记组件，故拆为两个场景函数而非泛型参数。
fn reload_fill() -> impl Scene {
    bsn! {
        ReloadFill
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            top: px(0.0),
            bottom: px(0.0),
            width: Val::Percent(100.0),
        }
        BackgroundColor(colors::PRIMARY)
    }
}

/// 装填分格线列表：一格最多一秒，格数由冷却间隔推导（5 秒 → 5 格 4 线）。
///
/// 冷却来自 lance 装备射速数据（[`FIRE_INTERVAL`]），换炮后分格自动跟随，
/// UI 不持有视觉常量投影；非整数秒向上取整保证一格不超过一秒。
/// 返回 `Vec<impl Scene>` 拼接进轨道的 Children 列表（`{ expr }` 拼接语法）。
fn reload_ticks(interval: f32) -> impl SceneList {
    let count = interval.ceil() as u32;
    (1..count)
        .map(|i| tick(i as f32 * 100.0 / count as f32))
        .collect::<Vec<_>>()
}

/// 单条分格线：1px 绝对节点按 percent（0..100）覆盖填充之上（.bar__ticks）。
fn tick(percent: f32) -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(percent),
            top: px(0.0),
            bottom: px(0.0),
            width: px(1.0),
        }
        BackgroundColor(colors::hud::TICK)
    }
}

/// 每帧推进两条进度：装填映射真实冷却，生命读组件（当前固定满血）。
pub(crate) fn update(
    gun: Option<Res<Gun>>,
    healths: Query<&Health, With<player::Tank>>,
    mut health_fills: Query<&mut Node, (With<HealthFill>, Without<ReloadFill>)>,
    mut reload_fills: Query<&mut Node, With<ReloadFill>>,
) {
    if let Ok(health) = healths.single() {
        if let Ok(mut node) = health_fills.single_mut() {
            node.width = Val::Percent(health.ratio() * 100.0);
        }
    }
    if let Some(gun) = gun {
        if let Ok(mut node) = reload_fills.single_mut() {
            node.width = Val::Percent(gun.reload_progress() * 100.0);
        }
    }
}

/// 把 HUD 根节点挂到跟随相机的 UI 目标上。
///
/// 跟随相机与状态簇同帧经命令生成，命令应用前查询不到；挂载成功后
/// `Without<UiTargetCamera>` 使本系统自动跳过，不产生逐帧开销。
pub(crate) fn attach_ui_target(
    mut commands: Commands,
    follow: Query<Entity, With<crate::gameplay::camera::FollowCamera>>,
    roots: Query<Entity, (With<HudRoot>, Without<UiTargetCamera>)>,
) {
    if let (Ok(camera), Ok(root)) = (follow.single(), roots.single()) {
        commands.entity(root).insert(UiTargetCamera(camera));
    }
}

/// 注册 HUD 生成与逐帧更新系统。
pub(crate) struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::InGame), setup)
            .add_systems(
                Update,
                (attach_ui_target, update).run_if(in_state(GameState::InGame)),
            );
    }
}

#[cfg(test)]
mod bsn_spike_tests {
    use super::*;
    use bevy::asset::AssetPlugin;
    use bevy::scene::ScenePlugin;

    /// 最小无头世界：AssetServer/Assets<ScenePatch> 是 World::spawn_scene 的
    /// 硬性资源依赖（spawn.rs 直接 resource() 取用），故挂两个插件而非裸 World。
    fn spike_app() -> App {
        let mut app = App::new();
        app.add_plugins((AssetPlugin::default(), ScenePlugin::default()));
        app
    }

    /// BSN 宏行为锁定：场景列表中 `{ expr }` 可拼接返回 `Vec<impl Scene>` 的
    /// 场景函数，按预期生成实体。
    ///
    /// 依据：bsn! 语法参考"Scene Lists"节（`[ #First -- { expr } -- #Last ]`
    /// → 拼接 expr 返回的 SceneList 的每个实体）+ `impl<S: Scene> SceneList
    /// for Vec<S>`（scene_list.rs）。本测试作为宏行为回归防线，升级 Bevy 后
    /// 若解析行为变化将在此处显式失败。
    #[test]
    fn children_list_accepts_vec_scene_list() {
        #[derive(Component, Default, Clone)]
        struct SpikeRoot;

        let mut app = spike_app();
        let world = app.world_mut();
        world
            .spawn_scene(bsn! {
                SpikeRoot
                Children [{ reload_ticks(5.0) }]
            })
            .expect("BSN 宏应支持 { expr } 拼接 Vec 场景列表");
        // 断言根节点的 Children 关系而非全实体数：插件基建会生成自有实体。
        // 5 秒冷却 → 5 格 4 线。
        let children = world
            .query_filtered::<&Children, With<SpikeRoot>>()
            .single(world)
            .expect("拼接后根节点应持有 Children 关系");
        assert_eq!(children.len(), 4, "5 秒冷却应产生 4 条分格线");
    }

    /// 一格一秒语义锁定：5 秒冷却 → 4 条分格线（20/40/60/80%）；非整数秒
    /// 向上取整。防止冷却数据变更时分格失真。
    #[test]
    fn reload_ticks_derive_from_interval() {
        #[derive(Component, Default, Clone)]
        struct SpikeRoot;

        let mut app = spike_app();
        let world = app.world_mut();
        world
            .spawn_scene(bsn! {
                SpikeRoot
                Children [{ reload_ticks(5.0) }]
            })
            .unwrap();
        let children = world
            .query_filtered::<&Children, With<SpikeRoot>>()
            .single(world)
            .unwrap();
        assert_eq!(children.len(), 4, "5 秒冷却应产生 4 条分格线");

        // 非整数秒向上取整：4.5 秒 → 5 格 4 线。
        let mut app = spike_app();
        let world = app.world_mut();
        world
            .spawn_scene(bsn! {
                SpikeRoot
                Children [{ reload_ticks(4.5) }]
            })
            .unwrap();
        let children = world
            .query_filtered::<&Children, With<SpikeRoot>>()
            .single(world)
            .unwrap();
        assert_eq!(children.len(), 4, "4.5 秒向上取整为 5 格 4 线");
    }
}

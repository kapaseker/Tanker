//! 战斗 HUD：左下状态簇（生命条 + 五格装填条），随对局生成与回收。
//!
//! 表现层只读：生命读玩家的 [`Health`] 组件，装填读 [`Gun`] 资源的真实
//! 冷却计时，不自建进度；布局对照网页设计稿 web/style.css 的 `.status`
//!（1280×720 参考分辨率，游戏内固定 24px 边距，无需网页的超宽取景逻辑）。

use bevy::prelude::*;

use crate::app::state::GameState;
use crate::gameplay::combat::firing::Gun;
use crate::gameplay::health::Health;
use crate::gameplay::player;
use crate::gameplay::session::SessionEntity;
use crate::ui::resources::{colors, dimens};

/// 状态簇根节点；标记 SessionEntity 复用对局统一清理，无需单独 OnExit 系统。
#[derive(Component)]
pub(crate) struct HudRoot;

/// 生命条填充节点标记；宽度每帧按 [`Health::ratio`] 更新。
#[derive(Component)]
pub(crate) struct HealthFill;

/// 装填条填充节点标记；宽度每帧按 [`Gun::reload_progress`] 更新。
#[derive(Component)]
pub(crate) struct ReloadFill;

/// 生命条轨道节点标记；供离屏验证读取轨道宽度换算填充比例。
#[derive(Component)]
pub(crate) struct HealthTrack;

/// 装填条轨道节点标记；供离屏验证读取轨道宽度换算填充比例。
#[derive(Component)]
pub(crate) struct ReloadTrack;

/// 进入对局时生成状态簇；退出清理由 SessionEntity 的统一回收系统完成。
///
/// UI 目标不在此处绑定：跟随相机由 session::setup 在同一状态过渡中经命令生成，
/// 当帧查询不到，改由 [`attach_ui_target`] 逐帧补挂，显式避免多相机歧义。
pub(crate) fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    let shield = assets.load("image/hud/shield.png");
    let bullet = assets.load("image/hud/bullet.png");
    commands
        .spawn((
            HudRoot,
            SessionEntity,
            Node {
                position_type: PositionType::Absolute,
                left: px(dimens::hud::INSET),
                bottom: px(dimens::hud::INSET),
                width: px(dimens::hud::WIDTH),
                padding: UiRect::all(px(dimens::hud::PADDING)),
                border: UiRect::all(px(dimens::BORDER_WIDTH)),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            // 半透明只作用于底板；填充与图标保持完全不透明。
            BackgroundColor(colors::hud::PLATE),
            BorderColor::all(colors::hud::PLATE_BORDER),
        ))
        .with_children(|cluster| {
            // 生命行（上）：盾形图标 + 连续进度条，轨道高 10px 形成主次。
            cluster
                .spawn(Node {
                    align_items: AlignItems::Center,
                    // 行间距由网页 gap 8px 换成上一行的下边距。
                    margin: UiRect::bottom(px(dimens::hud::ROW_GAP)),
                    ..default()
                })
                .with_children(|row| {
                    icon(row, shield);
                    row.spawn((
                        HealthTrack,
                        Node {
                            flex_grow: 1.0,
                            height: px(dimens::hud::HEALTH_TRACK_HEIGHT),
                            ..default()
                        },
                        BackgroundColor(colors::hud::TRACK),
                    ))
                    .with_children(|track| fill(track, HealthFill));
                });
            // 装填行（下）：子弹图标 + 五格进度条，轨道略矮。
            cluster
                .spawn(Node {
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    icon(row, bullet);
                    row.spawn((
                        ReloadTrack,
                        Node {
                            flex_grow: 1.0,
                            height: px(dimens::hud::RELOAD_TRACK_HEIGHT),
                            ..default()
                        },
                        BackgroundColor(colors::hud::TRACK),
                    ))
                    .with_children(|track| {
                        fill(track, ReloadFill);
                        // 分格线：1px 绝对节点按 20% 步进覆盖填充之上（.bar__ticks）。
                        for index in 1..dimens::hud::RELOAD_TICKS {
                            track.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: Val::Percent(
                                        index as f32 * 100.0 / dimens::hud::RELOAD_TICKS as f32,
                                    ),
                                    top: px(0.0),
                                    bottom: px(0.0),
                                    width: px(1.0),
                                    ..default()
                                },
                                BackgroundColor(colors::hud::TICK),
                            ));
                        }
                    });
                });
        });
}

/// 生成状态行左侧的 16px 图标；PNG 为 8 倍分辨率白色描边，统一着薄荷色。
fn icon(parent: &mut ChildSpawnerCommands, image: Handle<Image>) {
    parent.spawn((
        Node {
            width: px(dimens::hud::ICON_SIZE),
            height: px(dimens::hud::ICON_SIZE),
            margin: UiRect::right(px(dimens::hud::ROW_GAP)),
            ..default()
        },
        ImageNode {
            image,
            color: colors::PRIMARY,
            ..default()
        },
    ));
}

/// 生成进度条填充节点：绝对定位于轨道左缘，初始满宽，宽度由 update 每帧写入。
fn fill<M: Component>(parent: &mut ChildSpawnerCommands, marker: M) {
    parent.spawn((
        marker,
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            top: px(0.0),
            bottom: px(0.0),
            width: Val::Percent(100.0),
            ..default()
        },
        // 装填填充沿用薄荷平色近似网页的渐变；满格常驻不隐藏、不降亮度。
        BackgroundColor(colors::PRIMARY),
    ));
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

//! 常驻菜单 chrome：切页过渡时标题栏与两处直角保持亮度，仅页面内容被遮罩压暗。
//!
//! chrome 相机（order 0）与页面相机（order -1）写入同一张 `SceneTarget` 纹理且不清色，
//! 输出相机（order 1）再统一合成。chrome 根的子节点按生成顺序绘制：全屏黑色遮罩 →
//! 标题栏 → 左上/右下直角；遮罩只压暗先绘制的页面内容（背景、正文、版本文字），
//! 其上的 chrome 元素保持亮度，对齐网页设计稿的 z-index 分层（遮罩 4、标题栏 5、直角 6）。

use super::state::GameState;
use super::transition::{Fade, Phase, SceneTarget};
use crate::ui::font::UiFont;
use crate::ui::resources::{colors, strings_cn};
use crate::ui::widgets::{
    self, CornerOrientation, TitleBarActive, TitleBarBackArrow, TitleBarBackPower, TitleBarTitle,
};
use bevy::{
    camera::{RenderTarget, visibility::RenderLayers},
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    prelude::*,
};

/// 常驻 chrome 相机 marker；`transition::resize_target` 按菜单相机的缩放分支处理它。
#[derive(Component)]
pub(crate) struct ChromeCamera;

/// chrome UI 根 marker；子节点顺序即绘制顺序（遮罩在下、chrome 元素在上）。
#[derive(Component)]
pub(crate) struct ChromeRoot;

/// 全屏黑色遮罩节点 marker；alpha 由 `update_overlay` 按帧写入。
#[derive(Component)]
pub(crate) struct FadeOverlay;

/// 变体：标题文字与返回框图标的成对样式（首页 = 电源符号，画廊 = 返回箭头）。
#[derive(Clone, Copy, PartialEq, Eq)]
enum Variant {
    Menu,
    Gallery,
    /// 详情标题随选中的资产型号变化。
    Detail(usize),
}

/// 已应用的 chrome 变体；None 表示尚未同步，避免逐帧重写组件。
#[derive(Resource, Default, PartialEq, Eq)]
struct AppliedVariant(Option<Variant>);

/// chrome 是否可见：对局与局内 Esc 淡出期间不叠标题栏（ToGame 淡出走整体黑场，
/// chrome 仍在纹理内、被合成材质整体压黑，与旧行为一致）。
fn menu_visible(state: GameState) -> bool {
    !matches!(state, GameState::InGame | GameState::ToIndex)
}

/// 安装常驻 chrome：Startup 生成一次，Update 驱动遮罩与变体同步。
pub(crate) struct ChromePlugin;

impl Plugin for ChromePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AppliedVariant>()
            .add_systems(Startup, setup_chrome)
            .add_systems(
                Update,
                (update_overlay, sync_variant, gate_chrome)
                    .chain()
                    .before(crate::ui::widgets::title_bar::interaction),
            );
        // 每次进入状态都同步 chrome 相机开关与标题栏交互门控。
        app.add_systems(OnEnter(GameState::Index), gate_chrome)
            .add_systems(OnEnter(GameState::ToGame), gate_chrome)
            .add_systems(OnEnter(GameState::ToGallery), gate_chrome)
            .add_systems(OnEnter(GameState::Gallery), gate_chrome)
            .add_systems(OnEnter(GameState::GalleryToIndex), gate_chrome)
            .add_systems(OnEnter(GameState::GalleryToDetail), gate_chrome)
            .add_systems(OnEnter(GameState::Detail), gate_chrome)
            .add_systems(OnEnter(GameState::DetailToGallery), gate_chrome)
            .add_systems(OnEnter(GameState::ToIndex), gate_chrome)
            .add_systems(OnEnter(GameState::ToExit), gate_chrome)
            .add_systems(OnEnter(GameState::InGame), gate_chrome);
    }
}

/// Startup 生成 chrome 相机与常驻 UI。初始可见性直接读当前状态：离屏入口可能在
/// Startup 之前就已请求进入对局，错过首个 OnEnter 也不能让 chrome 叠在对局画面上。
fn setup_chrome(
    mut commands: Commands,
    target: Res<SceneTarget>,
    font: Res<UiFont>,
    assets: Res<AssetServer>,
    state: Res<State<GameState>>,
) {
    let camera = commands
        .spawn((
            ChromeCamera,
            Camera3d::default(),
            Camera {
                order: 0,
                // 不清色：在页面相机写入的内容之上叠加。
                clear_color: ClearColorConfig::None,
                is_active: menu_visible(*state.get()),
                ..default()
            },
            RenderTarget::Image(target.0.clone().into()),
            // 只渲染 UI（UiTargetCamera），不捡任何 3D 实体。
            RenderLayers::none(),
            Tonemapping::None,
            DebandDither::Disabled,
            Msaa::Off,
        ))
        .id();
    commands
        .spawn((
            ChromeRoot,
            UiTargetCamera(camera),
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
        ))
        .with_children(|root| {
            // 遮罩最先绘制：压暗页面相机已写入的内容；chrome 元素在其上保持亮度。
            root.spawn((
                FadeOverlay,
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ));
            // 标题栏常驻：初始为主菜单变体（电源符号 + “主菜单”），变体切换见 sync_variant。
            widgets::spawn_title_bar(
                root,
                &font,
                strings_cn::index::TITLE,
                assets.load("image/index/power.png"),
                true,
            );
            // 两处直角统一为设计稿 `.frame` 的单色（outline-accent），切页时保持亮度。
            widgets::spawn_frame_corner(
                root,
                CornerOrientation::TopLeft,
                colors::gallery::OUTLINE_ACTIVE,
            );
            widgets::spawn_frame_corner(
                root,
                CornerOrientation::BottomRight,
                colors::gallery::OUTLINE_ACTIVE,
            );
        });
}

/// 进入状态及每帧同步可见性/输入门控；揭幕后等待键鼠释放才恢复操作。
fn gate_chrome(
    state: Res<State<GameState>>,
    fade: Res<Fade>,
    mut cameras: Query<&mut Camera, With<ChromeCamera>>,
    mut active: ResMut<TitleBarActive>,
) {
    let visible = menu_visible(*state.get());
    // 可见性与可操作性独立：过渡锁期间丢弃返回框按下，不能把松开事件带到下一页。
    active.0 = visible && fade.accepts_input();
    for mut camera in &mut cameras {
        if camera.is_active != visible {
            camera.is_active = visible;
        }
    }
}

/// ContentOnly 过渡时遮罩 alpha 跟随 `Fade.alpha` 压暗内容；FullBlack 时遮罩全透明，
/// 整屏（含 chrome）由合成材质统一压黑。仅在数值变化时写入，避免逐帧弄脏实体。
fn update_overlay(fade: Res<Fade>, mut overlays: Query<&mut BackgroundColor, With<FadeOverlay>>) {
    let alpha = if fade.content_only { fade.alpha } else { 0.0 };
    let color = Color::linear_rgba(0.0, 0.0, 0.0, alpha);
    for mut background in &mut overlays {
        if background.0 != color {
            background.0 = color;
        }
    }
}

/// 淡出一开始就换标题/图标：ContentOnly 过渡期间按过渡目标取变体，
/// 其余时刻跟随当前页面状态。就地改文本与 `display`，不重建实体。
fn sync_variant(
    fade: Res<Fade>,
    state: Res<State<GameState>>,
    mut applied: ResMut<AppliedVariant>,
    selected: Option<Res<crate::pages::detail::SelectedModel>>,
    mut titles: Query<&mut Text, With<TitleBarTitle>>,
    // 箭头与电源节点必属不同实体；`Without` 互斥避免同时可变 `&mut Node` 触发 B0001。
    mut arrows: Query<&mut Node, (With<TitleBarBackArrow>, Without<TitleBarBackPower>)>,
    mut powers: Query<&mut Node, (With<TitleBarBackPower>, Without<TitleBarBackArrow>)>,
) {
    let detail = Variant::Detail(selected.map_or(0, |s| s.0));
    let desired = if fade.phase != Phase::Idle && fade.content_only {
        match fade.target {
            GameState::Gallery => Variant::Gallery,
            GameState::Detail => detail,
            _ => Variant::Menu,
        }
    } else {
        match state.get() {
            GameState::Gallery | GameState::GalleryToIndex | GameState::GalleryToDetail => {
                Variant::Gallery
            }
            GameState::Detail | GameState::DetailToGallery => detail,
            _ => Variant::Menu,
        }
    };
    if *applied == AppliedVariant(Some(desired)) {
        return;
    }
    *applied = AppliedVariant(Some(desired));
    let gallery = desired != Variant::Menu;
    let title = match desired {
        Variant::Gallery => strings_cn::gallery::TITLE,
        Variant::Menu => strings_cn::index::TITLE,
        Variant::Detail(index) => crate::pages::model_preview::ENTRIES[index].name(),
    };
    for mut text in &mut titles {
        text.0 = title.into();
    }
    for mut node in &mut arrows {
        node.display = if gallery {
            Display::Flex
        } else {
            Display::None
        };
    }
    for mut node in &mut powers {
        node.display = if gallery {
            Display::None
        } else {
            Display::Flex
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// 过渡中按下、揭幕后松开不能生成返回消息，防止积压的点击跨页生效。
    #[test]
    fn transition_discards_back_press_before_input_rearms() {
        use crate::ui::widgets::title_bar::{
            self, TitleBarBack, TitleBarBackActivated, TitleBarState,
        };
        let mut world = World::new();
        world.init_resource::<State<GameState>>();
        world.init_resource::<Fade>();
        world.init_resource::<TitleBarState>();
        world.insert_resource(TitleBarActive(true));
        world.init_resource::<Time>();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<ButtonInput<MouseButton>>();
        world.init_resource::<Messages<TitleBarBackActivated>>();
        world.spawn(TitleBarBack);
        world.resource_mut::<TitleBarState>().pointer = Some(Vec2::new(90.0, 40.0));
        world.resource_mut::<Fade>().begin(GameState::Detail, true);
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        world.run_system_once(gate_chrome).unwrap();
        world.run_system_once(title_bar::interaction).unwrap();
        world.insert_resource(Fade::default());
        world.resource_mut::<ButtonInput<MouseButton>>().clear();
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        world.run_system_once(gate_chrome).unwrap();
        world.run_system_once(title_bar::interaction).unwrap();
        assert_eq!(world.resource::<Messages<TitleBarBackActivated>>().len(), 0);
    }

    /// 搭一个含 chrome 实体与所需资源的最小世界。
    fn world_with_chrome() -> (World, Vec<Entity>) {
        let mut world = World::new();
        world.init_resource::<Fade>();
        world.init_resource::<State<GameState>>();
        world.init_resource::<AppliedVariant>();
        let title = world
            .spawn((TitleBarTitle, Text::new(strings_cn::index::TITLE)))
            .id();
        let arrow = world.spawn((TitleBarBackArrow, Node::default())).id();
        let power = world.spawn((TitleBarBackPower, Node::default())).id();
        let overlay = world
            .spawn((FadeOverlay, BackgroundColor(Color::NONE)))
            .id();
        (world, vec![title, arrow, power, overlay])
    }

    /// ContentOnly 淡出开始即按过渡目标换标题/图标；遮罩 alpha 逐帧跟随 Fade。
    #[test]
    fn content_only_switches_variant_and_overlay() {
        let (mut world, ids) = world_with_chrome();
        let (title, arrow, power, overlay) = (ids[0], ids[1], ids[2], ids[3]);
        assert!(world.resource_mut::<Fade>().begin(GameState::Gallery, true));
        world.resource_mut::<Fade>().alpha = 0.5;
        world.run_system_once(sync_variant).unwrap();
        world.run_system_once(update_overlay).unwrap();
        assert_eq!(
            world.get::<Text>(title).unwrap().0,
            strings_cn::gallery::TITLE
        );
        assert_eq!(world.get::<Node>(arrow).unwrap().display, Display::Flex);
        assert_eq!(world.get::<Node>(power).unwrap().display, Display::None);
        let alpha = world.resource::<Fade>().alpha;
        assert!(alpha > 0.0 && alpha < 1.0);
        assert_eq!(
            world.get::<BackgroundColor>(overlay).unwrap().0.alpha(),
            alpha
        );
    }

    /// FullBlack 过渡遮罩保持全透明（压黑由合成材质负责），变体仍跟随当前页面。
    #[test]
    fn full_black_keeps_overlay_clear_and_state_variant() {
        let (mut world, ids) = world_with_chrome();
        let overlay = ids[3];
        assert!(world.resource_mut::<Fade>().begin(GameState::InGame, false));
        world.resource_mut::<Fade>().alpha = 0.5;
        world.run_system_once(sync_variant).unwrap();
        world.run_system_once(update_overlay).unwrap();
        assert_eq!(
            world.get::<BackgroundColor>(overlay).unwrap().0.alpha(),
            0.0
        );
        assert_eq!(
            world.get::<Text>(ids[0]).unwrap().0,
            strings_cn::index::TITLE
        );
    }
}

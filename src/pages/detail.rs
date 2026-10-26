//! 单模型详情：真实视觉预览、手动旋转缩放与返回原画廊位置。
use super::model_preview::{self as preview, ENTRIES, Previews};
use crate::{
    app::{
        state::GameState,
        transition::{Fade, SceneTarget},
    },
    pages::index::{IndexBackground, IndexPalette},
    ui::{
        font::UiFont,
        resources::{
            colors,
            dimens::{detail as dims, title_bar as bar},
            strings_cn,
        },
        widgets::{self, text_font},
        zoom,
    },
};
use bevy::{
    camera::{RenderTarget, ScalingMode, visibility::RenderLayers},
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    input::mouse::{MouseScrollUnit, MouseWheel},
    prelude::*,
    ui_render::prelude::MaterialNode,
};

/// 由画廊确认操作设置的目录索引；详情不接受外部路径。
#[derive(Resource, Default)]
pub(crate) struct SelectedModel(pub(crate) usize);

/// 详情页面相机，随菜单整体缩放。
#[derive(Component)]
pub(crate) struct DetailCamera;
/// 详情拥有的 UI 根和相机；预览本身由共用模块清理。
#[derive(Component)]
pub(crate) struct DetailEntity;
/// 长宽高读数的中文排列顺序。
#[derive(Component)]
struct Dimension(usize);
/// 模型纹理填满标题栏以下区域。
#[derive(Component)]
struct Viewer;
/// 加载失败时覆盖模型查看区的说明文字。
#[derive(Component)]
struct Status;

/// 角度使用度，指针使用逻辑像素；无窗口验收注入同一资源。
#[derive(Resource)]
pub(crate) struct View {
    pub(crate) pointer: Option<Vec2>,
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
    pub(crate) zoom: f32,
    pub(crate) dragging: bool,
    last: Vec2,
}
impl Default for View {
    fn default() -> Self {
        Self {
            pointer: None,
            yaw: -28.0,
            pitch: -14.0,
            zoom: 1.0,
            dragging: false,
            last: Vec2::ZERO,
        }
    }
}
impl View {
    /// 水平方向连续旋转，俯仰限位防止翻面；相对移动不受 UI 缩放影响。
    fn rotate(&mut self, delta: Vec2) {
        self.yaw = (self.yaw + delta.x * 0.45).rem_euclid(360.0);
        // 屏幕 Y 向下；相机在 +Z，Bevy 的正 X 旋转使前侧表面下移，不能照搬 CSS 的减号。
        self.pitch = (self.pitch + delta.y * 0.45).clamp(-80.0, 80.0);
    }
    /// 当前模型姿态；输入与渲染共用，方向测试直接检查相机前侧表面的运动。
    fn orientation(&self) -> Quat {
        // 先绕模型 Y 轴水平转动，再绕屏幕水平轴俯仰，避免偏航后上下拖拽变成侧滚。
        Quat::from_rotation_x(self.pitch.to_radians())
            * Quat::from_rotation_y(self.yaw.to_radians())
    }
    /// 滚轮正方向放大；指数映射兼容细粒度触控板和多格滚轮。
    fn scale(&mut self, lines: f32) {
        self.zoom = (self.zoom * (lines * 0.08).exp()).clamp(0.5, 1.5);
    }
}

/// 安装独立详情生命周期，共用真实资源与预览就绪检查。
pub(crate) struct DetailPlugin;
impl Plugin for DetailPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SelectedModel>()
            .init_resource::<View>()
            .add_systems(
                OnEnter(GameState::Detail),
                (spawn_preview, spawn_ui).chain(),
            )
            .add_systems(
                Update,
                (
                    preview::prepare_previews,
                    input.before(widgets::title_bar::interaction),
                    update_view,
                )
                    .chain()
                    .run_if(in_detail),
            )
            .add_systems(
                OnExit(GameState::DetailToGallery),
                (cleanup, preview::cleanup),
            );
    }
}

/// 渐出期间保留最后一幅详情，输入仍由 Fade 门控。
fn in_detail(state: Res<State<GameState>>) -> bool {
    matches!(state.get(), GameState::Detail | GameState::DetailToGallery)
}

/// 只组装选中的一项；背景透明，使下方平面网格可见。
fn spawn_preview(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    selected: Res<SelectedModel>,
) {
    preview::spawn(&mut commands, &assets, &mut images, &[ENTRIES[selected.0]]);
}

/// 内容节点按网页原型布局；常驻标题栏和角标不在页面中重复生成。
fn spawn_ui(
    mut commands: Commands,
    target: Res<SceneTarget>,
    font: Res<UiFont>,
    previews: Res<Previews>,
    mut view: ResMut<View>,
    mut title: ResMut<widgets::TitleBarState>,
    mut materials: ResMut<Assets<IndexBackground>>,
) {
    *view = View::default();
    title.external_navigation = false;
    title.set_focus(false);
    commands.entity(previews.items[0].camera).insert((
        Camera {
            order: -3,
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 30.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    let camera = commands
        .spawn((
            DetailEntity,
            DetailCamera,
            Camera3d::default(),
            Camera {
                order: -1,
                ..default()
            },
            RenderTarget::Image(target.0.clone().into()),
            RenderLayers::none(),
            Tonemapping::None,
            DebandDither::Disabled,
            Msaa::Off,
        ))
        .id();
    let mut palette = IndexPalette::default();
    // 透明网格先在 sRGB 空间混合，保持原型 rgba(200,219,177,0.05) 的低对比。
    palette.grid = colors::detail::GRID.to_srgba().to_vec4();
    let background = materials.add(IndexBackground {
        viewport: Vec4::new(1280.0, 720.0, 0.0, 0.0),
        // W > 0 表示平面网格间距，Y 是查看区顶部，替换透视地面。
        cubes: Vec4::new(0.0, bar::HEIGHT, 0.0, dims::GRID),
        palette,
    });
    commands
        .spawn((
            DetailEntity,
            UiTargetCamera(camera),
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                MaterialNode(background),
            ));
            root.spawn((
                Viewer,
                Node {
                    position_type: PositionType::Absolute,
                    top: px(bar::HEIGHT),
                    width: percent(100),
                    ..default()
                },
                ImageNode::new(previews.items[0].image.clone()),
            ));
            root.spawn((Node {
                position_type: PositionType::Absolute,
                left: px(dims::SPECS_LEFT),
                top: px(bar::HEIGHT + dims::SPECS_TOP),
                flex_direction: FlexDirection::Column,
                row_gap: px(dims::GROUP_GAP),
                ..default()
            },))
                .with_children(|specs| {
                    specs.spawn((
                        Text::new(
                            strings_cn::gallery::CATEGORIES[previews.items[0].entry.category],
                        ),
                        text_font(&font, dims::LABEL_SIZE, 400.0),
                        TextColor(colors::ON_SURFACE_MUTED),
                    ));
                    for (i, label) in strings_cn::detail::DIMENSIONS.iter().enumerate() {
                        specs
                            .spawn(Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: px(dims::LABEL_GAP),
                                ..default()
                            })
                            .with_children(|group| {
                                group.spawn((
                                    Text::new(*label),
                                    text_font(&font, dims::LABEL_SIZE, 400.0),
                                    TextColor(colors::ON_SURFACE_MUTED),
                                ));
                                group
                                    .spawn(Node {
                                        align_items: AlignItems::Baseline,
                                        column_gap: px(dims::LABEL_GAP),
                                        ..default()
                                    })
                                    .with_children(|row| {
                                        row.spawn((
                                            Dimension(i),
                                            Text::new("—"),
                                            text_font(&font, dims::VALUE_SIZE, 700.0),
                                            TextColor(colors::gallery::NAME_ACTIVE),
                                        ));
                                        row.spawn((
                                            Text::new(strings_cn::gallery::UNIT),
                                            text_font(&font, dims::LABEL_SIZE, 400.0),
                                            TextColor(colors::ON_SURFACE_MUTED),
                                        ));
                                    });
                            });
                    }
                });
            root.spawn((
                Status,
                Node {
                    position_type: PositionType::Absolute,
                    top: percent(50),
                    width: percent(100),
                    ..default()
                },
                Text::new(""),
                TextLayout::justify(Justify::Center),
                text_font(&font, 18.0, 400.0),
                TextColor(colors::ON_SURFACE),
            ));
            widgets::spawn_version_text(root, &font);
        });
}

/// 控件只发返回消息，详情决定返回画廊；失焦/过渡立即取消拖拽。
#[allow(clippy::too_many_arguments)]
fn input(
    windows: Query<&Window>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut wheel: MessageReader<MouseWheel>,
    mut activated: MessageReader<widgets::TitleBarBackActivated>,
    mut view: ResMut<View>,
    images: Res<Assets<Image>>,
    target: Res<SceneTarget>,
    mut fade: ResMut<Fade>,
    mut next: ResMut<NextState<GameState>>,
) {
    let size = crate::ui::logical_viewport(&windows, &images, &target.0);
    let focused = windows.single().map_or(true, |w| w.focused);
    if let Ok(window) = windows.single() {
        view.pointer = window.cursor_position().map(|p| p / zoom(window.width()));
    }
    let scale = windows.single().map_or_else(
        |_| zoom(images.get(&target.0).unwrap().size().x as f32),
        |w| zoom(w.width()),
    );
    let delta: f32 = wheel
        .read()
        .map(|event| {
            event.y
                * if event.unit == MouseScrollUnit::Line {
                    1.0
                } else {
                    1.0 / (64.0 * scale)
                }
        })
        .sum();
    let back = activated.read().next().is_some();
    if !fade.accepts_input() || !focused {
        view.dragging = false;
        return;
    }
    if back && fade.begin(GameState::Gallery, true) {
        view.dragging = false;
        next.set(GameState::DetailToGallery);
        return;
    }
    let inside = view
        .pointer
        .is_some_and(|p| p.x >= 0.0 && p.x < size.x && p.y >= bar::HEIGHT && p.y < size.y);
    if mouse.just_pressed(MouseButton::Left) && inside {
        view.dragging = true;
        view.last = view.pointer.unwrap();
    }
    if view.dragging {
        if let Some(pointer) = view.pointer {
            let delta = pointer - view.last;
            view.rotate(delta);
            view.last = pointer;
        } else {
            view.dragging = false;
        }
    }
    if !mouse.pressed(MouseButton::Left) {
        view.dragging = false;
    }
    if inside {
        view.scale(delta);
    }
}

/// 尺寸、纹理和相机随窗口变化；旋转包络按纵横比适配，主动放大允许裁切。
#[allow(clippy::too_many_arguments)]
fn update_view(
    windows: Query<&Window>,
    target: Res<SceneTarget>,
    view: Res<View>,
    previews: Res<Previews>,
    mut images: ResMut<Assets<Image>>,
    mut transforms: Query<&mut Transform>,
    mut projections: Query<&mut Projection>,
    mut viewers: Query<&mut Node, With<Viewer>>,
    mut values: Query<(&Dimension, &mut Text), Without<Status>>,
    mut status: Query<&mut Text, With<Status>>,
    mut materials: ResMut<Assets<IndexBackground>>,
) {
    let size = crate::ui::logical_viewport(&windows, &images, &target.0);
    let height = (size.y - bar::HEIGHT).max(1.0);
    let p = &previews.items[0];
    let physical = images.get(&target.0).unwrap().size().as_vec2();
    let density = physical.x / size.x;
    let desired = UVec2::new(
        physical.x as u32,
        (height * density).round().max(1.0) as u32,
    );
    let mut image = images.get_mut(&p.image).unwrap();
    if image.size() != desired {
        image.resize(bevy::render::render_resource::Extent3d {
            width: desired.x,
            height: desired.y,
            depth_or_array_layers: 1,
        });
    }
    for mut node in &mut viewers {
        node.height = px(height);
        node.display = if p.error.is_some() {
            Display::None
        } else {
            Display::Flex
        };
    }
    if let Ok(mut model) = transforms.get_mut(p.root) {
        model.rotation = view.orientation();
    }
    if let Ok(mut projection) = projections.get_mut(p.camera) {
        if let Projection::Orthographic(ref mut ortho) = *projection {
            ortho.scaling_mode = ScalingMode::FixedVertical {
                viewport_height: (p.radius * 2.0 * 1.15 / (size.x / height).min(1.0) / view.zoom)
                    .max(0.1),
            };
        }
    }
    for (dimension, mut text) in &mut values {
        let value = p.size.map_or_else(
            || "—".into(),
            |s| format!("{:.1}", [s.x, s.z, s.y][dimension.0]),
        );
        if text.0 != value {
            text.0 = value;
        }
    }
    for mut text in &mut status {
        let value = if p.error.is_some() {
            strings_cn::gallery::FAILED
        } else if p.size.is_none() {
            strings_cn::gallery::LOADING
        } else {
            ""
        };
        if text.0 != value {
            text.0 = value.into();
        }
    }
    for (_, material) in materials.iter_mut() {
        material.viewport = Vec4::new(size.x, size.y, 0.0, 0.0);
    }
}

/// 黑场时删除详情 UI，预览的场景实例与动态纹理由共用模块释放。
fn cleanup(mut commands: Commands, roots: Query<Entity, With<DetailEntity>>) {
    for root in &roots {
        commands.entity(root).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// 只安装真实输入系统所需资源，避免用独立模拟替代拖拽状态机。
    fn input_app() -> App {
        let mut app = App::new();
        let mut images = Assets::<Image>::default();
        let image = images.add(Image::new_target_texture(
            1280,
            720,
            bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
            None,
        ));
        app.add_plugins(MinimalPlugins)
            .insert_resource(images)
            .insert_resource(SceneTarget(image))
            .init_resource::<View>()
            .init_resource::<Fade>()
            .init_resource::<NextState<GameState>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<MouseWheel>()
            .add_message::<widgets::TitleBarBackActivated>()
            .add_systems(Update, input);
        app
    }

    /// 标题栏不开始拖拽，查看区开始后进入标题栏仍持续，松开/过渡/失焦均取消。
    #[test]
    fn drag_lifecycle_uses_viewer_hit_and_input_lock() {
        let mut app = input_app();
        app.world_mut().resource_mut::<View>().pointer = Some(Vec2::new(400.0, 40.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(!app.world().resource::<View>().dragging);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
        app.world_mut().resource_mut::<View>().pointer = Some(Vec2::new(600.0, 400.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(app.world().resource::<View>().dragging);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        app.world_mut().resource_mut::<View>().pointer = Some(Vec2::new(650.0, 40.0));
        app.update();
        assert_ne!(app.world().resource::<View>().yaw, -28.0);
        assert!(app.world().resource::<View>().dragging);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        assert!(!app.world().resource::<View>().dragging);
        app.world_mut().resource_mut::<View>().dragging = true;
        app.world_mut()
            .resource_mut::<Fade>()
            .begin(GameState::Gallery, true);
        let before = app.world().resource::<View>().yaw;
        app.world_mut().resource_mut::<View>().pointer = Some(Vec2::new(800.0, 500.0));
        app.update();
        assert!(!app.world().resource::<View>().dragging);
        assert_eq!(app.world().resource::<View>().yaw, before);
        app.world_mut().insert_resource(Fade::default());
        app.world_mut().resource_mut::<View>().dragging = true;
        app.world_mut().spawn(Window {
            focused: false,
            ..default()
        });
        app.update();
        assert!(!app.world().resource::<View>().dragging);
    }

    /// 查看区外滚轮不改变缩放；返回消息交给实际 Fade 而非直接切状态。
    #[test]
    fn wheel_is_viewer_only_and_back_uses_transition() {
        let mut app = input_app();
        for (point, expected) in [
            (Vec2::new(500.0, 40.0), 1.0),
            (Vec2::new(500.0, 400.0), 1.5),
        ] {
            app.world_mut().resource_mut::<View>().pointer = Some(point);
            app.world_mut().write_message(MouseWheel {
                unit: MouseScrollUnit::Line,
                x: 0.0,
                y: 100.0,
                window: Entity::PLACEHOLDER,
                phase: bevy::input::touch::TouchPhase::Moved,
            });
            app.update();
            assert_eq!(app.world().resource::<View>().zoom, expected);
        }
        app.world_mut()
            .write_message(widgets::TitleBarBackActivated);
        app.update();
        let fade = app.world().resource::<Fade>();
        assert_eq!(fade.target, GameState::Gallery);
        assert!(fade.content_only && !fade.accepts_input());
        assert!(matches!(
            *app.world().resource::<NextState<GameState>>(),
            NextState::Pending(GameState::DetailToGallery)
        ));
    }

    /// 相机位于 +Z、屏幕向下为 +Y：前侧表面必须随拖拽下移，且不随水平转角反向。
    #[test]
    fn vertical_drag_follows_pointer_at_every_yaw() {
        for yaw in [0.0_f32, 45.0, 90.0, 180.0, 270.0] {
            let mut view = View {
                yaw,
                pitch: 0.0,
                ..default()
            };
            let front = Quat::from_rotation_y(-yaw.to_radians()) * Vec3::Z;
            let before = view.orientation() * front;
            view.rotate(Vec2::new(0.0, 20.0));
            let after = view.orientation() * front;
            assert!(
                after.y < before.y - 0.1,
                "向下拖拽时，前侧表面应下移：yaw={yaw}, before={before:?}, after={after:?}"
            );
            view.rotate(Vec2::new(0.0, -40.0));
            let up = view.orientation() * front;
            assert!(
                up.y > before.y + 0.1,
                "向上拖拽时，前侧表面应上移：yaw={yaw}"
            );
        }
    }

    /// 交互边界不依赖帧率；大幅拖拽或滚轮仍保持有限姿态和缩放。
    #[test]
    fn rotation_and_zoom_clamp_and_reset() {
        let mut view = View::default();
        view.rotate(Vec2::new(10000.0, -10000.0));
        assert_eq!(view.pitch, -80.0);
        assert!((0.0..360.0).contains(&view.yaw));
        view.rotate(Vec2::new(0.0, 10000.0));
        assert_eq!(view.pitch, 80.0);
        view.scale(1000.0);
        assert_eq!(view.zoom, 1.5);
        view.scale(-1000.0);
        assert_eq!(view.zoom, 0.5);
        view = View::default();
        assert_eq!(
            (view.yaw, view.pitch, view.zoom, view.dragging),
            (-28.0, -14.0, 1.0, false)
        );
    }
}

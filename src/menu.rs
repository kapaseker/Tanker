//! 首页使用原生 UI 排版与输入；装饰层复现 HTML 背景和 SVG 方块，标题由原稿导出。

use crate::{
    GameState,
    font::UiFont,
    transition::{Fade, SceneTarget},
};
use bevy::{
    camera::RenderTarget,
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
    text::{LetterSpacing, LineHeight},
    ui_render::{
        UiMaterialPlugin,
        prelude::{MaterialNode, UiMaterial},
    },
};

/// 菜单内容相机，始终写入内部纹理，不直接清除最终输出。
#[derive(Component)]
pub(crate) struct MenuCamera;
/// 菜单实体的共同根；进入游戏时递归清理。
#[derive(Component)]
pub(crate) struct MenuRoot;
/// 标题标记，文字由原稿以两倍分辨率导出以保留渐变和挤出。
#[derive(Component)]
pub(crate) struct MenuTitle;
/// 交互项顺序：0 开始，1 展示馆；两项均可焦点和悬停。
#[derive(Component)]
pub(crate) struct MenuItem {
    pub order: usize,
}
/// UI 布局角色，使窗口尺寸变化时更新已有节点而不重建实体。
#[derive(Component, Clone, Copy)]
enum Part {
    Title,
    Row(usize),
    Fill(usize),
    Index(usize),
    Label(usize),
    Arrow(usize),
    Hint,
    Build,
    Corner(bool),
    Background,
}

/// 焦点与鼠标悬停互相独立，初始没有焦点；点击不把展示馆变成游戏入口。
#[derive(Resource, Default)]
pub(crate) struct MenuInput {
    pub focus: Option<usize>,
    pub hover: Option<usize>,
    /// 离屏验证与窗口共用同一命中测试，None 表示鼠标不在窗口内。
    pub pointer: Option<Vec2>,
    /// 按下与松开必须落在同一项，拖出按钮不会意外开始游戏。
    pressed: Option<usize>,
    keyboard_focus: bool,
    animation: [HoverAnimation; 2],
}

/// 三种 CSS duration 独立计时，反向时从当前进度继续。
#[derive(Clone, Copy, Default)]
struct HoverAnimation {
    value: [f32; 3],
    from: [f32; 3],
    elapsed: [f32; 3],
    active: bool,
}
impl HoverAnimation {
    fn tick(&mut self, active: bool, delta: f32) {
        if self.active != active {
            self.from = self.value;
            self.elapsed = [0.0; 3];
            self.active = active;
        }
        for (i, duration) in [0.30, 0.28, 0.22].into_iter().enumerate() {
            self.elapsed[i] = (self.elapsed[i] + delta).min(duration);
            let t = self.elapsed[i] / duration;
            let eased = if i == 2 {
                cubic(t, 0.25, 0.1, 0.25, 1.0)
            } else {
                cubic(t, 0.2, 0.8, 0.2, 1.0)
            };
            self.value[i] =
                self.from[i] + ((if active { 1.0 } else { 0.0 }) - self.from[i]) * eased;
        }
    }
}

/// CSS cubic-bezier 的横轴反解，避免把时间参数直接当作曲线参数。
fn cubic(t: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let eval = |u: f32, a: f32, b: f32| {
        3.0 * (1.0 - u).powi(2) * u * a + 3.0 * (1.0 - u) * u * u * b + u.powi(3)
    };
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..16 {
        let mid = (lo + hi) * 0.5;
        if eval(mid, x1, x2) < t {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    eval((lo + hi) * 0.5, y1, y2)
}

/// CSS 像素坐标布局；视口缩放由内容相机的图像 scale_factor 统一处理。
#[derive(Resource, Clone, Copy)]
pub(crate) struct Layout {
    pub hero: Vec2,
    pub title_size: f32,
    pub menu_y: f32,
    pub menu_width: f32,
    pub row_height: f32,
    pub cube_rect: Vec4,
    label_size: f32,
}
impl Layout {
    fn new(size: Vec2) -> Self {
        let narrow = size.x <= 640.0;
        let title_size = if narrow {
            (size.x * 0.20).clamp(54.4, 80.0)
        } else {
            (size.x * 0.12).clamp(64.0, 176.0)
        };
        // MiSans 的真实浏览器字宽（含 .015em 字距），由 export_menu_title.cjs 输出。
        let hero_width = (title_size * (736.046875 / 176.0)).min(
            size.x
                - if narrow {
                    44.8
                } else {
                    2.0 * (size.x * 0.05).clamp(22.4, 80.0)
                },
        );
        let cube_width = if size.x <= 900.0 {
            0.0
        } else {
            (size.x * 0.30).clamp(260.0, 420.0)
        };
        let gap = if cube_width == 0.0 {
            0.0
        } else {
            (size.x * 0.06).clamp(32.0, 96.0)
        };
        let hero_width = hero_width
            .min((size.x - 2.0 * (size.x * 0.05).clamp(22.4, 80.0) - cube_width - gap).max(1.0));
        let label_size = if narrow {
            17.6
        } else {
            (size.x * 0.021).clamp(18.4, 24.0)
        };
        // MiSans 的 normal 行高为字号的 4/3，浏览器基准 24px -> 32px。
        let row_height = label_size * 4.0 / 3.0 + (if narrow { 30.4 } else { 33.6 }) + 1.0;
        let menu_margin = if narrow {
            28.8
        } else {
            (size.y * 0.045).clamp(32.0, 51.2)
        };
        let menu_y_offset = title_size * 0.84 + menu_margin;
        let hero_height = menu_y_offset + 1.0 + row_height * 2.0 + 21.6 + 15.0;
        let top = if narrow { 41.6 } else { 60.8 };
        let bottom = if narrow { 59.2 } else { 74.78 };
        let pad_top = if narrow { 24.0 } else { 16.0 };
        let pad_bottom = if narrow { 32.0 } else { 40.0 };
        let content_height = (size.y - top - bottom - pad_top - pad_bottom).max(hero_height);
        let hero = Vec2::new(
            (size.x - hero_width - gap - cube_width) * 0.5,
            top + pad_top + (content_height - hero_height) * 0.5,
        );
        let cube_height = cube_width * 320.0 / 470.0;
        Self {
            hero,
            title_size,
            menu_y: hero.y + menu_y_offset,
            menu_width: hero_width.min(480.0),
            row_height,
            cube_rect: Vec4::new(
                hero.x + hero_width + gap,
                hero.y + (hero_height - cube_height) * 0.5,
                cube_width,
                cube_height,
            ),
            label_size,
        }
    }
    fn hit(&self, point: Vec2) -> Option<usize> {
        (0..2).find(|&i| {
            Rect::from_corners(
                Vec2::new(self.hero.x, self.menu_y + 1.0 + i as f32 * self.row_height),
                Vec2::new(
                    self.hero.x + self.menu_width,
                    self.menu_y + 1.0 + (i + 1) as f32 * self.row_height,
                ),
            )
            .contains(point)
        })
    }
}

/// 背景材质包含实际可见的 glow/floor/scan/vignette 和九个 SVG 方块。
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
struct MenuBackground {
    #[uniform(0)]
    viewport: Vec4,
    #[uniform(1)]
    cubes: Vec4,
}
impl UiMaterial for MenuBackground {
    fn fragment_shader() -> ShaderRef {
        "shaders/menu.wesl".into()
    }
}

/// 首页插件，所有输入与动画只在 Menu 且过渡结束后响应。
pub(crate) struct MenuPlugin;
impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MenuInput>()
            .insert_resource(Layout::new(Vec2::new(1280.0, 720.0)))
            .add_plugins(UiMaterialPlugin::<MenuBackground>::default())
            .add_systems(OnEnter(GameState::Menu), spawn_menu)
            .add_systems(OnEnter(GameState::InGame), despawn_menu)
            .add_systems(
                Update,
                (menu_input, update_visual)
                    .chain()
                    .run_if(|state: Res<State<GameState>>| {
                        matches!(state.get(), GameState::Menu | GameState::ToGame)
                    }),
            );
    }
}

/// 创建 UI 结构；坐标由 update_visual 按视口计算，未显示的 CSS 文案不生成。
fn spawn_menu(
    mut commands: Commands,
    target: Res<SceneTarget>,
    assets: Res<AssetServer>,
    font: Res<UiFont>,
    mut input: ResMut<MenuInput>,
    mut materials: ResMut<Assets<MenuBackground>>,
) {
    *input = MenuInput::default();
    let camera = commands
        .spawn((
            Camera3d::default(),
            MenuCamera,
            Msaa::Off,
            Tonemapping::None,
            DebandDither::Disabled,
            Camera {
                order: -1,
                ..default()
            },
            RenderTarget::Image(target.0.clone().into()),
        ))
        .id();
    let background = materials.add(MenuBackground {
        viewport: Vec4::new(1280.0, 720.0, 0.0, 0.0),
        cubes: Vec4::ZERO,
    });
    commands
        .spawn((
            MenuRoot,
            UiTargetCamera(camera),
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                Part::Background,
                MaterialNode(background),
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
            ));
            root.spawn((
                Part::Title,
                MenuTitle,
                Name::new("V-TANK"),
                ImageNode::new(assets.load("menu/title.png")),
                Node {
                    position_type: PositionType::Absolute,
                    ..default()
                },
            ));
            for i in 0..2 {
                root.spawn((
                    MenuItem { order: i },
                    Part::Row(i),
                    Node {
                        position_type: PositionType::Absolute,
                        border: UiRect::bottom(px(1)),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BorderColor::all(Color::NONE),
                ))
                .with_children(|row| {
                    row.spawn((
                        Part::Fill(i),
                        Node {
                            position_type: PositionType::Absolute,
                            height: percent(100),
                            ..default()
                        },
                        BackgroundGradient::from(LinearGradient {
                            angle: std::f32::consts::FRAC_PI_2,
                            stops: vec![
                                ColorStop::new(Color::srgb_u8(200, 219, 177), percent(0)),
                                ColorStop::new(Color::srgb_u8(157, 182, 126), percent(100)),
                            ],
                            ..default()
                        }),
                    ));
                    for (part, label, size, weight, spacing) in [
                        (Part::Index(i), format!("{:02}", i + 1), 11.52, 400, 1.3824),
                        (
                            Part::Label(i),
                            ["START GAME", "MODEL GALLERY"][i].into(),
                            24.0,
                            700,
                            1.44,
                        ),
                        (Part::Arrow(i), "→".into(), 16.0, 400, 0.0),
                    ] {
                        row.spawn((
                            part,
                            Text::new(label),
                            TextFont {
                                font: font.0.clone().into(),
                                font_size: FontSize::Px(size),
                                weight: bevy::text::FontWeight(weight),
                                ..default()
                            },
                            LetterSpacing::Px(spacing),
                            LineHeight::RelativeToFont(4.0 / 3.0),
                            TextColor(Color::srgb_u8(235, 232, 223)),
                            Node {
                                position_type: PositionType::Absolute,
                                ..default()
                            },
                        ));
                    }
                });
            }
            for (part, label, size, spacing) in [
                (Part::Hint, "↑ ↓ 选择 · ENTER 确认", 11.2, 1.344),
                (Part::Build, "BUILD 0.1.0", 10.88, 1.9584),
            ] {
                root.spawn((
                    part,
                    Text::new(label),
                    TextFont {
                        font: font.0.clone().into(),
                        font_size: FontSize::Px(size),
                        ..default()
                    },
                    LetterSpacing::Px(spacing),
                    LineHeight::RelativeToFont(4.0 / 3.0),
                    TextColor(Color::srgb_u8(164, 157, 144)),
                    Node {
                        position_type: PositionType::Absolute,
                        ..default()
                    },
                ));
            }
            for upper in [true, false] {
                root.spawn((
                    Part::Corner(upper),
                    Node {
                        position_type: PositionType::Absolute,
                        width: px(22),
                        height: px(22),
                        border: if upper {
                            UiRect {
                                top: px(1),
                                left: px(1),
                                ..default()
                            }
                        } else {
                            UiRect {
                                bottom: px(1),
                                right: px(1),
                                ..default()
                            }
                        },
                        ..default()
                    },
                    BorderColor::all(Color::srgba(
                        200.0 / 255.0,
                        219.0 / 255.0,
                        177.0 / 255.0,
                        0.14,
                    )),
                ));
            }
        });
}

/// 屏幕坐标命中测试显式支持内部纹理相机；Bevy 默认 UI 拾取只接受窗口相机。
fn menu_input(
    windows: Query<&Window>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    layout: Res<Layout>,
    mut input: ResMut<MenuInput>,
    mut fade: ResMut<Fade>,
    mut next: ResMut<NextState<GameState>>,
) {
    if let Ok(window) = windows.single() {
        input.pointer = window.cursor_position();
    }
    if !fade.accepts_input() {
        return;
    }
    input.hover = input.pointer.and_then(|point| layout.hit(point));
    let step = if keys.just_pressed(KeyCode::ArrowUp)
        || (keys.just_pressed(KeyCode::Tab)
            && (keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight)))
    {
        -1
    } else if keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::Tab) {
        1
    } else {
        0
    };
    if step != 0 {
        // 原生链接从无焦点状态按 Shift+Tab 时进入最后一项；↑首次仍进入第一项。
        let initial = usize::from(keys.just_pressed(KeyCode::Tab) && step < 0);
        input.focus = Some(
            input
                .focus
                .map_or(initial, |i| (i as i32 + step).rem_euclid(2) as usize),
        );
        input.keyboard_focus = true;
    }
    if mouse.just_pressed(MouseButton::Left) {
        input.pressed = input.hover;
        input.focus = input.hover;
        input.keyboard_focus = false;
    }
    let clicked = if mouse.just_released(MouseButton::Left) {
        input
            .pressed
            .take()
            .filter(|&index| input.hover == Some(index))
    } else {
        None
    };
    let confirmed = if keys.just_pressed(KeyCode::Enter) {
        input.focus
    } else {
        clicked
    };
    if confirmed == Some(0) && fade.begin(GameState::InGame) {
        next.set(GameState::ToGame);
    }
}

/// 依照 CSS 更新布局和缓动，鼠标离开后悬停效果自动反向收回。
fn update_visual(
    windows: Query<&Window>,
    target: Res<SceneTarget>,
    images: Res<Assets<Image>>,
    time: Res<Time>,
    mut layout: ResMut<Layout>,
    mut input: ResMut<MenuInput>,
    mut nodes: Query<(
        &Part,
        &mut Node,
        Option<&mut TextColor>,
        Option<&mut TextFont>,
        Option<&mut LetterSpacing>,
        Option<&mut BorderColor>,
    )>,
    mut materials: ResMut<Assets<MenuBackground>>,
) {
    let size = windows
        .single()
        .map(|w| Vec2::new(w.width(), w.height()))
        .unwrap_or_else(|_| images.get(&target.0).unwrap().size().as_vec2());
    *layout = Layout::new(size);
    let l = *layout;
    for i in 0..2 {
        let active = input.hover == Some(i) || (input.keyboard_focus && input.focus == Some(i));
        input.animation[i].tick(active, time.delta_secs());
    }
    for (_, material) in materials.iter_mut() {
        material.viewport = Vec4::new(size.x, size.y, time.elapsed_secs(), 0.0);
        material.cubes = l.cube_rect;
    }
    // CSS 在 sRGB 中做透明合成；线性渲染器需降低 alpha 才得到相同的细线亮度。
    let line = Color::srgba(235.0 / 255.0, 232.0 / 255.0, 223.0 / 255.0, 0.028);
    for (part, mut node, color, font, spacing, border) in &mut nodes {
        let mut rect = None;
        match *part {
            Part::Background => {}
            Part::Title => {
                let s = l.title_size / 176.0;
                rect = Some((
                    l.hero.x - 80.0 * s,
                    l.hero.y - 40.0 * s,
                    897.0 * s,
                    278.0 * s,
                ));
            }
            Part::Row(i) => {
                rect = Some((
                    l.hero.x,
                    l.menu_y + 1.0 + i as f32 * l.row_height,
                    l.menu_width,
                    l.row_height,
                ));
                node.border.top = if i == 0 { px(1) } else { px(0) };
                if let Some(mut border) = border {
                    *border = BorderColor::all(line);
                }
            }
            Part::Fill(i) => {
                rect = Some((
                    0.0,
                    0.0,
                    l.menu_width * input.animation[i].value[0],
                    l.row_height,
                ))
            }
            Part::Index(i) | Part::Label(i) | Part::Arrow(i) => {
                let p = input.animation[i].value;
                let narrow = size.x <= 640.0;
                let left = (if narrow { 3.2 } else { 5.6 }) + 8.0 * p[1];
                let (x, fs, letter, normal) = match *part {
                    Part::Index(_) => (left, 11.52, 1.3824, [157.0, 182.0, 126.0]),
                    Part::Label(_) => (
                        left + 30.72,
                        l.label_size,
                        l.label_size * 0.06,
                        [235.0, 232.0, 223.0],
                    ),
                    _ => (
                        l.menu_width - (if narrow { 3.2 } else { 5.6 }) - 12.0 - 16.0 + 5.0 * p[1],
                        16.0,
                        0.0,
                        [200.0, 219.0, 177.0],
                    ),
                };
                node.left = px(x);
                node.top = px((l.row_height - 1.0 - fs * 4.0 / 3.0) * 0.5);
                if let Some(mut font) = font {
                    font.font_size = FontSize::Px(fs);
                }
                if let Some(mut spacing) = spacing {
                    *spacing = LetterSpacing::Px(letter);
                }
                if let Some(mut color) = color {
                    color.0 = Color::srgb(
                        (normal[0] + (18.0 - normal[0]) * p[2]) / 255.0,
                        (normal[1] + (17.0 - normal[1]) * p[2]) / 255.0,
                        (normal[2] + (13.0 - normal[2]) * p[2]) / 255.0,
                    );
                }
            }
            Part::Hint => {
                node.left = px(l.hero.x);
                node.top = px(l.menu_y + 1.0 + l.row_height * 2.0 + 21.6);
            }
            Part::Build => {
                node.bottom = px(if size.x <= 640.0 { 25.6 } else { 30.4 });
                if size.x <= 640.0 {
                    node.left = px(19.2);
                    node.right = Val::Auto;
                } else {
                    node.right = px((size.x * 0.045).clamp(22.4, 54.4));
                    node.left = Val::Auto;
                }
            }
            Part::Corner(upper) => {
                if upper {
                    node.top = px(18);
                    node.left = px(18);
                } else {
                    node.bottom = px(18);
                    node.right = px(18);
                }
            }
        }
        if let Some((x, y, w, h)) = rect {
            node.left = px(x);
            node.top = px(y);
            node.width = px(w);
            node.height = px(h);
        }
    }
}

/// 内容相机与 UI 根都属于首页，不能留给下一局重复使用。
fn despawn_menu(
    mut commands: Commands,
    entities: Query<Entity, Or<(With<MenuRoot>, With<MenuCamera>)>>,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

/// 标题纹理必须真正加载成功，不能只等字体加载完成便揭幕。
pub(crate) fn assets_ready(world: &mut World) -> bool {
    let handles: Vec<_> = world
        .query_filtered::<&ImageNode, With<MenuTitle>>()
        .iter(world)
        .map(|image| image.image.clone())
        .collect();
    !handles.is_empty()
        && handles.iter().all(|handle| {
            world
                .resource::<AssetServer>()
                .is_loaded_with_dependencies(handle)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// 使用同一输入系统验证首次聚焦、循环、Tab、Gallery 和过渡输入锁。
    #[test]
    fn keyboard_mouse_and_gallery_use_the_same_input_path() {
        let mut world = World::new();
        world.insert_resource(Layout::new(Vec2::new(1280.0, 720.0)));
        world.init_resource::<MenuInput>();
        world.init_resource::<Fade>();
        world.init_resource::<NextState<GameState>>();
        let press = |world: &mut World, key: KeyCode| {
            let mut keys = ButtonInput::default();
            keys.press(key);
            world.insert_resource(keys);
            world.insert_resource(ButtonInput::<MouseButton>::default());
            world.run_system_once(menu_input).unwrap();
        };
        assert_eq!(world.resource::<MenuInput>().focus, None);
        press(&mut world, KeyCode::ArrowUp);
        assert_eq!(world.resource::<MenuInput>().focus, Some(0));
        press(&mut world, KeyCode::ArrowDown);
        assert_eq!(world.resource::<MenuInput>().focus, Some(1));
        press(&mut world, KeyCode::Enter);
        assert_eq!(
            world.resource::<Fade>().phase,
            crate::transition::Phase::Idle
        );
        press(&mut world, KeyCode::Tab);
        assert_eq!(world.resource::<MenuInput>().focus, Some(0));
        press(&mut world, KeyCode::Enter);
        assert_eq!(
            world.resource::<Fade>().phase,
            crate::transition::Phase::Out
        );
        let focus = world.resource::<MenuInput>().focus;
        press(&mut world, KeyCode::ArrowDown);
        assert_eq!(world.resource::<MenuInput>().focus, focus);
    }

    /// 鼠标按下只获得焦点，松开在同一项才激活；拖出取消。
    #[test]
    fn mouse_activation_requires_release_inside_the_pressed_item() {
        let mut world = World::new();
        let layout = Layout::new(Vec2::new(1280.0, 720.0));
        let point = Vec2::new(layout.hero.x + 20.0, layout.menu_y + 20.0);
        world.insert_resource(layout);
        world.init_resource::<Fade>();
        world.init_resource::<NextState<GameState>>();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.insert_resource(MenuInput {
            pointer: Some(point),
            ..default()
        });
        let mut mouse = ButtonInput::default();
        mouse.press(MouseButton::Left);
        world.insert_resource(mouse);
        world.run_system_once(menu_input).unwrap();
        assert_eq!(
            world.resource::<Fade>().phase,
            crate::transition::Phase::Idle
        );
        world.resource_mut::<ButtonInput<MouseButton>>().clear();
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        world.resource_mut::<MenuInput>().pointer = None;
        world.run_system_once(menu_input).unwrap();
        assert_eq!(
            world.resource::<Fade>().phase,
            crate::transition::Phase::Idle
        );
        world.resource_mut::<MenuInput>().pointer = Some(point);
        world.resource_mut::<ButtonInput<MouseButton>>().clear();
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        world.run_system_once(menu_input).unwrap();
        world.resource_mut::<ButtonInput<MouseButton>>().clear();
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        world.run_system_once(menu_input).unwrap();
        assert_eq!(
            world.resource::<Fade>().phase,
            crate::transition::Phase::Out
        );
    }
    /// 两个方向首次进入第一项，再次移动循环，Gallery 确认可安全无动作。
    #[test]
    fn layout_hit_and_css_breakpoints() {
        let l = Layout::new(Vec2::new(1280.0, 720.0));
        assert!((l.hero.x - 88.42).abs() < 0.1);
        assert_eq!(l.hit(Vec2::new(l.hero.x + 10.0, l.menu_y + 10.0)), Some(0));
        assert_eq!(
            l.hit(Vec2::new(l.hero.x + 10.0, l.menu_y + l.row_height + 10.0)),
            Some(1)
        );
        assert_eq!(l.hit(Vec2::ZERO), None);
        assert_eq!(Layout::new(Vec2::new(900.0, 720.0)).cube_rect.z, 0.0);
        assert!(Layout::new(Vec2::new(901.0, 720.0)).cube_rect.z > 0.0);
        assert_eq!(Layout::new(Vec2::new(640.0, 720.0)).label_size, 17.6);
        assert!(Layout::new(Vec2::new(641.0, 720.0)).label_size > 17.6);
    }
    /// 颜色/位移分段缓动在反向时保持连续，最终回到初始态。
    #[test]
    fn hover_reverses_without_a_jump() {
        let mut animation = HoverAnimation::default();
        animation.tick(true, 0.1);
        let old = animation.value;
        animation.tick(false, 0.0);
        for i in 0..3 {
            assert!((animation.value[i] - old[i]).abs() < 0.001);
        }
        animation.tick(false, 0.3);
        assert!(animation.value.iter().all(|value| value.abs() < 0.001));
    }
}

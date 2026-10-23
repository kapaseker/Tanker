//! 首页使用原生 UI 排版与输入；装饰层复现 HTML 背景和 SVG 方块，标题由原稿导出。

use crate::{
    app::{
        state::GameState,
        transition::{Fade, SceneTarget},
    },
    font::UiFont,
    ui_resources::{colors, dimens},
    ui_resources::{
        colors::menu as palette, dimens::menu as dims, strings, strings_cn::menu as labels,
    },
};
use bevy::{
    camera::RenderTarget,
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    ecs::relationship::RelatedSpawnerCommands,
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
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

/// 独立页面返回首页时才恢复入口焦点，首次启动仍无选中项。
#[derive(Resource, Default)]
pub(crate) struct ReturnFocus(pub bool);

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

/// scale.js 的 zoom 语义：可用宽度不足参考宽度时整体等比缩小，只缩不放大，高度不参与。
pub(crate) fn zoom(available_width: f32) -> f32 {
    (available_width / dimens::REFERENCE_WIDTH).min(1.0)
}

/// 给定物理像素与缩放系数，返回整体缩放后的 CSS 像素视口；
/// 窄窗口时宽度恒等于参考宽度，纵向空间为窗口高度除以缩放（与 zoom 语义一致）。
fn css_viewport(pixels: Vec2, scale_factor: f32) -> Vec2 {
    pixels / (scale_factor * zoom(pixels.x / scale_factor))
}

/// CSS 像素坐标布局；控件取 1280px 设计常量，视口缩放由内容相机的图像
/// scale_factor 乘以 zoom 统一处理，本结构只按 CSS 视口决定整体居中位置。
#[derive(Resource, Clone, Copy)]
pub(crate) struct Layout {
    pub hero: Vec2,
    pub menu_y: f32,
    pub menu_width: f32,
    pub row_height: f32,
    pub cube_rect: Vec4,
}
impl Layout {
    fn new(size: Vec2) -> Self {
        // 设计常量下的固定排版；size 只影响整体居中位置，宽度始终不低于参考值。
        let title_size = dims::TITLE_SIZE;
        let hero_width = title_size * (dims::TITLE_TEXT_WIDTH / dims::TITLE_REFERENCE_SIZE);
        let cube_width = dims::CUBE_WIDTH;
        let gap = dims::HERO_GAP;
        // MiSans 的 normal 行高为字号的 4/3，浏览器基准 24px -> 32px。
        let row_height =
            dims::LABEL_SIZE * dimens::LINE_HEIGHT + dims::ROW_PADDING + dimens::BORDER_WIDTH;
        let menu_y_offset = title_size * dims::TITLE_LINE_HEIGHT + dims::MENU_MARGIN;
        let hero_height = menu_y_offset
            + dimens::BORDER_WIDTH
            + row_height * 2.0
            + dims::HINT_GAP
            + dims::HINT_HEIGHT;
        let content_height =
            (size.y - dims::TOP - dims::BOTTOM - dims::PADDING_TOP - dims::PADDING_BOTTOM)
                .max(hero_height);
        let hero = Vec2::new(
            (size.x - hero_width - gap - cube_width) * 0.5,
            dims::TOP + dims::PADDING_TOP + (content_height - hero_height) * 0.5,
        );
        let cube_height = cube_width * dims::CUBE_VIEWBOX_HEIGHT / dims::CUBE_VIEWBOX_WIDTH;
        Self {
            hero,
            menu_y: hero.y + menu_y_offset,
            menu_width: hero_width.min(dims::MENU_MAX_WIDTH),
            row_height,
            cube_rect: Vec4::new(
                hero.x + hero_width + gap,
                hero.y + (hero_height - cube_height) * 0.5,
                cube_width,
                cube_height,
            ),
        }
    }
    fn hit(&self, point: Vec2) -> Option<usize> {
        (0..2).find(|&i| {
            Rect::from_corners(
                Vec2::new(
                    self.hero.x,
                    self.menu_y + dimens::BORDER_WIDTH + i as f32 * self.row_height,
                ),
                Vec2::new(
                    self.hero.x + self.menu_width,
                    self.menu_y + dimens::BORDER_WIDTH + (i + 1) as f32 * self.row_height,
                ),
            )
            .contains(point)
        })
    }
}

/// 与 menu.wesl 的 MenuPalette 同布局；Vec4 的 RGB 为 sRGB，W 为合成强度。
#[derive(ShaderType, Clone, Debug)]
pub(crate) struct MenuPalette {
    /// 页面底色。
    background: Vec4,
    /// 暖色与薄荷环境光。
    warm: Vec4,
    mint: Vec4,
    /// 网格、扫描线、暗角与方块阴影。
    grid: Vec4,
    scan: Vec4,
    vignette: Vec4,
    shadow: Vec4,
    /// 四种方块的三面配色；顶面 W 额外携带所有面的不透明度。
    tops: [Vec4; 4],
    lefts: [Vec4; 4],
    rights: [Vec4; 4],
}

impl Default for MenuPalette {
    fn default() -> Self {
        Self {
            // 不用 LinearRgba：着色器在混色结束后统一转线性空间。
            background: palette::BACKGROUND.to_srgba().to_vec4(),
            warm: palette::ATMOSPHERE_WARM.to_srgba().to_vec4(),
            mint: palette::ATMOSPHERE_MINT.to_srgba().to_vec4(),
            grid: palette::GRID.to_srgba().to_vec4(),
            scan: palette::SCAN.to_srgba().to_vec4(),
            vignette: palette::VIGNETTE.to_srgba().to_vec4(),
            shadow: palette::BLOCK_SHADOW.to_srgba().to_vec4(),
            tops: palette::BLOCK_TOPS.map(|color| {
                color
                    .with_alpha(palette::BLOCK_OPACITY)
                    .to_srgba()
                    .to_vec4()
            }),
            lefts: palette::BLOCK_LEFTS.map(|color| color.to_srgba().to_vec4()),
            rights: palette::BLOCK_RIGHTS.map(|color| color.to_srgba().to_vec4()),
        }
    }
}

/// 背景材质包含实际可见的 glow/floor/scan/vignette 和九个 SVG 方块。
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub(crate) struct MenuBackground {
    /// XY 为逻辑视口尺寸，Z 为秒数，W 为装饰漂浮距离（逻辑像素）。
    #[uniform(0)]
    pub(crate) viewport: Vec4,
    #[uniform(1)]
    pub(crate) cubes: Vec4,
    /// 静态配色只在创建材质时由 UI 常量构建。
    #[uniform(2)]
    pub(crate) palette: MenuPalette,
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
            .init_resource::<ReturnFocus>()
            .insert_resource(Layout::new(Vec2::new(
                dimens::REFERENCE_WIDTH,
                dimens::REFERENCE_HEIGHT,
            )))
            .add_plugins(UiMaterialPlugin::<MenuBackground>::default())
            .add_systems(OnEnter(GameState::Menu), spawn_menu)
            .add_systems(OnEnter(GameState::InGame), despawn_menu)
            .add_systems(OnEnter(GameState::Gallery), despawn_menu)
            .add_systems(
                Update,
                (menu_input, update_visual)
                    .chain()
                    .run_if(|state: Res<State<GameState>>| {
                        matches!(
                            state.get(),
                            GameState::Menu | GameState::ToGame | GameState::ToGallery
                        )
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
    mut return_focus: ResMut<ReturnFocus>,
) {
    *input = MenuInput::default();
    if return_focus.0 {
        input.focus = Some(1);
        input.keyboard_focus = true;
        return_focus.0 = false;
    }
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
        viewport: Vec4::new(
            dimens::REFERENCE_WIDTH,
            dimens::REFERENCE_HEIGHT,
            0.0,
            dims::AMBIENT_TRAVEL,
        ),
        cubes: Vec4::ZERO,
        palette: MenuPalette::default(),
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
                        border: UiRect::bottom(px(dimens::BORDER_WIDTH)),
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
                            // 与网页 CSS 一致，在 sRGB 分量间插值，不在 OKLAB 混色。
                            color_space: InterpolationColorSpace::Srgba,
                            stops: vec![
                                ColorStop::new(colors::PRIMARY, percent(0)),
                                ColorStop::new(colors::PRIMARY_DEEP, percent(100)),
                            ],
                            ..default()
                        }),
                    ));
                    for (part, label, size, weight, spacing) in [
                        (
                            Part::Index(i),
                            format!("{:02}", i + 1),
                            dims::INDEX_SIZE,
                            400,
                            dims::INDEX_SPACING,
                        ),
                        (
                            Part::Label(i),
                            labels::ACTIONS[i].into(),
                            dims::LABEL_SIZE,
                            700,
                            dims::LABEL_SPACING,
                        ),
                        (
                            Part::Arrow(i),
                            strings::ARROW.into(),
                            dims::ARROW_SIZE,
                            400,
                            0.0,
                        ),
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
                            LineHeight::RelativeToFont(dimens::LINE_HEIGHT),
                            TextColor(colors::ON_SURFACE),
                            Node {
                                position_type: PositionType::Absolute,
                                ..default()
                            },
                        ));
                    }
                });
            }
            root.spawn((
                Part::Build,
                Text::new(labels::BUILD),
                TextFont {
                    font: font.0.clone().into(),
                    font_size: FontSize::Px(dims::BUILD_SIZE),
                    ..default()
                },
                LetterSpacing::Px(dims::BUILD_SPACING),
                LineHeight::RelativeToFont(dimens::LINE_HEIGHT),
                TextColor(colors::ON_SURFACE_MUTED),
                Node {
                    position_type: PositionType::Absolute,
                    ..default()
                },
            ));
            // 提示行复刻网页的 inline 键帽，顺序为 ↑ ↓ 选择 · ENTER 确认。
            // 说明：设计稿要求键帽与说明文字按基线对齐，但 Bevy 0.20 的 UI 布局
            // 不向 Taffy 提供文本基线，AlignItems::Baseline 会退化为底边对齐，
            // 导致键帽字符明显偏高；此处用 Center 近似（偏差小于半像素）。
            root.spawn((
                Part::Hint,
                Node {
                    position_type: PositionType::Absolute,
                    display: Display::Flex,
                    align_items: AlignItems::Center,
                    column_gap: px(dims::HINT_ITEM_GAP),
                    ..default()
                },
            ))
            .with_children(|hint| {
                // 生成一个键帽：固定字号、内边距与圆角的行内小方块。
                let keycap = |hint: &mut RelatedSpawnerCommands<ChildOf>, key: &str| {
                    hint.spawn((
                        Text::new(key),
                        TextFont {
                            font: font.0.clone().into(),
                            font_size: FontSize::Px(dims::KBD_SIZE),
                            ..default()
                        },
                        LetterSpacing::Px(dims::KBD_SPACING),
                        LineHeight::RelativeToFont(dimens::LINE_HEIGHT),
                        TextColor(colors::ON_SURFACE),
                        Node {
                            border: UiRect::all(px(dimens::BORDER_WIDTH)),
                            border_radius: BorderRadius::all(px(dims::KBD_RADIUS)),
                            padding: UiRect::axes(px(dims::KBD_PADDING_X), px(dims::KBD_PADDING_Y)),
                            ..default()
                        },
                        BorderColor::all(palette::SEPARATOR),
                        BackgroundColor(palette::KBD_BACKGROUND),
                    ));
                };
                // 生成一段说明文字。
                let caption = |hint: &mut RelatedSpawnerCommands<ChildOf>, label: &str| {
                    hint.spawn((
                        Text::new(label),
                        TextFont {
                            font: font.0.clone().into(),
                            font_size: FontSize::Px(dims::HINT_SIZE),
                            ..default()
                        },
                        LetterSpacing::Px(dims::HINT_SPACING),
                        LineHeight::RelativeToFont(dimens::LINE_HEIGHT),
                        TextColor(colors::ON_SURFACE_MUTED),
                    ));
                };
                keycap(hint, strings::KEY_UP);
                keycap(hint, strings::KEY_DOWN);
                caption(hint, labels::HINT_SELECT);
                keycap(hint, strings::KEY_ENTER);
                caption(hint, labels::HINT_CONFIRM);
            });
            for upper in [true, false] {
                root.spawn((
                    Part::Corner(upper),
                    Node {
                        position_type: PositionType::Absolute,
                        width: px(dims::CORNER_SIZE),
                        height: px(dims::CORNER_SIZE),
                        border: if upper {
                            UiRect {
                                top: px(dimens::BORDER_WIDTH),
                                left: px(dimens::BORDER_WIDTH),
                                ..default()
                            }
                        } else {
                            UiRect {
                                bottom: px(dimens::BORDER_WIDTH),
                                right: px(dimens::BORDER_WIDTH),
                                ..default()
                            }
                        },
                        ..default()
                    },
                    BorderColor::all(palette::CORNER),
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
        // 指针位于窗口逻辑像素；布局按整体缩放后的 CSS 视口计算，需除以 zoom。
        let s = zoom(window.width());
        input.pointer = window.cursor_position().map(|point| point / s);
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
    if confirmed == Some(1) && fade.begin(GameState::Gallery) {
        next.set(GameState::ToGallery);
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
    // 窗口模式取窗口物理尺寸与缩放系数；离屏无窗口时纹理尺寸即物理尺寸、缩放系数为 1。
    let size = match windows.single() {
        Ok(window) => css_viewport(
            window.resolution.physical_size().as_vec2(),
            window.resolution.scale_factor(),
        ),
        Err(_) => {
            let pixels = images.get(&target.0).unwrap().size().as_vec2();
            css_viewport(pixels, 1.0)
        }
    };
    *layout = Layout::new(size);
    let l = *layout;
    for i in 0..2 {
        let active = input.hover == Some(i) || (input.keyboard_focus && input.focus == Some(i));
        input.animation[i].tick(active, time.delta_secs());
    }
    for (_, material) in materials.iter_mut() {
        material.viewport = Vec4::new(size.x, size.y, time.elapsed_secs(), dims::AMBIENT_TRAVEL);
        material.cubes = l.cube_rect;
    }
    // CSS 在 sRGB 中做透明合成；线性渲染器需降低 alpha 才得到相同的细线亮度。
    let line = palette::SEPARATOR;
    for (part, mut node, color, font, spacing, border) in &mut nodes {
        let mut rect = None;
        match *part {
            Part::Background => {}
            Part::Title => {
                let s = dims::TITLE_SIZE / dims::TITLE_REFERENCE_SIZE;
                rect = Some((
                    l.hero.x - dims::TITLE_OVERHANG_X * s,
                    l.hero.y - dims::TITLE_OVERHANG_Y * s,
                    dims::TITLE_IMAGE_WIDTH * s,
                    dims::TITLE_IMAGE_HEIGHT * s,
                ));
            }
            Part::Row(i) => {
                rect = Some((
                    l.hero.x,
                    l.menu_y + dimens::BORDER_WIDTH + i as f32 * l.row_height,
                    l.menu_width,
                    l.row_height,
                ));
                node.border.top = if i == 0 {
                    px(dimens::BORDER_WIDTH)
                } else {
                    px(0)
                };
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
                let left = dims::ROW_INSET + dims::ACTION_TRAVEL * p[1];
                let (x, fs, letter, normal) = match *part {
                    Part::Index(_) => (
                        left,
                        dims::INDEX_SIZE,
                        dims::INDEX_SPACING,
                        colors::PRIMARY_DEEP,
                    ),
                    Part::Label(_) => (
                        left + dims::LABEL_OFFSET,
                        dims::LABEL_SIZE,
                        dims::LABEL_SPACING,
                        colors::ON_SURFACE,
                    ),
                    _ => (
                        l.menu_width - dims::ROW_INSET - dims::ARROW_INSET - dims::ARROW_SIZE
                            + dims::ARROW_TRAVEL * p[1],
                        dims::ARROW_SIZE,
                        0.0,
                        colors::PRIMARY,
                    ),
                };
                node.left = px(x);
                node.top =
                    px((l.row_height - dimens::BORDER_WIDTH - fs * dimens::LINE_HEIGHT) * 0.5);
                if let Some(mut font) = font {
                    // 值未变时不写入组件，避免每帧重新塑形中文文本。
                    if !matches!(font.font_size, FontSize::Px(value) if value == fs) {
                        font.font_size = FontSize::Px(fs);
                    }
                }
                if let Some(mut spacing) = spacing {
                    let next = LetterSpacing::Px(letter);
                    if *spacing != next {
                        *spacing = next;
                    }
                }
                if let Some(mut color) = color {
                    // 与原稿一致，在 sRGB 分量间插值，不在线性空间混色。
                    color.0 = normal
                        .to_srgba()
                        .mix(&colors::ON_PRIMARY.to_srgba(), p[2])
                        .into();
                }
            }
            Part::Hint => {
                node.left = px(l.hero.x);
                node.top =
                    px(l.menu_y + dimens::BORDER_WIDTH + l.row_height * 2.0 + dims::HINT_GAP);
            }
            Part::Build => {
                node.bottom = px(dims::BUILD_BOTTOM);
                node.right = px(dims::BUILD_RIGHT);
                node.left = Val::Auto;
            }
            Part::Corner(upper) => {
                if upper {
                    node.top = px(dims::CORNER_INSET);
                    node.left = px(dims::CORNER_INSET);
                } else {
                    node.bottom = px(dims::CORNER_INSET);
                    node.right = px(dims::CORNER_INSET);
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

    /// 材质必须传原始 sRGB 而非线性值，且保留网格与阴影的合成强度。
    #[test]
    fn shader_palette_preserves_srgb_and_alpha() {
        let palette = MenuPalette::default();
        assert!((palette.background.x - 15.0 / 255.0).abs() < 1e-7);
        assert!((palette.warm.w - 0.42).abs() < 1e-7);
        assert!((palette.grid.w - 0.06).abs() < 1e-7);
        assert!((palette.shadow.w - 0.1575).abs() < 1e-7);
        assert!((palette.tops[0].x - 226.0 / 255.0).abs() < 1e-7);
        assert!((palette.tops[0].w - 0.95).abs() < 1e-7);
        assert!((palette.lefts[3].y - 42.0 / 255.0).abs() < 1e-7);
        assert!((palette.rights[2].z - 55.0 / 255.0).abs() < 1e-7);
    }

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
            crate::app::transition::Phase::Out
        );
        world.insert_resource(Fade::default());
        press(&mut world, KeyCode::Tab);
        assert_eq!(world.resource::<MenuInput>().focus, Some(0));
        press(&mut world, KeyCode::Enter);
        assert_eq!(
            world.resource::<Fade>().phase,
            crate::app::transition::Phase::Out
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
            crate::app::transition::Phase::Idle
        );
        world.resource_mut::<ButtonInput<MouseButton>>().clear();
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        world.resource_mut::<MenuInput>().pointer = None;
        world.run_system_once(menu_input).unwrap();
        assert_eq!(
            world.resource::<Fade>().phase,
            crate::app::transition::Phase::Idle
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
            crate::app::transition::Phase::Out
        );
    }
    /// 布局取 1280px 设计常量；窗口适配只由 zoom 等比缩放承担，无断点分支。
    #[test]
    fn layout_uses_design_constants_and_uniform_zoom() {
        let l = Layout::new(Vec2::new(1280.0, 720.0));
        assert!((l.hero.x - 87.98).abs() < 0.1);
        assert_eq!(l.hit(Vec2::new(l.hero.x + 10.0, l.menu_y + 10.0)), Some(0));
        assert_eq!(
            l.hit(Vec2::new(l.hero.x + 10.0, l.menu_y + l.row_height + 10.0)),
            Some(1)
        );
        assert_eq!(l.hit(Vec2::ZERO), None);
        // 控件尺寸是固定设计常量，只随视口平移居中，不随窗口缩放或收起。
        let wide = Layout::new(Vec2::new(1920.0, 1080.0));
        assert_eq!(wide.cube_rect.z, dims::CUBE_WIDTH);
        assert!((wide.hero.x - 407.98).abs() < 0.1);
        // zoom 语义：只缩不放大，高度不参与（与 scale.js 一致）。
        assert_eq!(zoom(1920.0), 1.0);
        assert_eq!(zoom(1280.0), 1.0);
        assert!((zoom(640.0) - 0.5).abs() < 1e-6);
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

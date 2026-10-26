//! 首页使用原生 UI 排版与输入；装饰层复现 HTML 背景和 SVG 方块，标题由原稿导出。

use crate::{
    app::{
        state::GameState,
        transition::{Fade, SceneTarget},
    },
    ui::font::UiFont,
    ui::resources::{colors, dimens},
    ui::zoom,
    ui::resources::{
        colors::index as palette, dimens::index as dims, strings, strings_cn::index as labels,
    },
    ui::widgets::{self, CornerOrientation, text_font},
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
pub(crate) struct IndexCamera;
/// 菜单实体的共同根；进入游戏时递归清理。
#[derive(Component)]
pub(crate) struct IndexRoot;
/// 标题标记，文字由原稿以两倍分辨率导出以保留渐变和挤出。
#[derive(Component)]
pub(crate) struct IndexTitle;
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
    Background,
    /// 退出确认弹框：遮罩、面板、标题、按钮与按钮内文字/键帽（按角色着色）。
    DialogOverlay,
    DialogPanel,
    DialogTitle,
    DialogButton(ExitDialogSelection),
    DialogLabel(ExitDialogSelection),
    DialogKey(ExitDialogSelection),
}

/// 退出确认弹框的选中按钮；打开时默认确认（打开即 Enter 可退出，与设计稿一致）。
#[derive(Component, Clone, Copy, PartialEq, Eq, Default, Debug)]
pub(crate) enum ExitDialogSelection {
    #[default]
    Confirm,
    Cancel,
}

/// 首页退出确认弹框状态：遮罩 + 居中面板 + 取消/确认按钮（design.md 退出链路）。
/// 弹框存在期间独占全部键鼠输入；选中态由本组件维护，确认后淡出黑场退出。
#[derive(Component)]
pub(crate) struct ExitDialog {
    /// 当前选中按钮；←→/Tab/Y/N 切换，Enter 触发。
    pub(crate) selection: ExitDialogSelection,
    /// 遮罩与面板的淡入进度秒数（120ms ease-out）。
    pub(crate) elapsed: f32,
    /// [取消, 确认] 两个按钮的选中过渡进度 0..=1（220ms 连续过渡）。
    pub(crate) select: [f32; 2],
    /// 左键按下的按钮；按下与松开都落在同一按钮内才触发（与网页原生点击一致）。
    pub(crate) pressed: Option<ExitDialogSelection>,
}

/// 弹框面板与 [取消, 确认] 按钮的固定矩形；输入命中与视觉布局共用同一来源。
/// 面板水平垂直居中，最小宽 880px；按钮行右对齐、gap 32px、贴面板右下。
fn exit_dialog_rects(viewport: Vec2) -> (Rect, [Rect; 2]) {
    let button_height = dims::EXIT_BUTTON_HEIGHT;
    let panel_height = dims::EXIT_TITLE_PAD_TOP + dims::EXIT_TITLE_SIZE * dims::EXIT_TITLE_LINE_HEIGHT
        + dims::EXIT_TITLE_PAD_BOTTOM
        + button_height
        + dims::EXIT_ACTIONS_PAD_BOTTOM;
    let panel = Rect::from_center_size(
        viewport * 0.5,
        Vec2::new(dims::EXIT_PANEL_WIDTH, panel_height),
    );
    let bottom = panel.max.y - dims::EXIT_ACTIONS_PAD_BOTTOM;
    let confirm = Rect::from_corners(
        Vec2::new(
            panel.max.x - dims::EXIT_ACTIONS_PAD_RIGHT - dims::EXIT_BUTTON_WIDTH,
            bottom - button_height,
        ),
        Vec2::new(panel.max.x - dims::EXIT_ACTIONS_PAD_RIGHT, bottom),
    );
    let cancel = Rect::from_corners(
        Vec2::new(
            confirm.min.x - dims::EXIT_ACTIONS_GAP - dims::EXIT_BUTTON_WIDTH,
            bottom - button_height,
        ),
        Vec2::new(confirm.min.x - dims::EXIT_ACTIONS_GAP, bottom),
    );
    (panel, [cancel, confirm])
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

/// 给定物理像素与缩放系数，返回整体缩放后的 CSS 像素视口；
/// 窄窗口时宽度恒等于参考宽度，纵向空间为窗口高度除以缩放（与 zoom 语义一致）。
fn css_viewport(pixels: Vec2, scale_factor: f32) -> Vec2 {
    pixels / (scale_factor * zoom(pixels.x / scale_factor))
}

/// CSS 像素坐标布局；控件取 1280px 设计常量，视口缩放由内容相机的图像
/// scale_factor 乘以 zoom 统一处理，本结构只按 CSS 视口决定整体居中位置。
#[derive(Resource, Clone, Copy)]
pub(crate) struct Layout {
    /// 整体缩放后的 CSS 视口；弹框等居中浮层的矩形按此计算。
    pub viewport: Vec2,
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
        let content_height = (size.y - dims::TITLE_BAR - dims::BOTTOM - dims::PADDING_TOP
            - dims::PADDING_BOTTOM)
            .max(hero_height);
        let hero = Vec2::new(
            (size.x - hero_width - gap - cube_width) * 0.5,
            dims::TITLE_BAR + dims::PADDING_TOP + (content_height - hero_height) * 0.5,
        );
        let cube_height = cube_width * dims::CUBE_VIEWBOX_HEIGHT / dims::CUBE_VIEWBOX_WIDTH;
        Self {
            viewport: size,
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

/// 与 index.wesl 的 IndexPalette 同布局；Vec4 的 RGB 为 sRGB，W 为合成强度。
#[derive(ShaderType, Clone, Debug)]
pub(crate) struct IndexPalette {
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

impl Default for IndexPalette {
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
pub(crate) struct IndexBackground {
    /// XY 为逻辑视口尺寸，Z 为秒数，W 为装饰漂浮距离（逻辑像素）。
    #[uniform(0)]
    pub(crate) viewport: Vec4,
    #[uniform(1)]
    pub(crate) cubes: Vec4,
    /// 静态配色只在创建材质时由 UI 常量构建。
    #[uniform(2)]
    pub(crate) palette: IndexPalette,
}
impl UiMaterial for IndexBackground {
    fn fragment_shader() -> ShaderRef {
        "shaders/index.wesl".into()
    }
}

/// 首页插件，所有输入与动画只在 Index 且过渡结束后响应。
pub(crate) struct IndexPlugin;
impl Plugin for IndexPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MenuInput>()
            .init_resource::<ReturnFocus>()
            .insert_resource(Layout::new(Vec2::new(
                dimens::REFERENCE_WIDTH,
                dimens::REFERENCE_HEIGHT,
            )))
            .add_plugins(UiMaterialPlugin::<IndexBackground>::default())
            .add_systems(OnEnter(GameState::Index), spawn_index)
            .add_systems(OnEnter(GameState::InGame), despawn_index)
            .add_systems(OnEnter(GameState::Gallery), despawn_index)
            .add_systems(
                Update,
                // 输入先于视觉；menu_input 必须在标题栏 interaction 之后：弹框打开
                // 期间要吞掉它记下的 Tab 焦点，取消后又据其 focus 状态归还电源框焦点。
                (
                    menu_input.after(crate::ui::widgets::title_bar::interaction),
                    update_visual,
                )
                    .chain()
                    .run_if(|state: Res<State<GameState>>| {
                        matches!(
                            state.get(),
                            GameState::Index | GameState::ToGame | GameState::ToGallery
                        )
                    }),
            );
    }
}

/// 创建 UI 结构；坐标由 update_visual 按视口计算，未显示的 CSS 文案不生成。
fn spawn_index(
    mut commands: Commands,
    target: Res<SceneTarget>,
    assets: Res<AssetServer>,
    font: Res<UiFont>,
    mut input: ResMut<MenuInput>,
    mut materials: ResMut<Assets<IndexBackground>>,
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
            IndexCamera,
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
    let background = materials.add(IndexBackground {
        viewport: Vec4::new(
            dimens::REFERENCE_WIDTH,
            dimens::REFERENCE_HEIGHT,
            0.0,
            dims::AMBIENT_TRAVEL,
        ),
        cubes: Vec4::ZERO,
        palette: IndexPalette::default(),
    });
    commands
        .spawn((
            IndexRoot,
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
            // 首页常驻标题栏：电源符号返回框 + “主菜单”（design.md 标题栏规格）。
            // 必须晚于背景生成：同级 UI 按生成顺序绘制，否则会被全屏背景盖住。
            widgets::spawn_title_bar(
                root,
                &font,
                labels::TITLE,
                widgets::TitleBarIcon::Power(assets.load("image/index/power.png")),
            );
            root.spawn((
                Part::Title,
                IndexTitle,
                Name::new("V-TANK"),
                ImageNode::new(assets.load("image/index/title.png")),
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
            widgets::spawn_frame_corner(root, CornerOrientation::TopLeft, palette::CORNER);
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
                // 提示行键帽为静态样式，不附加驱动标记。
                widgets::spawn_keycap(hint, &font, strings::KEY_UP, ());
                widgets::spawn_keycap(hint, &font, strings::KEY_DOWN, ());
                caption(hint, labels::HINT_SELECT);
                widgets::spawn_keycap(hint, &font, strings::KEY_ENTER, ());
                caption(hint, labels::HINT_CONFIRM);
            });
            widgets::spawn_version_badge(root, &font, palette::CORNER);
        });
}

/// 生成退出确认弹框：全屏遮罩 + 居中面板 + 取消/确认按钮（含键帽提示）。
/// 坐标由 update_visual 按 exit_dialog_rects 逐帧更新，这里只建结构与静态样式。
fn spawn_exit_dialog(commands: &mut Commands, root: Entity, font: &UiFont) {
    commands.entity(root).with_children(|parent| {
        parent
            .spawn((
                ExitDialog {
                    selection: ExitDialogSelection::default(),
                    elapsed: 0.0,
                    // 初始选中确认：确认按钮直接呈现薄荷态（打开即 Enter 可退出）。
                    select: [0.0, 1.0],
                    pressed: None,
                },
                Part::DialogOverlay,
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ))
            .with_children(|overlay| {
                overlay
                    .spawn((
                        Part::DialogPanel,
                        Node {
                            position_type: PositionType::Absolute,
                            border: UiRect::all(px(dimens::BORDER_WIDTH)),
                            ..default()
                        },
                        BackgroundColor(Color::NONE),
                        BorderColor::all(Color::NONE),
                    ))
                    .with_children(|panel| {
                        panel.spawn((
                            Part::DialogTitle,
                            Text::new(labels::EXIT_TITLE),
                            text_font(font, dims::EXIT_TITLE_SIZE, 700.0),
                            LineHeight::RelativeToFont(dims::EXIT_TITLE_LINE_HEIGHT),
                            TextColor(Color::NONE),
                            Node {
                                position_type: PositionType::Absolute,
                                ..default()
                            },
                        ));
                        // 取消在左、确认在右；标签与键帽颜色随选中态过渡。
                        for (role, label, key) in [
                            (ExitDialogSelection::Cancel, labels::EXIT_CANCEL, strings::KEY_N),
                            (
                                ExitDialogSelection::Confirm,
                                labels::EXIT_CONFIRM,
                                strings::KEY_Y,
                            ),
                        ] {
                            panel
                                .spawn((
                                    Part::DialogButton(role),
                                    Node {
                                        position_type: PositionType::Absolute,
                                        // 1px 四边描边；内容弹性居中（网页 padding 8px 16px）。
                                        border: UiRect::all(px(dimens::BORDER_WIDTH)),
                                        display: Display::Flex,
                                        align_items: AlignItems::Center,
                                        justify_content: JustifyContent::Center,
                                        column_gap: px(dims::EXIT_BUTTON_GAP),
                                        ..default()
                                    },
                                    BorderColor::all(Color::NONE),
                                ))
                                .with_children(|button| {
                                    button.spawn((
                                        Part::DialogLabel(role),
                                        Text::new(label),
                                        text_font(font, dims::EXIT_BUTTON_SIZE, 700.0),
                                        TextColor(Color::NONE),
                                    ));
                                    // 弹框键帽复用统一控件；Part 标记让本页按选中态驱动
                                    // 文字与边框色（标题栏 interaction 只重着色自己的键帽）。
                                    widgets::spawn_keycap(
                                        button,
                                        font,
                                        key,
                                        Part::DialogKey(role),
                                    );
                                });
                        }
                    });
            });
    });
}

/// 取消路径：移除弹框并把焦点归还电源框（design.md 退出链路）。
fn close_exit_dialog(
    commands: &mut Commands,
    entity: Entity,
    title_bar: &mut crate::ui::widgets::TitleBarState,
) {
    commands.entity(entity).despawn();
    title_bar.set_focus(true);
}

/// 确认路径：移除弹框并沿用切页节奏淡出，黑场后退出进程（见 transition 的 ToExit）。
fn confirm_exit(
    commands: &mut Commands,
    entity: Entity,
    fade: &mut Fade,
    next: &mut NextState<GameState>,
) {
    commands.entity(entity).despawn();
    if fade.begin(GameState::ToExit) {
        next.set(GameState::ToExit);
    }
}

/// 屏幕坐标命中测试显式支持内部纹理相机；Bevy 默认 UI 拾取只接受窗口相机。
#[allow(clippy::too_many_arguments)]
fn menu_input(
    windows: Query<&Window>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    layout: Res<Layout>,
    mut input: ResMut<MenuInput>,
    mut fade: ResMut<Fade>,
    mut next: ResMut<NextState<GameState>>,
    mut commands: Commands,
    mut title_bar: ResMut<crate::ui::widgets::TitleBarState>,
    mut activated: MessageReader<crate::ui::widgets::TitleBarBackActivated>,
    mut dialogs: Query<(Entity, &mut ExitDialog)>,
    roots: Query<Entity, With<IndexRoot>>,
    font: Res<UiFont>,
) {
    if let Ok(window) = windows.single() {
        // 指针位于窗口逻辑像素；布局按整体缩放后的 CSS 视口计算，需除以 zoom。
        let s = zoom(window.width());
        input.pointer = window.cursor_position().map(|point| point / s);
    }
    if !fade.accepts_input() {
        return;
    }
    // 退出确认弹框独占输入：导航与确认只作用于最上层界面（design.md 导航与焦点归属）。
    if let Ok((entity, mut dialog)) = dialogs.single_mut() {
        // 吞掉标题栏 interaction 为本帧 Esc/点击写下的激活消息：否则关闭弹框后
        // 残留消息会在下一帧重新打开弹框。
        activated.read().for_each(drop);
        // ←→/Tab/Y/N 仅切换选中；Enter 触发选中按钮；Esc 取消；其余按键一律吞掉。
        if keys.just_pressed(KeyCode::KeyY) {
            dialog.selection = ExitDialogSelection::Confirm;
        } else if keys.just_pressed(KeyCode::KeyN) {
            dialog.selection = ExitDialogSelection::Cancel;
        } else if keys.any_just_pressed([KeyCode::ArrowLeft, KeyCode::ArrowRight, KeyCode::Tab]) {
            dialog.selection = match dialog.selection {
                ExitDialogSelection::Confirm => ExitDialogSelection::Cancel,
                ExitDialogSelection::Cancel => ExitDialogSelection::Confirm,
            };
        } else if keys.just_pressed(KeyCode::Escape) {
            close_exit_dialog(&mut commands, entity, &mut title_bar);
            return;
        } else if keys.just_pressed(KeyCode::Enter) {
            match dialog.selection {
                ExitDialogSelection::Confirm => {
                    confirm_exit(&mut commands, entity, &mut fade, &mut next);
                }
                ExitDialogSelection::Cancel => {
                    close_exit_dialog(&mut commands, entity, &mut title_bar);
                }
            }
            return;
        }
        // 鼠标：按下与松开都落在同一按钮内才触发；松开在面板外取消（含拖出）。
        let point = input.pointer.filter(|_| mouse.just_released(MouseButton::Left));
        if let Some(point) = point {
            let (panel, buttons) = exit_dialog_rects(layout.viewport);
            let hit = buttons
                .iter()
                .position(|rect| rect.contains(point))
                .map(|i| if i == 0 { ExitDialogSelection::Cancel } else { ExitDialogSelection::Confirm });
            let pressed = dialog.pressed.take();
            if pressed.is_some() && pressed == hit {
                match hit.unwrap() {
                    ExitDialogSelection::Confirm => {
                        confirm_exit(&mut commands, entity, &mut fade, &mut next);
                    }
                    ExitDialogSelection::Cancel => {
                        close_exit_dialog(&mut commands, entity, &mut title_bar);
                    }
                }
            } else if !panel.contains(point) {
                close_exit_dialog(&mut commands, entity, &mut title_bar);
            }
            return;
        }
        // 弹框期间按下按钮记录目标；标题栏 Tab 焦点被吞掉。
        if mouse.just_pressed(MouseButton::Left) {
            if let Some(point) = input.pointer {
                let (_, buttons) = exit_dialog_rects(layout.viewport);
                dialog.pressed = buttons
                    .iter()
                    .position(|rect| rect.contains(point))
                    .map(|i| if i == 0 { ExitDialogSelection::Cancel } else { ExitDialogSelection::Confirm });
            }
        }
        title_bar.set_focus(false);
        return;
    }
    // 稳态点击电源框或 Esc → 打开退出确认弹框；已存在时为空操作。
    // 消息必须每帧读完，否则残留会在后续帧误触发。
    let activate_back = activated.read().next().is_some();
    // Tab 会同时聚焦电源框与菜单项；同帧 Enter 且菜单持有键盘焦点时，
    // 激活优先归属菜单确认，避免误开弹框（电源框 Enter 仅在菜单无键盘焦点时生效）。
    let menu_enter = keys.just_pressed(KeyCode::Enter) && input.keyboard_focus;
    if keys.just_pressed(KeyCode::Escape) || (activate_back && !menu_enter) {
        if let Ok(root) = roots.single() {
            spawn_exit_dialog(&mut commands, root, &font);
        }
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
        // 首页的 Tab 语义是菜单导航；清掉标题栏 interaction 同帧记下的电源框焦点，避免双高亮。
        title_bar.set_focus(false);
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
        Option<&mut BackgroundColor>,
    )>,
    mut dialogs: Query<&mut ExitDialog>,
    mut materials: ResMut<Assets<IndexBackground>>,
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
    // 弹框淡入与选中态进度；无弹框时使用中性值（弹框 Part 也不存在）。
    let mut dialog_fade = 1.0;
    let mut dialog_select = [0.0f32, 1.0];
    if let Ok(mut dialog) = dialogs.single_mut() {
        dialog.elapsed = (dialog.elapsed + time.delta_secs()).min(dims::EXIT_FADE_IN);
        // 120ms ease-out：用 easeOutCubic 近似 CSS cubic-bezier(0,0,0.58,1)。
        let t = dialog.elapsed / dims::EXIT_FADE_IN;
        dialog_fade = 1.0 - (1.0 - t).powi(3);
        // 选中态 220ms 连续过渡，移入移出从当前进度继续。
        let step = time.delta_secs() / 0.22;
        for (i, role) in
            [ExitDialogSelection::Cancel, ExitDialogSelection::Confirm].into_iter().enumerate()
        {
            let target = f32::from(dialog.selection == role);
            dialog.select[i] += (target - dialog.select[i]).clamp(-step, step);
        }
        dialog_select = dialog.select;
    }
    for (part, mut node, color, font, spacing, border, bg) in &mut nodes {
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
            Part::DialogOverlay => {
                rect = Some((0.0, 0.0, l.viewport.x, l.viewport.y));
                if let Some(mut bg) = bg {
                    bg.0 = palette::EXIT_OVERLAY
                        .with_alpha(palette::EXIT_OVERLAY.alpha() * dialog_fade);
                }
            }
            Part::DialogPanel => {
                let (panel, _) = exit_dialog_rects(l.viewport);
                rect = Some((panel.min.x, panel.min.y, panel.width(), panel.height()));
                if let Some(mut bg) = bg {
                    bg.0 = palette::EXIT_PANEL.with_alpha(dialog_fade);
                }
                if let Some(mut border) = border {
                    *border = BorderColor::all(palette::EXIT_OUTLINE.with_alpha(dialog_fade));
                }
            }
            Part::DialogTitle => {
                let (panel, _) = exit_dialog_rects(l.viewport);
                // 标题是面板子节点：绝对定位坐标相对面板左上角。
                rect = Some((
                    dims::EXIT_TITLE_PAD_X,
                    dims::EXIT_TITLE_PAD_TOP,
                    panel.width() - dims::EXIT_TITLE_PAD_X * 2.0,
                    dims::EXIT_TITLE_SIZE * dims::EXIT_TITLE_LINE_HEIGHT,
                ));
                if let Some(mut color) = color {
                    color.0 = colors::ON_SURFACE.with_alpha(dialog_fade);
                }
            }
            Part::DialogButton(role) => {
                let (panel, buttons) = exit_dialog_rects(l.viewport);
                let i = usize::from(role == ExitDialogSelection::Confirm);
                let r = &buttons[i];
                // 按钮同样是面板子节点：换算为面板内相对坐标。
                rect = Some((r.min.x - panel.min.x, r.min.y - panel.min.y, r.width(), r.height()));
                let p = dialog_select[i];
                if let Some(mut bg) = bg {
                    // 透明底过渡到薄荷填充：sRGB 分量插值，与网页一致。
                    bg.0 = Color::NONE.mix(&colors::PRIMARY, p).with_alpha(p * dialog_fade);
                }
                if let Some(mut border) = border {
                    *border = BorderColor::all(
                        palette::EXIT_OUTLINE
                            .mix(&colors::PRIMARY, p)
                            .with_alpha(dialog_fade),
                    );
                }
            }
            Part::DialogLabel(role) | Part::DialogKey(role) => {
                let i = usize::from(role == ExitDialogSelection::Confirm);
                let p = dialog_select[i];
                if let Some(mut color) = color {
                    color.0 = colors::ON_SURFACE
                        .mix(&colors::ON_PRIMARY, p)
                        .with_alpha(dialog_fade);
                }
                if let Some(mut border) = border {
                    *border = BorderColor::all(
                        colors::KBD_BORDER
                            .mix(&colors::KBD_BORDER_ACTIVE, p)
                            .with_alpha(dialog_fade),
                    );
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
fn despawn_index(
    mut commands: Commands,
    entities: Query<Entity, Or<(With<IndexRoot>, With<IndexCamera>)>>,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

/// 标题纹理必须真正加载成功，不能只等字体加载完成便揭幕。
pub(crate) fn assets_ready(world: &mut World) -> bool {
    let handles: Vec<_> = world
        .query_filtered::<&ImageNode, With<IndexTitle>>()
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
    use crate::app::transition::Phase;
    use crate::ui::widgets::TitleBarState;
    use bevy::ecs::system::RunSystemOnce;

    /// 弹框输入测试的公共场景：布局、输入资源、返回框状态与一个 IndexRoot 实体。
    fn dialog_world() -> World {
        let mut world = World::new();
        world.insert_resource(Layout::new(Vec2::new(1280.0, 720.0)));
        world.init_resource::<MenuInput>();
        world.init_resource::<Fade>();
        world.init_resource::<NextState<GameState>>();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<ButtonInput<MouseButton>>();
        world.init_resource::<TitleBarState>();
        world.init_resource::<Messages<crate::ui::widgets::TitleBarBackActivated>>();
        world.insert_resource(UiFont(
            Handle::<Font>::default(),
            crate::ui::font::TextOptics::default(),
        ));
        world.spawn(IndexRoot);
        world
    }

    /// 以“仅该键刚按下”的输入运行一次菜单输入系统。
    fn press(world: &mut World, key: KeyCode) {
        let mut keys = ButtonInput::default();
        keys.press(key);
        world.insert_resource(keys);
        world.insert_resource(ButtonInput::<MouseButton>::default());
        world.run_system_once(menu_input).unwrap();
    }

    /// 在指定 CSS 坐标完成一次按下与松开（两次系统运行）。
    fn click(world: &mut World, point: Vec2) {
        world.resource_mut::<MenuInput>().pointer = Some(point);
        let mut mouse = ButtonInput::default();
        mouse.press(MouseButton::Left);
        world.insert_resource(mouse);
        world.insert_resource(ButtonInput::<KeyCode>::default());
        world.run_system_once(menu_input).unwrap();
        world.resource_mut::<MenuInput>().pointer = Some(point);
        world.resource_mut::<ButtonInput<MouseButton>>().clear();
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        world.run_system_once(menu_input).unwrap();
    }

    /// 当前弹框选中值；None 表示弹框不存在。
    fn dialog(world: &mut World) -> Option<ExitDialogSelection> {
        world
            .query::<&ExitDialog>()
            .iter(world)
            .next()
            .map(|d| d.selection)
    }

    /// Esc 打开弹框且初始选中确认；弹框已存在时重复打开是空操作。
    #[test]
    fn exit_dialog_opens_on_escape_with_confirm_selected() {
        let mut world = dialog_world();
        press(&mut world, KeyCode::Escape);
        assert_eq!(dialog(&mut world), Some(ExitDialogSelection::Confirm));
        // 电源框激活（点击返回框）同样打开；已打开时不重复生成。
        world.resource_mut::<ButtonInput<KeyCode>>().clear();
        world
            .resource_mut::<Messages<crate::ui::widgets::TitleBarBackActivated>>()
            .write(crate::ui::widgets::TitleBarBackActivated);
        world.run_system_once(menu_input).unwrap();
        let count = world.query::<&ExitDialog>().iter(&world).count();
        assert_eq!(count, 1);
    }

    /// ←→/Tab/Y/N 只切换选中；Enter 触发选中按钮：确认进入退出淡出，取消关闭弹框。
    #[test]
    fn exit_dialog_keys_switch_selection_and_enter_triggers() {
        let mut world = dialog_world();
        press(&mut world, KeyCode::Escape);
        press(&mut world, KeyCode::ArrowLeft);
        assert_eq!(dialog(&mut world), Some(ExitDialogSelection::Cancel));
        press(&mut world, KeyCode::KeyN);
        assert_eq!(dialog(&mut world), Some(ExitDialogSelection::Cancel));
        press(&mut world, KeyCode::KeyY);
        assert_eq!(dialog(&mut world), Some(ExitDialogSelection::Confirm));
        press(&mut world, KeyCode::Tab);
        assert_eq!(dialog(&mut world), Some(ExitDialogSelection::Cancel));
        press(&mut world, KeyCode::ArrowRight);
        assert_eq!(dialog(&mut world), Some(ExitDialogSelection::Confirm));
        // Enter 触发确认：进入退出淡出，弹框随黑场流程移除。
        press(&mut world, KeyCode::Enter);
        assert_eq!(dialog(&mut world), None);
        assert_eq!(world.resource::<Fade>().phase, Phase::Out);
        assert!(matches!(
            world.resource::<NextState<GameState>>(),
            NextState::Pending(state) if *state == GameState::ToExit
        ));
    }

    /// Esc 或取消路径关闭弹框并把焦点归还电源框，页面过渡不受影响。
    #[test]
    fn exit_dialog_cancel_restores_back_box_focus() {
        let mut world = dialog_world();
        press(&mut world, KeyCode::Escape);
        press(&mut world, KeyCode::Escape);
        assert_eq!(dialog(&mut world), None);
        assert_eq!(world.resource::<Fade>().phase, Phase::Idle);
        assert!(world.resource::<TitleBarState>().focus);
        // Enter 选中取消按钮同样只关闭弹框。
        press(&mut world, KeyCode::Escape);
        press(&mut world, KeyCode::ArrowLeft);
        press(&mut world, KeyCode::Enter);
        assert_eq!(dialog(&mut world), None);
        assert_eq!(world.resource::<Fade>().phase, Phase::Idle);
        assert!(world.resource::<TitleBarState>().focus);
    }

    /// 菜单持有键盘焦点时 Enter 归属菜单确认，同帧的电源框激活消息被吞掉、不误开弹框。
    #[test]
    fn menu_keyboard_focus_wins_enter_over_back_box() {
        let mut world = dialog_world();
        // 模拟弹框取消后的状态：电源框有焦点，菜单仍持有键盘焦点。
        world.resource_mut::<MenuInput>().focus = Some(0);
        world.resource_mut::<MenuInput>().keyboard_focus = true;
        world
            .resource_mut::<Messages<crate::ui::widgets::TitleBarBackActivated>>()
            .write(crate::ui::widgets::TitleBarBackActivated);
        press(&mut world, KeyCode::Enter);
        assert_eq!(dialog(&mut world), None);
        assert_eq!(world.resource::<Fade>().phase, Phase::Out);
        assert!(matches!(
            world.resource::<NextState<GameState>>(),
            NextState::Pending(state) if *state == GameState::ToGame
        ));
    }

    /// 菜单没有键盘焦点时，Enter 归属聚焦的电源框 → 打开退出确认弹框。
    #[test]
    fn back_box_enter_opens_dialog_without_menu_keyboard_focus() {
        let mut world = dialog_world();
        world
            .resource_mut::<Messages<crate::ui::widgets::TitleBarBackActivated>>()
            .write(crate::ui::widgets::TitleBarBackActivated);
        press(&mut world, KeyCode::Enter);
        assert_eq!(dialog(&mut world), Some(ExitDialogSelection::Confirm));
    }

    /// 弹框打开期间吞掉菜单导航与点击，防止穿透到开始游戏/展示馆。
    #[test]
    fn exit_dialog_swallows_menu_input_while_open() {
        let mut world = dialog_world();
        press(&mut world, KeyCode::Escape);
        world.resource_mut::<MenuInput>().focus = Some(0);
        world.resource_mut::<MenuInput>().keyboard_focus = true;
        press(&mut world, KeyCode::ArrowDown);
        assert_eq!(world.resource::<MenuInput>().focus, Some(0));
        // 菜单项区域（面板外）的完整点击只取消弹框，不触发菜单项。
        let layout = *world.resource::<Layout>();
        click(
            &mut world,
            Vec2::new(layout.hero.x + 20.0, layout.menu_y + 20.0),
        );
        assert_eq!(world.resource::<Fade>().phase, Phase::Idle);
        assert_eq!(dialog(&mut world), None);
    }

    /// 鼠标点击按钮直接触发；面板外点击取消；按钮上按下拖出面板也取消。
    #[test]
    fn exit_dialog_mouse_triggers_buttons_and_cancels_outside() {
        let (panel, [cancel, confirm]) = exit_dialog_rects(Vec2::new(1280.0, 720.0));
        let mut world = dialog_world();
        press(&mut world, KeyCode::Escape);
        // 点击确认 → 进入退出淡出。
        click(&mut world, confirm.center());
        assert_eq!(world.resource::<Fade>().phase, Phase::Out);
        assert_eq!(dialog(&mut world), None);
        // 点击取消 → 关闭并归还焦点。
        let mut world = dialog_world();
        press(&mut world, KeyCode::Escape);
        click(&mut world, cancel.center());
        assert_eq!(dialog(&mut world), None);
        assert_eq!(world.resource::<Fade>().phase, Phase::Idle);
        assert!(world.resource::<TitleBarState>().focus);
        // 面板外（且不在返回框内）点击 → 取消。
        let outside = Vec2::new(panel.center().x, 100.0);
        assert!(!panel.contains(outside));
        let mut world = dialog_world();
        press(&mut world, KeyCode::Escape);
        click(&mut world, outside);
        assert_eq!(dialog(&mut world), None);
        assert_eq!(world.resource::<Fade>().phase, Phase::Idle);
        // 按下确认后拖出面板松开 → 取消而非退出。
        let mut world = dialog_world();
        press(&mut world, KeyCode::Escape);
        world.resource_mut::<MenuInput>().pointer = Some(confirm.center());
        let mut mouse = ButtonInput::default();
        mouse.press(MouseButton::Left);
        world.insert_resource(mouse);
        world.insert_resource(ButtonInput::<KeyCode>::default());
        world.run_system_once(menu_input).unwrap();
        world.resource_mut::<MenuInput>().pointer = Some(outside);
        world.resource_mut::<ButtonInput<MouseButton>>().clear();
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        world.run_system_once(menu_input).unwrap();
        assert_eq!(dialog(&mut world), None);
        assert_eq!(world.resource::<Fade>().phase, Phase::Idle);
        let _ = panel;
    }

    /// 弹框矩形：面板居中、按钮行贴右下、命中区与视觉常量一致。
    #[test]
    fn exit_dialog_rects_center_panel_and_align_buttons_bottom_right() {
        let viewport = Vec2::new(1280.0, 720.0);
        let (panel, [cancel, confirm]) = exit_dialog_rects(viewport);
        assert!((panel.center() - viewport * 0.5).length() < 1e-4);
        assert_eq!(panel.width(), dims::EXIT_PANEL_WIDTH);
        // 确认在右、取消在左，间距 32px，均贴面板右下角。
        assert!((confirm.max.x - (panel.max.x - dims::EXIT_ACTIONS_PAD_RIGHT)).abs() < 1e-4);
        assert!((confirm.max.y - (panel.max.y - dims::EXIT_ACTIONS_PAD_BOTTOM)).abs() < 1e-4);
        assert!((cancel.max.x - (confirm.min.x - dims::EXIT_ACTIONS_GAP)).abs() < 1e-4);
        assert_eq!(cancel.width(), dims::EXIT_BUTTON_WIDTH);
        assert!(panel.contains(confirm.center()));
        assert!(panel.contains(cancel.center()));
    }

    /// 材质必须传原始 sRGB 而非线性值，且保留网格与阴影的合成强度。
    #[test]
    fn shader_palette_preserves_srgb_and_alpha() {
        let palette = IndexPalette::default();
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
        let mut world = dialog_world();
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
        let mut world = dialog_world();
        let layout = *world.resource::<Layout>();
        let point = Vec2::new(layout.hero.x + 20.0, layout.menu_y + 20.0);
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

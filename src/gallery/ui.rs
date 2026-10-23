//! 固定尺寸画廊排版与输入；所有命中坐标均为整体缩放后的逻辑像素。
use super::*;
use crate::{
    font::UiFont,
    menu::{MenuBackground, MenuPalette},
    ui_resources::{
        colors,
        dimens::gallery as dims,
        strings_cn::{gallery as labels, menu as menu_labels},
    },
};
use bevy::{
    core_pipeline::tonemapping::DebandDither,
    input::mouse::{MouseScrollUnit, MouseWheel},
    text::{LetterSpacing, LineHeight},
    ui_render::prelude::MaterialNode,
};

/// 滚动与焦点状态，独立于模型渲染；无窗口验证可注入同一逻辑指针。
#[derive(Resource, Default)]
pub(crate) struct View {
    pub scroll: f32,
    pub pointer: Option<Vec2>,
    size: Vec2,
    focus: bool,
    pressed: bool,
    hover: [f32; 8],
    line: [f32; 8],
}
/// 节点角色用于窗口变化时更新已有布局，不反复创建实体。
#[derive(Component, Clone, Copy)]
pub(super) enum Part {
    Background,
    Header,
    Back,
    BackText,
    Title,
    Viewport,
    Content,
    Index(usize),
    Heading(usize),
    Count(usize),
    Card(usize),
    Model,
    Name(usize),
    Meta(usize),
    Line(usize),
    Build,
    Corner(bool),
}

/// 卡片有两项仍占固定四列中的前两列，不按数据量拉伸。
fn card_rect(index: usize, left: f32, scroll: f32) -> Rect {
    let row = index / 2;
    let x = left + (index % 2) as f32 * (dims::CARD_WIDTH + dims::GAP);
    let y = dims::HEADER_HEIGHT
        + dims::SECTION_GAP
        + row as f32 * section_height()
        + dims::SECTION_TO_CARD
        - scroll;
    Rect::from_corners(
        Vec2::new(x, y),
        Vec2::new(x + dims::CARD_WIDTH, y + dims::CARD_HEIGHT),
    )
}
/// 分类间的固定节距，含标题、卡片和栏目留白。
fn section_height() -> f32 {
    dims::SECTION_GAP + dims::SECTION_TO_CARD + dims::CARD_HEIGHT
}
/// 滚动最大值由逻辑视口决定，短窗口与窗口缩放共用。
pub(super) fn max_scroll(height: f32) -> f32 {
    (section_height() * 4.0 + dims::CONTENT_BOTTOM
        - (height - dims::HEADER_HEIGHT - dims::FOOTER_HEIGHT).max(0.0))
    .max(0.0)
}
/// 只提供实际可操作的返回键；卡片没有按钮或焦点组件。
pub(super) fn spawn(
    mut commands: Commands,
    target: Res<SceneTarget>,
    font: Res<UiFont>,
    previews: Res<Previews>,
    mut view: ResMut<View>,
    mut materials: ResMut<Assets<MenuBackground>>,
) {
    *view = View::default();
    let camera = commands
        .spawn((
            GalleryEntity,
            GalleryCamera,
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
    let background = materials.add(MenuBackground {
        viewport: Vec4::new(1280.0, 720.0, 0.0, 0.0),
        cubes: Vec4::ZERO,
        palette: MenuPalette::default(),
    });
    commands
        .spawn((
            GalleryEntity,
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
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                MaterialNode(background),
            ));
            root.spawn((
                Part::Viewport,
                Node {
                    position_type: PositionType::Absolute,
                    overflow: Overflow::clip(),
                    ..default()
                },
            ))
            .with_children(|viewport| {
                viewport
                    .spawn((
                        Part::Content,
                        Node {
                            position_type: PositionType::Absolute,
                            ..default()
                        },
                    ))
                    .with_children(|content| {
                        for (category, label) in labels::CATEGORIES.iter().enumerate() {
                            content.spawn((
                                Part::Index(category),
                                Text::new(format!("{:02}", category + 1)),
                                text_font(&font, dims::NAME_SIZE, 400.0),
                                TextColor(colors::PRIMARY_DEEP),
                                LetterSpacing::Px(dims::LETTER_SPACING),
                                absolute(),
                            ));
                            content.spawn((
                                Part::Heading(category),
                                Text::new(*label),
                                text_font(&font, dims::HEADING_SIZE, 700.0),
                                TextColor(colors::ON_SURFACE),
                                absolute(),
                            ));
                            content.spawn((
                                Part::Count(category),
                                Text::new(format!(
                                    "{} {}",
                                    ENTRIES.iter().filter(|e| e.category == category).count(),
                                    labels::COUNT
                                )),
                                text_font(&font, dims::META_SIZE, 400.0),
                                TextColor(colors::ON_SURFACE_MUTED),
                                absolute(),
                            ));
                        }
                        for (index, entry) in ENTRIES.iter().enumerate() {
                            content
                                .spawn((
                                    Part::Card(index),
                                    absolute(),
                                    BackgroundColor(colors::gallery::SURFACE),
                                    BorderColor::all(colors::gallery::OUTLINE),
                                ))
                                .with_children(|card| {
                                    card.spawn((
                                        Part::Model,
                                        absolute(),
                                        ImageNode::new(previews.items[index].image.clone()),
                                    ));
                                    card.spawn((
                                        Part::Name(index),
                                        Text::new(entry.name()),
                                        text_font(&font, dims::NAME_SIZE, 700.0),
                                        TextColor(colors::ON_SURFACE),
                                        LetterSpacing::Px(dims::LETTER_SPACING),
                                        LineHeight::RelativeToFont(1.3333),
                                        absolute(),
                                    ));
                                    card.spawn((
                                        Part::Meta(index),
                                        Text::new(labels::LOADING),
                                        text_font(&font, dims::META_SIZE, 400.0),
                                        TextColor(colors::ON_SURFACE_MUTED),
                                        absolute(),
                                    ));
                                    card.spawn((
                                        Part::Line(index),
                                        absolute(),
                                        BackgroundColor(colors::PRIMARY),
                                    ));
                                });
                        }
                    });
            });
            root.spawn((
                Part::Header,
                absolute(),
                BackgroundColor(colors::menu::BACKGROUND),
                BorderColor::all(colors::gallery::OUTLINE),
            ))
            .with_children(|header| {
                header
                    .spawn((
                        Part::Back,
                        absolute(),
                        BackgroundColor(colors::gallery::SURFACE),
                        BorderColor::all(colors::gallery::OUTLINE),
                    ))
                    .with_children(|back| {
                        back.spawn((
                            Part::BackText,
                            Text::new(format!(
                                "{}   {}",
                                crate::ui_resources::strings::BACK_ARROW,
                                crate::ui_resources::strings::KEY_ESCAPE
                            )),
                            text_font(&font, dims::META_SIZE, 400.0),
                            TextColor(colors::ON_SURFACE),
                            absolute(),
                        ));
                    });
                header.spawn((
                    Part::Title,
                    Text::new(labels::TITLE),
                    text_font(&font, dims::TITLE_SIZE, 700.0),
                    TextColor(colors::ON_SURFACE),
                    absolute(),
                ));
            });
            root.spawn((
                Part::Build,
                Text::new(menu_labels::BUILD),
                text_font(&font, 10.0, 400.0),
                TextColor(colors::ON_SURFACE_MUTED),
                absolute(),
            ));
            for bottom in [false, true] {
                root.spawn((
                    Part::Corner(bottom),
                    absolute(),
                    BorderColor::all(colors::gallery::OUTLINE_ACTIVE),
                ));
            }
        });
}
/// UI 节点使用固定逻辑像素，窗口适配只通过页面相机统一缩放。
fn absolute() -> Node {
    Node {
        position_type: PositionType::Absolute,
        ..default()
    }
}
/// 页面字体统一来自现有 MiSans 资源。
fn text_font(font: &UiFont, size: f32, weight: f32) -> TextFont {
    TextFont {
        font: font.0.clone().into(),
        font_size: size.into(),
        weight: FontWeight(weight as u16),
        ..default()
    }
}
/// 当前页面逻辑视口；离屏直接读取内容纹理尺寸。
fn viewport(windows: &Query<&Window>, images: &Assets<Image>, target: &SceneTarget) -> Vec2 {
    if let Ok(window) = windows.single() {
        Vec2::new(window.width(), window.height()) / crate::menu::zoom(window.width())
    } else {
        let size = images.get(&target.0).unwrap().size().as_vec2();
        size / crate::menu::zoom(size.x)
    }
}
/// 输入先经过过渡锁；鼠标按下和松开必须同在返回键内。
pub(super) fn input(
    windows: Query<&Window>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut wheel: MessageReader<MouseWheel>,
    mut view: ResMut<View>,
    images: Res<Assets<Image>>,
    target: Res<SceneTarget>,
    mut fade: ResMut<Fade>,
    mut next: ResMut<NextState<GameState>>,
    mut return_focus: ResMut<crate::menu::ReturnFocus>,
) {
    let size = viewport(&windows, &images, &target);
    view.size = size;
    if let Ok(window) = windows.single() {
        view.pointer = window
            .cursor_position()
            .map(|p| p / crate::menu::zoom(window.width()));
    }
    let scale = windows.single().map_or_else(
        |_| crate::menu::zoom(images.get(&target.0).unwrap().size().x as f32),
        |window| crate::menu::zoom(window.width()),
    );
    let delta: f32 = wheel
        .read()
        .map(|event| {
            -event.y
                * if event.unit == MouseScrollUnit::Line {
                    dims::SCROLL_STEP
                } else {
                    1.0 / scale
                }
        })
        .sum();
    if !fade.accepts_input() {
        view.pressed = false;
        return;
    }
    let max = max_scroll(size.y);
    let mut scroll = view.scroll + delta;
    for (key, amount) in [
        (KeyCode::ArrowUp, -dims::SCROLL_STEP),
        (KeyCode::ArrowDown, dims::SCROLL_STEP),
        (
            KeyCode::PageUp,
            -(size.y - dims::HEADER_HEIGHT - dims::FOOTER_HEIGHT - dims::SCROLL_STEP)
                .max(dims::SCROLL_STEP),
        ),
        (
            KeyCode::PageDown,
            (size.y - dims::HEADER_HEIGHT - dims::FOOTER_HEIGHT - dims::SCROLL_STEP)
                .max(dims::SCROLL_STEP),
        ),
    ] {
        if keys.just_pressed(key) {
            scroll += amount;
        }
    }
    if keys.just_pressed(KeyCode::Home) {
        scroll = 0.0;
    }
    if keys.just_pressed(KeyCode::End) {
        scroll = max;
    }
    view.scroll = scroll.clamp(0.0, max);
    if keys.just_pressed(KeyCode::Tab) {
        view.focus = true;
    }
    let inside = view.pointer.is_some_and(|p| back_rect().contains(p));
    if mouse.just_pressed(MouseButton::Left) {
        view.pressed = inside;
        view.focus = false;
    }
    let clicked = mouse.just_released(MouseButton::Left) && view.pressed && inside;
    if mouse.just_released(MouseButton::Left) {
        view.pressed = false;
    }
    if (clicked
        || keys.just_pressed(KeyCode::Escape)
        || (view.focus && keys.just_pressed(KeyCode::Enter)))
        && fade.begin(GameState::Menu)
    {
        return_focus.0 = true;
        next.set(GameState::GalleryToMenu);
    }
}
/// 返回键与设计稿保持同一尺寸、位置和命中区域。
fn back_rect() -> Rect {
    let (left, top, width, height) = dims::BACK_RECT;
    Rect::from_corners(Vec2::new(left, top), Vec2::new(left + width, top + height))
}

/// 三次贝塞尔以二分求时间参数，与网页的缓动曲线一致。
fn eased(t: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
    for _ in 0..12 {
        let u = (lo + hi) * 0.5;
        let x = 3.0 * (1.0 - u).powi(2) * u * x1 + 3.0 * (1.0 - u) * u * u * x2 + u * u * u;
        if x < t {
            lo = u;
        } else {
            hi = u;
        }
    }
    let u = (lo + hi) * 0.5;
    3.0 * (1.0 - u).powi(2) * u * y1 + 3.0 * (1.0 - u) * u * u * y2 + u * u * u
}

/// 连续反馈保留反向时的当前位置，边框/文字和底线分别采用220/300毫秒。
pub(super) fn update(
    mut view: ResMut<View>,
    time: Res<Time>,
    previews: Res<Previews>,
    mut nodes: Query<(
        &Part,
        &mut Node,
        Option<&mut BackgroundColor>,
        Option<&mut BorderColor>,
        Option<&mut TextColor>,
        Option<&mut Text>,
    )>,
    mut materials: ResMut<Assets<MenuBackground>>,
) {
    let size = view.size;
    let left = (size.x - dims::WIDTH) * 0.5;
    let scroll = view.scroll;
    let content_height = section_height() * 4.0 + dims::CONTENT_BOTTOM;
    for i in 0..8 {
        let active = view.pointer.is_some_and(|p| {
            p.y >= dims::HEADER_HEIGHT
                && p.y < size.y - dims::FOOTER_HEIGHT
                && card_rect(i, left, scroll).contains(p)
        });
        let direction = if active { 1.0 } else { -1.0 };
        view.hover[i] = (view.hover[i] + direction * time.delta_secs() / 0.22).clamp(0.0, 1.0);
        view.line[i] = (view.line[i] + direction * time.delta_secs() / 0.30).clamp(0.0, 1.0);
    }
    for (_, material) in materials.iter_mut() {
        material.viewport = Vec4::new(size.x, size.y, time.elapsed_secs(), 0.0);
        material.cubes = Vec4::ZERO;
    }
    let back_active = view.focus || view.pointer.is_some_and(|p| back_rect().contains(p));
    for (part, mut node, background, border, text_color, text) in &mut nodes {
        let rect = match *part {
            Part::Background => (0.0, 0.0, size.x, size.y),
            Part::Header => {
                node.border.bottom = px(1.0);
                (0.0, 0.0, size.x, dims::HEADER_HEIGHT)
            }
            Part::Back => {
                node.border = UiRect::all(px(1.0));
                if let Some(mut bg) = background {
                    bg.0 = if back_active {
                        colors::PRIMARY
                    } else {
                        colors::gallery::SURFACE
                    };
                }
                if let Some(mut border) = border {
                    *border = BorderColor::all(if back_active {
                        colors::PRIMARY
                    } else {
                        colors::gallery::OUTLINE
                    });
                }
                dims::BACK_RECT
            }
            Part::BackText => {
                if let Some(mut color) = text_color {
                    color.0 = if back_active {
                        colors::ON_PRIMARY
                    } else {
                        colors::ON_SURFACE
                    };
                }
                dims::BACK_TEXT_RECT
            }
            Part::Title => dims::TITLE_RECT,
            Part::Viewport => (
                left,
                dims::HEADER_HEIGHT,
                dims::WIDTH,
                (size.y - dims::HEADER_HEIGHT - dims::FOOTER_HEIGHT).max(0.0),
            ),
            Part::Content => (0.0, -scroll, dims::WIDTH, content_height),
            Part::Index(i) => (
                0.0,
                dims::SECTION_GAP + i as f32 * section_height() + 6.0,
                40.0,
                26.0,
            ),
            Part::Heading(i) => (
                48.0,
                dims::SECTION_GAP + i as f32 * section_height(),
                552.0,
                32.0,
            ),
            Part::Count(i) => (
                dims::WIDTH - 86.0,
                dims::SECTION_GAP + i as f32 * section_height() + 12.0,
                86.0,
                20.0,
            ),
            Part::Card(i) => {
                node.border = UiRect::all(px(1.0));
                if let Some(mut border) = border {
                    *border = BorderColor::all(colors::gallery::OUTLINE.mix(
                        &colors::gallery::OUTLINE_ACTIVE,
                        eased(view.hover[i], 0.25, 0.1, 0.25, 1.0),
                    ));
                }
                let r = card_rect(i, 0.0, 0.0);
                (
                    r.min.x,
                    r.min.y - dims::HEADER_HEIGHT,
                    dims::CARD_WIDTH,
                    dims::CARD_HEIGHT,
                )
            }
            Part::Model => (
                dims::CARD_PADDING,
                dims::CARD_PADDING,
                dims::CARD_WIDTH - 2.0 * dims::CARD_PADDING,
                dims::PREVIEW_HEIGHT,
            ),
            Part::Name(i) => {
                if let Some(mut color) = text_color {
                    color.0 = colors::ON_SURFACE.mix(
                        &colors::gallery::NAME_ACTIVE,
                        eased(view.hover[i], 0.25, 0.1, 0.25, 1.0),
                    );
                }
                dims::NAME_RECT
            }
            Part::Meta(i) => {
                if let Some(mut text) = text {
                    let p = &previews.items[i];
                    let value = if let Some(size) = p.size {
                        format!(
                            "{:.1} × {:.1} × {:.1} {}",
                            size.x,
                            size.z,
                            size.y,
                            labels::UNIT
                        )
                    } else if p.error.is_some() {
                        labels::FAILED.into()
                    } else {
                        labels::LOADING.into()
                    };
                    if text.0 != value {
                        text.0 = value;
                    }
                }
                dims::META_RECT
            }
            Part::Line(i) => (
                0.0,
                dims::CARD_HEIGHT - dims::HOVER_LINE_HEIGHT - 1.0,
                (dims::CARD_WIDTH - 2.0) * eased(view.line[i], 0.2, 0.8, 0.2, 1.0),
                2.0,
            ),
            Part::Build => (size.x - 148.0, size.y - 38.0, 110.0, 20.0),
            Part::Corner(bottom) => {
                let inset = crate::ui_resources::dimens::menu::CORNER_INSET;
                let side = crate::ui_resources::dimens::menu::CORNER_SIZE;
                node.border = if bottom {
                    UiRect {
                        right: px(1.0),
                        bottom: px(1.0),
                        ..default()
                    }
                } else {
                    UiRect {
                        left: px(1.0),
                        top: px(1.0),
                        ..default()
                    }
                };
                if bottom {
                    (size.x - inset - side, size.y - inset - side, side, side)
                } else {
                    (inset, inset, side, side)
                }
            }
        };
        node.left = px(rect.0);
        node.top = px(rect.1);
        node.width = px(rect.2);
        node.height = px(rect.3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// 仅安装页面输入需要的资源，不加载渲染器或创建窗口。
    fn input_app() -> App {
        let mut app = App::new();
        let mut images = Assets::<Image>::default();
        let image = images.add(Image::new_target_texture(
            1280,
            720,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
        app.add_plugins(MinimalPlugins)
            .insert_resource(images)
            .insert_resource(SceneTarget(image))
            .init_resource::<View>()
            .init_resource::<Fade>()
            .init_resource::<NextState<GameState>>()
            .init_resource::<crate::menu::ReturnFocus>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<MouseWheel>()
            .add_systems(Update, input);
        app
    }
    /// 一次按键经过实际页面输入系统，避免测试独立模拟逻辑。
    fn key(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
    }
    /// 视口变化只改变可滚动高度，卡片保持固定尺寸和列位置。
    #[test]
    fn fixed_grid_and_scroll_bounds() {
        assert_eq!(card_rect(0, 0.0, 0.0).width(), 270.0);
        assert_eq!(card_rect(1, 0.0, 0.0).min.x, 294.0);
        assert!(max_scroll(720.0) > 0.0);
        assert_eq!(max_scroll(3000.0), 0.0);
        assert!(max_scroll(400.0) > max_scroll(720.0));
    }

    /// 滚动按键、首尾边界、卡片无确认动作及过渡锁共用实际系统。
    #[test]
    fn keyboard_scroll_return_and_transition_lock() {
        let mut app = input_app();
        key(&mut app, KeyCode::ArrowDown);
        assert_eq!(app.world().resource::<View>().scroll, 64.0);
        key(&mut app, KeyCode::End);
        assert_eq!(app.world().resource::<View>().scroll, max_scroll(720.0));
        key(&mut app, KeyCode::Home);
        assert_eq!(app.world().resource::<View>().scroll, 0.0);
        key(&mut app, KeyCode::ArrowUp);
        assert_eq!(app.world().resource::<View>().scroll, 0.0);
        key(&mut app, KeyCode::Enter);
        assert_eq!(
            app.world().resource::<Fade>().phase,
            crate::transition::Phase::Idle
        );
        app.world_mut().resource_mut::<Fade>().phase = crate::transition::Phase::Out;
        key(&mut app, KeyCode::End);
        assert_eq!(app.world().resource::<View>().scroll, 0.0);
        key(&mut app, KeyCode::Escape);
        assert!(!app.world().resource::<crate::menu::ReturnFocus>().0);
        app.world_mut().insert_resource(Fade::default());
        key(&mut app, KeyCode::Tab);
        key(&mut app, KeyCode::Enter);
        assert!(app.world().resource::<crate::menu::ReturnFocus>().0);
        assert_eq!(
            app.world().resource::<Fade>().phase,
            crate::transition::Phase::Out
        );
    }

    /// 返回键拖出取消，只有同处按下松开才触发返回。
    #[test]
    fn back_requires_matching_mouse_press_and_release() {
        let mut app = input_app();
        app.world_mut().resource_mut::<View>().pointer = Some(Vec2::new(90.0, 40.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.world_mut().resource_mut::<View>().pointer = Some(Vec2::new(300.0, 200.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        assert_eq!(
            app.world().resource::<Fade>().phase,
            crate::transition::Phase::Idle
        );
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
        app.world_mut().resource_mut::<View>().pointer = Some(Vec2::new(90.0, 40.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        assert!(app.world().resource::<crate::menu::ReturnFocus>().0);
    }
}

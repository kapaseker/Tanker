//! 固定尺寸画廊排版与输入；所有命中坐标均为整体缩放后的逻辑像素。
use super::*;
use crate::{
    pages::index::{IndexBackground, IndexPalette},
    ui::{
        font::UiFont,
        resources::{
            colors,
            dimens::{gallery as dims, title_bar as bar},
            strings_cn::gallery as labels,
        },
        widgets::{self, TitleBarBackActivated, eased, text_font},
        zoom,
    },
};
use bevy::{
    camera::RenderTarget,
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    input::mouse::{MouseScrollUnit, MouseWheel},
    text::{LetterSpacing, LineHeight},
    ui_render::prelude::MaterialNode,
};

/// 滚动与焦点状态，独立于模型渲染；无窗口验证可注入同一逻辑指针。
/// 返回框交互（悬停/按下/焦点）已内建于 widgets::title_bar，不再挂在本页状态上。
#[derive(Resource, Default)]
pub(crate) struct View {
    pub scroll: f32,
    pub pointer: Option<Vec2>,
    /// 卡片键盘焦点；None 时焦点可落在共用返回框。
    pub focus: Option<usize>,
    /// 详情返回时保留画廊位置，首页进入时重置。
    pub restore: bool,
    pressed: Option<usize>,
    size: Vec2,
    hover: [f32; ENTRIES.len()],
    line: [f32; ENTRIES.len()],
}
/// 节点角色用于窗口变化时更新已有布局，不反复创建实体。
/// 标题栏（头部/返回框/标题）由 widgets::title_bar 控件负责，不再列入。
#[derive(Component, Clone, Copy)]
pub(super) enum Part {
    Background,
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
}

/// 卡片有两项仍占固定四列中的前两列，不按数据量拉伸。
pub(crate) fn card_rect(index: usize, left: f32, scroll: f32) -> Rect {
    let row = index / 2;
    let x = left + (index % 2) as f32 * (dims::CARD_WIDTH + dims::GAP);
    let y = bar::HEIGHT + dims::SECTION_GAP + row as f32 * section_height() + dims::SECTION_TO_CARD
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
/// 标题栏下方一直滚动到窗口底边；页脚留白属于内容，不缩短视口。
pub(super) fn max_scroll(height: f32) -> f32 {
    (section_height() * labels::CATEGORIES.len() as f32 + dims::CONTENT_BOTTOM
        - (height - bar::HEIGHT).max(0.0))
    .max(0.0)
}
/// 初次进入重置浏览位置；详情返回恢复原卡片与滚动。
pub(super) fn spawn(
    mut commands: Commands,
    target: Res<SceneTarget>,
    font: Res<UiFont>,
    previews: Res<Previews>,
    mut view: ResMut<View>,
    mut materials: ResMut<Assets<IndexBackground>>,
    mut title: ResMut<widgets::TitleBarState>,
) {
    if !view.restore {
        *view = View::default();
    }
    view.restore = false;
    view.pointer = None;
    view.pressed = None;
    title.external_navigation = true;
    title.set_focus(false);
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
    let background = materials.add(IndexBackground {
        viewport: Vec4::new(1280.0, 720.0, 0.0, 0.0),
        cubes: Vec4::ZERO,
        palette: IndexPalette::default(),
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
            // 标题栏、左上直角与右下直角由常驻 chrome 生成（见 app::chrome）；
            // 版本文字属于内容层（随遮罩变暗），仍随页面生成。
            widgets::spawn_version_text(root, &font);
        });
}
/// UI 节点使用固定逻辑像素，窗口适配只通过页面相机统一缩放。
fn absolute() -> Node {
    Node {
        position_type: PositionType::Absolute,
        ..default()
    }
}
/// 实际两列网格导航：上下保持列，四边不环绕，首次方向键落首卡。
fn navigate(focus: Option<usize>, key: KeyCode) -> usize {
    let Some(index) = focus else {
        return 0;
    };
    match key {
        KeyCode::ArrowLeft => index - index % 2,
        KeyCode::ArrowRight => (index / 2 * 2 + 1).min(ENTRIES.len() - 1),
        KeyCode::ArrowUp => {
            if index >= 2 {
                index - 2
            } else {
                index
            }
        }
        KeyCode::ArrowDown if index + 2 < ENTRIES.len() => index + 2,
        _ => index,
    }
}

/// 焦点优先完整落在标题栏下方；高度不足时对齐卡片顶部。
fn reveal_focus(view: &mut View, size: Vec2) {
    if let Some(index) = view.focus {
        let rect = card_rect(index, 0.0, view.scroll);
        if rect.max.y > size.y {
            view.scroll += rect.max.y - size.y;
        }
        let top = card_rect(index, 0.0, view.scroll).min.y;
        if top < bar::HEIGHT {
            view.scroll -= bar::HEIGHT - top;
        }
        view.scroll = view.scroll.clamp(0.0, max_scroll(size.y));
    }
}

/// 输入先经过过渡锁；返回框激活（点击/ESC/焦点 Enter）由标题栏控件发消息，本页只响应去向。
pub(super) fn input(
    windows: Query<&Window>,
    keys: Res<ButtonInput<KeyCode>>,
    mut wheel: MessageReader<MouseWheel>,
    mut activated: MessageReader<TitleBarBackActivated>,
    mut view: ResMut<View>,
    images: Res<Assets<Image>>,
    target: Res<SceneTarget>,
    mut fade: ResMut<Fade>,
    mut next: ResMut<NextState<GameState>>,
    mut return_focus: ResMut<crate::pages::index::ReturnFocus>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut title: ResMut<widgets::TitleBarState>,
    mut selected: ResMut<crate::pages::detail::SelectedModel>,
) {
    let size = crate::ui::logical_viewport(&windows, &images, &target.0);
    view.size = size;
    let focused = windows.single().map_or(true, |w| w.focused);
    if let Ok(window) = windows.single() {
        view.pointer = window.cursor_position().map(|p| p / zoom(window.width()));
    }
    let scale = windows.single().map_or_else(
        |_| zoom(images.get(&target.0).unwrap().size().x as f32),
        |window| zoom(window.width()),
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
    let back = activated.read().next().is_some();
    title.external_navigation = true;
    if !fade.accepts_input() || !focused {
        view.pressed = None;
        return;
    }
    if back || (title.focus && keys.just_pressed(KeyCode::Enter)) {
        if fade.begin(GameState::Index, true) {
            return_focus.0 = Some(1);
            next.set(GameState::GalleryToIndex);
        }
        return;
    }
    let max = max_scroll(size.y);
    let mut scroll = view.scroll + delta;
    for (key, amount) in [
        (
            KeyCode::PageUp,
            -(size.y - bar::HEIGHT - dims::SCROLL_STEP).max(dims::SCROLL_STEP),
        ),
        (
            KeyCode::PageDown,
            (size.y - bar::HEIGHT - dims::SCROLL_STEP).max(dims::SCROLL_STEP),
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
    let mut changed = false;
    if keys.just_pressed(KeyCode::Tab) {
        // 0 为返回框，其后为真实卡片；首次 Tab 进入返回框，Shift+Tab 进入末卡。
        let current = view.focus.map(|i| i + 1).or(title.focus.then_some(0));
        let reverse = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        let slot = match current {
            Some(slot) => (slot + if reverse { ENTRIES.len() } else { 1 }) % (ENTRIES.len() + 1),
            None => {
                if reverse {
                    ENTRIES.len()
                } else {
                    0
                }
            }
        };
        view.focus = slot.checked_sub(1);
        title.set_focus(slot == 0);
        changed = true;
    }
    for key in [
        KeyCode::ArrowLeft,
        KeyCode::ArrowRight,
        KeyCode::ArrowUp,
        KeyCode::ArrowDown,
    ] {
        if keys.just_pressed(key) {
            view.focus = Some(navigate(view.focus, key));
            title.set_focus(false);
            changed = true;
        }
    }
    if changed {
        reveal_focus(&mut view, size);
    }
    let hit = view
        .pointer
        .filter(|p| p.y >= bar::HEIGHT && p.y < size.y)
        .and_then(|p| {
            (0..ENTRIES.len())
                .find(|&i| card_rect(i, (size.x - dims::WIDTH) * 0.5, view.scroll).contains(p))
        });
    if mouse.just_pressed(MouseButton::Left) {
        view.pressed = hit;
        view.focus = None;
        title.set_focus(false);
    }
    // 一旦离开原卡即取消，不能拖出后移回再松开激活。
    if view.pressed != hit {
        view.pressed = None;
    }
    let clicked = mouse
        .just_released(MouseButton::Left)
        .then_some(view.pressed)
        .flatten();
    if mouse.just_released(MouseButton::Left) {
        view.pressed = None;
    }
    let confirmed = clicked.or_else(|| {
        keys.just_pressed(KeyCode::Enter)
            .then_some(view.focus)
            .flatten()
    });
    if let Some(index) = confirmed {
        if fade.begin(GameState::Detail, true) {
            selected.0 = index;
            view.focus = Some(index);
            view.restore = true;
            next.set(GameState::GalleryToDetail);
        }
    }
}

/// 连续反馈保留反向时的当前位置，边框/文字和底线分别采用220/300毫秒。
/// 标题栏与返回框的颜色过渡已内建于 widgets::title_bar::interaction，此处只管卡片。
pub(super) fn update(
    mut view: ResMut<View>,
    time: Res<Time>,
    previews: Res<Previews>,
    mut nodes: Query<(
        &Part,
        &mut Node,
        Option<&mut BorderColor>,
        Option<&mut TextColor>,
        Option<&mut Text>,
    )>,
    mut materials: ResMut<Assets<IndexBackground>>,
) {
    let size = view.size;
    let left = (size.x - dims::WIDTH) * 0.5;
    let scroll = view.scroll;
    let content_height = section_height() * labels::CATEGORIES.len() as f32 + dims::CONTENT_BOTTOM;
    for i in 0..ENTRIES.len() {
        let active = view.focus == Some(i)
            || view.pointer.is_some_and(|p| {
                p.y >= bar::HEIGHT && p.y < size.y && card_rect(i, left, scroll).contains(p)
            });
        let direction = if active { 1.0 } else { -1.0 };
        view.hover[i] = (view.hover[i] + direction * time.delta_secs() / 0.22).clamp(0.0, 1.0);
        view.line[i] = (view.line[i] + direction * time.delta_secs() / 0.30).clamp(0.0, 1.0);
    }
    for (_, material) in materials.iter_mut() {
        material.viewport = Vec4::new(size.x, size.y, time.elapsed_secs(), 0.0);
        material.cubes = Vec4::ZERO;
    }
    for (part, mut node, border, text_color, text) in &mut nodes {
        let rect = match *part {
            Part::Background => (0.0, 0.0, size.x, size.y),
            Part::Viewport => (
                left,
                bar::HEIGHT,
                dims::WIDTH,
                (size.y - bar::HEIGHT).max(0.0),
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
                    r.min.y - bar::HEIGHT,
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
    use crate::ui::widgets::title_bar::{TitleBarBack, TitleBarState};
    use bevy::render::render_resource::TextureFormat;
    /// 第五类只有一张卡片：不能移向空位，右列向下也不能跨列跳到它。
    #[test]
    fn single_obstacle_card_has_no_phantom_neighbor() {
        assert_eq!(navigate(Some(6), KeyCode::ArrowDown), 8);
        assert_eq!(navigate(Some(7), KeyCode::ArrowDown), 7);
        assert_eq!(navigate(Some(8), KeyCode::ArrowRight), 8);
        assert_eq!(navigate(Some(8), KeyCode::ArrowDown), 8);
        assert_eq!(navigate(Some(8), KeyCode::ArrowUp), 6);
    }
    /// 仅安装页面输入需要的资源与共享控件系统，不加载渲染器或创建窗口；
    /// 返回框交互在控件内，需提供标题栏 marker 实体与 TitleBarState。
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
            .init_resource::<crate::pages::detail::SelectedModel>()
            .init_resource::<Fade>()
            .init_resource::<NextState<GameState>>()
            .init_resource::<crate::pages::index::ReturnFocus>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<MouseWheel>()
            .add_systems(Update, input.before(widgets::title_bar::interaction));
        app.add_plugins(widgets::WidgetsPlugin);
        app.world_mut().spawn(TitleBarBack);
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
    /// 网格边界不跨列、不环绕；焦点滚入标题栏下方。
    #[test]
    fn navigation_edges_and_focus_reveal() {
        for i in 0..ENTRIES.len() {
            assert_eq!(navigate(Some(i), KeyCode::ArrowLeft), i / 2 * 2);
            assert_eq!(
                navigate(Some(i), KeyCode::ArrowRight),
                (i / 2 * 2 + 1).min(ENTRIES.len() - 1)
            );
            assert_eq!(
                navigate(Some(i), KeyCode::ArrowUp),
                if i < 2 { i } else { i - 2 }
            );
            assert_eq!(
                navigate(Some(i), KeyCode::ArrowDown),
                if i + 2 >= ENTRIES.len() { i } else { i + 2 }
            );
        }
        let mut view = View {
            focus: Some(ENTRIES.len() - 1),
            ..default()
        };
        reveal_focus(&mut view, Vec2::new(1280.0, 720.0));
        let rect = card_rect(ENTRIES.len() - 1, 0.0, view.scroll);
        assert!(rect.min.y >= bar::HEIGHT && rect.max.y <= 720.0);
        view.focus = Some(0);
        reveal_focus(&mut view, Vec2::new(1280.0, 720.0));
        assert_eq!(card_rect(0, 0.0, view.scroll).min.y, bar::HEIGHT);
    }

    /// 返回框和卡片只有一个键盘焦点，Tab 循环和 Shift+Tab 反向一致。
    #[test]
    fn tab_cycles_back_and_cards_without_double_activation() {
        let mut app = input_app();
        key(&mut app, KeyCode::Tab);
        assert!(app.world().resource::<TitleBarState>().focus);
        for i in 0..ENTRIES.len() {
            key(&mut app, KeyCode::Tab);
            assert_eq!(app.world().resource::<View>().focus, Some(i));
            assert!(!app.world().resource::<TitleBarState>().focus);
        }
        key(&mut app, KeyCode::Tab);
        assert_eq!(app.world().resource::<View>().focus, None);
        assert!(app.world().resource::<TitleBarState>().focus);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ShiftLeft);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Tab);
        app.update();
        assert_eq!(app.world().resource::<View>().focus, Some(ENTRIES.len() - 1));
        key(&mut app, KeyCode::Enter);
        assert_eq!(
            app.world()
                .resource::<crate::pages::detail::SelectedModel>()
                .0,
            ENTRIES.len() - 1
        );
        assert_eq!(app.world().resource::<Fade>().target, GameState::Detail);
        assert!(app.world().resource::<crate::pages::index::ReturnFocus>().0.is_none());
    }

    /// 卡片拖出后即使移回也取消；配对点击才进入详情并保存恢复状态。
    #[test]
    fn card_click_cancels_on_leave_and_restores_origin() {
        let mut app = input_app();
        app.world_mut().resource_mut::<View>().pointer = Some(Vec2::new(120.0, 250.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        app.world_mut().resource_mut::<View>().pointer = Some(Vec2::new(900.0, 250.0));
        app.update();
        app.world_mut().resource_mut::<View>().pointer = Some(Vec2::new(120.0, 250.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        assert_eq!(
            app.world().resource::<Fade>().phase,
            crate::app::transition::Phase::Idle
        );
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
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
        let view = app.world().resource::<View>();
        assert_eq!(view.focus, Some(0));
        assert!(view.restore);
        assert_eq!(app.world().resource::<Fade>().target, GameState::Detail);
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

    /// 首次方向键选择首卡而非滚动，确认后打开真实详情。
    #[test]
    fn arrows_select_cards_and_enter_opens_detail() {
        let mut app = input_app();
        key(&mut app, KeyCode::ArrowDown);
        assert_eq!(app.world().resource::<View>().scroll, 0.0);
        key(&mut app, KeyCode::Enter);
        assert_eq!(
            format!("{:?}", app.world().resource::<Fade>().target),
            "Detail"
        );
    }

    /// 翻页与首尾边界、无焦点确认及过渡锁共用实际系统。
    #[test]
    fn keyboard_scroll_return_and_transition_lock() {
        let mut app = input_app();
        key(&mut app, KeyCode::ArrowDown);
        assert_eq!(app.world().resource::<View>().scroll, 0.0);
        key(&mut app, KeyCode::Home);
        key(&mut app, KeyCode::PageDown);
        assert_eq!(app.world().resource::<View>().scroll, 568.0);
        key(&mut app, KeyCode::End);
        assert_eq!(app.world().resource::<View>().scroll, max_scroll(720.0));
        key(&mut app, KeyCode::Home);
        assert_eq!(app.world().resource::<View>().scroll, 0.0);
        app.world_mut().resource_mut::<View>().focus = None;
        key(&mut app, KeyCode::Enter);
        assert_eq!(
            app.world().resource::<Fade>().phase,
            crate::app::transition::Phase::Idle
        );
        app.world_mut().resource_mut::<Fade>().phase = crate::app::transition::Phase::Out;
        key(&mut app, KeyCode::End);
        assert_eq!(app.world().resource::<View>().scroll, 0.0);
        key(&mut app, KeyCode::Escape);
        assert!(app.world().resource::<crate::pages::index::ReturnFocus>().0.is_none());
        app.world_mut().insert_resource(Fade::default());
        key(&mut app, KeyCode::Tab);
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.world().resource::<crate::pages::index::ReturnFocus>().0, Some(1));
        assert_eq!(
            app.world().resource::<Fade>().phase,
            crate::app::transition::Phase::Out
        );
    }

    /// 返回键拖出取消，只有同处按下松开才触发返回；指针注入控件状态。
    #[test]
    fn back_requires_matching_mouse_press_and_release() {
        let mut app = input_app();
        app.world_mut().resource_mut::<TitleBarState>().pointer = Some(Vec2::new(90.0, 40.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.world_mut().resource_mut::<TitleBarState>().pointer = Some(Vec2::new(300.0, 200.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        assert_eq!(
            app.world().resource::<Fade>().phase,
            crate::app::transition::Phase::Idle
        );
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
        app.world_mut().resource_mut::<TitleBarState>().pointer = Some(Vec2::new(90.0, 40.0));
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
        // 页面输入先于控件，返回消息下一帧消费。
        app.update();
        assert_eq!(app.world().resource::<crate::pages::index::ReturnFocus>().0, Some(1));
    }
}

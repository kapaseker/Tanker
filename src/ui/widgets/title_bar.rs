//! 统一标题栏控件：全屏宽头部 = 统一高度 + 左侧 ESC 返回框 + 页面标题 + 贯穿整屏宽的底部分割线。
//!
//! 几何全部静态定位（percent 宽度 + 常量矩形），无需逐帧布局；交互（悬停、点击、
//! ESC / Tab 聚焦 + Enter、220ms 颜色过渡）内建于控件，激活时发 `TitleBarBackActivated`
//! 消息，跳转去向由页面决定（见 ADR-0004）。

use super::keycap::{Keycap, spawn_keycap};
use super::{eased, text_font};
use crate::ui::{
    font::UiFont,
    resources::{colors, dimens, strings},
    zoom,
};
use bevy::{
    ecs::relationship::RelatedSpawnerCommands,
    prelude::*,
};

use colors::title_bar as palette;
use dimens::title_bar as dims;

/// 标题栏根节点 marker（全屏宽头部，底边框即分割线）。
#[derive(Component)]
pub(crate) struct TitleBar;

/// ESC 返回框 marker；interaction 据此更新高亮，页面与测试据此判断标题栏存在。
#[derive(Component)]
pub(crate) struct TitleBarBack;

/// 返回箭头文字 marker；随返回框同一高亮进度变色。
#[derive(Component)]
pub(crate) struct TitleBarBackArrow;

/// 返回框被激活（点击、ESC 或焦点下 Enter）；控件只报告事件，页面决定去向与过渡。
#[derive(Message)]
pub(crate) struct TitleBarBackActivated;

/// 标题栏交互状态；全局单例基于"同屏最多一个标题栏"的当前事实（见 ADR-0004）。
#[derive(Resource, Default)]
pub(crate) struct TitleBarState {
    /// 整体缩放后的逻辑像素指针位置；None 表示不在窗口内。
    /// 窗口模式每帧被真实光标覆盖；离屏验证直接注入本字段。
    pub(crate) pointer: Option<Vec2>,
    /// 返回框高亮进度 0..=1；悬停/聚焦时推进，220ms 缓动后驱动颜色混合。
    progress: f32,
    /// 左键按下是否发生在返回框内；拖出取消。
    pressed: bool,
    /// Tab 聚焦返回框；任意左键按下清除。
    focus: bool,
}

/// 生成统一标题栏：全屏宽头部（高 `dims::HEIGHT`，底边框即分割线）+ ESC 返回框 + 页面标题。
pub(crate) fn spawn_title_bar(
    parent: &mut RelatedSpawnerCommands<ChildOf>,
    font: &UiFont,
    title: &str,
) {
    parent
        .spawn((
            TitleBar,
            Node {
                position_type: PositionType::Absolute,
                // 宽度跟随全屏 root 自适应，替代页面逐帧手写视口宽度。
                width: percent(100),
                height: px(dims::HEIGHT),
                border: UiRect {
                    bottom: px(dimens::BORDER_WIDTH),
                    ..default()
                },
                ..default()
            },
            BackgroundColor(palette::BACKGROUND),
            BorderColor::all(palette::OUTLINE),
        ))
        .with_children(|header| {
            let (back_left, back_top, back_width, back_height) = dims::BACK_RECT;
            header
                .spawn((
                    TitleBarBack,
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(back_left),
                        top: px(back_top),
                        width: px(back_width),
                        height: px(back_height),
                        // 内容用弹性布局自动排版（对齐设计稿 padding 18 / gap 12）。
                        display: Display::Flex,
                        align_items: AlignItems::Center,
                        column_gap: px(dims::BACK_GAP),
                        padding: UiRect::axes(px(dims::BACK_PADDING_X), px(0.0)),
                        ..default()
                    },
                    BackgroundColor(palette::BOX_SURFACE),
                    BorderColor::all(palette::OUTLINE),
                ))
                .with_children(|back| {
                    back.spawn((
                        TitleBarBackArrow,
                        Text::new(strings::BACK_ARROW),
                        text_font(font, dims::BACK_ARROW_SIZE, 400.0),
                        TextColor(colors::ON_SURFACE),
                    ));
                    spawn_keycap(back, font, strings::KEY_ESCAPE);
                });
            let (title_left, title_top, title_width, title_height) = dims::TITLE_RECT;
            header.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(title_left),
                    top: px(title_top),
                    width: px(title_width),
                    height: px(title_height),
                    ..default()
                },
                Text::new(title),
                text_font(font, dims::TITLE_SIZE, 700.0),
                TextColor(colors::ON_SURFACE),
            ));
        });
}

/// 返回框命中区域；标题栏贴屏幕左上角，BACK_RECT 即整体缩放后的屏幕逻辑坐标。
pub(crate) fn back_rect() -> Rect {
    let (left, top, width, height) = dims::BACK_RECT;
    Rect::from_corners(Vec2::new(left, top), Vec2::new(left + width, top + height))
}

/// 标题栏交互：指针命中、键鼠状态机、220ms 高亮与激活消息；无标题栏实体时空跑并复位状态。
/// 过渡锁（Fade）归页面管：锁期间消息仍会发出，由页面 fade.begin 拒绝，最终行为一致。
#[allow(clippy::too_many_arguments)]
pub(crate) fn interaction(
    mut state: ResMut<TitleBarState>,
    time: Res<Time>,
    windows: Query<&Window>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    boxes: Query<(), With<TitleBarBack>>,
    mut box_visuals: Query<(&mut BackgroundColor, &mut BorderColor), With<TitleBarBack>>,
    // 返回框内容（箭头/键帽）与框节点用 Without 互斥，避免同时可变 BorderColor 触发 B0001。
    mut contents: Query<
        (&mut TextColor, Option<&mut BorderColor>),
        (Or<(With<TitleBarBackArrow>, With<Keycap>)>, Without<TitleBarBack>),
    >,
    mut activated: MessageWriter<TitleBarBackActivated>,
) {
    if boxes.is_empty() {
        // 标题栏随页面销毁后清掉按下/聚焦残留，下次进入从初始态开始。
        state.pressed = false;
        state.focus = false;
        return;
    }
    // 窗口模式每帧覆盖注入指针；离屏（无窗口）保留验证注入值。
    if let Ok(window) = windows.single() {
        state.pointer = window.cursor_position().map(|p| p / zoom(window.width()));
    }
    let inside = state.pointer.is_some_and(|p| back_rect().contains(p));
    if mouse.just_pressed(MouseButton::Left) {
        state.pressed = inside;
        state.focus = false;
    }
    let clicked = mouse.just_released(MouseButton::Left) && state.pressed && inside;
    if mouse.just_released(MouseButton::Left) {
        state.pressed = false;
    }
    if keys.just_pressed(KeyCode::Tab) {
        state.focus = true;
    }
    if clicked
        || keys.just_pressed(KeyCode::Escape)
        || (state.focus && keys.just_pressed(KeyCode::Enter))
    {
        activated.write(TitleBarBackActivated);
    }
    // 高亮进度：指针悬停与键盘焦点共用，220ms 连续过渡，反向从当前进度继续。
    let active = state.focus || inside;
    let direction = if active { 1.0 } else { -1.0 };
    state.progress = (state.progress + direction * time.delta_secs() / 0.22).clamp(0.0, 1.0);
    let t = eased(state.progress, 0.25, 0.1, 0.25, 1.0);
    for (mut background, mut border) in &mut box_visuals {
        background.0 = palette::BOX_SURFACE.mix(&colors::PRIMARY, t);
        *border = BorderColor::all(palette::OUTLINE.mix(&colors::PRIMARY, t));
    }
    for (mut text_color, border) in &mut contents {
        text_color.0 = colors::ON_SURFACE.mix(&colors::ON_PRIMARY, t);
        if let Some(mut border) = border {
            *border = BorderColor::all(palette::OUTLINE.mix(&colors::KBD_BORDER_ACTIVE, t));
        }
    }
}

//! 统一标题栏控件：全屏宽头部 = 统一高度 + 左侧 ESC 返回框 + 页面标题 + 贯穿整屏宽的底部分割线。
//!
//! 几何全部静态定位（percent 宽度 + 常量矩形），无需逐帧布局；交互（悬停、点击、
//! ESC / Tab 聚焦 + Enter、220ms 颜色过渡）内建于控件，激活时发 `TitleBarBackActivated`
//! 消息，跳转去向由页面决定（见 ADR-0004）。

use super::keycap::spawn_keycap;
use super::{eased, text_font};
use crate::ui::{
    font::UiFont,
    resources::{colors, dimens, strings},
    zoom,
};
use bevy::{ecs::relationship::RelatedSpawnerCommands, prelude::*};
use bevy::window::PrimaryWindow;
use bevy::winit::WINIT_WINDOWS;

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

/// 电源符号图像 marker（首页变体）；与箭头同位同尺寸语义，随高亮进度染色。
#[derive(Component)]
pub(crate) struct TitleBarBackPower;

/// 页面标题文本节点 marker；常驻 chrome 在切页发起时就地改写其文字。
#[derive(Component)]
pub(crate) struct TitleBarTitle;

/// 标题栏是否处于可交互页面（菜单可见状态）。局内 chrome 相机不渲染标题栏，
/// interaction 据此空跑，避免 Esc 等按键在对局中触发返回框激活。
#[derive(Resource)]
pub(crate) struct TitleBarActive(pub(crate) bool);

/// 标题栏返回框键帽 marker；interaction 只重着色带此标记的键帽，
/// 页面上其它键帽（提示行、弹框）由各自页面驱动，互不串扰。
#[derive(Component)]
pub(crate) struct TitleBarKeycap;

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
    /// Tab 聚焦返回框；任意左键按下清除；页面据此断言弹框取消后的焦点归还。
    pub(crate) focus: bool,
    /// 页面有其它可聚焦控件时，由页面统一处理 Tab/Enter；Esc 和鼠标仍由标题栏处理。
    pub(crate) external_navigation: bool,
}

impl TitleBarState {
    /// 页面强制设置键盘焦点：弹框打开期间吞掉 Tab 记下的焦点，取消后归还。
    pub(crate) fn set_focus(&mut self, focused: bool) {
        self.focus = focused;
    }
}

/// 生成统一标题栏：全屏宽头部（高 `dims::HEIGHT`，底边框即分割线）+ ESC 返回框 + 页面标题。
/// 返回框同时生成箭头与电源图标两个节点，用 `display` 切换可见性（常驻 chrome 需要
/// 就地换变体，不能销毁重建）；`power_visible` 决定初始显示哪一个。
/// 电源图标用预渲染 PNG：Bevy UI 无 SVG、MiSans 缺 U+23FB 字形。
pub(crate) fn spawn_title_bar(
    parent: &mut RelatedSpawnerCommands<ChildOf>,
    font: &UiFont,
    title: &str,
    power: Handle<Image>,
    power_visible: bool,
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
                        // 1px 四边描边；BorderColor 只管颜色，宽度必须在此声明。
                        border: UiRect::all(px(dimens::BORDER_WIDTH)),
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
                    // 返回箭头（画廊等内页变体）：文本字形，默认隐藏。
                    back.spawn((
                        TitleBarBackArrow,
                        Text::new(strings::BACK_ARROW),
                        text_font(font, dims::BACK_ARROW_SIZE, 400.0),
                        TextColor(colors::ON_SURFACE),
                        Node {
                            display: if power_visible {
                                Display::None
                            } else {
                                Display::Flex
                            },
                            ..default()
                        },
                    ));
                    // 电源符号（首页变体）：与箭头同视觉位置，固定 16px 方形。
                    back.spawn((
                        TitleBarBackPower,
                        ImageNode {
                            image: power,
                            color: colors::ON_SURFACE,
                            ..default()
                        },
                        Node {
                            display: if power_visible {
                                Display::Flex
                            } else {
                                Display::None
                            },
                            width: px(dims::POWER_ICON_SIZE),
                            height: px(dims::POWER_ICON_SIZE),
                            ..default()
                        },
                    ));
                    spawn_keycap(back, font, strings::KEY_ESCAPE, TitleBarKeycap);
                });
            let (title_left, title_top, title_width, title_height) = dims::TITLE_RECT;
            header.spawn((
                TitleBarTitle,
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

/// 标题栏拖动命中判定：左键落在标题栏条内（y ≤ 栏高）且避开返回框（留给返回/退出交互）。
fn drag_hit(logical: Vec2) -> bool {
    logical.y <= dims::HEIGHT && !back_rect().contains(logical)
}

/// 无边框窗口的拖动：左键按在标题栏条内时调用 winit `drag_window`，系统接管进入
/// 模态拖动循环直到左键释放。局内标题栏未激活，顶部点击属于游戏画面，不劫持；
/// WinitWindows 是 bevy_winit 的线程局部存储（0.20 起替代 NonSend 资源），
/// 测试/离屏环境为空表，get_window 返回 None 自然空跑。
pub(crate) fn drag_window(
    windows: Query<(Entity, &Window), With<PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    active: Res<TitleBarActive>,
) {
    if !active.0 || !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Ok((entity, window)) = windows.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    if !drag_hit(cursor / zoom(window.width())) {
        return;
    }
    WINIT_WINDOWS.with_borrow(|winit_windows| {
        if let Some(winit_window) = winit_windows.get_window(entity) {
            // 拖动失败（如已处于其它模态循环）静默忽略，不影响游戏内交互。
            let _ = winit_window.drag_window();
        }
    });
}

/// 标题栏交互：指针命中、键鼠状态机、220ms 高亮与激活消息；无标题栏实体时空跑并复位状态。
/// 应用通过 TitleBarActive 门控过渡输入；关闭时清掉按下残留，不积压跨页返回消息。
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
        (
            Or<(With<TitleBarBackArrow>, With<TitleBarKeycap>)>,
            Without<TitleBarBack>,
        ),
    >,
    // 电源符号图像（首页变体）：无 TextColor，单独染色。
    mut power_icons: Query<&mut ImageNode, With<TitleBarBackPower>>,
    mut activated: MessageWriter<TitleBarBackActivated>,
    active: Res<TitleBarActive>,
) {
    // 标题栏实体常驻 chrome；局内相机不渲染它，交互也必须空跑，
    // 否则对局中的 Esc/点击会写出去向消息或留下高亮残留。
    if boxes.is_empty() || !active.0 {
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
    if !state.external_navigation && keys.just_pressed(KeyCode::Tab) {
        state.focus = true;
    }
    if clicked
        || keys.just_pressed(KeyCode::Escape)
        || (!state.external_navigation && state.focus && keys.just_pressed(KeyCode::Enter))
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
    // 电源符号与箭头共用同一高亮节奏：普通 on-surface，激活后 on-primary 深色。
    for mut image in &mut power_icons {
        image.color = colors::ON_SURFACE.mix(&colors::ON_PRIMARY, t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 拖动命中区 = 标题栏条减去返回框：条内可拖、条外与返回框内不可拖。
    #[test]
    fn drag_hit_is_strip_minus_back_rect() {
        let (left, top, width, height) = dims::BACK_RECT;
        // 标题栏条标题区（返回框右侧）：可拖
        assert!(drag_hit(Vec2::new(400.0, dims::HEIGHT * 0.5)));
        // 条下方一像素：不可拖（属页面内容）
        assert!(!drag_hit(Vec2::new(640.0, dims::HEIGHT + 1.0)));
        // 返回框中心：不可拖（返回/退出按钮）
        assert!(!drag_hit(Vec2::new(
            left + width * 0.5,
            top + height * 0.5,
        )));
    }
}

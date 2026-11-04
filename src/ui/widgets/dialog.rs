//! 统一确认对话框控件：全屏遮罩 + 居中面板 + 取消/确认按钮（含键帽提示）。
//!
//! 输入状态机与视觉动画内建于控件；打开方（页面或对局）只提供标题、按钮文案与
//! 确认/取消回调（`FnOnce(&mut World)`，触发时经 `Commands` 执行，见 ADR-0002）。
//! 弹框存在期间独占全部键鼠输入，并吞掉标题栏的 Tab 焦点与返回激活消息；
//! 取消后焦点是否归还由打开方的取消回调决定。
//! 视觉对照网页 `exit-dialog`：120ms ease-out 淡入、220ms 选中过渡、面板右下按钮行。

use super::keycap::spawn_keycap;
use super::{text_font, TitleBarBackActivated, TitleBarState};
use crate::app::transition::SceneTarget; // 只读取内容纹理尺寸；渲染管线资源，非页面/游戏状态
use crate::ui::{
    font::UiFont,
    resources::{colors, dimens},
    zoom,
};
use bevy::{prelude::*, text::LineHeight};
use colors::index as palette;
use dimens::index as dims;

/// 弹框选中按钮；打开时默认确认（打开即 Enter 可确认，与设计稿一致）。
#[derive(Component, Clone, Copy, PartialEq, Eq, Default, Debug)]
pub(crate) enum DialogSelection {
    #[default]
    Confirm,
    Cancel,
}

/// 弹框子节点的布局角色；视觉系统据此定位与着色，页面不直接引用。
#[derive(Component, Clone, Copy)]
pub(crate) enum DialogPart {
    Overlay,
    Panel,
    Title,
    Button(DialogSelection),
    Label(DialogSelection),
    Key(DialogSelection),
}

/// 统一确认对话框状态；挂在全屏遮罩根节点上，存在期间独占键鼠输入。
#[derive(Component)]
pub(crate) struct Dialog {
    /// true：随首页内容相机整体缩放（视口为 CSS 像素、指针除以 zoom）；
    /// false：对局 HUD 1:1 逻辑像素（视口 = 物理像素 / 窗口缩放系数）。
    pub(crate) css_zoom: bool,
    /// 当前选中按钮；←→/Tab/Y/N 切换，Enter 触发。
    pub(crate) selection: DialogSelection,
    /// 遮罩与面板的淡入进度秒数（120ms ease-out）；为 0 表示开场当帧，不消费输入。
    pub(crate) elapsed: f32,
    /// [取消, 确认] 两个按钮的选中过渡进度 0..=1（220ms 连续过渡）。
    pub(crate) select: [f32; 2],
    /// 左键按下的按钮；按下与松开都落在同一按钮内才触发（与网页原生点击一致）。
    pub(crate) pressed: Option<DialogSelection>,
    /// 指针在弹框视口坐标系中的位置；None 表示不在窗口内。
    /// 窗口模式每帧被真实光标覆盖；离屏验证直接注入本字段。
    pub(crate) pointer: Option<Vec2>,
    /// 弹框视口尺寸（CSS 或逻辑像素）；输入命中与视觉布局共用同一来源。
    pub(crate) viewport: Vec2,
    /// 确认回调；触发后弹框移除，去向由回调决定（页面过渡或退出进程）。
    on_confirm: Option<Box<dyn FnOnce(&mut World) + Send + Sync>>,
    /// 取消回调；Esc、取消按钮与面板外松开三路取消共用。
    on_cancel: Option<Box<dyn FnOnce(&mut World) + Send + Sync>>,
}

/// 弹框面板与 [取消, 确认] 按钮的固定矩形；输入命中与视觉布局共用同一来源。
/// 面板水平垂直居中；按钮行右对齐、贴面板右下（尺寸为 design 常量）。
pub(crate) fn dialog_rects(viewport: Vec2) -> (Rect, [Rect; 2]) {
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

/// 给定物理像素与缩放系数，返回整体缩放后的 CSS 像素视口（与首页 zoom 语义一致）。
fn css_viewport(pixels: Vec2, scale_factor: f32) -> Vec2 {
    pixels / (scale_factor * zoom(pixels.x / scale_factor))
}

/// 弹框视口尺寸：css_zoom 时为整体缩放后的 CSS 像素，否则为窗口逻辑像素。
/// 有窗口时取窗口物理尺寸；离屏（无窗口）取内容纹理尺寸。纹理缺失返回 None，
/// 调用方沿用上次值。
fn viewport_size(
    windows: &Query<&Window>,
    images: Option<&Assets<Image>>,
    target: Option<&SceneTarget>,
    css_zoom: bool,
) -> Option<Vec2> {
    let (pixels, scale) = if let Ok(window) = windows.single() {
        (
            window.resolution.physical_size().as_vec2(),
            window.resolution.scale_factor(),
        )
    } else {
        // 离屏（无窗口）：内容纹理尺寸即物理像素，缩放系数取 1。
        let (Some(target), Some(images)) = (target, images) else {
            return None;
        };
        let Some(image) = images.get(&target.0) else {
            return None;
        };
        (image.size().as_vec2(), 1.0)
    };
    Some(if css_zoom {
        css_viewport(pixels, scale)
    } else {
        pixels / scale
    })
}

/// 生成统一确认对话框。`parent` 提供时作为其子节点（随页面根一起销毁）；
/// None 时生成独立全屏根实体并返回其 Entity（由调用方挂相机与生命周期标记）。
/// 回调在触发当帧经 `Commands` 执行，弹框随后移除。
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_dialog(
    commands: &mut Commands,
    parent: Option<Entity>,
    font: &UiFont,
    title: &str,
    cancel_label: &str,
    confirm_label: &str,
    css_zoom: bool,
    on_confirm: impl FnOnce(&mut World) + Send + Sync + 'static,
    on_cancel: impl FnOnce(&mut World) + Send + Sync + 'static,
) -> Entity {
    let entity = commands
        .spawn((
            Dialog {
                css_zoom,
                selection: DialogSelection::default(),
                elapsed: 0.0,
                // 初始选中确认：确认按钮直接呈现薄荷态（打开即 Enter 可确认）。
                select: [0.0, 1.0],
                pressed: None,
                pointer: None,
                viewport: Vec2::ZERO,
                on_confirm: Some(Box::new(on_confirm)),
                on_cancel: Some(Box::new(on_cancel)),
            },
            DialogPart::Overlay,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .id();
    if let Some(parent) = parent {
        commands.entity(parent).add_child(entity);
    }
    // 坐标由 dialog_visual 按 dialog_rects 逐帧更新，这里只建结构与静态样式。
    commands.entity(entity).with_children(|overlay| {
        overlay
            .spawn((
                DialogPart::Panel,
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
                    DialogPart::Title,
                    Text::new(title),
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
                    (DialogSelection::Cancel, cancel_label, crate::ui::resources::strings::KEY_N),
                    (DialogSelection::Confirm, confirm_label, crate::ui::resources::strings::KEY_Y),
                ] {
                    panel
                        .spawn((
                            DialogPart::Button(role),
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
                                DialogPart::Label(role),
                                Text::new(label),
                                text_font(font, dims::EXIT_BUTTON_SIZE, 700.0),
                                TextColor(Color::NONE),
                            ));
                            // 弹框键帽复用统一控件；标记让本控件按选中态驱动
                            // 文字与边框色（标题栏 interaction 只重着色自己的键帽）。
                            spawn_keycap(button, font, key, DialogPart::Key(role));
                        });
                }
            });
    });
    entity
}

/// 触发回调并移除弹框；回调经 `Commands` 在世界访问点执行。
fn fire(
    commands: &mut Commands,
    entity: Entity,
    dialog: &mut Dialog,
    confirm: bool,
) {
    let callback = if confirm {
        dialog.on_confirm.take()
    } else {
        dialog.on_cancel.take()
    };
    if let Some(callback) = callback {
        commands.queue(callback);
    }
    commands.entity(entity).despawn();
}

/// 弹框独占输入：←→/Tab/Y/N 切换选中、Enter 触发、Esc 取消；鼠标按下与松开
/// 落在同一按钮内才触发，松开在面板外取消（含拖出）。无弹框实体时空跑。
#[allow(clippy::too_many_arguments)]
pub(crate) fn dialog_input(
    mut commands: Commands,
    mut title_bar: ResMut<TitleBarState>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    images: Option<Res<Assets<Image>>>,
    target: Option<Res<SceneTarget>>,
    // 吞掉标题栏 interaction 为本帧 Esc/点击写下的激活消息：否则关闭弹框后
    // 残留消息会在下一帧重新打开弹框。
    mut activated: MessageReader<TitleBarBackActivated>,
    mut dialogs: Query<(Entity, &mut Dialog)>,
) {
    let Ok((entity, mut dialog)) = dialogs.single_mut() else {
        return;
    };
    // 独占输入期间吞掉标题栏记下的 Tab 焦点（取消后是否归还由取消回调决定）。
    title_bar.set_focus(false);
    activated.read().for_each(drop);
    // 视口与指针：窗口模式每帧覆盖；离屏保留注入值（验证直接写组件字段）。
    if let Some(viewport) =
        viewport_size(&windows, images.as_deref(), target.as_deref(), dialog.css_zoom)
    {
        dialog.viewport = viewport;
    }
    if let Ok(window) = windows.single() {
        let z = zoom(window.width());
        dialog.pointer =
            window.cursor_position().map(|p| if dialog.css_zoom { p / z } else { p });
    }
    // 开场当帧不消费输入：页面同帧打开弹框时本系统可能在其后运行，
    // 不能把打开弹框的 Esc/Enter 立刻当成弹框内的取消/确认。
    if dialog.elapsed == 0.0 {
        return;
    }
    // ←→/Tab/Y/N 仅切换选中；Enter 触发选中按钮；Esc 取消；其余按键一律吞掉。
    if keys.just_pressed(KeyCode::KeyY) {
        dialog.selection = DialogSelection::Confirm;
    } else if keys.just_pressed(KeyCode::KeyN) {
        dialog.selection = DialogSelection::Cancel;
    } else if keys.any_just_pressed([KeyCode::ArrowLeft, KeyCode::ArrowRight, KeyCode::Tab]) {
        dialog.selection = match dialog.selection {
            DialogSelection::Confirm => DialogSelection::Cancel,
            DialogSelection::Cancel => DialogSelection::Confirm,
        };
    } else if keys.just_pressed(KeyCode::Escape) {
        fire(&mut commands, entity, &mut dialog, false);
        return;
    } else if keys.just_pressed(KeyCode::Enter) {
        let confirm = dialog.selection == DialogSelection::Confirm;
        fire(&mut commands, entity, &mut dialog, confirm);
        return;
    }
    // 鼠标：按下与松开都落在同一按钮内才触发；松开在面板外取消（含拖出）。
    let point = dialog.pointer.filter(|_| mouse.just_released(MouseButton::Left));
    if let Some(point) = point {
        let (panel, buttons) = dialog_rects(dialog.viewport);
        let hit = buttons
            .iter()
            .position(|rect| rect.contains(point))
            .map(|i| if i == 0 { DialogSelection::Cancel } else { DialogSelection::Confirm });
        let pressed = dialog.pressed.take();
        if pressed.is_some() && pressed == hit {
            fire(&mut commands, entity, &mut dialog, hit == Some(DialogSelection::Confirm));
        } else if !panel.contains(point) {
            fire(&mut commands, entity, &mut dialog, false);
        }
        return;
    }
    // 弹框期间按下按钮记录目标；命中区与视觉布局共用同一矩形来源。
    if mouse.just_pressed(MouseButton::Left) {
        if let Some(point) = dialog.pointer {
            let (_, buttons) = dialog_rects(dialog.viewport);
            dialog.pressed = buttons
                .iter()
                .position(|rect| rect.contains(point))
                .map(|i| if i == 0 { DialogSelection::Cancel } else { DialogSelection::Confirm });
        }
    }
}

/// 弹框视觉：视口回写、120ms 淡入与 220ms 选中过渡，按 DialogPart 布局与着色。
#[allow(clippy::too_many_arguments)]
pub(crate) fn dialog_visual(
    windows: Query<&Window>,
    images: Option<Res<Assets<Image>>>,
    target: Option<Res<SceneTarget>>,
    time: Res<Time>,
    mut dialogs: Query<&mut Dialog>,
    mut nodes: Query<(
        &DialogPart,
        &mut Node,
        Option<&mut TextColor>,
        Option<&mut BorderColor>,
        Option<&mut BackgroundColor>,
    )>,
) {
    let Ok(mut dialog) = dialogs.single_mut() else {
        return;
    };
    // 输入命中与布局共用同一视口来源；纹理缺失时沿用上次值。
    if let Some(viewport) =
        viewport_size(&windows, images.as_deref(), target.as_deref(), dialog.css_zoom)
    {
        dialog.viewport = viewport;
    }
    dialog.elapsed = (dialog.elapsed + time.delta_secs()).min(dims::EXIT_FADE_IN);
    // 120ms ease-out：用 easeOutCubic 近似 CSS cubic-bezier(0,0,0.58,1)。
    let t = dialog.elapsed / dims::EXIT_FADE_IN;
    let dialog_fade = 1.0 - (1.0 - t).powi(3);
    // 选中态 220ms 连续过渡，移入移出从当前进度继续。
    let step = time.delta_secs() / 0.22;
    for i in 0..2 {
        let target_value = f32::from(dialog.selection == [DialogSelection::Cancel, DialogSelection::Confirm][i]);
        dialog.select[i] += (target_value - dialog.select[i]).clamp(-step, step);
    }
    let dialog_select = dialog.select;
    let viewport = dialog.viewport;
    for (part, mut node, color, border, bg) in &mut nodes {
        let mut rect = None;
        match *part {
            DialogPart::Overlay => {
                rect = Some((0.0, 0.0, viewport.x, viewport.y));
                if let Some(mut bg) = bg {
                    bg.0 = palette::EXIT_OVERLAY
                        .with_alpha(palette::EXIT_OVERLAY.alpha() * dialog_fade);
                }
            }
            DialogPart::Panel => {
                let (panel, _) = dialog_rects(viewport);
                rect = Some((panel.min.x, panel.min.y, panel.width(), panel.height()));
                if let Some(mut bg) = bg {
                    bg.0 = palette::EXIT_PANEL.with_alpha(dialog_fade);
                }
                if let Some(mut border) = border {
                    *border = BorderColor::all(palette::EXIT_OUTLINE.with_alpha(dialog_fade));
                }
            }
            DialogPart::Title => {
                let (panel, _) = dialog_rects(viewport);
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
            DialogPart::Button(role) => {
                let (panel, buttons) = dialog_rects(viewport);
                let i = usize::from(role == DialogSelection::Confirm);
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
            DialogPart::Label(role) | DialogPart::Key(role) => {
                let i = usize::from(role == DialogSelection::Confirm);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::resources::strings_cn::index as labels;
    use bevy::ecs::system::RunSystemOnce;

    /// 标志资源：回调把触发结果写进来，供测试断言控件与回调的接线。
    #[derive(Resource, Default)]
    struct Flags {
        confirmed: bool,
        cancelled: bool,
    }

    /// 弹框输入测试的公共场景：无页面依赖的独立弹框（elapsed 已过开场帧）。
    /// 指针与视口由测试按需注入组件字段（无窗口，系统不会覆盖）。
    fn dialog_world() -> World {
        let mut world = World::new();
        world.init_resource::<Time>();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<ButtonInput<MouseButton>>();
        world.init_resource::<TitleBarState>();
        world.init_resource::<Messages<TitleBarBackActivated>>();
        world.init_resource::<Flags>();
        world.insert_resource(UiFont(
            Handle::<Font>::default(),
            crate::ui::font::TextOptics::default(),
        ));
        world
            .run_system_once(|mut commands: Commands, font: Res<UiFont>| {
                spawn_dialog(
                    &mut commands,
                    None,
                    &font,
                    labels::EXIT_TITLE,
                    labels::EXIT_CANCEL,
                    labels::EXIT_CONFIRM,
                    true,
                    |world: &mut World| world.resource_mut::<Flags>().confirmed = true,
                    |world: &mut World| world.resource_mut::<Flags>().cancelled = true,
                );
            })
            .unwrap();
        world.flush();
        // 跳过开场帧守卫，模拟弹框已淡入一帧后的稳定态（视口与指针由测试注入）。
        let entity = world
            .query_filtered::<Entity, With<Dialog>>()
            .single(&world)
            .unwrap();
        let mut dialog = world.get_mut::<Dialog>(entity).unwrap();
        dialog.elapsed = 0.1;
        dialog.viewport = Vec2::new(1280.0, 720.0);
        world
    }

    /// 以“仅该键刚按下”的输入运行一次弹框输入系统。
    fn press(world: &mut World, key: KeyCode) {
        let mut keys = ButtonInput::default();
        keys.press(key);
        world.insert_resource(keys);
        world.insert_resource(ButtonInput::<MouseButton>::default());
        world.run_system_once(dialog_input).unwrap();
        world.flush();
    }

    /// 当前选中值；None 表示弹框不存在。
    fn selection(world: &mut World) -> Option<DialogSelection> {
        world.query::<&Dialog>().iter(world).next().map(|d| d.selection)
    }

    /// 弹框实体；None 表示弹框不存在。
    fn dialog_entity(world: &mut World) -> Option<Entity> {
        world
            .query_filtered::<Entity, With<Dialog>>()
            .iter(world)
            .next()
    }

    /// 在指定视口坐标完成一次按下与松开（两次系统运行）。
    /// release 必须落在同一个 ButtonInput 实例内，just_released 才会生效。
    fn click(world: &mut World, point: Vec2) {
        let set_pointer = |world: &mut World, point: Vec2| {
            let Some(entity) = dialog_entity(world) else {
                return;
            };
            world.get_mut::<Dialog>(entity).unwrap().pointer = Some(point);
        };
        set_pointer(world, point);
        let mut mouse = ButtonInput::default();
        mouse.press(MouseButton::Left);
        world.insert_resource(mouse);
        world.insert_resource(ButtonInput::<KeyCode>::default());
        world.run_system_once(dialog_input).unwrap();
        set_pointer(world, point);
        let mut mouse = ButtonInput::default();
        mouse.press(MouseButton::Left);
        mouse.release(MouseButton::Left);
        world.insert_resource(mouse);
        world.run_system_once(dialog_input).unwrap();
        world.flush();
    }

    /// ←→/Tab/Y/N 只切换选中；Enter 触发选中按钮的回调。
    #[test]
    fn keys_switch_selection_and_enter_fires_callback() {
        let mut world = dialog_world();
        press(&mut world, KeyCode::ArrowLeft);
        assert_eq!(selection(&mut world), Some(DialogSelection::Cancel));
        press(&mut world, KeyCode::KeyN);
        assert_eq!(selection(&mut world), Some(DialogSelection::Cancel));
        press(&mut world, KeyCode::KeyY);
        assert_eq!(selection(&mut world), Some(DialogSelection::Confirm));
        press(&mut world, KeyCode::Tab);
        assert_eq!(selection(&mut world), Some(DialogSelection::Cancel));
        press(&mut world, KeyCode::ArrowRight);
        assert_eq!(selection(&mut world), Some(DialogSelection::Confirm));
        // Enter 触发确认回调并移除弹框。
        press(&mut world, KeyCode::Enter);
        assert_eq!(selection(&mut world), None);
        assert!(world.resource::<Flags>().confirmed);
        assert!(!world.resource::<Flags>().cancelled);
    }

    /// Esc 取消回调执行且弹框移除；打开期间吞掉标题栏 Tab 焦点。
    #[test]
    fn escape_fires_cancel_callback() {
        let mut world = dialog_world();
        world.resource_mut::<TitleBarState>().set_focus(true);
        press(&mut world, KeyCode::Escape);
        assert_eq!(selection(&mut world), None);
        assert!(world.resource::<Flags>().cancelled);
        assert!(!world.resource::<Flags>().confirmed);
        assert!(!world.resource::<TitleBarState>().focus);
    }

    /// 开场当帧不消费输入：打开弹框的 Esc 不会被弹框立刻当成取消。
    #[test]
    fn first_frame_ignores_input() {
        let mut world = dialog_world();
        let entity = dialog_entity(&mut world).unwrap();
        world.get_mut::<Dialog>(entity).unwrap().elapsed = 0.0;
        press(&mut world, KeyCode::Escape);
        assert_eq!(selection(&mut world), Some(DialogSelection::Confirm));
        assert!(!world.resource::<Flags>().cancelled);
    }

    /// 鼠标点击按钮触发回调；面板外松开取消；按下按钮拖出面板也取消。
    #[test]
    fn mouse_triggers_buttons_and_cancels_outside() {
        let viewport = Vec2::new(1280.0, 720.0);
        let (panel, [cancel, confirm]) = dialog_rects(viewport);
        // 点击确认 → 确认回调。
        let mut world = dialog_world();
        click(&mut world, confirm.center());
        assert!(world.resource::<Flags>().confirmed);
        // 点击取消 → 取消回调。
        let mut world = dialog_world();
        click(&mut world, cancel.center());
        assert!(world.resource::<Flags>().cancelled);
        // 面板外松开 → 取消。
        let outside = Vec2::new(panel.center().x, 100.0);
        assert!(!panel.contains(outside));
        let mut world = dialog_world();
        click(&mut world, outside);
        assert!(world.resource::<Flags>().cancelled);
        // 按下确认后拖出面板松开 → 取消而非确认。
        let mut world = dialog_world();
        let set_pointer = |world: &mut World, point: Vec2| {
            let entity = dialog_entity(world).unwrap();
            world.get_mut::<Dialog>(entity).unwrap().pointer = Some(point);
        };
        set_pointer(&mut world, confirm.center());
        let mut mouse = ButtonInput::default();
        mouse.press(MouseButton::Left);
        world.insert_resource(mouse);
        world.insert_resource(ButtonInput::<KeyCode>::default());
        world.run_system_once(dialog_input).unwrap();
        set_pointer(&mut world, outside);
        let mut mouse = ButtonInput::default();
        mouse.press(MouseButton::Left);
        mouse.release(MouseButton::Left);
        world.insert_resource(mouse);
        world.run_system_once(dialog_input).unwrap();
        world.flush();
        assert!(world.resource::<Flags>().cancelled);
        assert!(!world.resource::<Flags>().confirmed);
    }

    /// 弹框矩形：面板居中、按钮行贴右下、命中区与视觉常量一致。
    #[test]
    fn rects_center_panel_and_align_buttons_bottom_right() {
        let viewport = Vec2::new(1280.0, 720.0);
        let (panel, [cancel, confirm]) = dialog_rects(viewport);
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

    /// css_zoom=false 视口：无窗口时按内容纹理 1:1 折算（对局离屏路径）。
    #[test]
    fn viewport_follows_texture_without_window() {
        let mut world = World::new();
        world.init_resource::<Time>();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<ButtonInput<MouseButton>>();
        world.init_resource::<TitleBarState>();
        world.init_resource::<Messages<TitleBarBackActivated>>();
        world.init_resource::<Flags>();
        world.insert_resource(UiFont(
            Handle::<Font>::default(),
            crate::ui::font::TextOptics::default(),
        ));
        world.init_resource::<Assets<Image>>();
        let image = Image::new_target_texture(
            1280,
            720,
            bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
            None,
        );
        let handle = world.resource_mut::<Assets<Image>>().add(image);
        world.insert_resource(SceneTarget(handle));
        world
            .run_system_once(|mut commands: Commands, font: Res<UiFont>| {
                spawn_dialog(
                    &mut commands, None, &font, "标题", "否", "是", false,
                    |_: &mut World| {}, |_: &mut World| {},
                );
            })
            .unwrap();
        world.flush();
        world.run_system_once(dialog_visual).unwrap();
        let entity = dialog_entity(&mut world).unwrap();
        assert_eq!(
            world.get::<Dialog>(entity).unwrap().viewport,
            Vec2::new(1280.0, 720.0)
        );
    }
}

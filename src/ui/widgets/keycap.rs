//! 行内键帽控件：固定字号、内边距、圆角与边框的小方块；首页提示行与画廊返回框共用。

use crate::ui::{
    font::UiFont,
    resources::{colors, dimens},
};
use bevy::{
    ecs::relationship::RelatedSpawnerCommands,
    prelude::*,
    text::{LetterSpacing, LineHeight},
};

/// 键帽 marker；页面据此在悬停等状态下更新文字与边框颜色。
#[derive(Component)]
pub(crate) struct Keycap;

/// 生成一个键帽：样式一次写死（10px、2px 字距、4:3 行高、1px 边框、4px 圆角、6×4 内边距）。
pub(crate) fn spawn_keycap(
    parent: &mut RelatedSpawnerCommands<ChildOf>,
    font: &UiFont,
    key: &str,
) {
    parent.spawn((
        Keycap,
        Text::new(key),
        TextFont {
            font: font.0.clone().into(),
            font_size: FontSize::Px(dimens::index::KBD_SIZE),
            ..default()
        },
        LetterSpacing::Px(dimens::index::KBD_SPACING),
        LineHeight::RelativeToFont(dimens::LINE_HEIGHT),
        TextColor(colors::ON_SURFACE),
        Node {
            border: UiRect::all(px(dimens::BORDER_WIDTH)),
            border_radius: BorderRadius::all(px(dimens::index::KBD_RADIUS)),
            padding: UiRect::axes(px(dimens::index::KBD_PADDING_X), px(dimens::index::KBD_PADDING_Y)),
            ..default()
        },
        BorderColor::all(colors::KBD_BORDER),
        BackgroundColor(colors::KBD_BACKGROUND),
    ));
}

//! 行内键帽控件：固定字号、内边距、圆角与边框的小方块；首页提示行、标题栏返回框与退出弹框共用。

use crate::ui::{
    font::UiFont,
    resources::{colors, dimens},
};
use bevy::{
    ecs::relationship::RelatedSpawnerCommands,
    prelude::*,
    text::LetterSpacing,
};

/// 生成一个键帽：样式一次写死（10px、2px 字距、1px 边框、4px 圆角、6×4 内边距）。
/// 不设行高覆盖，沿用 Bevy 默认行高（1.2×字号）；大写字母在该行框内天然偏上，
/// 补偿量由字体光学度量按 parley 量化模型算出（见 [`TextOptics::optical_shift`]），
/// 上内边距加、下内边距减同一数值，上下之和不变，键帽总高度不受影响。
/// `extra` 为调用方附加的标记组件（如标题栏重着色标记、弹框角色），
/// 决定该键帽由哪个系统驱动颜色；不附加则保持默认静态样式。
///
/// [`TextOptics::optical_shift`]: crate::ui::font::TextOptics::optical_shift
pub(crate) fn spawn_keycap(
    parent: &mut RelatedSpawnerCommands<ChildOf>,
    font: &UiFont,
    key: &str,
    extra: impl Bundle,
) {
    // 光学补偿量（CSS 像素）：随字号与字体度量自动标定，无硬编码。
    let shift = font.1.optical_shift(dimens::index::KBD_SIZE);
    // parley 的字距加在每个字符之后（含最后一个），文字盒比末字符墨迹右侧
    // 多出 1 个字距；居中排布时墨迹整体偏左 spacing/2。左右内边距反向补偿
    // 同一数值，键帽总宽度不变。
    let shift_x = dimens::index::KBD_SPACING / 2.0;
    parent.spawn((
        Text::new(key),
        TextFont {
            font: font.0.clone().into(),
            font_size: FontSize::Px(dimens::index::KBD_SIZE),
            ..default()
        },
        LetterSpacing::Px(dimens::index::KBD_SPACING),
        TextColor(colors::ON_SURFACE),
        Node {
            border: UiRect::all(px(dimens::BORDER_WIDTH)),
            border_radius: BorderRadius::all(px(dimens::index::KBD_RADIUS)),
            padding: UiRect {
                left: px(dimens::index::KBD_PADDING_X + shift_x),
                right: px(dimens::index::KBD_PADDING_X - shift_x),
                top: px(dimens::index::KBD_PADDING_Y + shift),
                bottom: px(dimens::index::KBD_PADDING_Y - shift),
            },
            ..default()
        },
        BorderColor::all(colors::KBD_BORDER),
        BackgroundColor(colors::KBD_BACKGROUND),
        extra,
    ));
}

//! 直角角标与版本号徽标：首页与画廊共用的装饰控件。
//!
//! 样式（尺寸、字距、边框、颜色）在 spawn 辅助里一次写好；`layout_frame` 只负责
//! 按 marker 设置视口角锚点。控件实体是全屏 root 的子节点，随页面 OnExit 一并销毁。

use crate::ui::{
    font::UiFont,
    resources::{colors, dimens, strings_cn},
};
use crate::ui::resources::dimens::index as dims;
use bevy::{
    ecs::relationship::RelatedSpawnerCommands,
    prelude::*,
    text::{LetterSpacing, LineHeight},
};

/// 直角边的朝向：左上（标题侧）或右下（版本号侧）。
#[derive(Clone, Copy)]
pub(crate) enum CornerOrientation {
    TopLeft,
    BottomRight,
}

/// 直角角标 marker；layout_frame 据此设置视口角锚点。
#[derive(Component)]
pub(crate) struct FrameCorner {
    pub orientation: CornerOrientation,
}

/// 版本号徽标 marker（标记版本文字节点）；layout_frame 据此设置右下锚点。
#[derive(Component)]
pub(crate) struct VersionBadge;

/// 生成一个直角角标：尺寸、边框边、颜色在 spawn 写死；定位交给 layout_frame。
pub(crate) fn spawn_frame_corner(
    parent: &mut RelatedSpawnerCommands<ChildOf>,
    orientation: CornerOrientation,
    color: Color,
) {
    // 边框只点亮朝向对应的两条边，形成 L 形直角。
    let border = match orientation {
        CornerOrientation::TopLeft => UiRect {
            top: px(dimens::BORDER_WIDTH),
            left: px(dimens::BORDER_WIDTH),
            ..default()
        },
        CornerOrientation::BottomRight => UiRect {
            bottom: px(dimens::BORDER_WIDTH),
            right: px(dimens::BORDER_WIDTH),
            ..default()
        },
    };
    parent.spawn((
        FrameCorner { orientation },
        Node {
            position_type: PositionType::Absolute,
            width: px(dims::CORNER_SIZE),
            height: px(dims::CORNER_SIZE),
            border,
            ..default()
        },
        BorderColor::all(color),
    ));
}

/// 生成版本号徽标 = 右下直角边 + 版本文字（统一首页规格：10px/2px 字距/行高/次要色）。
pub(crate) fn spawn_version_badge(
    parent: &mut RelatedSpawnerCommands<ChildOf>,
    font: &UiFont,
    corner_color: Color,
) {
    spawn_frame_corner(parent, CornerOrientation::BottomRight, corner_color);
    parent.spawn((
        VersionBadge,
        Text::new(strings_cn::index::BUILD),
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
}

/// 统一布局：只设静态锚点，父节点为全屏 root；无 marker 实体时空跑。
/// 两个查询用 `Without` 互斥，避免同时写 `&mut Node` 触发 B0001 冲突。
pub(super) fn layout_frame(
    mut corners: Query<(&FrameCorner, &mut Node), Without<VersionBadge>>,
    mut badges: Query<&mut Node, (With<VersionBadge>, Without<FrameCorner>)>,
) {
    for (corner, mut node) in &mut corners {
        match corner.orientation {
            CornerOrientation::TopLeft => {
                node.top = px(dims::CORNER_INSET);
                node.left = px(dims::CORNER_INSET);
                node.right = Val::Auto;
                node.bottom = Val::Auto;
            }
            CornerOrientation::BottomRight => {
                node.bottom = px(dims::CORNER_INSET);
                node.right = px(dims::CORNER_INSET);
                node.top = Val::Auto;
                node.left = Val::Auto;
            }
        }
    }
    for mut node in &mut badges {
        node.right = px(dims::BUILD_RIGHT);
        node.bottom = px(dims::BUILD_BOTTOM);
        node.top = Val::Auto;
        node.left = Val::Auto;
    }
}

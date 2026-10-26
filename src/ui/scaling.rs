//! UI 整体缩放规则：窄窗口等比缩小，宽窗口保持设计尺寸。

use crate::ui::resources::dimens;
use bevy::prelude::*;

/// scale.js 的 zoom 语义：可用宽度不足参考宽度时整体等比缩小，只缩不放大，高度不参与。
pub(crate) fn zoom(available_width: f32) -> f32 {
    (available_width / dimens::REFERENCE_WIDTH).min(1.0)
}

/// 菜单内容的逻辑视口：窗口模式使用逻辑分辨率，离屏模式使用目标纹理尺寸。
/// 只接收图像句柄，不依赖应用或页面状态。
pub(crate) fn logical_viewport(
    windows: &Query<&Window>,
    images: &Assets<Image>,
    target: &Handle<Image>,
) -> Vec2 {
    if let Ok(window) = windows.single() {
        Vec2::new(window.width(), window.height()) / zoom(window.width())
    } else {
        let size = images.get(target).unwrap().size().as_vec2();
        size / zoom(size.x)
    }
}

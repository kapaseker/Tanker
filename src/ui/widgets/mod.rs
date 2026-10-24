//! 跨页面复用的 UI 控件；只依赖 bevy 与 ui::resources，不引用页面/游戏状态（见 ADR-0002）。
pub(crate) mod frame;
pub(crate) mod keycap;
pub(crate) mod title_bar;

pub(crate) use frame::{CornerOrientation, spawn_frame_corner, spawn_version_badge};
pub(crate) use keycap::spawn_keycap;
pub(crate) use title_bar::{TitleBarBackActivated, TitleBarState, spawn_title_bar};

use crate::ui::font::UiFont;
use bevy::prelude::*;
use frame::layout_frame;
use title_bar::interaction;

/// 注册共享控件的布局与交互系统。系统状态无关：无 marker 实体时空跑。
pub(crate) struct WidgetsPlugin;
impl Plugin for WidgetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TitleBarState>()
            .add_message::<TitleBarBackActivated>()
            .add_systems(Update, (layout_frame, interaction));
    }
}

/// 三次贝塞尔以二分求时间参数，与网页的缓动曲线一致；控件与页面动效共用。
pub(crate) fn eased(t: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
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

/// 页面与控件的文字样式统一来自现有 MiSans 资源。
pub(crate) fn text_font(font: &UiFont, size: f32, weight: f32) -> TextFont {
    TextFont {
        font: font.0.clone().into(),
        font_size: size.into(),
        weight: FontWeight(weight as u16),
        ..default()
    }
}

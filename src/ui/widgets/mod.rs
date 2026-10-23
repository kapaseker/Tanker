//! 跨页面复用的 UI 控件；只依赖 bevy 与 ui::resources，不引用页面/游戏状态（见 ADR-0002）。
pub(crate) mod frame;

pub(crate) use frame::{CornerOrientation, spawn_frame_corner, spawn_version_badge};

use bevy::prelude::*;
use frame::layout_frame;

/// 注册共享控件的布局系统。系统状态无关：无 marker 实体时空跑。
pub(crate) struct WidgetsPlugin;
impl Plugin for WidgetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, layout_frame);
    }
}

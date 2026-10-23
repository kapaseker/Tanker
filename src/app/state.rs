//! 首页、模型画廊与对局共用的页面状态。

use bevy::prelude::States;

/// 页面状态：菜单与对局经各自的淡出状态双向切换，揭幕阶段由 transition 管理。
///
/// 场景生成（相机/地形/坦克）从 Startup 移到 `OnEnter(InGame)`，
/// 菜单期不加载 3D 战斗场景；离屏测试直接把状态置为 InGame 跳过菜单。
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) enum GameState {
    /// 主菜单首页（web/index.html 设计还原）。
    #[default]
    Menu,
    /// Start Game 确认后：黑幕淡出，淡至全黑时生成场景并切 InGame。
    ToGame,
    /// 首页渐出后进入模型画廊。
    ToGallery,
    /// 真实模型的只读展示页。
    Gallery,
    /// 画廊渐出，黑场时回到首页。
    GalleryToMenu,
    /// Esc 返回首页，先保持当前场景并渐出。
    ToMenu,
    /// 游戏内：黑幕淡入完成后的正常 gameplay。
    InGame,
}

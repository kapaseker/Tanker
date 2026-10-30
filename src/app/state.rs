//! 首页、模型画廊、模型详情与对局共用的页面状态。

use bevy::prelude::States;

/// 页面状态：菜单与对局经各自的淡出状态双向切换，揭幕阶段由 transition 管理。
///
/// 场景生成（相机/地形/坦克）从 Startup 移到 `OnEnter(InGame)`，
/// 菜单期不加载 3D 战斗场景；离屏测试直接把状态置为 InGame 跳过菜单。
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) enum GameState {
    /// 首页（web/index.html 设计还原）。
    #[default]
    Index,
    /// Start Game 确认后：黑幕淡出，淡至全黑时生成场景并切 InGame。
    ToGame,
    /// 首页全屏淡出后进入田园视觉样板。
    ToTestScene,
    /// 无坦克的独立田园预览。
    TestScene,
    /// 首页渐出后进入模型画廊。
    ToGallery,
    /// 真实模型的只读展示页。
    Gallery,
    /// 画廊渐出，黑场时回到首页。
    GalleryToIndex,
    /// 画廊内容渐出，黑场交接到单模型详情。
    GalleryToDetail,
    /// 可拖拽旋转与缩放的真实模型详情。
    Detail,
    /// 详情渐出，黑场返回原画廊位置。
    DetailToGallery,
    /// Esc 返回首页，先保持当前场景并渐出。
    ToIndex,
    /// 首页退出确认后：淡出至黑场即退出进程，不进入任何页面（见 design.md 退出链路）。
    ToExit,
    /// 游戏内：黑幕淡入完成后的正常 gameplay。
    InGame,
}

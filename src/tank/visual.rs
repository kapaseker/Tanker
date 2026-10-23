//! 游戏与画廊共用的炮管视觉挂载；三部件按车型共同画布零偏移对齐。
use bevy::prelude::*;

/// 玩家默认炮管包。
pub(crate) const PLAYER_BARREL: &str = "tank/barrels/player/lance/barrel.vox";
/// 敌人默认炮管包。
pub(crate) const ENEMY_BARREL: &str = "tank/barrels/enemy/spit/barrel.vox";

/// 只挂载视觉资产，不附加武器、物理或驾驶行为。
pub(crate) fn mount_barrel(
    commands: &mut Commands,
    assets: &AssetServer,
    parent: Entity,
    path: &'static str,
) -> Entity {
    let barrel = commands.spawn(WorldAssetRoot(assets.load(path))).id();
    commands.entity(parent).add_child(barrel);
    barrel
}

//! 游戏与画廊共用的武器配置和炮管视觉挂载。
use bevy::prelude::*;

/// 炮管视觉路径；弹药性能由同目录配置独立管理。
pub(crate) struct BarrelSpec {
    /// 炮管模型包路径，相对 assets/。
    pub(crate) path: &'static str,
}

/// 敌人默认炮管装备：spit；敌人尚未接入开火。
pub(crate) const ENEMY_BARREL: BarrelSpec = BarrelSpec {
    path: "tank/enemy/carapace/barrel.vox",
};
/// 当前唯一已交付的弹丸：APFSDS 脱壳后飞行体，无敌方弹丸占位路径。
pub(crate) const PLAYER_BULLET: &str = "tank/ammunition/apfsds/bullet.vox";

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

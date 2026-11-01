//! 游戏与画廊共用的武器配置和炮管视觉挂载。
use bevy::prelude::*;

/// 炮管装备参数：视觉资产与武器性能同源，切换炮管型号时整体替换配置。
pub(crate) struct BarrelSpec {
    /// 炮管模型包路径，相对 assets/。
    pub(crate) path: &'static str,
    /// 炮口初速（muzzle velocity），单位：米/秒。
    pub(crate) muzzle_velocity: f32,
    /// 射速（rate of fire），单位：发/分钟；开火间隔由系统按 60/射速换算。
    pub(crate) rate_of_fire: f32,
}

/// astraH 整车炮管；沿用已有 1200 米/秒初速、12 发/分钟射速和弹丸资产。
pub(crate) const PLAYER_BARREL: BarrelSpec = BarrelSpec {
    path: crate::tank::model::astra::BARREL_PATH,
    muzzle_velocity: 1200.0,
    rate_of_fire: 12.0,
};
/// 敌人默认炮管装备：spit；敌人尚未接入开火，性能字段为未设定占位值。
pub(crate) const ENEMY_BARREL: BarrelSpec = BarrelSpec {
    path: "tank/enemy/carapace/barrel.vox",
    muzzle_velocity: 0.0,
    rate_of_fire: 0.0,
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

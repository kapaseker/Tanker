//! 游戏与画廊共用的炮管视觉挂载；三部件按车型共同画布零偏移对齐。
use bevy::prelude::*;

/// 炮管装备参数：视觉资产与武器性能同源，切换炮管型号时整体替换配置。
pub(crate) struct BarrelSpec {
    /// 炮管模型包路径，相对 assets/。
    pub(crate) path: &'static str,
    /// 该炮管发射的子弹模型包路径，相对 assets/。
    pub(crate) bullet: &'static str,
    /// 炮口初速（muzzle velocity），单位：米/秒。
    pub(crate) muzzle_velocity: f32,
    /// 射速（rate of fire），单位：发/分钟；开火间隔由系统按 60/射速换算。
    pub(crate) rate_of_fire: f32,
}

/// 玩家默认炮管装备：lance，性能按豹 2 同级口径取 1200 米/秒初速、12 发/分钟射速。
pub(crate) const PLAYER_BARREL: BarrelSpec = BarrelSpec {
    path: "tank/barrels/player/lance/barrel.vox",
    bullet: "tank/barrels/player/lance/bullet.vox",
    muzzle_velocity: 1200.0,
    rate_of_fire: 12.0,
};
/// 敌人默认炮管装备：spit；敌人尚未接入开火，性能字段为未设定占位值。
pub(crate) const ENEMY_BARREL: BarrelSpec = BarrelSpec {
    path: "tank/barrels/enemy/spit/barrel.vox",
    bullet: "tank/barrels/enemy/spit/bullet.vox",
    muzzle_velocity: 0.0,
    rate_of_fire: 0.0,
};
/// lance 的伴随子弹，只在模型预览中作组合示意展示。
pub(crate) const LANCE_BULLET: &str = PLAYER_BARREL.bullet;

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

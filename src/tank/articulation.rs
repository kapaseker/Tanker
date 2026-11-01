//! 玩家整车活动层级；角度由系统或测试设置，本模块不定义瞄准输入。

use super::model::astra;
use bevy::{prelude::*, transform::TransformSystems};

/// 炮塔相对车体的偏航和炮管相对炮塔的俯仰，单位弧度；默认朝前水平。
#[derive(Component, Default, Clone, Copy)]
pub(crate) struct TankAim {
    /// 绕车体 +Y 旋转，正值朝局部 +Z。
    pub(crate) yaw: f32,
    /// 正值抬高炮口；绕炮管局部 −Z 旋转。
    pub(crate) pitch: f32,
}

/// 活动节点所属的整车根，避免多个预览或车辆共享角度。
#[derive(Component)]
struct TurretJoint(Entity);
/// 炮管及随动防盾的俯仰节点；所属整车根与炮塔相同。
#[derive(Component)]
struct BarrelJoint(Entity);

/// 注册角度到局部变换的同步，必须先于世界变换传播与发射。
pub(crate) struct ArticulationPlugin;
impl Plugin for ArticulationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, apply_aim.before(TransformSystems::Propagate));
    }
}

/// 车体根已加载 body；炮塔附件整组跟转，炮管挂在炮塔轴下并返回其场景根。
pub(crate) fn mount_player(
    commands: &mut Commands,
    assets: &AssetServer,
    parent: Entity,
) -> Entity {
    let turret = commands
        .spawn((
            TurretJoint(parent),
            WorldAssetRoot(assets.load(astra::TURRET_PATH)),
            Transform::from_translation(Vec3::from_array(astra::TURRET_POSITION)),
            Visibility::Inherited,
        ))
        .id();
    let barrel = commands
        .spawn((
            BarrelJoint(parent),
            WorldAssetRoot(assets.load(astra::BARREL_PATH)),
            Transform::from_translation(Vec3::from_array(astra::BARREL_POSITION)),
            Visibility::Inherited,
        ))
        .id();
    commands.entity(parent).add_child(turret);
    commands.entity(turret).add_child(barrel);
    barrel
}

/// 有限角度才参与变换，防止上游错误把整车世界变换污染为 NaN。
fn apply_aim(
    aims: Query<&TankAim>,
    mut turrets: Query<(&TurretJoint, &mut Transform), Without<BarrelJoint>>,
    mut barrels: Query<(&BarrelJoint, &mut Transform), Without<TurretJoint>>,
) {
    for (joint, mut transform) in &mut turrets {
        if let Ok(aim) = aims.get(joint.0) {
            if aim.yaw.is_finite() {
                transform.rotation = Quat::from_rotation_y(aim.yaw);
            }
        }
    }
    for (joint, mut transform) in &mut barrels {
        if let Ok(aim) = aims.get(joint.0) {
            if aim.pitch.is_finite() {
                transform.rotation = Quat::from_rotation_z(-aim.pitch);
            }
        }
    }
}

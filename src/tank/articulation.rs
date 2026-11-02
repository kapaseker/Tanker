//! 玩家整车活动层级与相对角度积分；输入来源由对局和详情页各自决定。

use super::model::astra;
use bevy::{prelude::*, transform::TransformSystems};
use serde::Deserialize;

/// 车型的炮塔运动参数；JSONC 使用度和秒，运行姿态使用弧度。
#[derive(Component, Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AimSpec {
    /// 炮塔相对车体的最大水平角速度，度/秒；水平允许连续整圈旋转。
    pub(crate) traverse_speed_deg_s: f32,
    /// 炮管相对炮塔的最大俯仰角速度，度/秒。
    pub(crate) elevation_speed_deg_s: f32,
    /// 水平炮轴为零，负值向下，单位：度。
    pub(crate) min_elevation_deg: f32,
    /// 最大仰角，单位：度。
    pub(crate) max_elevation_deg: f32,
}

impl AimSpec {
    /// 配置进入运行前拒绝无效速度、反向区间或无法容纳默认水平姿态的限位。
    pub(crate) fn validate(&self) -> Result<(), String> {
        if !self.traverse_speed_deg_s.is_finite()
            || self.traverse_speed_deg_s <= 0.0
            || !self.elevation_speed_deg_s.is_finite()
            || self.elevation_speed_deg_s <= 0.0
            || !self.min_elevation_deg.is_finite()
            || !self.max_elevation_deg.is_finite()
            || self.min_elevation_deg <= -90.0
            || self.min_elevation_deg > 0.0
            || self.max_elevation_deg < 0.0
            || self.max_elevation_deg >= 90.0
            || self.min_elevation_deg >= self.max_elevation_deg
        {
            return Err(
                "炮塔角速度必须为有限正数，俯仰区间须位于 (-90,90) 度并包含水平姿态".into(),
            );
        }
        Ok(())
    }

    /// 按秒积分两轴意图（−1..=1），不改变车体；松手保持相对角度。
    pub(crate) fn advance(&self, aim: &mut TankAim, yaw: f32, pitch: f32, dt: f32) {
        aim.yaw = (aim.yaw + yaw * self.traverse_speed_deg_s.to_radians() * dt)
            .rem_euclid(std::f32::consts::TAU);
        aim.pitch = (aim.pitch + pitch * self.elevation_speed_deg_s.to_radians() * dt).clamp(
            self.min_elevation_deg.to_radians(),
            self.max_elevation_deg.to_radians(),
        );
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    /// 一秒的双轴输入独立积分；俯仰限位、水平回绕与松手不漂移。
    #[test]
    fn aim_motion_respects_rates_limits_and_release() {
        let spec = AimSpec {
            traverse_speed_deg_s: 40.0,
            elevation_speed_deg_s: 20.0,
            min_elevation_deg: -9.0,
            max_elevation_deg: 20.0,
        };
        let mut aim = TankAim::default();
        spec.advance(&mut aim, 1.0, 1.0, 1.0);
        assert!((aim.yaw.to_degrees() - 40.0).abs() < 1e-4);
        assert!((aim.pitch.to_degrees() - 20.0).abs() < 1e-4);
        spec.advance(&mut aim, 0.0, -1.0, 2.0);
        assert!((aim.pitch.to_degrees() + 9.0).abs() < 1e-4);
        let stopped = aim;
        spec.advance(&mut aim, 0.0, 0.0, 1.0);
        assert_eq!(aim.yaw, stopped.yaw);
        assert_eq!(aim.pitch, stopped.pitch);
        spec.advance(&mut aim, 1.0, 0.0, 9.0);
        assert!((aim.yaw.to_degrees() - 40.0).abs() < 1e-3);
        let mut smooth = TankAim::default();
        for _ in 0..60 {
            spec.advance(&mut smooth, 1.0, 1.0, 1.0 / 60.0);
        }
        assert!((smooth.yaw.to_degrees() - 40.0).abs() < 1e-3);
        assert!((smooth.pitch.to_degrees() - 20.0).abs() < 1e-3);
    }
}

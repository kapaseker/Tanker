//! 玩家整车活动层级与相对角度积分；输入来源由对局和详情页各自决定。

use super::model::astra;
use bevy::{prelude::*, transform::TransformSystems};
use serde::Deserialize;

/// 车型的炮塔运动参数；JSONC 使用度和秒，运行姿态使用弧度。
#[derive(Component, Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AimSpec {
    /// 水平全周范围，单位：度；当前车型支持 360 度连续旋转。
    pub(crate) traverse_range_deg: f32,
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
        if self.traverse_range_deg != 360.0 {
            return Err("当前炮塔仅支持 traverse_range_deg = 360 度连续旋转".into());
        }
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
            .rem_euclid(self.traverse_range_deg.to_radians());
        aim.pitch = (aim.pitch + pitch * self.elevation_speed_deg_s.to_radians() * dt).clamp(
            self.min_elevation_deg.to_radians(),
            self.max_elevation_deg.to_radians(),
        );
    }

    /// 按车型角速度推进进行中的回正：炮塔取含绕圈的最短路径，炮管直线回到零位；
    /// 到达零位即清除对应标记。dt 单位秒。
    pub(crate) fn reset_step(&self, aim: &mut TankAim, reset: &mut AimReset, dt: f32) {
        if reset.yaw {
            let step = self.traverse_speed_deg_s.to_radians() * dt;
            // yaw 已被 advance 归一到 [0, 全圈)；偏移折算到 (−半圈, 半圈] 取最短回零方向。
            let full = self.traverse_range_deg.to_radians();
            let offset = (aim.yaw + full * 0.5).rem_euclid(full) - full * 0.5;
            if offset.abs() <= step {
                aim.yaw = 0.0;
                reset.yaw = false;
            } else {
                aim.yaw = (aim.yaw - offset.signum() * step).rem_euclid(full);
            }
        }
        if reset.pitch {
            let step = self.elevation_speed_deg_s.to_radians() * dt;
            if aim.pitch.abs() <= step {
                aim.pitch = 0.0;
                reset.pitch = false;
            } else {
                aim.pitch -= aim.pitch.signum() * step;
            }
        }
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

/// R/F 触发的两轴回正进行标记；由 [`AimSpec::reset_step`] 消费清除。
#[derive(Component, Default, Clone, Copy)]
pub(crate) struct AimReset {
    /// 炮塔水平轴正在回正。
    pub(crate) yaw: bool,
    /// 炮管俯仰轴正在回正。
    pub(crate) pitch: bool,
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
            traverse_range_deg: 360.0,
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

    /// 回正按车型角速度渐进到零：炮塔走含绕圈的最短路径，炮管直线返回，到达即停。
    #[test]
    fn reset_step_returns_along_shortest_path_at_spec_speed() {
        let spec = AimSpec {
            traverse_range_deg: 360.0,
            traverse_speed_deg_s: 30.0,
            elevation_speed_deg_s: 9.0,
            min_elevation_deg: -9.0,
            max_elevation_deg: 20.0,
        };
        let mut reset = AimReset::default();
        // 350° 的炮塔回正应向前绕过零位（10° 路径），而不是倒转 350°。
        let mut aim = TankAim {
            yaw: 350_f32.to_radians(),
            pitch: 0.0,
        };
        reset.yaw = true;
        spec.reset_step(&mut aim, &mut reset, 0.1);
        assert!(
            ((aim.yaw.to_degrees() - 353.0).abs() < 1e-3),
            "350° 应沿最短路径前进到 353°：{}",
            aim.yaw.to_degrees()
        );
        for _ in 0..5 {
            spec.reset_step(&mut aim, &mut reset, 0.1);
        }
        assert_eq!(aim.yaw, 0.0, "绕圈回正应到达零位");
        assert!(!reset.yaw, "到达零位后应清除回正标记");
        // 负俯仰直线回升；步长大于剩余角度时直接落在零位。
        let mut aim = TankAim { yaw: 0.0, pitch: -4_f32.to_radians() };
        reset.pitch = true;
        spec.reset_step(&mut aim, &mut reset, 1.0);
        assert_eq!(aim.pitch, 0.0, "步长覆盖剩余角度时应停在零位");
        assert!(!reset.pitch);
    }
}

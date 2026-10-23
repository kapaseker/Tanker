//! 实际坦克、地形碰撞与相机状态检查。

use super::*;
use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};

/// 捕获当前验收对象状态；默认玩家，敌人 wheels 预设显式选择测试靶车。
pub(in crate::validation) fn subject_state(
    world: &mut World,
) -> Result<(Transform, [f32; 2]), String> {
    if let Some(target) = world.get_resource::<WheelTarget>() {
        let entity = world.entity(target.0);
        return entity
            .get::<Transform>()
            .zip(entity.get::<crate::tank::track::TrackAnim>())
            .map(|(transform, tracks)| (*transform, tracks.distances()))
            .ok_or_else(|| "敌人轮组状态异常".into());
    }
    let mut query =
        world.query_filtered::<(&Transform, &crate::tank::track::TrackAnim), With<crate::gameplay::player::Tank>>();
    query
        .single(world)
        .map(|(transform, tracks)| (*transform, tracks.distances()))
        .map_err(|error| format!("玩家或履带状态异常：{error}"))
}

/// 场景与相机的最低运行断言，图片只作为额外证据而非替代状态检查。
pub(in crate::validation) fn validate_scene(world: &mut World) -> Result<(), String> {
    world
        .run_system_cached(validate_scene_state)
        .map_err(|e| format!("场景断言系统异常：{e}"))?
}

/// 从上方向实际地形碰撞体投射射线，检查车体底面，不能只用全局高度范围判断穿地。
fn validate_scene_state(
    spatial: SpatialQuery,
    terrain: Query<(), With<crate::terrain::TerrainSurface>>,
    tanks: Query<
        (&Transform, Has<crate::gameplay::player::Tank>),
        Or<(
            With<crate::gameplay::player::Tank>,
            With<crate::gameplay::enemy::EnemyTank>,
        )>,
    >,
    players: Query<&Transform, With<crate::gameplay::player::Tank>>,
    cameras: Query<&Transform, With<crate::gameplay::camera::FollowCamera>>,
) -> Result<(), String> {
    let player = players.single().map_err(|e| format!("玩家状态异常：{e}"))?;
    for (tank, is_player) in &tanks {
        ensure(
            tank.translation.is_finite() && tank.rotation.is_finite() && tank.translation.y < 4.0,
            "坦克位置非有限值或超出落地范围",
        )?;
        let origin = Vec3::new(tank.translation.x, 10.0, tank.translation.z);
        let hit = spatial
            .cast_ray_predicate(
                origin,
                Dir3::NEG_Y,
                20.0,
                true,
                &SpatialQueryFilter::default(),
                &|entity| terrain.contains(entity),
            )
            .ok_or("坦克下方未找到地形碰撞体")?;
        let half_height = if is_player {
            crate::tank::driving::TANK_SIZE.y
        } else {
            crate::gameplay::enemy::ENEMY_SIZE.y
        } / 2.0;
        ensure(
            tank.translation.y - half_height >= origin.y - hit.distance - 0.1,
            "坦克底面穿入地形超过 0.1 米",
        )?;
    }
    let camera = cameras.single().map_err(|e| format!("跟随相机异常：{e}"))?;
    let expected = crate::gameplay::camera::camera_transform(&player);
    // 直接比较分量，避免 acos 将同一姿态的归一化舍入误差放大成角度误差。
    ensure(
        camera.translation.distance(expected.translation) < 0.01
            && camera.rotation.abs_diff_eq(expected.rotation, 0.00001),
        "相机未跟随玩家",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;

    #[test]
    fn scene_assertions_detect_penetration_against_actual_collider() {
        use avian3d::prelude::{Collider, PhysicsPlugins, RigidBody};
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            PhysicsPlugins::default(),
            crate::tank::driving::DrivingPlugin,
            crate::gameplay::player::PlayerPlugin,
            crate::gameplay::camera::FollowCameraPlugin,
        ))
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(STEP));
        app.finish();
        app.cleanup();
        app.world_mut().spawn((
            crate::terrain::TerrainSurface,
            RigidBody::Static,
            Collider::cuboid(200.0, 1.0, 200.0),
            Transform::from_xyz(0.0, -0.5, 0.0),
        ));
        let player = app
            .world_mut()
            .spawn((
                crate::gameplay::player::Tank,
                crate::tank::driving::tank_body(),
                Transform::from_xyz(0.0, 0.75, 0.0),
            ))
            .id();
        app.world_mut()
            .spawn((crate::gameplay::camera::FollowCamera, Transform::default()));
        for _ in 0..10 {
            app.update();
        }
        validate_scene(app.world_mut()).unwrap();
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation
            .y = 0.2;
        assert!(
            validate_scene(app.world_mut())
                .unwrap_err()
                .contains("穿入地形")
        );
    }
}

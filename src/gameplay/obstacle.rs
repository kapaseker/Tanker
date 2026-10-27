//! 对局内唯一的混凝土墩样品：VOX 视觉与不可推动的梯形凸碰撞体。

use avian3d::prelude::*;
use bevy::prelude::*;

use super::session::SessionEntity;

/// 混凝土墩整体包围尺寸（世界 X 厚、Y 高、Z 长），单位米；0.1 米一格。长与高各 8 米。
pub(crate) const SIZE: Vec3 = Vec3::new(1.5, 8.0, 8.0);
/// 顶部厚度，单位米；底部厚度取 SIZE.x。
const TOP_THICKNESS: f32 = 0.9;
/// 出生点正前方 25 米，中心高度保证底部落在 Y=0。
pub(crate) const POSITION: Vec3 = Vec3::new(-25.0, SIZE.y / 2.0, 0.0);
/// 唯一 VOX 资源，作者化轴向为厚度、长度、高度，不需要运行时旋转。
const VOX_PATH: &str = "obstacles/concrete/obstacle.vox";

/// 标记不可摧毁但能挡弹的混凝土墩；不能复用 EnemyTank 的击毁语义。
#[derive(Component)]
pub(crate) struct Concrete;

/// 对局生成时挂载视觉，资源及实例就绪和清理复用 session 的统一入口。
pub(super) fn spawn(commands: &mut Commands, assets: &AssetServer) {
    commands.spawn((
        SessionEntity,
        WorldAssetRoot(assets.load(VOX_PATH)),
        body(),
        Transform::from_translation(POSITION),
    ));
}

/// 八个角点定义连续梯形外轮廓，与 VOX 分层收窄表面的偏差不超过一格。
pub(crate) fn body() -> impl Bundle {
    let mut points = Vec::with_capacity(8);
    for (y, thickness) in [(-SIZE.y / 2.0, SIZE.x), (SIZE.y / 2.0, TOP_THICKNESS)] {
        for x in [-thickness / 2.0, thickness / 2.0] {
            for z in [-SIZE.z / 2.0, SIZE.z / 2.0] {
                points.push(Vec3::new(x, y, z).into());
            }
        }
    }
    (
        Concrete,
        RigidBody::Static,
        Collider::convex_hull(&points).expect("混凝土墩八个角点必须构成凸体"),
        // 坦克持续写入水平速度，零摩擦斜面会将推力累积成向上速度，需粗糙接触阻止爬墩。
        Friction::new(0.8).with_combine_rule(CoefficientCombine::Max),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    /// 真实驾驶持续撞墩仍被挡住，墩体既不位移也不倾斜。
    #[test]
    fn concrete_blocks_driving_and_stays_fixed() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            PhysicsPlugins::default(),
            crate::tank::driving::DrivingPlugin,
            crate::gameplay::player::PlayerPlugin,
        ))
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / 60.0,
        )))
        .add_systems(Startup, |mut commands: Commands| {
            crate::terrain::spawn_arena(&mut commands)
        });
        app.finish();
        app.cleanup();
        let obstacle = app
            .world_mut()
            .spawn((body(), Transform::from_translation(POSITION)))
            .id();
        let tank = app
            .world_mut()
            .spawn((
                crate::gameplay::player::Tank,
                crate::tank::driving::tank_body(),
                crate::tank::track::PLAYER_TRACKS,
                Transform::from_xyz(0.0, 3.0, 0.0),
            ))
            .id();
        for _ in 0..120 {
            app.update();
        }
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowUp);
        for _ in 0..360 {
            app.update();
            let position = app.world().get::<Transform>(tank).unwrap().translation;
            assert!(
                position.x > -21.0 && position.y < 1.4,
                "持续撞墩不能爬升或穿透：{position:?}"
            );
        }
        let position = app.world().get::<Transform>(tank).unwrap().translation;
        assert!(
            position.x < -19.0 && position.x > -21.0,
            "应停在墩前而非穿过或提前受阻：{position:?}"
        );
        assert!((position.y - crate::tank::driving::TANK_SIZE.y / 2.0).abs() < 0.05);
        let fixed = app.world().get::<Transform>(obstacle).unwrap();
        assert_eq!(fixed.translation, POSITION);
        assert_eq!(fixed.rotation, Quat::IDENTITY);
    }
}

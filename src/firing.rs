//! 坦克空格连发：按炮口世界方向发射，沿飞行路径检测敌人，命中或射程耗尽时清理。

use crate::enemy::{EnemyDestroyed, EnemyTank};
use avian3d::prelude::{Collider, ShapeCastConfig, SpatialQuery, SpatialQueryFilter};
use bevy::{
    asset::LoadState,
    prelude::*,
    transform::TransformSystems,
    world_serialization::{WorldAsset, WorldInstance, WorldInstanceSpawner},
};

/// 点按与长按共用的最小发射间隔，单位：秒。
const FIRE_INTERVAL: f32 = 0.5;
/// 子弹的匀速飞行速度，单位：世界单位/秒。
const BULLET_SPEED: f32 = 40.0;
/// 当前弹体为 4×4×4 格立方体，扫掠尺寸与资源保持一致，单位：米。
const BULLET_SIZE: f32 = 0.4;
/// 从出生点累计计算的最大飞行距离，单位：世界单位。
const BULLET_RANGE: f32 = 2000.0;
/// 炮口与作者化炮管共用格坐标；子弹中心前移半边长，避免与炮口重叠。
const MUZZLE_POSITION: Vec3 = Vec3::new(
    -(crate::player_model::BARREL_X.1 as f32 - crate::player_model::CANVAS.0 as f32 / 2.0)
        * crate::VOXEL_SIZE
        - BULLET_SIZE / 2.0,
    (crate::player_model::BORE_HEIGHT - crate::player_model::CANVAS.2 as f32 / 2.0)
        * crate::VOXEL_SIZE,
    0.0,
);

/// 玩家独立炮管的场景根；资源位于 tank/barrels/player/，随车身销毁，不拥有碰撞体。
#[derive(Component)]
struct PlayerBarrel;

/// 玩家根出现时挂载一次炮管，三包按共同画布中心零偏移组合。
fn spawn_barrels(
    mut commands: Commands,
    assets: Res<AssetServer>,
    tanks: Query<Entity, Added<Tank>>,
) {
    for tank in &tanks {
        let barrel = commands
            .spawn((
                PlayerBarrel,
                WorldAssetRoot(assets.load("tank/barrels/player/default.vox")),
            ))
            .id();
        commands.entity(tank).add_child(barrel);
    }
}

/// 标记坦克场景根节点，供发射系统定位炮口的世界变换。
#[derive(Component)]
pub(super) struct Tank;

/// 子弹根节点的飞行状态；独立于坦克场景层级，发射后保持原方向。
#[derive(Component)]
pub(super) struct Bullet {
    /// 发射瞬间确定的世界空间单位方向向量，不随相机或坦克转向。
    direction: Vec3,
    /// 自出生以来累计飞行的距离，单位：世界单位。
    distance: f32,
}

/// 当前坦克共用的武器资源，持有子弹场景和连发计时状态。
#[derive(Resource)]
struct Gun {
    /// 预加载并由各颗子弹复用的场景句柄，保持资源存活。
    bullet_scene: Handle<WorldAsset>,
    /// 每次发射后的剩余冷却时间，松开空格或资源未就绪也继续计时。
    cadence: FireCadence,
}

/// 离屏运行须在开始计帧前确认弹体及其依赖已加载，避免首次输入被吞掉。
pub(super) fn assets_ready(world: &World) -> bool {
    world.get_resource::<Gun>().is_some_and(|gun| {
        world
            .resource::<AssetServer>()
            .is_loaded_with_dependencies(&gun.bullet_scene)
    })
}

/// 新对局从首发可用开始，不继承上一局射击冷却。
pub(crate) fn reset_session(world: &mut World) {
    if let Some(mut gun) = world.get_resource_mut::<Gun>() {
        gun.cadence = FireCadence::default();
    }
}

/// 注册子弹资源加载、空格发射与飞行清理系统。
pub(super) struct FiringPlugin;

impl Plugin for FiringPlugin {
    /// 在变换传播完成后发射，确保读取到坦克本帧的世界位置和方向。
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_bullet)
            .add_systems(Update, (spawn_barrels, move_bullets, report_bullet))
            .add_systems(PostUpdate, fire_bullets.after(TransformSystems::Propagate));
    }
}

/// 启动时预加载子弹场景；加载完成前由发射系统忽略开火输入。
fn load_bullet(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(Gun {
        bullet_scene: assets.load("bullet.vox"),
        cadence: default(),
    });
}

/// 成功或失败只汇报一次；`reported` 跨帧保留，成功须等待依赖资源一并就绪。
fn report_bullet(gun: Res<Gun>, assets: Res<AssetServer>, mut reported: Local<bool>) {
    if *reported {
        return;
    }
    match assets.get_load_state(&gun.bullet_scene) {
        Some(LoadState::Failed(error)) => {
            error!("子弹资源加载失败：{error}");
            *reported = true;
        }
        _ if assets.is_loaded_with_dependencies(&gun.bullet_scene) => {
            info!("子弹资源加载完成：assets/bullet.vox");
            *reported = true;
        }
        _ => {}
    }
}

/// 连发节奏：立即首发，随后按固定间隔发射，卡顿时不累计待补发子弹。
#[derive(Default)]
struct FireCadence {
    /// 距离下一发的剩余时间，单位：秒；初始为零，允许立即首发。
    remaining: f32,
}

impl FireCadence {
    /// 推进计时并返回本帧是否发射；`delta` 为本帧游戏时间增量，单位：秒。
    /// `enabled` 表示空格按住且资源就绪；松开期间继续冷却，但不积攒发射次数。
    fn tick(&mut self, enabled: bool, delta: f32) -> bool {
        self.remaining = (self.remaining - delta).max(0.0);
        if enabled && self.remaining <= 0.0 {
            // 每帧最多一发，重置间隔而不追补卡顿期间的子弹。
            self.remaining = FIRE_INTERVAL;
            return true;
        }
        false
    }
}

/// 将局部炮口数据换算到世界空间，依次返回子弹出生中心和归一化飞行方向。
fn muzzle_pose(tank: &GlobalTransform) -> (Vec3, Vec3) {
    (
        tank.transform_point(MUZZLE_POSITION),
        tank.affine().transform_vector3(Vec3::NEG_X).normalize(),
    )
}

/// 处理空格连发；仅在唯一坦克实例及双方资源依赖就绪后生成独立子弹根节点。
fn fire_bullets(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut gun: ResMut<Gun>,
    assets: Res<AssetServer>,
    instances: Res<WorldInstanceSpawner>,
    tanks: Query<(&GlobalTransform, &WorldInstance, &WorldAssetRoot), With<Tank>>,
    barrels: Query<(&WorldAssetRoot, &WorldInstance), With<PlayerBarrel>>,
    fade: Option<Res<crate::transition::Fade>>,
) {
    // 场景资源加载完成不等于实体已实例化，两者都就绪后才能确定发射位置。
    let barrel_ready = barrels.single().ok().is_some_and(|(root, instance)| {
        instances.instance_is_ready(**instance) && assets.is_loaded_with_dependencies(&root.0)
    });
    let tank = tanks.single().ok().filter(|&(_, instance, root)| {
        barrel_ready
            && instances.instance_is_ready(**instance)
            && assets.is_loaded_with_dependencies(&root.0)
            && assets.is_loaded_with_dependencies(&gun.bullet_scene)
    });
    if !gun.cadence.tick(
        keys.pressed(KeyCode::Space)
            && tank.is_some()
            && fade.as_ref().is_none_or(|fade| fade.accepts_input()),
        time.delta_secs(),
    ) {
        return;
    }
    // tick 返回 true 时必有可用坦克，因此这里可以取出此前筛选的结果。
    let (tank, _, _) = tank.unwrap();
    // 位置和方向均为世界空间数据，子弹不成为坦克的子实体。
    let (position, direction) = muzzle_pose(tank);
    debug!("发射子弹：位置={position:?}，方向={direction:?}");
    let transform = Transform::from_translation(position).with_rotation(tank.rotation());
    commands.spawn((
        crate::transition::SessionEntity,
        Bullet {
            direction,
            distance: 0.0,
        },
        WorldAssetRoot(gun.bullet_scene.clone()),
        transform,
        // 此时本帧变换传播已结束，显式初始化世界变换，避免出生帧落在原点。
        GlobalTransform::from(transform),
    ));
}

/// 沿完整位移扫掠弹体，仅命中最近敌人；命中或射程耗尽时清理场景根及后代。
fn move_bullets(
    mut commands: Commands,
    time: Res<Time>,
    spatial: SpatialQuery,
    // 用 GlobalTransform 读取敌人位置，避免与子弹的 &mut Transform 查询冲突（B0001）；
    // 敌人都是场景根，两者等值。
    enemies: Query<&GlobalTransform, With<EnemyTank>>,
    mut bullets: Query<(Entity, &mut Transform, &mut Bullet)>,
    mut destroyed_enemies: MessageWriter<EnemyDestroyed>,
) {
    // 本帧前进的世界距离，按游戏时间计算，单位：米。
    let step = BULLET_SPEED * time.delta_secs();
    let shape = Collider::cuboid(BULLET_SIZE, BULLET_SIZE, BULLET_SIZE);
    // 子弹投射只认敌人，不会撞墙；越过场地边界（±FIELD_SIDE/2）即视为失效，
    // 防止穿墙飞满射程。单帧最大位移约 0.7 米，越界误差可忽略。
    let half_field = crate::ground::FIELD_SIDE / 2.0;
    // Commands 在系统结束后才删除实体，同帧后续弹体必须主动跳过已击毁目标。
    let mut destroyed = Vec::new();
    let enemy_count = enemies.iter().count();
    for (entity, mut transform, mut bullet) in &mut bullets {
        // 先判越界再移动：出生点在场地内，不会误删刚生成的子弹。
        if transform.translation.x.abs() > half_field || transform.translation.z.abs() > half_field
        {
            debug!("子弹飞出场地，销毁：{entity}");
            commands.entity(entity).despawn();
            continue;
        }
        let remaining = (BULLET_RANGE - bullet.distance).max(0.0);
        let travel = step.min(remaining);
        let hit = spatial.cast_shape_predicate(
            &shape,
            transform.translation,
            transform.rotation,
            Dir3::new(bullet.direction).expect("子弹方向必须为非零单位向量"),
            &ShapeCastConfig::from_max_distance(travel),
            &SpatialQueryFilter::default(),
            &|target| enemies.contains(target) && !destroyed.contains(&target),
        );
        if let Some(hit) = hit {
            destroyed.push(hit.entity);
            // 爆炸中心取被毁车体的世界位置：与 despawn 对象一一对应，且不偏向受击面。
            if let Ok(enemy) = enemies.get(hit.entity) {
                destroyed_enemies.write(EnemyDestroyed {
                    position: enemy.translation(),
                });
            }
            commands.entity(hit.entity).despawn();
            commands.entity(entity).despawn();
            let left = enemy_count - destroyed.len();
            info!("击毁敌人坦克，剩余 {left} 辆");
            if left == 0 {
                info!("敌人已全部清除，可继续自由驾驶");
            }
        } else if step >= remaining {
            debug!("子弹达到射程，销毁：{entity}");
            commands.entity(entity).despawn();
        } else {
            bullet.distance += travel;
            transform.translation += bullet.direction * travel;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        asset::AssetPlugin,
        camera::{primitives::MeshAabb, visibility::VisibilityClass},
        image::ImagePlugin,
        light::FogVolume,
        time::TimeUpdateStrategy,
        transform::components::TransformTreeChanged,
        world_serialization::WorldSerializationPlugin,
    };
    use bevy_vox_scene::{VoxLoaderSettings, VoxScenePlugin};
    use std::time::{Duration, Instant};

    /// 汇总场景后代网格的世界包围盒，覆盖模型坐标与根节点变换。
    fn scene_bounds(world: &World, root: Entity) -> (usize, Vec3, Vec3) {
        let mut pending = vec![root];
        let mut mesh_count = 0;
        let mut minimum = Vec3::splat(f32::INFINITY);
        let mut maximum = Vec3::splat(f32::NEG_INFINITY);
        while let Some(entity) = pending.pop() {
            if let Some(children) = world.get::<Children>(entity) {
                pending.extend(children.iter());
            }
            if let Some(mesh) = world.get::<Mesh3d>(entity) {
                mesh_count += 1;
                let aabb = world
                    .resource::<Assets<Mesh>>()
                    .get(&mesh.0)
                    .unwrap()
                    .get_aabb()
                    .unwrap();
                let transform = world.get::<GlobalTransform>(entity).unwrap();
                for x in [-1.0, 1.0] {
                    for y in [-1.0, 1.0] {
                        for z in [-1.0, 1.0] {
                            let corner = Vec3::from(aabb.center)
                                + Vec3::from(aabb.half_extents) * Vec3::new(x, y, z);
                            let point = transform.transform_point(corner);
                            minimum = minimum.min(point);
                            maximum = maximum.max(point);
                        }
                    }
                }
            }
        }
        (mesh_count, minimum, maximum)
    }

    /// 验证立即首发及持续按住时每半秒发射一次。
    #[test]
    fn space_fires_immediately_then_every_half_second() {
        let mut cadence = FireCadence::default();
        assert!(cadence.tick(true, 0.016));
        assert!(!cadence.tick(true, 0.25));
        assert!(cadence.tick(true, 0.25));
        assert!(!cadence.tick(true, 0.25));
        assert!(cadence.tick(true, 0.25));
    }

    /// 快速点按与长按共用冷却，松开或同帧重按都不能提前发射。
    #[test]
    fn rapid_taps_cannot_bypass_the_fire_interval() {
        let mut cadence = FireCadence::default();
        assert!(cadence.tick(true, 0.0));
        assert!(!cadence.tick(true, 0.0));
        assert!(!cadence.tick(false, 0.125));
        assert!(!cadence.tick(true, 0.125));
        assert!(!cadence.tick(false, 0.125));
        assert!(cadence.tick(true, 0.125));
    }

    /// 验证松开或资源不可用时继续冷却，冷却结束后也不积攒发射次数。
    #[test]
    fn release_or_unavailable_assets_reset_firing_without_queued_shots() {
        let mut cadence = FireCadence::default();
        assert!(!cadence.tick(false, 1.0));
        assert!(cadence.tick(true, 0.016));
        assert!(!cadence.tick(false, 0.125));
        assert!(!cadence.tick(true, 0.125));
        assert!(!cadence.tick(false, 0.25));
        assert!(cadence.tick(true, 0.0));
        assert!(!cadence.tick(false, 10.0));
        assert!(cadence.tick(true, 0.0));
        assert!(!cadence.tick(true, 0.016));
    }

    /// 验证长帧最多触发一发，后续帧不集中补发。
    #[test]
    fn long_frames_do_not_leave_a_backlog() {
        let mut cadence = FireCadence::default();
        assert!(cadence.tick(true, 0.016));
        assert!(cadence.tick(true, 2.0));
        assert!(!cadence.tick(true, 0.016));
    }

    /// 验证坦克平移、旋转后，炮口位置和方向正确转换到世界空间。
    #[test]
    fn muzzle_follows_the_tank_world_transform() {
        let tank = GlobalTransform::from(
            Transform::from_xyz(3.0, 2.0, 1.0)
                .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
        );
        let (position, direction) = muzzle_pose(&tank);
        assert!(position.abs_diff_eq(Vec3::new(3.0, 2.7, 7.95), 1e-5));
        assert!(direction.abs_diff_eq(Vec3::Z, 1e-5));
    }

    /// 用真实物理宽阶段预热目标，再以独立游戏时间运行子弹系统。
    fn hit_test_app(targets: &[(Vec3, f32)]) -> (App, Vec<Entity>) {
        use avian3d::prelude::*;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugins::default()))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
                1.0 / 64.0,
            )))
            // move_bullets 的 MessageWriter 要求消息类型已注册。
            .add_message::<EnemyDestroyed>();
        app.finish();
        app.cleanup();
        let enemies = targets
            .iter()
            .map(|&(position, yaw)| {
                app.world_mut()
                    .spawn((
                        crate::enemy::EnemyTank,
                        RigidBody::Static,
                        Collider::cuboid(
                            crate::enemy::ENEMY_SIZE.x,
                            crate::enemy::ENEMY_SIZE.y,
                            crate::enemy::ENEMY_SIZE.z,
                        ),
                        Transform::from_translation(position)
                            .with_rotation(Quat::from_rotation_y(yaw)),
                    ))
                    .id()
            })
            .collect();
        for _ in 0..3 {
            app.update();
        }
        (app, enemies)
    }

    /// 将子弹直接交给生产系统，独立控制单帧时间以覆盖高速和射程末段。
    fn advance_test_bullets(app: &mut App, seconds: f32) {
        use bevy::ecs::system::RunSystemOnce;
        let mut time = Time::<()>::default();
        time.advance_by(Duration::from_secs_f32(seconds));
        app.insert_resource(time);
        app.world_mut().run_system_once(move_bullets).unwrap();
    }

    /// 创建不依赖资源加载的弹体；姿态与实际发射保持一致。
    fn test_bullet(app: &mut App, origin: Vec3, direction: Vec3, distance: f32) -> Entity {
        app.world_mut()
            .spawn((
                Bullet {
                    direction,
                    distance,
                },
                Transform::from_translation(origin)
                    .with_rotation(Quat::from_rotation_arc(Vec3::NEG_X, direction)),
            ))
            .id()
    }

    /// 玩家真实炮口水平命中平地敌人；目标底部远高于弹道时仍应掠过。
    #[test]
    fn modern_muzzle_hits_ground_level_enemy_without_platform() {
        let origin = MUZZLE_POSITION + Vec3::Y * crate::driving::TANK_SIZE.y / 2.0;
        for (height, should_hit) in [(crate::enemy::ENEMY_SIZE.y / 2.0, true), (4.0, false)] {
            let (mut app, enemies) = hit_test_app(&[(Vec3::new(-12.0, height, 0.0), 0.0)]);
            let bullet = test_bullet(&mut app, origin, Vec3::NEG_X, 0.0);
            advance_test_bullets(&mut app, 0.5);
            assert_eq!(app.world().get_entity(enemies[0]).is_err(), should_hit);
            assert_eq!(app.world().get_entity(bullet).is_err(), should_hit);
        }
    }

    /// 一帧跨过两个目标时只击毁最近目标，同时清理双方场景后代。
    #[test]
    fn fast_bullet_hits_nearest_enemy_and_cleans_children() {
        let (mut app, enemies) = hit_test_app(&[
            (Vec3::new(-12.0, 0.7, 0.0), 0.0),
            (Vec3::new(-6.0, 0.7, 0.0), 0.7),
        ]);
        let bullet = test_bullet(&mut app, Vec3::new(0.0, 1.2, 0.0), Vec3::NEG_X, 0.0);
        let child = app.world_mut().spawn(ChildOf(enemies[1])).id();
        let grandchild = app.world_mut().spawn(ChildOf(child)).id();
        let bullet_child = app.world_mut().spawn(ChildOf(bullet)).id();
        advance_test_bullets(&mut app, 0.5);
        assert!(app.world().get_entity(enemies[0]).is_ok());
        for entity in [enemies[1], child, grandchild, bullet, bullet_child] {
            assert!(app.world().get_entity(entity).is_err());
        }
    }

    /// 擦过旋转车体、超出剩余射程的目标都不会被击毁。
    #[test]
    fn misses_and_targets_beyond_range_survive() {
        let (mut app, enemies) = hit_test_app(&[(Vec3::new(-6.0, 1.3, 0.0), 0.8)]);
        let miss = test_bullet(&mut app, Vec3::new(0.0, 1.2, 5.0), Vec3::NEG_X, 0.0);
        let expired = test_bullet(
            &mut app,
            Vec3::new(0.0, 1.2, 0.0),
            Vec3::NEG_X,
            BULLET_RANGE - 1.0,
        );
        advance_test_bullets(&mut app, 0.5);
        assert!(app.world().get_entity(enemies[0]).is_ok());
        assert!(app.world().get_entity(miss).is_ok());
        assert!(app.world().get_entity(expired).is_err());
        // 最后一段射程内仍需执行命中检测，而不是提前丢弃整段路径。
        let hit = test_bullet(
            &mut app,
            Vec3::new(-3.5, 1.2, 0.0),
            Vec3::NEG_X,
            BULLET_RANGE - 1.0,
        );
        advance_test_bullets(&mut app, 0.5);
        assert!(app.world().get_entity(enemies[0]).is_err());
        assert!(app.world().get_entity(hit).is_err());
    }

    /// 同帧两发弹跳过已排队销毁的敌人，第二发可继续命中后方敌人。
    #[test]
    fn simultaneous_bullets_skip_already_destroyed_targets() {
        let (mut app, enemies) = hit_test_app(&[
            (Vec3::new(-6.0, 0.7, 0.0), 0.0),
            (Vec3::new(-12.0, 0.7, 0.0), 0.0),
        ]);
        let bullets: Vec<_> = (0..3)
            .map(|_| test_bullet(&mut app, Vec3::new(0.0, 1.2, 0.0), Vec3::NEG_X, 0.0))
            .collect();
        advance_test_bullets(&mut app, 0.5);
        assert!(enemies.iter().all(|&e| app.world().get_entity(e).is_err()));
        assert_eq!(
            bullets
                .iter()
                .filter(|&&e| app.world().get_entity(e).is_ok())
                .count(),
            1
        );
    }

    /// 子弹越过场地边界即失效，场内飞行方向不受影响。
    #[test]
    fn bullets_expire_outside_the_field() {
        let (mut app, _) = hit_test_app(&[]);
        // 299 米处在界内（边界 ±300），一帧 20 米后越界，第二帧被销毁。
        let outbound = test_bullet(&mut app, Vec3::new(299.0, 1.2, 0.0), Vec3::X, 0.0);
        let inbound = test_bullet(&mut app, Vec3::new(299.0, 1.2, 0.0), Vec3::NEG_X, 0.0);
        advance_test_bullets(&mut app, 0.5);
        assert!(app.world().get_entity(outbound).is_ok());
        advance_test_bullets(&mut app, 0.5);
        assert!(app.world().get_entity(outbound).is_err());
        assert!(app.world().get_entity(inbound).is_ok());
    }

    /// 用固定半秒步长验证飞行速度，以及越过场地边界后对子弹及其子实体的清理。
    #[test]
    fn bullets_move_independently_and_despawn_with_children_at_field_edge() {
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(Duration::from_secs_f32(0.5));
        app.insert_resource(time)
            .init_resource::<avian3d::collider_tree::ColliderTrees>()
            .add_message::<EnemyDestroyed>()
            .add_systems(Update, move_bullets);
        let bullet = app
            .world_mut()
            .spawn((
                Bullet {
                    direction: Vec3::NEG_X,
                    distance: 0.0,
                },
                Transform::from_xyz(-1.8, 0.45, -0.1),
            ))
            .id();
        let child = app.world_mut().spawn(ChildOf(bullet)).id();
        app.update();
        assert!(
            app.world()
                .get::<Transform>(bullet)
                .unwrap()
                .translation
                .abs_diff_eq(Vec3::new(-21.8, 0.45, -0.1), 1e-5)
        );
        assert_eq!(app.world().get::<Bullet>(bullet).unwrap().distance, 20.0);
        app.update();
        assert_eq!(app.world().get::<Bullet>(bullet).unwrap().distance, 40.0);
        assert!(
            app.world()
                .get::<Transform>(bullet)
                .unwrap()
                .translation
                .abs_diff_eq(Vec3::new(-41.8, 0.45, -0.1), 1e-5)
        );
        // 出生点 x=-1.8，每帧向 -X 飞 20 米；第 15 帧后 x=-301.8 越界，下一帧销毁。
        for _ in 2..15 {
            app.update();
        }
        assert!(app.world().get_entity(bullet).is_ok());
        app.update();
        assert!(app.world().get_entity(bullet).is_err());
        assert!(app.world().get_entity(child).is_err());
    }

    /// 无需渲染窗口，使用真实 VOX 加载器验证连发、尺寸、材质和实例注销。
    #[test]
    fn real_vox_assets_fire_and_clean_up_scene_instances() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            ImagePlugin::default(),
            WorldSerializationPlugin,
            TransformPlugin,
            VoxScenePlugin {
                global_settings: Some(VoxLoaderSettings {
                    voxel_size: crate::VOXEL_SIZE,
                    ..default()
                }),
            },
            FiringPlugin,
            crate::ground::GroundPlugin,
            crate::track::TrackAnimationPlugin,
        ))
        .init_resource::<avian3d::collider_tree::ColliderTrees>()
        .init_asset::<StandardMaterial>()
        .init_asset::<Mesh>()
        .register_type::<Visibility>()
        .register_type::<ViewVisibility>()
        .register_type::<InheritedVisibility>()
        .register_type::<VisibilityClass>()
        .register_type::<Transform>()
        .register_type::<GlobalTransform>()
        .register_type::<TransformTreeChanged>()
        .register_type::<Mesh3d>()
        .register_type::<MeshMaterial3d<StandardMaterial>>()
        .register_type::<FogVolume>()
        .init_resource::<ButtonInput<KeyCode>>()
        .add_message::<EnemyDestroyed>()
        .add_systems(Update, crate::enemy::spawn_enemy_barrels)
        .add_systems(Startup, crate::ground::load_ground)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            0.25,
        )));
        // 允许测试直接推进 50 秒游戏时间，避免被默认的单帧时间上限截断。
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .set_max_delta(Duration::from_secs(60));
        let tank_asset = app
            .world()
            .resource::<AssetServer>()
            .load("tank/player/default/body.vox");
        let tank = app
            .world_mut()
            .spawn((
                Tank,
                crate::track::PLAYER_TRACKS,
                WorldAssetRoot(tank_asset),
            ))
            .id();
        let enemy_asset: Handle<WorldAsset> = app
            .world()
            .resource::<AssetServer>()
            .load("tank/enemy/default/body.vox");
        let enemy = app
            .world_mut()
            .spawn((
                WorldAssetRoot(enemy_asset.clone()),
                crate::track::ENEMY_TRACKS,
                EnemyTank,
                Transform::from_xyz(15.0, crate::enemy::ENEMY_SIZE.y / 2.0, 0.0),
            ))
            .id();
        // 真实资源异步加载；这里使用现实时间限制等待，避免失败时测试无限循环。
        let mut terrain_surface = app
            .world_mut()
            .query_filtered::<Entity, With<crate::ground::TerrainSurface>>();
        let mut terrain_chunks = app
            .world_mut()
            .query_filtered::<Entity, With<crate::ground::TerrainChunk>>();
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            app.update();
            let world = app.world();
            let assets = world.resource::<AssetServer>();
            let ready = world.get::<WorldInstance>(tank).is_some_and(|instance| {
                world
                    .resource::<WorldInstanceSpawner>()
                    .instance_is_ready(**instance)
            });
            let barrel_ready = world
                .iter_entities()
                .filter(|e| {
                    e.contains::<WorldAssetRoot>()
                        && e.contains::<ChildOf>()
                        && !e.contains::<crate::track::TracksValidated>()
                })
                .all(|e| {
                    e.get::<WorldInstance>().is_some_and(|i| {
                        world
                            .resource::<WorldInstanceSpawner>()
                            .instance_is_ready(**i)
                    })
                });
            if ready
                && barrel_ready
                && world.get::<crate::track::TracksValidated>(tank).is_some()
                && assets.is_loaded_with_dependencies(&world.resource::<Gun>().bullet_scene)
                && assets.is_loaded_with_dependencies(&enemy_asset)
                && world.get::<crate::track::TracksValidated>(enemy).is_some()
                && world.get::<WorldInstance>(enemy).is_some_and(|instance| {
                    world
                        .resource::<WorldInstanceSpawner>()
                        .instance_is_ready(**instance)
                })
                && terrain_surface.iter(world).next().is_some()
            {
                break;
            }
            assert!(Instant::now() < deadline, "坦克、子弹或地板资源未能就绪");
            std::thread::sleep(Duration::from_millis(1));
        }

        let (_, enemy_minimum, enemy_maximum) = scene_bounds(app.world(), enemy);
        assert!((enemy_maximum - enemy_minimum).abs_diff_eq(Vec3::new(9.0, 2.6, 3.4), 1e-4));
        // 炮管和所有轮相位的实例也必须随敌人根注销。
        let mut pending = vec![enemy];
        let mut enemy_instances = Vec::new();
        while let Some(root) = pending.pop() {
            if let Some(children) = app.world().get::<Children>(root) {
                pending.extend(children.iter());
            }
            if let Some(instance) = app.world().get::<WorldInstance>(root) {
                enemy_instances.push(**instance);
            }
        }
        assert_eq!(enemy_instances.len(), 20);
        assert!(enemy_minimum.y.abs() < 1e-5, "敌人履带应贴合 Y=0 地面");

        app.world_mut().despawn(enemy);
        app.update();
        assert!(enemy_instances.into_iter().all(|instance| {
            !app.world()
                .resource::<WorldInstanceSpawner>()
                .instance_is_ready(instance)
        }));

        let (_, tank_minimum, tank_maximum) = scene_bounds(app.world(), tank);
        // 三包组合必须覆盖真实尺寸；底部与两侧由履带补全；炮管超出碰撞框是预期行为。
        assert!(
            (tank_maximum - tank_minimum).abs_diff_eq(Vec3::new(10.5, 2.6, 3.6), 1e-4),
            "车身与炮管组合尺寸：{tank_minimum:?}～{tank_maximum:?}"
        );
        assert!((MUZZLE_POSITION.x + BULLET_SIZE / 2.0 - tank_minimum.x).abs() < 1e-4);
        assert!(tank_minimum.x < -crate::driving::TANK_SIZE.x / 2.0);

        // 多色车身仍须通过 UV 采样到生成器中的三种装甲色，不能退化成单色材质。
        let armor_colors = [[83, 99, 55, 255], [108, 121, 72, 255], [58, 70, 40, 255]];
        let mut sampled_colors = std::collections::BTreeSet::new();
        let mut pending = vec![tank];
        while let Some(entity) = pending.pop() {
            let world = app.world();
            if let Some(children) = world.get::<Children>(entity) {
                pending.extend(children.iter());
            }
            let Some(mesh) = world.get::<Mesh3d>(entity) else {
                continue;
            };
            let material = world
                .get::<MeshMaterial3d<StandardMaterial>>(entity)
                .unwrap();
            let material = world
                .resource::<Assets<StandardMaterial>>()
                .get(&material.0)
                .unwrap();
            let Some(texture) = &material.base_color_texture else {
                continue;
            };
            assert_eq!(material.base_color, Color::WHITE);
            let atlas = world.resource::<Assets<Image>>().get(texture).unwrap();
            assert_eq!(
                atlas.texture_descriptor.format,
                bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb
            );
            let pixels = atlas.data.as_ref().expect("调色板像素必须可读");
            let mesh = world.resource::<Assets<Mesh>>().get(&mesh.0).unwrap();
            let Some(bevy::mesh::VertexAttributeValues::Float32x2(uvs)) =
                mesh.attribute(Mesh::ATTRIBUTE_UV_0)
            else {
                panic!("多色网格必须提供二维 UV");
            };
            assert!(!uvs.is_empty());
            let size = atlas.texture_descriptor.size;
            for &[u, v] in uvs {
                assert!(
                    u.is_finite()
                        && v.is_finite()
                        && (0.0..=1.0).contains(&u)
                        && (0.0..=1.0).contains(&v)
                );
                let x = ((u * size.width as f32) as u32).min(size.width - 1);
                let y = ((v * size.height as f32) as u32).min(size.height - 1);
                let offset = ((y * size.width + x) * 4) as usize;
                sampled_colors.insert(<[u8; 4]>::try_from(&pixels[offset..offset + 4]).unwrap());
            }
        }
        for color in armor_colors {
            assert!(
                sampled_colors.contains(&color),
                "UV 未采样到装甲色 {color:?}：{sampled_colors:?}"
            );
        }

        // 地形已改为程序化编译：两张 1200×1200 字节图生成 10×10 块世界坐标网格与一个高度场碰撞体。
        assert!(
            terrain_surface.iter(app.world()).next().is_some(),
            "高度场物理表面应已铺设"
        );
        let mut merged = (
            0usize,
            Vec3::splat(f32::INFINITY),
            Vec3::splat(f32::NEG_INFINITY),
        );
        for chunk in terrain_chunks.iter(app.world()) {
            let (count, minimum, maximum) = scene_bounds(app.world(), chunk);
            merged = (
                merged.0 + count,
                merged.1.min(minimum),
                merged.2.max(maximum),
            );
        }
        let (mesh_count, minimum, maximum) = merged;
        assert_eq!(mesh_count, 100, "地形应分成 10×10 块网格");
        let half_side = crate::ground::FIELD_SIDE / 2.0;
        assert!(
            minimum.x <= -half_side && maximum.x >= half_side,
            "地形应铺满 X 方向 {half_side} 米：{minimum:?}～{maximum:?}"
        );
        assert!(
            minimum.z <= -half_side && maximum.z >= half_side,
            "地形应铺满 Z 方向 {half_side} 米：{minimum:?}～{maximum:?}"
        );
        assert!(
            minimum.y >= -1.001 && maximum.y <= 1.001,
            "地形起伏应控制在 ±1 米：{minimum:?}～{maximum:?}"
        );

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        app.update();
        let bullet = app
            .world_mut()
            .query_filtered::<Entity, With<Bullet>>()
            .single(app.world())
            .unwrap();
        assert_eq!(
            app.world().get::<Transform>(bullet).unwrap().translation,
            MUZZLE_POSITION
        );
        assert!(app.world().get::<ChildOf>(bullet).is_none());
        // 先松开 0.125 秒再重按，验证实际输入路径也不能绕过发射间隔。
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            0.125,
        )));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::Space);
        app.update();
        let instance = **app.world().get::<WorldInstance>(bullet).unwrap();
        assert!(
            app.world()
                .resource::<WorldInstanceSpawner>()
                .instance_is_ready(instance)
        );
        let (_, bullet_minimum, bullet_maximum) = scene_bounds(app.world(), bullet);
        assert!((bullet_maximum - bullet_minimum).abs_diff_eq(Vec3::splat(0.4), 1e-5));
        let bullet_center = (bullet_minimum + bullet_maximum) / 2.0;
        assert!(bullet_center.abs_diff_eq(
            app.world().get::<Transform>(bullet).unwrap().translation,
            1e-5,
        ));
        // 保留场景直接子实体的编号，用于确认销毁根节点时一并清理层级。
        let descendants: Vec<_> = app
            .world()
            .get::<Children>(bullet)
            .unwrap()
            .iter()
            .collect();
        let mut meshes = app
            .world_mut()
            .query::<(&Mesh3d, &MeshMaterial3d<StandardMaterial>)>();
        // 按 0.2 的半尺寸定位子弹网格，区别于同场景中的坦克网格。
        let (_, material) = meshes
            .iter(app.world())
            .find(|(mesh, _)| {
                app.world()
                    .resource::<Assets<Mesh>>()
                    .get(&mesh.0)
                    .unwrap()
                    .get_aabb()
                    .is_some_and(|aabb| {
                        aabb.half_extents.abs_diff_eq(Vec3::splat(0.2).into(), 1e-5)
                    })
            })
            .expect("应加载边长 0.4 的子弹网格");
        let material = app
            .world()
            .resource::<Assets<StandardMaterial>>()
            .get(&material.0)
            .unwrap();
        assert!(material.perceptual_roughness >= 0.8);
        assert_eq!(material.metallic, 0.0);
        // 0.23 将单色模型直接存为材质颜色，不再生成调色板纹理。
        assert!(material.base_color_texture.is_none());
        assert_eq!(
            material.base_color.to_srgba().to_u8_array(),
            [139, 90, 43, 255]
        );

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        app.update();
        assert_eq!(
            app.world_mut().query::<&Bullet>().iter(app.world()).count(),
            1
        );
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            0.25,
        )));
        app.update();
        assert_eq!(
            app.world_mut().query::<&Bullet>().iter(app.world()).count(),
            2
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::Space);
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs(50)));
        app.update();
        assert_eq!(
            app.world_mut().query::<&Bullet>().iter(app.world()).count(),
            0
        );
        assert!(app.world().get_entity(tank).is_ok());
        assert!(
            descendants
                .iter()
                .all(|&entity| app.world().get_entity(entity).is_err())
        );
        assert!(
            !app.world()
                .resource::<WorldInstanceSpawner>()
                .instance_is_ready(instance)
        );
        // 玩家销毁后，车身、炮管、六个履带与十四个轮实例都必须注销。
        let mut scene_roots = app.world_mut().query::<&WorldInstance>();
        let player_instances: Vec<_> = scene_roots.iter(app.world()).map(|i| **i).collect();
        assert_eq!(player_instances.len(), 22);
        app.world_mut().despawn(tank);
        app.update();
        assert!(player_instances.into_iter().all(|i| {
            !app.world()
                .resource::<WorldInstanceSpawner>()
                .instance_is_ready(i)
        }));
        assert_eq!(
            app.world_mut()
                .query::<&PlayerBarrel>()
                .iter(app.world())
                .count(),
            0
        );
    }
}

//! 坦克空格连发：按炮口世界方向发射，沿飞行路径检测敌人，命中或射程耗尽时清理。

use crate::enemy::EnemyTank;
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
/// 子弹出生中心在坦克根节点下的局部坐标；炮口前方为局部 −X。
/// 炮口端面为 4×4 格；子弹中心向外偏移半个边长，避免与炮管重叠。
// shortcut: 坐标按当前 tank.vox 标定，修改模型或独立转动炮塔时改用炮口节点。
const MUZZLE_POSITION: Vec3 = Vec3::new(-1.8, 0.45, -0.1);

/// 标记坦克场景根节点，供发射系统定位炮口的世界变换。
#[derive(Component)]
pub(super) struct Tank;

/// 子弹根节点的飞行状态；独立于坦克场景层级，发射后保持原方向。
#[derive(Component)]
struct Bullet {
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

/// 注册子弹资源加载、空格发射与飞行清理系统。
pub(super) struct FiringPlugin;

impl Plugin for FiringPlugin {
    /// 在变换传播完成后发射，确保读取到坦克本帧的世界位置和方向。
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_bullet)
            .add_systems(Update, (move_bullets, report_bullet))
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
) {
    // 场景资源加载完成不等于实体已实例化，两者都就绪后才能确定发射位置。
    let tank = tanks.single().ok().filter(|&(_, instance, root)| {
        instances.instance_is_ready(**instance)
            && assets.is_loaded_with_dependencies(&root.0)
            && assets.is_loaded_with_dependencies(&gun.bullet_scene)
    });
    if !gun.cadence.tick(
        keys.pressed(KeyCode::Space) && tank.is_some(),
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
    enemies: Query<Entity, With<EnemyTank>>,
    mut bullets: Query<(Entity, &mut Transform, &mut Bullet)>,
) {
    // 本帧前进的世界距离，按游戏时间计算，单位：米。
    let step = BULLET_SPEED * time.delta_secs();
    let shape = Collider::cuboid(BULLET_SIZE, BULLET_SIZE, BULLET_SIZE);
    // Commands 在系统结束后才删除实体，同帧后续弹体必须主动跳过已击毁目标。
    let mut destroyed = Vec::new();
    let enemy_count = enemies.iter().count();
    for (entity, mut transform, mut bullet) in &mut bullets {
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
                    .compute_aabb()
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
        assert!(position.abs_diff_eq(Vec3::new(2.9, 2.45, 2.8), 1e-5));
        assert!(direction.abs_diff_eq(Vec3::Z, 1e-5));
    }

    /// 用真实物理宽阶段预热目标，再以独立游戏时间运行子弹系统。
    fn hit_test_app(targets: &[(Vec3, f32)]) -> (App, Vec<Entity>) {
        use avian3d::prelude::*;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugins::default()))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
                1.0 / 64.0,
            )));
        app.finish();
        app.cleanup();
        let enemies = targets
            .iter()
            .map(|&(position, yaw)| {
                app.world_mut()
                    .spawn((
                        crate::enemy::EnemyTank,
                        RigidBody::Static,
                        Collider::cuboid(3.4, 1.4, 2.6),
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
        let (mut app, enemies) = hit_test_app(&[(Vec3::new(-6.0, 0.7, 0.0), 0.8)]);
        let miss = test_bullet(&mut app, Vec3::new(0.0, 1.2, 3.0), Vec3::NEG_X, 0.0);
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

    /// 用固定半秒步长验证飞行速度，以及 50 秒后对子弹及其子实体的清理。
    #[test]
    fn bullets_move_independently_and_despawn_with_their_children_at_range() {
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(Duration::from_secs_f32(0.5));
        app.insert_resource(time)
            .init_resource::<avian3d::collider_tree::ColliderTrees>()
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
        for _ in 2..99 {
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
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            0.25,
        )));
        // 允许测试直接推进 50 秒游戏时间，避免被默认的单帧时间上限截断。
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .set_max_delta(Duration::from_secs(60));
        let tank_asset = app.world().resource::<AssetServer>().load("tank.vox");
        let tank = app
            .world_mut()
            .spawn((Tank, WorldAssetRoot(tank_asset)))
            .id();
        let enemy_asset: Handle<WorldAsset> =
            app.world().resource::<AssetServer>().load("enemy_tank.vox");
        let enemy = app
            .world_mut()
            .spawn((
                WorldAssetRoot(enemy_asset.clone()),
                Transform::from_xyz(10.0, 0.7, 0.0),
            ))
            .id();
        // 真实资源异步加载；这里使用现实时间限制等待，避免失败时测试无限循环。
        let ground_asset: Handle<WorldAsset> =
            app.world().resource::<AssetServer>().load("ground.vox");
        let mut ground_instances = app.world_mut().query::<(&WorldAssetRoot, &WorldInstance)>();
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
            if ready
                && assets.is_loaded_with_dependencies(&world.resource::<Gun>().bullet_scene)
                && assets.is_loaded_with_dependencies(&enemy_asset)
                && world.get::<WorldInstance>(enemy).is_some_and(|instance| {
                    world
                        .resource::<WorldInstanceSpawner>()
                        .instance_is_ready(**instance)
                })
                && assets.is_loaded_with_dependencies(&ground_asset)
                && ground_instances.iter(world).any(|(root, instance)| {
                    root.0 == ground_asset
                        && world
                            .resource::<WorldInstanceSpawner>()
                            .instance_is_ready(**instance)
                })
            {
                break;
            }
            assert!(Instant::now() < deadline, "坦克、子弹或地板资源未能就绪");
            std::thread::sleep(Duration::from_millis(1));
        }

        let (_, enemy_minimum, enemy_maximum) = scene_bounds(app.world(), enemy);
        assert!((enemy_maximum - enemy_minimum).abs_diff_eq(crate::enemy::ENEMY_SIZE, 1e-5));
        assert!(enemy_minimum.y.abs() < 1e-5, "敌人履带应贴合 Y=0 地面");
        let enemy_instance = **app.world().get::<WorldInstance>(enemy).unwrap();
        app.world_mut().despawn(enemy);
        app.update();
        assert!(
            !app.world()
                .resource::<WorldInstanceSpawner>()
                .instance_is_ready(enemy_instance)
        );

        let (_, tank_minimum, tank_maximum) = scene_bounds(app.world(), tank);
        assert!(
            (tank_maximum - tank_minimum).abs_diff_eq(Vec3::new(3.2, 1.5, 2.4), 1e-5),
            "坦克尺寸与碰撞体应一致：{tank_minimum:?}～{tank_maximum:?}"
        );
        // 当前炮管端面在 X=-1.6，弹体半长为 0.2 米，出生时恰好位于炮口外侧。
        assert!((MUZZLE_POSITION.x + 0.2 - tank_minimum.x).abs() < 1e-5);

        // 检查一个场景根下的 16 个模型，以及实际世界包围盒；覆盖定位、缩放和竖直偏移。
        let mut roots = app.world_mut().query::<(Entity, &WorldAssetRoot)>();
        let ground_roots: Vec<_> = roots
            .iter(app.world())
            .filter(|(_, root)| root.0 == ground_asset)
            .map(|(entity, _)| entity)
            .collect();
        assert_eq!(ground_roots.len(), 1);
        let (mesh_count, minimum, maximum) = scene_bounds(app.world(), ground_roots[0]);
        let half_side = 4000.0_f32.sqrt() / 2.0;
        assert_eq!(mesh_count, 16);
        assert!(
            minimum.abs_diff_eq(Vec3::new(-half_side, -0.4, -half_side), 1e-4),
            "地板下界：{minimum:?}"
        );
        assert!(
            maximum.abs_diff_eq(Vec3::new(half_side, 0.1, half_side), 1e-4),
            "地板上界：{maximum:?}"
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
                    .compute_aabb()
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
        // 材质颜色存于调色板图集，检查其像素以确认褐色确实进入加载后的资源。
        let atlas = app
            .world()
            .resource::<Assets<Image>>()
            .get(material.base_color_texture.as_ref().unwrap())
            .unwrap();
        assert_eq!(&atlas.data.as_ref().unwrap()[..4], &[139, 90, 43, 255]);

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
    }
}

//! 坦克空格连发：预加载子弹场景，按炮口世界方向生成子弹，并在射程耗尽时清理。

use bevy::{
    asset::LoadState,
    prelude::*,
    transform::TransformSystems,
    world_serialization::{WorldAsset, WorldInstance, WorldInstanceSpawner},
};

/// 点按与长按共用的最小发射间隔，单位：秒。
const FIRE_INTERVAL: f32 = 0.5;
/// 子弹的匀速飞行速度，单位：世界单位/秒。
const BULLET_SPEED: f32 = 20.0;
/// 从出生点累计计算的最大飞行距离，单位：世界单位。
const BULLET_RANGE: f32 = 1000.0;
/// 子弹出生中心在坦克根节点下的局部坐标；炮口前方为局部 −X。
/// 炮口端面为 4×4 格；子弹中心向外偏移半个边长，避免与炮管重叠。
// shortcut: 坐标按当前 tank.vox 标定，修改模型或独立转动炮塔时改用炮口节点。
const MUZZLE_POSITION: Vec3 = Vec3::new(-0.9, 0.225, -0.05);

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

/// 按游戏时间推进子弹；累计距离达到射程时，连同场景子实体一起销毁。
fn move_bullets(
    mut commands: Commands,
    time: Res<Time>,
    mut bullets: Query<(Entity, &mut Transform, &mut Bullet)>,
) {
    // 本帧前进的世界距离，按时间计算以保持不同帧率下的速度一致。
    let step = BULLET_SPEED * time.delta_secs();
    for (entity, mut transform, mut bullet) in &mut bullets {
        bullet.distance += step;
        if bullet.distance >= BULLET_RANGE {
            debug!("子弹达到射程，销毁：{entity}");
            commands.entity(entity).despawn();
        } else {
            transform.translation += bullet.direction * step;
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
        assert!(position.abs_diff_eq(Vec3::new(2.95, 2.225, 1.9), 1e-5));
        assert!(direction.abs_diff_eq(Vec3::Z, 1e-5));
    }

    /// 用固定半秒步长验证飞行速度，以及 50 秒后对子弹及其子实体的清理。
    #[test]
    fn bullets_move_independently_and_despawn_with_their_children_at_range() {
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(Duration::from_secs_f32(0.5));
        app.insert_resource(time).add_systems(Update, move_bullets);
        let bullet = app
            .world_mut()
            .spawn((
                Bullet {
                    direction: Vec3::NEG_X,
                    distance: 0.0,
                },
                Transform::from_xyz(-0.9, 0.225, -0.05),
            ))
            .id();
        let child = app.world_mut().spawn(ChildOf(bullet)).id();
        app.update();
        assert!(
            app.world()
                .get::<Transform>(bullet)
                .unwrap()
                .translation
                .abs_diff_eq(Vec3::new(-10.9, 0.225, -0.05), 1e-5)
        );
        assert_eq!(app.world().get::<Bullet>(bullet).unwrap().distance, 10.0);
        for _ in 1..99 {
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
                    voxel_size: 0.05,
                    ..default()
                }),
            },
            FiringPlugin,
        ))
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
        // 真实资源异步加载；这里使用现实时间限制等待，避免失败时测试无限循环。
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            app.update();
            let world = app.world();
            let assets = world.resource::<AssetServer>();
            let ready = world.get::<WorldInstance>(tank).is_some_and(|instance| {
                world
                    .resource::<WorldInstanceSpawner>()
                    .instance_is_ready(**instance)
            });
            if ready && assets.is_loaded_with_dependencies(&world.resource::<Gun>().bullet_scene) {
                break;
            }
            assert!(Instant::now() < deadline, "坦克或子弹资源未能就绪");
            std::thread::sleep(Duration::from_millis(1));
        }

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
        // 按 0.1 的半尺寸定位子弹网格，区别于同场景中的坦克网格。
        let (_, material) = meshes
            .iter(app.world())
            .find(|(mesh, _)| {
                app.world()
                    .resource::<Assets<Mesh>>()
                    .get(&mesh.0)
                    .unwrap()
                    .compute_aabb()
                    .is_some_and(|aabb| {
                        aabb.half_extents.abs_diff_eq(Vec3::splat(0.1).into(), 1e-5)
                    })
            })
            .expect("应加载边长 0.2 的子弹网格");
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

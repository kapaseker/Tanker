//! 开局生成静止敌人靶车；独立于玩家 Tank 标记。
//!
//! 敌人从地形上方落下，由重力沉降到地形表面（动态刚体）；
//! 测试中的 Static 固定靶见 [`mod@tests`]。

use bevy::{asset::LoadState, prelude::*, world_serialization::WorldAsset};

use crate::tank::{driving, track, visual};

/// 非炮管碰撞尺寸（长、高、宽），单位米；尺寸契约统一由敌人模型推导，
/// 高度复用玩家底盘的权威值，避免与玩家驾驶刚体出现两套数据源。
pub(crate) const ENEMY_SIZE: Vec3 = Vec3::from_array(crate::tank::model::enemy::COLLISION_SIZE);
/// 单轮敌人数；被击毁后不补充。
pub(crate) const ENEMY_COUNT: usize = 10;
/// 交战区半径，单位：米；出生中心在该半径内均匀采样，离 300 米场地边缘留有充足余量。
const SPAWN_LIMIT: f32 = 120.0;
/// 水平外接圆之间保留的净空，单位米；覆盖随机朝向，避免出生时接触。
const SPAWN_GAP: f32 = 1.0;

/// 两种底盘的最小中心距，外接圆保证任意偏航均有净空。
fn spawn_clearance(a: Vec3, b: Vec3) -> f32 {
    a.xz().length() / 2.0 + b.xz().length() / 2.0 + SPAWN_GAP
}
/// 拒绝采样总次数上限，防止地图或间距变动后卡住启动。
const SPAWN_ATTEMPTS: usize = 1000;
/// 出生下落起点高度，单位：米；高于纯平地面加半车高（1.3），
/// 落体由物理引擎沉降到地形表面，无需采样高度图。
const SPAWN_DROP_Y: f32 = 3.0;

/// 仅标记敌人根实体，供炮弹筛选碰撞目标及统计剩余数量。
#[derive(Component)]
pub(crate) struct EnemyTank;

/// 敌人被击毁的消息；位置为被毁车体中心的世界坐标，供表现层（爆炸）消费。
/// 消息类型放在敌人域：射击系统是生产方，爆炸表现是消费方，后者无需感知射击。
#[derive(Message)]
pub(crate) struct EnemyDestroyed {
    pub(super) position: Vec3,
}

/// 保持共享敌人资源存活，并汇报依赖加载结果。
#[derive(Resource)]
struct EnemyAsset(Handle<WorldAsset>);

/// 注册开局随机放置和一次性加载状态汇报。
pub(crate) struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        // 生成等待 TerrainReady（Update 轮询），确保下落前地形碰撞体已存在。
        app.add_systems(Update, (spawn_enemies, spawn_enemy_barrels, report_enemy));
    }
}

/// 敌人根加入时零偏移安装独立炮管；无碰撞体，不赋予敌人开火能力。
pub(super) fn spawn_enemy_barrels(
    mut commands: Commands,
    assets: Res<AssetServer>,
    enemies: Query<Entity, Added<EnemyTank>>,
) {
    for enemy in &enemies {
        let barrel = visual::mount_barrel(&mut commands, &assets, enemy, visual::ENEMY_BARREL.path);
        commands.entity(barrel).insert(Name::new("enemy_barrel"));
    }
}

/// 返回完整一组出生姿态；失败时不生成部分敌人。玩家坐标只取水平分量。
fn spawn_transforms(rng: &mut fastrand::Rng, player: Vec2) -> Result<Vec<Transform>, &'static str> {
    let mut transforms: Vec<Transform> = Vec::with_capacity(ENEMY_COUNT);
    for _ in 0..SPAWN_ATTEMPTS {
        let position = Vec2::new(
            (rng.f32() * 2.0 - 1.0) * SPAWN_LIMIT,
            (rng.f32() * 2.0 - 1.0) * SPAWN_LIMIT,
        );
        if position.distance(player) < spawn_clearance(ENEMY_SIZE, driving::TANK_SIZE)
            || position.distance(super::obstacle::POSITION.xz())
                < spawn_clearance(ENEMY_SIZE, super::obstacle::SIZE)
            || transforms.iter().any(|other| {
                position.distance(other.translation.xz()) < spawn_clearance(ENEMY_SIZE, ENEMY_SIZE)
            })
        {
            continue;
        }
        transforms.push(
            Transform::from_xyz(position.x, SPAWN_DROP_Y, position.y)
                .with_rotation(Quat::from_rotation_y(rng.f32() * std::f32::consts::TAU)),
        );
        if transforms.len() == ENEMY_COUNT {
            return Ok(transforms);
        }
    }
    Err("在采样次数上限内无法放下全部敌人，请检查地图与间距设置")
}

/// 与玩家共用驾驶刚体和逐车输入状态，仅碰撞体按敌人尺寸标定。
/// 零输入控制器停止水平驱动；保留重力和接触求解，允许碰撞导致短暂位移。
// shortcut: 整车圆角框包含装甲间空隙，需要逐部件命中时再拆分；炮管不参与碰撞。
fn enemy_body() -> impl Bundle {
    driving::body_for_size(ENEMY_SIZE)
}

/// 清除本局生成标志；共享资产缓存仍由 AssetServer 复用。
pub(crate) fn reset_session(world: &mut World) {
    world.remove_resource::<EnemyAsset>();
}

/// 等地形铺设完成后随机放置敌人；玩家在地图中心出生。
/// 随机来源为启动注入的 `SpawnConfig`（业务不读验证入口的 Options）。
fn spawn_enemies(
    mut commands: Commands,
    spawned: Option<Res<EnemyAsset>>,
    ready: Option<Res<crate::terrain::TerrainReady>>,
    assets: Res<AssetServer>,
    config: Res<crate::gameplay::session::SpawnConfig>,
) {
    // 地形图异步加载，碰撞体就绪前下落会穿透场地，故等待就绪信号。
    if spawned.is_some() || ready.is_none() {
        return;
    }
    let mut rng = match config.fixed_seed {
        Some(seed) => fastrand::Rng::with_seed(seed),
        None => fastrand::Rng::new(),
    };
    let transforms = match spawn_transforms(&mut rng, Vec2::ZERO) {
        Ok(transforms) => transforms,
        Err(message) => {
            error!("敌人生成失败：{message}");
            return;
        }
    };
    let scene = assets.load("tank/bodies/enemy/carapace/body.vox");
    for transform in transforms {
        commands.spawn((
            EnemyTank,
            crate::gameplay::session::SessionEntity,
            track::ENEMY_TRACKS,
            WorldAssetRoot(scene.clone()),
            enemy_body(),
            transform,
        ));
    }
    commands.insert_resource(EnemyAsset(scene));
    info!(
        "已随机放置 {ENEMY_COUNT} 辆敌人坦克；碰撞尺寸 {ENEMY_SIZE:?} 米，炮管 {:.1} 米、轴高 {:.1} 米",
        (crate::tank::model::enemy::BARREL_X.1 - crate::tank::model::enemy::BARREL_X.0) as f32
            * crate::tank::model::REFERENCE_VOXEL_SIZE,
        crate::tank::model::enemy::BORE_HEIGHT * crate::tank::model::REFERENCE_VOXEL_SIZE
    );
}

/// 加载失败或包含依赖的加载成功各只汇报一次。
fn report_enemy(
    scene: Option<Res<EnemyAsset>>,
    assets: Res<AssetServer>,
    mut reported: Local<bool>,
) {
    let Some(scene) = scene else { return };
    if *reported {
        return;
    }
    match assets.get_load_state(&scene.0) {
        Some(LoadState::Failed(error)) => {
            error!("敌人资源加载失败：{error}");
            *reported = true;
        }
        _ if assets.is_loaded_with_dependencies(&scene.0) => {
            info!("vox 场景加载完成：assets/tank/bodies/enemy/carapace/body.vox");
            *reported = true;
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use avian3d::prelude::*;

    /// 玩家撞上静止敌人会停下，击毁后原来的通路恢复。
    #[test]
    fn static_enemy_blocks_driving_until_removed() {
        use crate::gameplay::player::Tank;
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            PhysicsPlugins::default(),
            driving::DrivingPlugin,
            crate::gameplay::player::PlayerPlugin,
        ))
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / 64.0,
        )))
        .add_systems(Startup, |mut commands: Commands| {
            crate::terrain::spawn_arena(&mut commands)
        });
        app.finish();
        app.cleanup();
        let player = app
            .world_mut()
            .spawn((
                Tank,
                driving::tank_body(),
                crate::tank::track::PLAYER_TRACKS,
                Transform::from_xyz(0.0, 0.75, 0.0),
            ))
            .id();
        let obstacle_transform = Transform::from_xyz(-8.0, ENEMY_SIZE.y / 2.0, 0.0);
        let enemy = app
            .world_mut()
            .spawn((
                EnemyTank,
                RigidBody::Static,
                Collider::cuboid(ENEMY_SIZE.x, ENEMY_SIZE.y, ENEMY_SIZE.z),
                Friction::ZERO,
                obstacle_transform,
            ))
            .id();
        for _ in 0..10 {
            app.update();
        }
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowUp);
        for _ in 0..128 {
            app.update();
        }
        let stopped = app.world().get::<Transform>(player).unwrap().translation;
        assert!(
            (stopped.x
                - (obstacle_transform.translation.x + (driving::TANK_SIZE.x + ENEMY_SIZE.x) / 2.0))
                .abs()
                < 0.05,
            "应在敌人前停车：{stopped:?}"
        );
        assert_eq!(
            *app.world().get::<Transform>(enemy).unwrap(),
            obstacle_transform
        );
        app.world_mut().despawn(enemy);
        for _ in 0..64 {
            app.update();
        }
        assert!(app.world().get::<Transform>(player).unwrap().translation.x < stopped.x - 5.0);
    }

    /// 敌人与玩家同高；真实圆角碰撞体在不同偏航下仍遵守非炮管边界。
    #[test]
    fn enemy_collision_matches_player_height_and_rotated_contract() {
        use avian3d::prelude::Collider;
        assert_eq!(ENEMY_SIZE.y, driving::TANK_SIZE.y);
        assert_eq!(crate::tank::model::enemy::CANVAS.2, crate::tank::model::player::CANVAS.2);
        assert!(ENEMY_SIZE.abs_diff_eq(Vec3::new(6.6, 2.6, 3.4), 1e-5));
        let mut world = World::new();
        let entity = world.spawn(enemy_body()).id();
        let collider = world.get::<Collider>(entity).unwrap();
        for yaw in [
            0.0,
            std::f32::consts::FRAC_PI_4,
            std::f32::consts::FRAC_PI_2,
        ] {
            let rotation = Quat::from_rotation_y(yaw);
            let core = ENEMY_SIZE / 2.0 - Vec3::splat(0.2);
            let extent = (rotation * Vec3::X * core.x).abs()
                + Vec3::Y * core.y
                + (rotation * Vec3::Z * core.z).abs()
                + Vec3::splat(0.2);
            let actual = collider.aabb(Vec3::ZERO, rotation, 0.0);
            assert!(actual.min.abs_diff_eq(-extent, 1e-5));
            assert!(actual.max.abs_diff_eq(extent, 1e-5));
        }
    }

    /// 多个固定种子验证完整数量、安全距离及旋转后每个角都在地图内。
    #[test]
    fn random_spawns_respect_clearance_and_rotated_bounds() {
        for seed in 0..100 {
            let transforms =
                spawn_transforms(&mut fastrand::Rng::with_seed(seed), Vec2::ZERO).unwrap();
            assert_eq!(transforms.len(), ENEMY_COUNT);
            for (index, transform) in transforms.iter().enumerate() {
                let p = transform.translation.xz();
                assert!(p.length() >= spawn_clearance(ENEMY_SIZE, driving::TANK_SIZE));
                assert!(
                    p.distance(super::super::obstacle::POSITION.xz())
                        >= spawn_clearance(ENEMY_SIZE, super::super::obstacle::SIZE)
                );
                assert_eq!(transform.translation.y, SPAWN_DROP_Y);
                for other in &transforms[..index] {
                    assert!(
                        p.distance(other.translation.xz())
                            >= spawn_clearance(ENEMY_SIZE, ENEMY_SIZE)
                    );
                }
                for x in [-1.0, 1.0] {
                    for z in [-1.0, 1.0] {
                        let corner =
                            transform.transform_point(ENEMY_SIZE * Vec3::new(x, 0.0, z) / 2.0);
                        let half_side = crate::terrain::FIELD_SIDE / 2.0;
                        assert!(corner.x.abs() < half_side);
                        assert!(corner.z.abs() < half_side);
                    }
                }
            }
        }
        let a = spawn_transforms(&mut fastrand::Rng::with_seed(42), Vec2::ZERO).unwrap();
        let b = spawn_transforms(&mut fastrand::Rng::with_seed(43), Vec2::ZERO).unwrap();
        assert_ne!(a, b);
    }
}

//! 开局生成静止敌人靶车；独立于玩家 Tank 标记。
//!
//! 敌人从地形上方落下，由重力沉降到地形表面（动态刚体）；
//! 测试中的 Static 固定靶见 [`mod@tests`]。

use avian3d::prelude::*;
use bevy::{asset::LoadState, prelude::*, world_serialization::WorldAsset};

/// 敌人模型的 X 长、Y 高、Z 宽，单位：米，与生成器的 34×26×14 格对应。
pub(super) const ENEMY_SIZE: Vec3 = Vec3::new(3.4, 1.4, 2.6);
/// 单轮敌人数；被击毁后不补充。
const ENEMY_COUNT: usize = 10;
/// 交战区半径，单位：米；出生中心在该半径内均匀采样，离 300 米场地边缘留有充足余量。
const SPAWN_LIMIT: f32 = 120.0;
/// 敌人与玩家出生点的最小水平中心距离，单位：米。
const PLAYER_CLEARANCE: f32 = 10.0;
/// 敌人之间的最小水平中心距离，单位：米。
const ENEMY_CLEARANCE: f32 = 6.0;
/// 拒绝采样总次数上限，防止地图或间距变动后卡住启动。
const SPAWN_ATTEMPTS: usize = 1000;
/// 出生下落起点高度，单位：米；高于全场最高地形（+1.0）加半车高（0.7），
/// 落体由物理引擎沉降到地形表面，无需采样高度图。
const SPAWN_DROP_Y: f32 = 3.0;

/// 仅标记敌人根实体，供炮弹筛选碰撞目标及统计剩余数量。
#[derive(Component)]
pub(super) struct EnemyTank;

/// 敌人被击毁的消息；位置为被毁车体中心的世界坐标，供表现层（爆炸）消费。
/// 消息类型放在敌人域：射击系统是生产方，爆炸表现是消费方，后者无需感知射击。
#[derive(Message)]
pub(super) struct EnemyDestroyed {
    pub(super) position: Vec3,
}

/// 保持共享敌人资源存活，并汇报依赖加载结果。
#[derive(Resource)]
struct EnemyAsset(Handle<WorldAsset>);

/// 注册开局随机放置和一次性加载状态汇报。
pub(super) struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        // 生成等待 TerrainReady（Update 轮询），确保下落前地形碰撞体已存在。
        app.add_systems(Update, (spawn_enemies, report_enemy));
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
        if position.distance(player) < PLAYER_CLEARANCE
            || transforms
                .iter()
                .any(|other| position.distance(other.translation.xz()) < ENEMY_CLEARANCE)
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

/// 敌人刚体：动态落地沉降到地形表面；与玩家 `tank_body()` 的差异均有理由：
/// 碰撞体按敌人模型标定（ENEMY_SIZE）；保留默认摩擦以在坡面站住（玩家零摩擦靠
/// 每帧覆写速度，敌人无速度控制器）；允许休眠保持静止省开销。
// shortcut: 整车箱形包含炮管旁的空隙，需要精确命中时拆分碰撞形状。
fn enemy_body() -> impl Bundle {
    (
        RigidBody::Dynamic,
        Collider::cuboid(ENEMY_SIZE.x, ENEMY_SIZE.y, ENEMY_SIZE.z),
        LockedAxes::new().lock_rotation_x().lock_rotation_z(),
        Friction::default(),
    )
}

/// 等地形铺设完成后随机放置敌人；玩家在地图中心出生。
fn spawn_enemies(
    mut commands: Commands,
    mut spawned: Local<bool>,
    ready: Option<Res<crate::ground::TerrainReady>>,
    assets: Res<AssetServer>,
) {
    // 地形图异步加载，高度场碰撞体就绪前下落会穿透场地，故等待就绪信号。
    if *spawned || ready.is_none() {
        return;
    }
    *spawned = true;
    let transforms = match spawn_transforms(&mut fastrand::Rng::new(), Vec2::ZERO) {
        Ok(transforms) => transforms,
        Err(message) => {
            error!("敌人生成失败：{message}");
            return;
        }
    };
    let scene = assets.load("tank/enemy/default/default.vox");
    for transform in transforms {
        commands.spawn((
            EnemyTank,
            WorldAssetRoot(scene.clone()),
            enemy_body(),
            transform,
        ));
    }
    commands.insert_resource(EnemyAsset(scene));
    info!("已随机放置 {ENEMY_COUNT} 辆敌人坦克");
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
            info!("vox 场景加载完成：assets/tank/enemy/default/default.vox");
            *reported = true;
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 玩家撞上静止敌人会停下，击毁后原来的通路恢复。
    #[test]
    fn static_enemy_blocks_driving_until_removed() {
        use crate::{driving, firing::Tank};
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            PhysicsPlugins::default(),
            driving::DrivingPlugin,
        ))
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / 64.0,
        )))
        .add_systems(Startup, |mut commands: Commands| {
            crate::ground::spawn_arena(&mut commands)
        });
        app.finish();
        app.cleanup();
        let player = app
            .world_mut()
            .spawn((
                Tank,
                driving::tank_body(),
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
            (stopped.x + 4.7).abs() < 0.05,
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
        assert!(app.world().get::<Transform>(player).unwrap().translation.x < -10.0);
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
                assert!(p.length() >= PLAYER_CLEARANCE);
                assert_eq!(transform.translation.y, SPAWN_DROP_Y);
                for other in &transforms[..index] {
                    assert!(p.distance(other.translation.xz()) >= ENEMY_CLEARANCE);
                }
                for x in [-1.0, 1.0] {
                    for z in [-1.0, 1.0] {
                        let corner =
                            transform.transform_point(ENEMY_SIZE * Vec3::new(x, 0.0, z) / 2.0);
                        let half_side = crate::ground::FIELD_SIDE / 2.0;
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

//! 履带多帧滚动动画：履带是独立 vox 资产（`assets/tank/tracks/<底盘ID>/<样式>/`），
//! 每侧 3 个相位变体节点（`track_l_0..2` / `track_r_0..2`，齿图案相位错位 1 格）。
//! 启动时以坦克子实体形式 spawn 履带包并直接持有变体句柄，运行时按左右履带
//! 线速度积分里程、切换变体可见性产生滚动感；原地转向时左右差速反向滚动。

use avian3d::prelude::*;
use bevy::camera::primitives::MeshAabb;
use bevy::prelude::*;

use crate::driving::{TANK_SIZE, TurnRate};
use crate::firing::Tank;

/// 履带齿距，单位：米（3 体素 × 0.1）。相位每推进一个齿距循环一轮。
const TRACK_PITCH: f32 = 0.3;
/// 相位数（与履带包变体节点数一致）
const TRACK_PHASES: i64 = 3;
/// 半履带距，单位：米（车宽 2.6 / 2），用于把转向角速度换算成两侧履带线速度差
const HALF_TRACK_WIDTH: f32 = 1.3;
/// 履带包资源路径（相对 `assets/`）：`tracks/<底盘ID>/<样式>/default.vox`。
/// 换履带 = 换这里的样式段；目录首层底盘 ID 必须与车体底盘匹配，见 `validate_tracks`。
const TRACKS_PATH: &str = "tank/tracks/player/lugged/default.vox";
/// 相位变体节点名，顺序 [左, 右] × [0, 1, 2]，与履带包场景图一致。
const VARIANT_NAMES: [[&str; TRACK_PHASES as usize]; 2] = [
    ["track_l_0", "track_l_1", "track_l_2"],
    ["track_r_0", "track_r_1", "track_r_2"],
];

/// 挂在玩家坦克根实体上的履带动画状态。
// ponytail: 当前单辆受控坦克；多坦克时 TrackAnim 仍按坦克逐辆持有，无需改结构。
#[derive(Component)]
pub(crate) struct TrackAnim {
    /// 左右两侧的相位变体实体：`variants[侧][相位]`
    variants: [[Entity; TRACK_PHASES as usize]; 2],
    /// 带符号累计里程，单位：米；负值表示履带倒转
    dist: [f32; 2],
    /// 当前显示的相位，与 spawn_tracks 设置的外层初始显隐一致。
    phase: [usize; 2],
}

/// 履带包几何校验通过标记，防止校验系统重复运行。
#[derive(Component)]
struct TracksValidated;

/// 注册履带包的 spawn、几何校验与相位推进系统。
pub(crate) struct TrackAnimationPlugin;

impl Plugin for TrackAnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (validate_tracks, advance_track_phases));
    }
}

/// 在坦克根实体下 spawn 履带包的 6 个相位变体子实体，并直接挂上 [`TrackAnim`]。
/// 在 `setup_scene` 之后链式执行（依赖坦克根实体已存在）。
///
/// 对齐说明：履带包与车体的 model 均使用统一 SIZE 且体素为全局坐标，
/// bevy_vox_scene 按模型中心对齐后天然重合，因此这里 Transform 为零；
/// 更换履带包时必须沿用该约定，否则需要在此补平移偏移。
pub(crate) fn spawn_tracks(
    mut commands: Commands,
    assets: Res<AssetServer>,
    tanks: Query<Entity, With<Tank>>,
) {
    let Ok(tank) = tanks.single() else {
        return;
    };
    let mut variants = [[Entity::PLACEHOLDER; TRACK_PHASES as usize]; 2];
    let mut children = Vec::with_capacity(6);
    for (side, names) in VARIANT_NAMES.iter().enumerate() {
        for (phase, name) in names.iter().enumerate() {
            let entity = commands
                .spawn((
                    Name::new(*name),
                    WorldAssetRoot(assets.load(format!("{TRACKS_PATH}#{name}"))),
                    // 显隐只由外层控制；vox 内部节点必须继承，不能带 _hidden。
                    if phase == 0 {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    },
                ))
                .id();
            variants[side][phase] = entity;
            children.push(entity);
        }
    }
    commands.entity(tank).add_children(&children);
    commands.entity(tank).insert(TrackAnim {
        variants,
        dist: [0.0; 2],
        phase: [0; 2],
    });
    info!("履带包已挂载：assets/{TRACKS_PATH}");
}

/// 在变体子树中查找第一个 mesh 并返回其模型空间 AABB 的 (min, max) 角点。
/// 履带包节点无旋转/平移变换，mesh AABB 即变体几何。
fn first_mesh_aabb(
    root: Entity,
    children: &Query<&Children>,
    meshes: &Query<&Mesh3d>,
    assets: &Assets<Mesh>,
) -> Option<(Vec3, Vec3)> {
    let mut pending = vec![root];
    while let Some(entity) = pending.pop() {
        if let Ok(kids) = children.get(entity) {
            pending.extend(kids.iter());
        }
        if let Ok(mesh) = meshes.get(entity) {
            return assets.get(&mesh.0).and_then(|m| {
                m.compute_aabb().map(|a| {
                    let center = Vec3::from(a.center);
                    let half = Vec3::from(a.half_extents);
                    (center - half, center + half)
                })
            });
        }
    }
    None
}

/// 几何校验：履带包与底盘的宽度契约。左右相位 0 变体的模型空间 AABB
/// 外沿应触及车宽两端（±TANK_SIZE.z / 2，容差 0.1m，允许半个体素误差）。
/// 装错底盘（如把 player 履带包装到更宽的底盘）会在此报错而非画面穿插。
fn validate_tracks(
    mut commands: Commands,
    mesh_assets: ResMut<Assets<Mesh>>,
    anims: Query<(Entity, &TrackAnim), Without<TracksValidated>>,
    children: Query<&Children>,
    mesh_parts: Query<&Mesh3d>,
) {
    let Ok((tank, anim)) = anims.single() else {
        return;
    };
    // 两个相位 0 变体分别代表左右履带几何；未加载完（无 mesh）则下帧再试
    let Some(left) = first_mesh_aabb(anim.variants[0][0], &children, &mesh_parts, &mesh_assets)
    else {
        return;
    };
    let Some(right) = first_mesh_aabb(anim.variants[1][0], &children, &mesh_parts, &mesh_assets)
    else {
        return;
    };
    let half = TANK_SIZE.z / 2.0;
    let tolerance = 0.1;
    // 依赖 bevy_vox_scene 的轴映射：vox 的 y（宽）落在世界 z。左右两侧外沿
    // 一正一负，符号可能随映射翻转，故按"两侧各抵一端"判定。
    let covers_left = (left.0.z + half).abs() < tolerance || (right.0.z + half).abs() < tolerance;
    let covers_right = (left.1.z - half).abs() < tolerance || (right.1.z - half).abs() < tolerance;
    if covers_left && covers_right {
        commands.entity(tank).insert(TracksValidated);
        info!("履带包几何校验通过：宽度与底盘匹配（±{half} m）");
    } else {
        error!(
            "履带包与底盘宽度不匹配：左外沿 z={:.3}，右外沿 z={:.3}，期望 ±{half:.3}。请核对 {} 是否适配当前底盘",
            left.0.z, right.1.z, TRACKS_PATH
        );
        // 校验失败移除动画状态，避免在错误几何上继续切换相位
        commands.entity(tank).remove::<TrackAnim>();
    }
}

/// 由带符号里程计算当前相位；负里程（倒转）向相位减小方向回绕。
fn phase_for_dist(dist: f32) -> usize {
    ((dist / TRACK_PITCH).floor() as i64).rem_euclid(TRACK_PHASES) as usize
}

/// 单侧履带接触点的线速度沿车头方向的分量，单位：米/秒。
/// `v_side = (v + ω×r) · forward`：平动速度叠加上转向产生的切向速度。
fn side_speed(linear: Vec3, angular_y: f32, r_side: Vec3, forward: Vec3) -> f32 {
    let omega_cross_r = Vec3::Y.cross(r_side) * angular_y;
    (linear + omega_cross_r).dot(forward)
}

/// 按左右履带线速度积分里程，里程跨越齿距时切换可见的相位变体。
fn advance_track_phases(
    time: Res<Time>,
    turn_rate: Res<TurnRate>,
    mut tanks: Query<(&Rotation, &LinearVelocity, &mut TrackAnim), With<Tank>>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    for (rotation, linear, mut anim) in &mut tanks {
        let heading = rotation.0;
        // 车头方向与两侧履带相对车心的位置（宽在车体局部 Z 轴）
        let forward = heading * Vec3::NEG_X;
        let offsets = [
            heading * Vec3::new(0.0, 0.0, HALF_TRACK_WIDTH),
            heading * Vec3::new(0.0, 0.0, -HALF_TRACK_WIDTH),
        ];
        for side in 0..2 {
            let speed = side_speed(linear.0, turn_rate.0, offsets[side], forward);
            anim.dist[side] += speed * dt;
            let phase = phase_for_dist(anim.dist[side]);
            // 相位变化时才写组件：显示新相位、隐藏其余
            if phase != anim.phase[side] {
                for (p, &entity) in anim.variants[side].iter().enumerate() {
                    commands.entity(entity).insert(if p == phase {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    });
                }
                anim.phase[side] = phase;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        camera::visibility::{VisibilityClass, VisibilityPlugin},
        light::FogVolume,
        mesh::skinning::SkinnedMeshInverseBindposes,
        time::TimeUpdateStrategy,
        world_serialization::WorldSerializationPlugin,
    };
    use bevy_vox_scene::{VoxLoaderSettings, VoxScenePlugin};
    use std::time::{Duration, Instant};

    /// 检查真实网格的继承可见性，避免只验证外层开关而漏掉资产内部的隐藏节点。
    fn assert_visible_tracks(app: &mut App, tank: Entity) {
        let anim = app.world().get::<TrackAnim>(tank).expect("履带动画应保留");
        for side in 0..2 {
            for phase in 0..TRACK_PHASES as usize {
                let mut pending = vec![anim.variants[side][phase]];
                let mut mesh_count = 0;
                while let Some(entity) = pending.pop() {
                    if let Some(children) = app.world().get::<Children>(entity) {
                        pending.extend(children.iter());
                    }
                    if app.world().get::<Mesh3d>(entity).is_some() {
                        mesh_count += 1;
                        assert_eq!(
                            app.world()
                                .get::<InheritedVisibility>(entity)
                                .unwrap()
                                .get(),
                            phase == anim.phase[side],
                            "侧 {side} 相位 {phase} 的实际网格显隐错误，当前相位 {}",
                            anim.phase[side],
                        );
                    }
                }
                assert!(mesh_count > 0, "侧 {side} 相位 {phase} 应加载出网格");
            }
        }
    }

    /// 真实履带资源在前进、倒车、左右原地转向及停车后，每侧始终只显示当前相位。
    #[test]
    fn real_tracks_remain_visible_when_moving_turning_and_stopping() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            ImagePlugin::default(),
            WorldSerializationPlugin,
            TransformPlugin,
            VisibilityPlugin,
            VoxScenePlugin {
                global_settings: Some(VoxLoaderSettings {
                    voxel_size: crate::VOXEL_SIZE,
                    ..default()
                }),
            },
            TrackAnimationPlugin,
        ))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_asset::<SkinnedMeshInverseBindposes>()
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
        .init_resource::<TurnRate>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            50,
        )))
        .add_systems(Startup, spawn_tracks);
        let tank = app
            .world_mut()
            .spawn((
                Tank,
                Transform::default(),
                Visibility::Inherited,
                Rotation::default(),
                LinearVelocity::ZERO,
            ))
            .id();
        // 异步加载用墙钟限时；游戏时间固定步进，不依赖机器帧率。
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            app.update();
            let world = app.world_mut();
            let mesh_count = world.query::<&Mesh3d>().iter(world).count();
            if mesh_count == 6 && world.get::<TracksValidated>(tank).is_some() {
                break;
            }
            assert!(Instant::now() < deadline, "履带资源加载或几何校验超时");
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_visible_tracks(&mut app, tank);
        // 直行与差速转向分别覆盖全部相位；每种运动结束后验证停车不会丢失履带。
        for (linear, turn) in [
            (Vec3::NEG_X * 2.0, 0.0),
            (Vec3::X * 2.0, 0.0),
            (Vec3::ZERO, 2.0 / HALF_TRACK_WIDTH),
            (Vec3::ZERO, -2.0 / HALF_TRACK_WIDTH),
        ] {
            app.world_mut().get_mut::<LinearVelocity>(tank).unwrap().0 = linear;
            app.world_mut().resource_mut::<TurnRate>().0 = turn;
            let mut visited = [[false; 3]; 2];
            for _ in 0..18 {
                app.update();
                assert_visible_tracks(&mut app, tank);
                let anim = app.world().get::<TrackAnim>(tank).unwrap();
                for side in 0..2 {
                    visited[side][anim.phase[side]] = true;
                }
            }
            assert!(
                visited.into_iter().flatten().all(|seen| seen),
                "每侧应遍历全部相位"
            );
            let phase = app.world().get::<TrackAnim>(tank).unwrap().phase;
            app.world_mut().get_mut::<LinearVelocity>(tank).unwrap().0 = Vec3::ZERO;
            app.world_mut().resource_mut::<TurnRate>().0 = 0.0;
            for _ in 0..4 {
                app.update();
                assert_visible_tracks(&mut app, tank);
                assert_eq!(app.world().get::<TrackAnim>(tank).unwrap().phase, phase);
            }
        }
    }

    /// 相位随里程推进；倒转（负里程）相位回退；齿距整数倍处周期回绕。
    // 注：避开浮点上恰好落在齿距边界的值（如 0.9 = 0.3×3 在 f32 里略小于 3）。
    #[test]
    fn phase_advances_reverses_and_wraps() {
        assert_eq!(phase_for_dist(0.0), 0);
        assert_eq!(phase_for_dist(0.31), 1);
        assert_eq!(phase_for_dist(0.61), 2);
        assert_eq!(phase_for_dist(0.92), 0);
        // 倒转 0.05m：相位从 0 回绕到 2
        assert_eq!(phase_for_dist(-0.05), 2);
        // 倒转越过整齿距后相位继续回退
        assert_eq!(phase_for_dist(-0.31), 1);
        assert_eq!(phase_for_dist(-0.61), 0);
    }

    /// 直行时两侧履带速度相等；左转（ω>0）时两侧一正一负（原地转向反向滚动）。
    #[test]
    fn side_speed_is_equal_when_straight_and_opposed_when_pivoting() {
        let forward = Vec3::NEG_X;
        let left = Vec3::new(0.0, 0.0, HALF_TRACK_WIDTH);
        let right = Vec3::new(0.0, 0.0, -HALF_TRACK_WIDTH);
        // 直行 6 m/s：两侧都是 6
        let v = forward * 6.0;
        assert_eq!(side_speed(v, 0.0, left, forward), 6.0);
        assert_eq!(side_speed(v, 0.0, right, forward), 6.0);
        // 原地左转（+Y 角速度）：两侧速度大小相等、方向相反
        let l = side_speed(Vec3::ZERO, 1.0, left, forward);
        let r = side_speed(Vec3::ZERO, 1.0, right, forward);
        assert_eq!(l, -r);
        assert!((l + HALF_TRACK_WIDTH).abs() < 1e-5);
        // 边转边前进：一侧叠加加速、另一侧减速（差速转向）
        let l = side_speed(v, 1.0, left, forward);
        let r = side_speed(v, 1.0, right, forward);
        assert!((l - (6.0 - HALF_TRACK_WIDTH)).abs() < 1e-5);
        assert!((r - (6.0 + HALF_TRACK_WIDTH)).abs() < 1e-5);
    }
}

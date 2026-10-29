//! 履带多帧滚动动画：履带是独立 vox 资产（`assets/tank/tracks/<底盘ID>/<变体>/track.vox`），
//! 每侧 3 个相位变体节点（`track_l_0..2` / `track_r_0..2`，齿图案相位错位 1 格）。
//! 随坦克出生以子实体形式 spawn 履带包并直接持有变体句柄，运行时按左右履带
//! 线速度积分里程、切换变体可见性产生滚动感；原地转向时左右差速反向滚动。

use avian3d::prelude::*;
use bevy::camera::primitives::MeshAabb;
use bevy::prelude::*;

use super::driving::{TANK_SIZE, TurnRate};

/// 每相位沿履带轮廓移动一体素，单位：米；三相位合计为 0.3 米齿距。
const TRACK_PHASE_STEP: f32 = crate::tank::model::REFERENCE_VOXEL_SIZE;
/// 相位数（与履带包变体节点数一致）
const TRACK_PHASES: i64 = 3;
/// 每辆车的履带资产与整车尺寸契约；添加此组件后自动挂载一次履带。
#[derive(Component, Clone, Copy)]
pub(crate) struct TrackSpec {
    /// 相对 assets/ 的履带包路径，包内必须包含左右各三个相位节点。
    path: &'static str,
    /// 整车 X 长、Y 高、Z 宽，单位：米；动画差速及几何校验共用。
    size: Vec3,
    /// 各侧独立轮节点，支持不同车型的轮数。
    wheels: [&'static [crate::tank::model::WheelSpec]; 2],
    /// 场景安装轴心，单位参考格；扩高画布仍用旧轴心保持离地高度，不能用
    /// 非炮管碰撞尺寸代替。
    pivot: (f32, f32, f32),
    /// 内腔竖直开区间，单位体素格；完整旋转包络必须在其中。
    clearance: (f32, f32),
    /// 履带驱动的前进速度，单位：米/秒；驾驶系统按此换算水平推力。
    pub(crate) forward_speed: f32,
    /// 履带驱动的倒车速度，单位：米/秒。
    pub(crate) reverse_speed: f32,
}

/// 玩家默认底盘适配的 cleat 履带，速度按用户指定：前进 16 米/秒、倒车 8 米/秒。
pub(crate) const PLAYER_TRACKS: TrackSpec = TrackSpec {
    path: "tank/tracks/player/cleat/track.vox",
    size: TANK_SIZE,
    wheels: [
        &crate::tank::model::player::WHEELS[0],
        &crate::tank::model::player::WHEELS[1],
    ],
    pivot: crate::tank::model::player::CANVAS_PIVOT,
    clearance: crate::tank::model::player::WHEEL_CLEARANCE,
    forward_speed: 16.0,
    reverse_speed: 8.0,
};
/// 默认敌人底盘适配的 crawl 履带，左右各四个负重轮和两个端轮。
/// 尺寸与敌人生成同源：直接推导自敌人模型契约，不依赖敌人生成模块。
/// 敌人当前零输入不驾驶，速度为未设定占位值，接入敌方 AI 前需赋真实数值。
pub(crate) const ENEMY_TRACKS: TrackSpec = TrackSpec {
    path: "tank/tracks/enemy/crawl/track.vox",
    size: Vec3::from_array(crate::tank::model::enemy::COLLISION_SIZE),
    wheels: [
        &crate::tank::model::enemy::WHEELS[0],
        &crate::tank::model::enemy::WHEELS[1],
    ],
    pivot: crate::tank::model::enemy::CANVAS_PIVOT,
    clearance: crate::tank::model::enemy::WHEEL_CLEARANCE,
    forward_speed: 0.0,
    reverse_speed: 0.0,
};
/// 相位变体节点名，顺序 [左, 右] × [0, 1, 2]，与履带包场景图一致。
const VARIANT_NAMES: [[&str; TRACK_PHASES as usize]; 2] = [
    ["track_l_0", "track_l_1", "track_l_2"],
    ["track_r_0", "track_r_1", "track_r_2"],
];

/// 每辆坦克根实体各自持有履带动画状态，击毁时随子树一起销毁。
#[derive(Component)]
pub(crate) struct TrackAnim {
    /// 左右两侧的相位变体实体：`variants[侧][相位]`
    variants: [[Entity; TRACK_PHASES as usize]; 2],
    /// 带符号累计里程，单位：米；负值表示履带倒转
    dist: [f32; 2],
    /// 轮子与相位平级挂载，按车型保存实体，随坦克销毁。
    wheels: [Vec<Entity>; 2],
    /// 当前显示的相位，与 spawn_tracks 设置的外层初始显隐一致。
    phase: [usize; 2],
}

impl TrackAnim {
    /// 离屏断言读取累计里程；只比较最终相位会漏掉恰好绕回初始相位的运动。
    pub(crate) fn distances(&self) -> [f32; 2] {
        self.dist
    }
}

/// 履带包几何校验通过标记，防止校验系统重复运行。
#[derive(Component)]
pub(crate) struct TracksValidated;

/// 挂载或几何错误保留在车辆上，离屏入口据此失败而非无限等待。
#[derive(Component)]
pub(crate) struct TracksFailed(pub String);

/// 注册履带包的 spawn、几何校验与相位推进系统。
pub(crate) struct TrackAnimationPlugin;

impl Plugin for TrackAnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (spawn_tracks, validate_tracks, advance_track_phases).chain(),
        );
    }
}

/// 在坦克根实体下挂载六个相位及车型指定的独立轮，并持有全部实体。
/// 在 Update 中处理新加入的 TrackSpec，覆盖开局玩家及地形就绪后出生的敌人。
///
/// 对齐说明：履带包与车体的 model 均使用统一 SIZE 且体素为全局坐标，
/// bevy_vox_scene 按模型中心对齐后天然重合，因此这里 Transform 为零；
/// 轮节点则采用局部 SIZE，按共享轮位单独平移，不套用相位的零偏移约定。
pub(crate) fn spawn_tracks(
    mut commands: Commands,
    assets: Res<AssetServer>,
    tanks: Query<(Entity, &TrackSpec), Added<TrackSpec>>,
) {
    for (tank, spec) in &tanks {
        let mut variants = [[Entity::PLACEHOLDER; TRACK_PHASES as usize]; 2];
        let mut children = Vec::with_capacity(6);
        for (side, names) in VARIANT_NAMES.iter().enumerate() {
            for (phase, name) in names.iter().enumerate() {
                let entity = commands
                    .spawn((
                        Name::new(*name),
                        WorldAssetRoot(assets.load(format!("{}#{name}", spec.path))),
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
        let wheels = spec.wheels.map(|side| {
            side.iter()
                .map(|wheel| {
                    let entity = commands
                        .spawn((
                            Name::new(wheel.name),
                            WorldAssetRoot(assets.load(format!("{}#{}", spec.path, wheel.name))),
                            wheel_transform(wheel, spec.pivot, 0.0),
                            Visibility::Inherited,
                        ))
                        .id();
                    children.push(entity);
                    entity
                })
                .collect()
        });
        commands.entity(tank).add_children(&children);
        commands.entity(tank).insert(TrackAnim {
            variants,
            dist: [0.0; 2],
            wheels,
            phase: [0; 2],
        });
        info!("履带包已挂载：assets/{}", spec.path);
    }
}

/// 合并子树中所有网格在根节点局部坐标系下的包围盒。
fn subtree_mesh_aabb(
    root: Entity,
    children: &Query<&Children>,
    transforms: &Query<&Transform>,
    meshes: &Query<&Mesh3d>,
    assets: &Assets<Mesh>,
) -> Option<(Vec3, Vec3)> {
    let mut pending = vec![(root, bevy::math::Affine3A::IDENTITY)];
    let mut bounds: Option<(Vec3, Vec3)> = None;
    while let Some((entity, transform)) = pending.pop() {
        if let Ok(kids) = children.get(entity) {
            for child in kids.iter() {
                let local = transforms
                    .get(child)
                    .map_or(bevy::math::Affine3A::IDENTITY, Transform::compute_affine);
                pending.push((child, transform * local));
            }
        }
        if let Ok(mesh) = meshes.get(entity) {
            let aabb = assets.get(&mesh.0)?.get_aabb()?;
            let center = Vec3::from(aabb.center);
            let half = Vec3::from(aabb.half_extents);
            for x in [-1.0, 1.0] {
                for y in [-1.0, 1.0] {
                    for z in [-1.0, 1.0] {
                        let corner = transform.transform_point3(center + half * Vec3::new(x, y, z));
                        bounds = Some(match bounds {
                            Some((low, high)) => (low.min(corner), high.max(corner)),
                            None => (corner, corner),
                        });
                    }
                }
            }
        }
    }
    bounds
}

/// 几何校验：履带包与底盘的宽度契约。左右相位 0 变体的模型空间 AABB
/// 外沿应触及各自车宽两端（容差 0.1m，允许半个体素误差）。
/// 装错底盘（如把 player 履带包装到更宽的底盘）会在此报错而非画面穿插。
fn validate_tracks(
    mut commands: Commands,
    mesh_assets: Res<Assets<Mesh>>,
    server: Res<AssetServer>,
    spawner: Res<bevy::world_serialization::WorldInstanceSpawner>,
    roots: Query<(
        &WorldAssetRoot,
        Option<&bevy::world_serialization::WorldInstance>,
    )>,
    anims: Query<(Entity, &TrackSpec, &TrackAnim), Without<TracksValidated>>,
    children: Query<&Children>,
    mesh_parts: Query<&Mesh3d>,
    transforms: Query<&Transform>,
) {
    for (tank, spec, anim) in &anims {
        // 每个相位与独立轮均需实例和网格就绪；空节点不能被当成加载成功。
        let all_roots: Vec<_> = anim
            .variants
            .iter()
            .flatten()
            .copied()
            .chain(anim.wheels.iter().flatten().copied())
            .collect();
        let mut ready = true;
        let mut failure = None;
        for root in all_roots {
            let Ok((asset, instance)) = roots.get(root) else {
                ready = false;
                continue;
            };
            if let Some(bevy::asset::LoadState::Failed(error)) = server.get_load_state(asset.0.id())
            {
                failure = Some(format!("履带或轮节点加载失败：{error}"));
                break;
            }
            if !server.is_loaded_with_dependencies(&asset.0)
                || !instance.is_some_and(|i| spawner.instance_is_ready(**i))
            {
                ready = false;
            } else if subtree_mesh_aabb(root, &children, &transforms, &mesh_parts, &mesh_assets)
                .is_none()
            {
                failure = Some(format!("履带或轮节点缺少网格：{root:?}"));
                break;
            }
        }
        if ready && failure.is_none() {
            let wheels = &anim.wheels;
            for (side, entities) in wheels.iter().enumerate() {
                for (index, &root) in entities.iter().enumerate() {
                    let wheel = &spec.wheels[side][index];
                    if let Some((low, high)) =
                        subtree_mesh_aabb(root, &children, &transforms, &mesh_parts, &mesh_assets)
                    {
                        let size = Vec3::new(
                            wheel.size.0 as f32,
                            wheel.size.2 as f32,
                            wheel.size.1 as f32,
                        ) * crate::tank::model::REFERENCE_VOXEL_SIZE;
                        if !(high - low).abs_diff_eq(size, 1e-4)
                            || (high + low).length() > 1e-4
                            || wheel.center[2] - wheel.envelope <= spec.clearance.0
                            || wheel.center[2] + wheel.envelope >= spec.clearance.1
                        {
                            failure =
                                Some(format!("轮节点轴心、尺寸或旋转净空不匹配：{}", wheel.name));
                        }
                    }
                }
            }
        }
        if let Some(message) = failure {
            error!("{message}");
            commands
                .entity(tank)
                .insert(TracksFailed(message))
                .remove::<TrackAnim>();
            continue;
        }
        if !ready {
            continue;
        }
        // 宽度只检查履带相位，不把局部轮的 AABB 混入共同画布。
        let Some(left) = subtree_mesh_aabb(
            anim.variants[0][0],
            &children,
            &transforms,
            &mesh_parts,
            &mesh_assets,
        ) else {
            continue;
        };
        let Some(right) = subtree_mesh_aabb(
            anim.variants[1][0],
            &children,
            &transforms,
            &mesh_parts,
            &mesh_assets,
        ) else {
            continue;
        };
        let half = spec.size.z / 2.0;
        let tolerance = 0.1;
        // 依赖 bevy_vox_scene 的轴映射：vox 的 y（宽）落在世界 z。左右两侧外沿
        // 一正一负，符号可能随映射翻转，故按"两侧各抵一端"判定。
        let covers_left =
            (left.0.z + half).abs() < tolerance || (right.0.z + half).abs() < tolerance;
        let covers_right =
            (left.1.z - half).abs() < tolerance || (right.1.z - half).abs() < tolerance;
        if covers_left && covers_right {
            commands.entity(tank).insert(TracksValidated);
            info!("履带包几何校验通过：宽度与底盘匹配（±{half} m）");
        } else {
            error!(
                "履带包与底盘宽度不匹配：左外沿 z={:.3}，右外沿 z={:.3}，期望 ±{half:.3}。请核对 {} 是否适配当前底盘",
                left.0.z, right.1.z, spec.path
            );
            // 校验失败移除动画状态，避免在错误几何上继续切换相位
            commands
                .entity(tank)
                .insert(TracksFailed(format!("履带包宽度不匹配：{}", spec.path)))
                .remove::<TrackAnim>();
        }
    }
}

/// 由带符号里程计算当前相位；负里程（倒转）向相位减小方向回绕。
fn phase_for_dist(dist: f32) -> usize {
    ((dist / TRACK_PHASE_STEP).floor() as i64).rem_euclid(TRACK_PHASES) as usize
}

/// 单侧履带接触点的线速度沿车头方向的分量，单位：米/秒。
/// `v_side = (v + ω×r) · forward`：平动速度叠加上转向产生的切向速度。
fn side_speed(linear: Vec3, angular_y: f32, r_side: Vec3, forward: Vec3) -> f32 {
    let omega_cross_r = Vec3::Y.cross(r_side) * angular_y;
    (linear + omega_cross_r).dot(forward)
}

/// VOX 安装轴心映射为游戏（−X、Z、Y）；正转角使接地点抵消 −X 前进速度。
fn wheel_transform(
    wheel: &crate::tank::model::WheelSpec,
    pivot: (f32, f32, f32),
    distance: f32,
) -> Transform {
    let [x, y, z] = wheel.center;
    let (px, py, pz) = pivot;
    Transform::from_xyz(
        (px - x) * crate::tank::model::REFERENCE_VOXEL_SIZE,
        (z - pz) * crate::tank::model::REFERENCE_VOXEL_SIZE,
        (y - py) * crate::tank::model::REFERENCE_VOXEL_SIZE,
    )
    .with_rotation(Quat::from_rotation_z(
        distance / (wheel.radius * crate::tank::model::REFERENCE_VOXEL_SIZE),
    ))
}

/// 按左右履带线速度积分里程，每移动一体素切换相位；倒车反向沿闭环推进。
fn advance_track_phases(
    time: Res<Time>,
    mut tanks: Query<(
        &Rotation,
        &LinearVelocity,
        &TurnRate,
        &TrackSpec,
        &mut TrackAnim,
    )>,
    mut commands: Commands,
    mut transforms: Query<&mut Transform>,
) {
    let dt = time.delta_secs();
    for (rotation, linear, turn_rate, spec, mut anim) in &mut tanks {
        let heading = rotation.0;
        // 车头方向与两侧履带相对车心的位置（宽在车体局部 Z 轴）
        let forward = heading * Vec3::NEG_X;
        let half_width = spec.size.z / 2.0;
        let offsets = [
            // l 节点在 VOX Y 小于画布中心的一侧，即游戏局部 −Z。
            heading * Vec3::new(0.0, 0.0, -half_width),
            heading * Vec3::new(0.0, 0.0, half_width),
        ];
        for side in 0..2 {
            let speed = side_speed(linear.0, turn_rate.0, offsets[side], forward);
            anim.dist[side] += speed * dt;
            let wheels = &anim.wheels;
            for (index, &entity) in wheels[side].iter().enumerate() {
                if let Ok(mut transform) = transforms.get_mut(entity) {
                    transform.rotation =
                        wheel_transform(&spec.wheels[side][index], spec.pivot, anim.dist[side])
                            .rotation;
                }
            }
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
    use crate::gameplay::player::Tank;

    /// 测试所用默认底盘半宽，单位：米；玩家碰撞宽 3.8 米，敌人为 3.4 米宽。
    const HALF_TRACK_WIDTH: f32 = TANK_SIZE.z / 2.0;
    use bevy::{
        camera::visibility::{VisibilityClass, VisibilityPlugin},
        light::FogVolume,
        mesh::skinning::SkinnedMeshInverseBindposes,
        time::TimeUpdateStrategy,
        world_serialization::WorldSerializationPlugin,
    };
    use bevy_vox_scene::VoxScenePlugin;
    use std::time::{Duration, Instant};

    /// 检查真实网格的继承可见性，避免只验证外层开关而漏掉资产内部的隐藏节点。
    fn assert_visible_tracks(app: &mut App, tank: Entity) {
        assert_wheels(app, tank);
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

    /// 多车真实履带在前进、倒车、原地转向及停车时独立显隐，销毁后注销全部实例。
    #[test]
    fn real_tracks_remain_visible_when_moving_turning_and_stopping() {
        let _asset_guard = crate::tank::model::lock_real_assets();
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            ImagePlugin::default(),
            WorldSerializationPlugin,
            TransformPlugin,
            VisibilityPlugin,
            VoxScenePlugin::default(),
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
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            50,
        )));
        let tank = app
            .world_mut()
            .spawn((
                Tank,
                PLAYER_TRACKS,
                TurnRate::default(),
                Transform::default(),
                Visibility::Inherited,
                Rotation::default(),
                LinearVelocity::ZERO,
            ))
            .id();
        // 玩家先生成，敌人在后续 Update 加入，验证异步出生也只挂载一次。
        app.update();
        let mut enemies = Vec::new();
        for _ in 0..2 {
            enemies.push(
                app.world_mut()
                    .spawn((
                        ENEMY_TRACKS,
                        TurnRate::default(),
                        Transform::default(),
                        Visibility::Inherited,
                        Rotation::default(),
                        LinearVelocity::ZERO,
                    ))
                    .id(),
            );
        }
        let idle_enemy = enemies[1];
        // 异步加载用墙钟限时；游戏时间固定步进，不依赖机器帧率。
        // 玩家包 24 个命名节点各自触发一次带 label 的资源加载，慢机器上 10 秒会临界抖动。
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            app.update();
            let world = app.world_mut();
            if [tank, enemies[0], idle_enemy]
                .into_iter()
                .all(|entity| world.get::<TracksValidated>(entity).is_some())
            {
                break;
            }
            assert!(Instant::now() < deadline, "履带资源加载或几何校验超时");
            std::thread::sleep(Duration::from_millis(1));
        }
        let player = tank;
        for tank in [player, enemies[0]] {
            assert_visible_tracks(&mut app, tank);
            // 直行与差速转向分别覆盖全部相位；每种运动结束后验证停车不会丢失履带。
            for (linear, turn) in [
                (Vec3::NEG_X * 2.0, 0.0),
                (Vec3::X * 2.0, 0.0),
                (Vec3::ZERO, 2.0 / HALF_TRACK_WIDTH),
                (Vec3::ZERO, -2.0 / HALF_TRACK_WIDTH),
            ] {
                app.world_mut().get_mut::<LinearVelocity>(tank).unwrap().0 = linear;
                app.world_mut().get_mut::<TurnRate>(tank).unwrap().0 = turn;
                let mut visited = [[false; 3]; 2];
                for _ in 0..18 {
                    app.update();
                    assert_visible_tracks(&mut app, tank);
                    assert_visible_tracks(&mut app, idle_enemy);
                    assert_eq!(
                        app.world().get::<TrackAnim>(idle_enemy).unwrap().dist,
                        [0.0; 2]
                    );
                    assert_eq!(
                        app.world().get::<TrackAnim>(idle_enemy).unwrap().phase,
                        [0; 2]
                    );
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
                app.world_mut().get_mut::<TurnRate>(tank).unwrap().0 = 0.0;
                for _ in 0..4 {
                    app.update();
                    assert_visible_tracks(&mut app, tank);
                    assert_eq!(app.world().get::<TrackAnim>(tank).unwrap().phase, phase);
                }
            }
        }
        // 重复 Update 不应重复生成履带；销毁敌人必须清除每个异步场景实例。
        let anim = app.world().get::<TrackAnim>(enemies[0]).unwrap();
        let roots: Vec<Entity> = anim
            .variants
            .iter()
            .flatten()
            .copied()
            .chain(anim.wheels.iter().flatten().copied())
            .collect();
        // 保存整个子树，销毁后逐个检查，分块数量变化也不能掩盖残留。
        let mut descendants = roots.clone();
        let mut index = 0;
        while index < descendants.len() {
            if let Some(children) = app.world().get::<Children>(descendants[index]) {
                descendants.extend(children.iter());
            }
            index += 1;
        }
        let instances: Vec<_> = roots
            .iter()
            .map(|entity| {
                **app
                    .world()
                    .get::<bevy::world_serialization::WorldInstance>(*entity)
                    .unwrap()
            })
            .collect();
        app.world_mut().despawn(enemies[0]);
        app.update();
        for entity in descendants {
            assert!(app.world().get_entity(entity).is_err());
        }
        let spawner = app
            .world()
            .resource::<bevy::world_serialization::WorldInstanceSpawner>();
        for instance in instances {
            assert!(!spawner.instance_is_ready(instance));
        }
        assert_visible_tracks(&mut app, player);
        assert_visible_tracks(&mut app, idle_enemy);
        // 敌人每侧仅6轮：误声明玩家9轮规格必须报错，不能静默缺轮。
        let broken = app
            .world_mut()
            .spawn((
                TrackSpec {
                    wheels: PLAYER_TRACKS.wheels,
                    ..ENEMY_TRACKS
                },
                Transform::default(),
                Visibility::Inherited,
            ))
            .id();
        // 缺失 label 的加载失败上报实测约 13 秒（每个缺失节点独立走一遍文件加载），
        // 10 秒会临界抖动，放宽到 30 秒。
        let deadline = Instant::now() + Duration::from_secs(30);
        while app.world().get::<TracksFailed>(broken).is_none() {
            app.update();
            assert!(Instant::now() < deadline, "缺失轮节点必须报告失败");
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(app.world().get::<TrackAnim>(broken).is_none());
        assert!(app.world().get::<TracksValidated>(broken).is_none());
        app.world_mut().despawn(broken);
        // 玩家履带包含分块，检查销毁后所有场景实例和子实体都已清理。
        let anim = app.world().get::<TrackAnim>(player).unwrap();
        let mut descendants: Vec<_> = anim
            .variants
            .iter()
            .flatten()
            .copied()
            .chain(anim.wheels.iter().flatten().copied())
            .collect();
        let instances: Vec<_> = descendants
            .iter()
            .map(|&root| {
                **app
                    .world()
                    .get::<bevy::world_serialization::WorldInstance>(root)
                    .unwrap()
            })
            .collect();
        let mut index = 0;
        while index < descendants.len() {
            if let Some(children) = app.world().get::<Children>(descendants[index]) {
                descendants.extend(children.iter());
            }
            index += 1;
        }
        app.world_mut().despawn(player);
        app.update();
        assert!(
            descendants
                .iter()
                .all(|&entity| app.world().get_entity(entity).is_err())
        );
        let spawner = app
            .world()
            .resource::<bevy::world_serialization::WorldInstanceSpawner>();
        assert!(
            instances
                .into_iter()
                .all(|instance| !spawner.instance_is_ready(instance))
        );
    }
    /// 运动学在真实加载网格上验证；轮轴不随角度漂移，左右里程互相独立。
    fn assert_wheels(app: &App, tank: Entity) {
        let anim = app.world().get::<TrackAnim>(tank).unwrap();
        let wheels = &anim.wheels;
        let track_spec = app.world().get::<TrackSpec>(tank).unwrap();
        for side in 0..2 {
            for (index, &entity) in wheels[side].iter().enumerate() {
                let spec = &track_spec.wheels[side][index];
                let transform = app.world().get::<Transform>(entity).unwrap();
                let expected = wheel_transform(spec, track_spec.pivot, anim.dist[side]);
                assert!(
                    transform
                        .translation
                        .abs_diff_eq(expected.translation, 1e-5)
                );
                assert!(transform.rotation.abs_diff_eq(expected.rotation, 1e-5));
                let mut pending = vec![(entity, bevy::math::Affine3A::IDENTITY)];
                let mut bounds: Option<(Vec3, Vec3)> = None;
                while let Some((child, affine)) = pending.pop() {
                    if let Some(children) = app.world().get::<Children>(child) {
                        for next in children.iter() {
                            let local = app
                                .world()
                                .get::<Transform>(next)
                                .map_or(bevy::math::Affine3A::IDENTITY, Transform::compute_affine);
                            pending.push((next, affine * local));
                        }
                    }
                    if let Some(mesh) = app.world().get::<Mesh3d>(child) {
                        assert!(app.world().get::<InheritedVisibility>(child).unwrap().get());
                        let mesh = app.world().resource::<Assets<Mesh>>().get(&mesh.0).unwrap();
                        let aabb = mesh.get_aabb().unwrap();
                        for x in [-1.0, 1.0] {
                            for y in [-1.0, 1.0] {
                                for z in [-1.0, 1.0] {
                                    let corner = affine.transform_point3(
                                        Vec3::from(aabb.center)
                                            + Vec3::from(aabb.half_extents) * Vec3::new(x, y, z),
                                    );
                                    bounds = Some(match bounds {
                                        Some((low, high)) => (low.min(corner), high.max(corner)),
                                        None => (corner, corner),
                                    });
                                }
                            }
                        }
                    }
                }
                let (low, high) = bounds.expect("轮节点应包含网格");
                let size = Vec3::new(spec.size.0 as f32, spec.size.2 as f32, spec.size.1 as f32)
                    * crate::tank::model::REFERENCE_VOXEL_SIZE;
                assert!((high + low).length() < 1e-4, "轮子整体应绕轴心居中");
                assert!(
                    (high - low).abs_diff_eq(size, 1e-4),
                    "轮子整体尺寸应保持不变"
                );
            }
        }
    }

    /// 前进时接地点沿 +X 抵消车身速度；倒车和两侧反向里程必须反转。
    #[test]
    fn wheel_roll_direction_matches_ground_contact() {
        let spec = &crate::tank::model::player::WHEELS[0][0];
        let forward = wheel_transform(spec, PLAYER_TRACKS.pivot, 0.001);
        let reverse = wheel_transform(spec, PLAYER_TRACKS.pivot, -0.001);
        let bottom = Vec3::NEG_Y * spec.radius * crate::tank::model::REFERENCE_VOXEL_SIZE;
        assert!((forward.rotation * bottom).x > 0.0);
        assert!((reverse.rotation * bottom).x < 0.0);
        // 正 Z 角速度叉乘底部 −Y，得到 +X 接触速度。
    }

    /// 画布扩高只增加顶部空间；轮轴安装高度仍由独立轴心保持，不能随画布中心漂移。
    #[test]
    fn expanded_canvas_keeps_wheel_install_height() {
        let spec = &crate::tank::model::player::WHEELS[0][0];
        let transform = wheel_transform(spec, PLAYER_TRACKS.pivot, 0.0);
        // 若误用扩高后画布中心（CANVAS.2/2），轮轴会整体下沉 0.5 米；轴心必须保持 13.5。
        let canvas_center = (
            PLAYER_TRACKS.pivot.0,
            PLAYER_TRACKS.pivot.1,
            crate::tank::model::player::CANVAS.2 as f32 / 2.0,
        );
        let with_canvas_center = wheel_transform(spec, canvas_center, 0.0);
        assert!((transform.translation.y - with_canvas_center.translation.y - 0.5).abs() < 1e-5);
        assert_eq!(PLAYER_TRACKS.pivot.2, 13.5);
        assert_eq!(
            crate::tank::model::player::CANVAS.2,
            37,
            "高度扩展用于车顶附件，与物理碰撞分开"
        );
    }

    /// 每相位移动一格（0.1 米），三相位对应完整 0.3 米齿距；倒车按反向回绕。
    #[test]
    fn phase_advances_reverses_and_wraps() {
        assert_eq!(phase_for_dist(0.0), 0);
        assert_eq!(phase_for_dist(0.09), 0);
        assert_eq!(phase_for_dist(0.11), 1);
        assert_eq!(phase_for_dist(0.21), 2);
        assert_eq!(phase_for_dist(0.31), 0);
        // 倒转 0.05m：相位从 0 回绕到 2
        assert_eq!(phase_for_dist(-0.05), 2);
        // 倒转越过整齿距后相位继续回退
        assert_eq!(phase_for_dist(-0.11), 1);
        assert_eq!(phase_for_dist(-0.21), 0);
    }

    /// 直行时两侧履带速度相等；左转（ω>0）时两侧一正一负（原地转向反向滚动）。
    #[test]
    fn side_speed_is_equal_when_straight_and_opposed_when_pivoting() {
        let forward = Vec3::NEG_X;
        let left = Vec3::new(0.0, 0.0, -HALF_TRACK_WIDTH);
        let right = Vec3::new(0.0, 0.0, HALF_TRACK_WIDTH);
        // 直行 6 m/s：两侧都是 6
        let v = forward * 6.0;
        assert_eq!(side_speed(v, 0.0, left, forward), 6.0);
        assert_eq!(side_speed(v, 0.0, right, forward), 6.0);
        // 原地左转（+Y 角速度）：两侧速度大小相等、方向相反
        let l = side_speed(Vec3::ZERO, 1.0, left, forward);
        let r = side_speed(Vec3::ZERO, 1.0, right, forward);
        assert_eq!(l, -r);
        assert!((l - HALF_TRACK_WIDTH).abs() < 1e-5);
        // 边转边前进：一侧叠加加速、另一侧减速（差速转向）
        let l = side_speed(v, 1.0, left, forward);
        let r = side_speed(v, 1.0, right, forward);
        assert!((l - (6.0 + HALF_TRACK_WIDTH)).abs() < 1e-5);
        assert!((r - (6.0 - HALF_TRACK_WIDTH)).abs() < 1e-5);
    }
}

//! 无窗口验证入口：固定输入与模拟时钟，复用实际游戏系统并保存关键帧。

use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::mpsc,
    time::Duration,
};

use avian3d::prelude::{LinearVelocity, Position, Rotation, SpatialQuery, SpatialQueryFilter};
use bevy::{
    app::PluginsState,
    asset::UntypedAssetLoadFailedEvent,
    camera::RenderTarget,
    image::ImageFormat,
    input::InputSystems,
    prelude::*,
    render::{
        RenderApp, RenderPlugin,
        mesh::RenderMesh,
        pipelined_rendering::PipelinedRenderingPlugin,
        render_asset::RenderAssets,
        render_resource::{CachedPipelineState, PipelineCache, PollType, TextureFormat},
        renderer::{RenderAdapterInfo, RenderDevice},
        texture::GpuImage,
        view::screenshot::{Screenshot, ScreenshotCaptured},
    },
    time::TimeUpdateStrategy,
    window::ExitCondition,
    winit::WinitPlugin,
    world_serialization::{WorldInstance, WorldInstanceSpawner},
};

/// 所有离屏场景共用的随机种子，仅改变验证进程的敌人布局。
pub(super) const SEED: u64 = 42;
/// 图片尺寸固定，避免窗口大小和桌面缩放参与画面对比。
const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;
/// 精确的同一 Duration 同时用于可变步长时钟与固定物理时钟。
const STEP: Duration = Duration::from_nanos(16_666_667);
/// 使用墙钟限制加载、GPU 等待与模拟总耗时，防止无人值守进程残留。
const TIMEOUT: Duration = Duration::from_secs(120);

/// 内置验证场景，不提供独立于游戏逻辑的测试实现。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scenario {
    Scene,
    Drive,
    Combat,
}

impl Scenario {
    /// 计帧从资源就绪并完成落地预热后开始；0 为动作前基准图。
    fn frames(self) -> &'static [u32] {
        match self {
            Self::Scene => &[0],
            // 体素地形是有意障碍：直行只模拟 1 秒（60 帧），车头前缘
            // （半长 3.75 m + 行驶 6 m）仍留在半径 24 m 的出生平区内；
            // 平区外的台阶全部是可翻越的 1 档坎，不构成驾驶失效判据。
            Self::Drive => &[0, 60, 180],
            Self::Combat => &[0, 6, 24, 120],
        }
    }

    /// 按固定模拟帧注入按键，经过游戏现有输入系统，而非直接调用业务逻辑。
    fn keys(self, frame: u32) -> &'static [KeyCode] {
        match (self, frame) {
            (Self::Drive, 1..=60) => &[KeyCode::ArrowUp],
            (Self::Drive, 61..=180) => &[KeyCode::ArrowUp, KeyCode::ArrowLeft],
            (Self::Combat, 1) => &[KeyCode::Space],
            _ => &[],
        }
    }
}

/// 命令行指定的输出目标；单张截图不生成额外报告。
#[derive(Debug, PartialEq, Eq, Resource, Clone)]
pub(super) struct Options {
    scenario: Scenario,
    output: PathBuf,
    single_image: bool,
}

impl Options {
    /// 无参数返回普通游戏；其余参数必须完整匹配已公布的接口。
    pub(super) fn parse(args: Vec<OsString>) -> Result<Option<Self>, String> {
        let usage =
            "用法：--capture <图片.png> 或 --offscreen-test <scene|drive|combat> --output <目录>";
        if args.is_empty() {
            return Ok(None);
        }
        let options = if args.len() == 2 && args[0] == "--capture" {
            let output = PathBuf::from(&args[1]);
            if output
                .extension()
                .is_none_or(|ext| !ext.eq_ignore_ascii_case("png"))
            {
                return Err(format!("截图必须使用 .png 扩展名。{usage}"));
            }
            Self {
                scenario: Scenario::Scene,
                output,
                single_image: true,
            }
        } else if args.len() == 4 && args[0] == "--offscreen-test" && args[2] == "--output" {
            let scenario = match args[1].to_str() {
                Some("scene") => Scenario::Scene,
                Some("drive") => Scenario::Drive,
                Some("combat") => Scenario::Combat,
                _ => return Err(usage.into()),
            };
            Self {
                scenario,
                output: PathBuf::from(&args[3]),
                single_image: false,
            }
        } else {
            return Err(usage.into());
        };
        if options.output.as_os_str().is_empty() {
            return Err("输出路径不能为空".into());
        }
        Ok(Some(options))
    }

    /// 将所有目标提前检查一遍；实际写入仍用 create_new 防止并发覆盖。
    pub(super) fn prepare(&self) -> Result<(), String> {
        for path in self.paths() {
            if fs::symlink_metadata(&path).is_ok() {
                return Err(format!("目标已存在，拒绝覆盖：{}", path.display()));
            }
            if let Some(parent) = path.parent().filter(|path| !path.as_os_str().is_empty()) {
                fs::create_dir_all(parent).map_err(|e| format!("无法创建输出目录：{e}"))?;
            }
        }
        Ok(())
    }

    /// 启动即计时；独立监视线程也能处理 GPU 驱动或同步管线编译阻塞。
    pub(super) fn watchdog(&self) -> mpsc::Sender<()> {
        let report = (!self.single_image).then(|| self.output.join("report.txt"));
        spawn_watchdog(TIMEOUT, move || {
            let message = "离屏验证超过 120 秒，强制结束；结果：失败";
            eprintln!("{message}");
            if let Some(path) = report {
                let _ = write_new(&path, message.as_bytes());
            }
            std::process::exit(1);
        })
    }

    /// 单张与批量截图使用相同关键帧流程，只改变产物路径。
    fn image_path(&self, frame: u32) -> PathBuf {
        if self.single_image {
            self.output.clone()
        } else {
            self.output.join(format!("frame-{frame:06}.png"))
        }
    }

    /// 返回本次运行会写入的全部产物，供启动前拒绝覆盖。
    fn paths(&self) -> Vec<PathBuf> {
        let mut paths: Vec<_> = self
            .scenario
            .frames()
            .iter()
            .map(|&frame| self.image_path(frame))
            .collect();
        if !self.single_image {
            paths.push(self.output.join("report.txt"));
        }
        paths
    }
}

/// 相机共用的离屏目标；Screenshot 直接从该纹理读回最终后处理画面。
#[derive(Resource)]
struct Target(Handle<Image>);

/// 下一次更新要注入的模拟帧；None 用于加载、预热和截图等待。
#[derive(Resource)]
struct InputFrame(Scenario, Option<u32>);

/// 每帧记录真实游戏事件与子弹轨迹，防止只看最终状态造成假通过。
#[derive(Resource, Default)]
struct Evidence {
    errors: Vec<String>,
    destroyed: usize,
    bullet_seen: bool,
    bullet_moved: bool,
    last_bullet: Option<Vec3>,
    explosion_seen: bool,
}

/// 截图 observer 的完成结果；必须等编码与落盘完成后才能推进场景。
#[derive(Resource, Default)]
struct CaptureResult(Option<Result<(), String>>);

/// 配置真正无窗口的渲染器；禁用并行渲染以便受控更新后检查 GPU 状态。
pub(super) fn configure(app: &mut App) {
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .set(RenderPlugin {
                synchronous_pipeline_compilation: true,
                ..default()
            })
            .disable::<WinitPlugin>()
            .disable::<PipelinedRenderingPlugin>(),
    );
}

/// 场景搭建后只替换相机输出目标，原有渲染组件完全保留。
fn setup_target(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    cameras: Query<Entity, With<crate::driving::FollowCamera>>,
) {
    let image = images.add(Image::new_target_texture(
        WIDTH,
        HEIGHT,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    for camera in &cameras {
        commands
            .entity(camera)
            .insert(RenderTarget::Image(image.clone().into()));
    }
    commands.insert_resource(Target(image));
}

/// InputPlugin 清理本帧输入后再注入，FixedPostUpdate 驾驶和 PostUpdate 开火均能读到。
fn inject_input(frame: Res<InputFrame>, mut keys: ResMut<ButtonInput<KeyCode>>) {
    let desired = frame.1.map_or(&[][..], |index| frame.0.keys(index));
    let released: Vec<_> = keys
        .get_pressed()
        .filter(|key| !desired.contains(key))
        .copied()
        .collect();
    for key in released {
        keys.release(key);
    }
    for &key in desired {
        keys.press(key);
    }
}

/// Last 中读取，确保本帧 Update 命中消息与 PostUpdate 新生子弹均已提交。
fn collect_evidence(
    mut evidence: ResMut<Evidence>,
    mut failures: MessageReader<UntypedAssetLoadFailedEvent>,
    mut destroyed: MessageReader<crate::enemy::EnemyDestroyed>,
    bullets: Query<&Transform, With<crate::firing::Bullet>>,
    explosions: Query<(), With<crate::explosion::Explosion>>,
) {
    for failure in failures.read() {
        evidence
            .errors
            .push(format!("资源加载失败：{}：{}", failure.path, failure.error));
    }
    evidence.destroyed += destroyed.read().count();
    for bullet in &bullets {
        evidence.bullet_seen = true;
        if evidence
            .last_bullet
            .is_some_and(|last| last.distance(bullet.translation) > 0.1)
        {
            evidence.bullet_moved = true;
        }
        evidence.last_bullet = Some(bullet.translation);
    }
    evidence.explosion_seen |= !explosions.is_empty();
}

/// 完成插件初始化后手动更新；不调用 winit runner，也不创建任何可见窗口。
pub(super) fn run(mut app: App, options: Options) -> Result<(), String> {
    app.insert_resource(options.clone())
        .insert_resource(InputFrame(options.scenario, None))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
        .insert_resource(Time::<Fixed>::from_duration(STEP))
        .init_resource::<Evidence>()
        .init_resource::<CaptureResult>()
        .add_systems(PostStartup, setup_target)
        .add_systems(PreUpdate, inject_input.after(InputSystems))
        .add_systems(Last, collect_evidence);
    while app.plugins_state() == PluginsState::Adding {
        bevy::tasks::tick_global_task_pools_on_main_thread();
        std::thread::sleep(Duration::from_millis(1));
    }
    app.finish();
    app.cleanup();
    let mut report = format!(
        "场景：{:?}\n随机种子：{SEED}\n分辨率：{WIDTH}×{HEIGHT}\n模拟步长：{STEP:?}\n",
        options.scenario
    );
    if let Some(adapter) = app.world().get_resource::<RenderAdapterInfo>() {
        report.push_str(&format!("GPU/后端：{:?}\n", adapter.0));
    }
    let result = run_scenario(&mut app, &options, &mut report);
    report.push_str(&match &result {
        Ok(()) => "结果：通过\n".into(),
        Err(error) => format!("结果：失败\n原因：{error}\n"),
    });
    if !options.single_image {
        write_new(&options.output.join("report.txt"), report.as_bytes())?;
    }
    result
}

/// 每次更新都检查资源失败、退出请求和管线错误；等待时也让出 CPU 并轮询读回。
fn update(app: &mut App, duration: Duration, frame: Option<u32>) -> Result<(), String> {
    app.world_mut()
        .insert_resource(TimeUpdateStrategy::ManualDuration(duration));
    app.world_mut().resource_mut::<InputFrame>().1 = frame;
    app.update();
    ensure(
        app.world_mut()
            .query::<&Window>()
            .iter(app.world())
            .next()
            .is_none(),
        "离屏进程不应创建窗口实体",
    )?;
    if let Some(error) = app.world().resource::<Evidence>().errors.first() {
        return Err(error.clone());
    }
    if let Some(exit) = app.should_exit() {
        return Err(format!("场景在验证完成前请求退出：{exit:?}"));
    }
    pipelines_ready(app)?;
    app.world()
        .resource::<RenderDevice>()
        .wgpu_device()
        .poll(PollType::Poll)
        .map_err(|error| format!("GPU 轮询失败：{error}"))?;
    std::thread::sleep(Duration::from_millis(1));
    Ok(())
}

/// 同步编译仍可能跨帧发现新管线；失败不能伪装为尚未就绪。
fn pipelines_ready(app: &App) -> Result<bool, String> {
    let render = app.sub_app(RenderApp).world();
    let cache = render.resource::<PipelineCache>();
    for pipeline in cache.pipelines() {
        if let CachedPipelineState::Err(error) = &pipeline.state {
            return Err(format!("渲染管线失败：{error}"));
        }
    }
    Ok(cache.waiting_pipelines().next().is_none())
}

/// 同时确认 CPU 场景、依赖、实例和 GPU 网格/纹理，避免仅凭加载日志提前截黑图。
fn scene_ready(app: &mut App) -> bool {
    let world = app.world_mut();
    if !world.contains_resource::<crate::ground::TerrainReady>()
        || !crate::firing::assets_ready(world)
        || !crate::explosion::assets_ready(world)
    {
        return false;
    }
    let tank_count = world
        .query_filtered::<(), Or<(With<crate::firing::Tank>, With<crate::enemy::EnemyTank>)>>()
        .iter(world)
        .count();
    let validated = world
        .query_filtered::<(), With<crate::track::TracksValidated>>()
        .iter(world)
        .count();
    if tank_count != crate::enemy::ENEMY_COUNT + 1 || validated != tank_count {
        return false;
    }
    let mut roots = world.query::<(&WorldAssetRoot, Option<&WorldInstance>)>();
    let server = world.resource::<AssetServer>();
    let spawner = world.resource::<WorldInstanceSpawner>();
    if !roots.iter(world).all(|(root, instance)| {
        server.is_loaded_with_dependencies(&root.0)
            && instance.is_some_and(|instance| spawner.instance_is_ready(**instance))
    }) {
        return false;
    }
    if !world
        .resource::<Assets<Image>>()
        .ids()
        .all(|id| !server.is_managed(id) || server.is_loaded_with_dependencies(id))
    {
        return false;
    }
    gpu_assets_ready(app)
}

/// 检查实际场景引用的网格和渲染用纹理已上传，而非只检查文件加载完成。
fn gpu_assets_ready(app: &mut App) -> bool {
    let world = app.world_mut();
    let meshes: Vec<_> = world
        .query::<&Mesh3d>()
        .iter(world)
        .map(|mesh| mesh.0.id())
        .collect();
    let images: Vec<_> = world
        .resource::<Assets<Image>>()
        .iter()
        .filter(|(_, image)| {
            image
                .asset_usage
                .contains(bevy::asset::RenderAssetUsages::RENDER_WORLD)
        })
        .map(|(id, _)| id)
        .collect();
    let render = app.sub_app(RenderApp).world();
    let gpu_meshes = render.resource::<RenderAssets<RenderMesh>>();
    let gpu_images = render.resource::<RenderAssets<GpuImage>>();
    meshes.iter().all(|&id| gpu_meshes.get(id).is_some())
        && images.iter().all(|&id| gpu_images.get(id).is_some())
}

/// 连续八个完整更新无新 GPU 资源或管线等待后才开始模拟或截图。
fn wait_render(app: &mut App, require_scene: bool) -> Result<(), String> {
    let mut stable = 0;
    while stable < 8 {
        update(app, Duration::ZERO, None)?;
        if pipelines_ready(app)? && gpu_assets_ready(app) && (!require_scene || scene_ready(app)) {
            stable += 1;
        } else {
            stable = 0;
        }
    }
    Ok(())
}

/// 将真实敌人调整为战斗测试布局；仍保留原有动态刚体、碰撞体和资源层级。
fn arrange_combat(world: &mut World) {
    // 旧敌人仅高 1.4 米；显式垫高靶车以验证 2 米炮口的水平命中，不修改敌人规格。
    let mesh = world
        .resource_mut::<Assets<Mesh>>()
        .add(Cuboid::new(4.0, 1.3, 3.4));
    let material = world
        .resource_mut::<Assets<StandardMaterial>>()
        .add(Color::srgb(0.25, 0.27, 0.24));
    world.spawn((
        avian3d::prelude::RigidBody::Static,
        avian3d::prelude::Collider::cuboid(4.0, 1.3, 3.4),
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_xyz(-12.0, 0.65, 0.0),
    ));
    let mut query = world.query_filtered::<Entity, With<crate::enemy::EnemyTank>>();
    let mut enemies: Vec<_> = query.iter(world).collect();
    enemies.sort_by_key(|entity| entity.to_bits());
    for (index, entity) in enemies.into_iter().enumerate() {
        let position = if index == 0 {
            Vec3::new(-12.0, 3.0, 0.0)
        } else {
            Vec3::new(30.0 + index as f32 * 6.0, 3.0, 30.0)
        };
        world.entity_mut(entity).insert((
            Transform::from_translation(position),
            Position(position),
            Rotation::default(),
            LinearVelocity::ZERO,
        ));
    }
}

/// 捕获用于断言的游戏状态；坐标为世界米制，履带值为两侧累计里程。
fn player_state(world: &mut World) -> Result<(Transform, [f32; 2]), String> {
    let mut query =
        world.query_filtered::<(&Transform, &crate::track::TrackAnim), With<crate::firing::Tank>>();
    query
        .single(world)
        .map(|(transform, tracks)| (*transform, tracks.distances()))
        .map_err(|error| format!("玩家或履带状态异常：{error}"))
}

/// 场景与相机的最低运行断言，图片只作为额外证据而非替代状态检查。
fn validate_scene(world: &mut World) -> Result<(), String> {
    world
        .run_system_cached(validate_scene_state)
        .map_err(|e| format!("场景断言系统异常：{e}"))?
}

/// 从上方向实际地形碰撞体投射射线，检查车体底面，不能只用全局高度范围判断穿地。
fn validate_scene_state(
    spatial: SpatialQuery,
    terrain: Query<(), With<crate::ground::TerrainSurface>>,
    tanks: Query<
        (&Transform, Has<crate::firing::Tank>),
        Or<(With<crate::firing::Tank>, With<crate::enemy::EnemyTank>)>,
    >,
    players: Query<&Transform, With<crate::firing::Tank>>,
    cameras: Query<&Transform, With<crate::driving::FollowCamera>>,
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
            crate::driving::TANK_SIZE.y
        } else {
            crate::enemy::ENEMY_SIZE.y
        } / 2.0;
        ensure(
            tank.translation.y - half_height >= origin.y - hit.distance - 0.1,
            "坦克底面穿入地形超过 0.1 米",
        )?;
    }
    let camera = cameras.single().map_err(|e| format!("跟随相机异常：{e}"))?;
    let expected = crate::driving::camera_transform(&player);
    // 直接比较分量，避免 acos 将同一姿态的归一化舍入误差放大成角度误差。
    ensure(
        camera.translation.distance(expected.translation) < 0.01
            && camera.rotation.abs_diff_eq(expected.rotation, 0.00001),
        "相机未跟随玩家",
    )
}

/// 执行固定预热、模拟、状态断言及关键帧输出；截图等待不推进游戏时间。
fn run_scenario(app: &mut App, options: &Options, report: &mut String) -> Result<(), String> {
    wait_render(app, true)?;
    if options.scenario == Scenario::Combat {
        arrange_combat(app.world_mut());
        report.push_str("战斗布局：旧尺寸靶车放在 1.3 米实体台座上；平地掠过另由逻辑测试验证。\n");
    }
    for _ in 0..120 {
        update(app, STEP, None)?;
    }
    validate_scene(app.world_mut())?;
    let (start, start_tracks) = player_state(app.world_mut())?;
    report.push_str(&format!(
        "预热：120 帧；断言通过：地形就绪、坦克位置、相机跟随\n起点：{:?}\n",
        start.translation
    ));
    let world = app.world_mut();
    let mut enemies: Vec<_> = world
        .query_filtered::<&Transform, With<crate::enemy::EnemyTank>>()
        .iter(world)
        .map(|transform| transform.translation)
        .collect();
    enemies.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.z.total_cmp(&b.z)));
    report.push_str(&format!("敌人布局：{enemies:?}\n"));
    let mut failures = Vec::new();
    capture(app, &options.image_path(0))?;
    report.push_str(&format!(
        "模拟帧：0；图片：{}\n",
        options.image_path(0).display()
    ));
    for frame in 1..=*options.scenario.frames().last().unwrap() {
        update(app, STEP, Some(frame))?;
        validate_scene(app.world_mut())?;
        if options.scenario == Scenario::Drive && frame == 60 {
            let (state, tracks) = player_state(app.world_mut())?;
            report.push_str(&format!(
                "直行检查：位置={:?}，履带={tracks:?}，游戏时间={:?}，固定时间={:?}\n",
                state.translation,
                app.world().resource::<Time<Virtual>>().elapsed(),
                app.world().resource::<Time<Fixed>>().elapsed()
            ));
            // 出生平地内 1 秒应前进约 6 米；幽灵接触阻塞时只有约 0.15 米，
            // 4 米阈值保持强判别力，同时不依赖平地之外的地形起伏。
            record_check(
                report,
                &mut failures,
                (state.translation - start.translation).dot(start.rotation * Vec3::NEG_X) > 4.0,
                "直行前进至少 4 米",
            );
            record_check(
                report,
                &mut failures,
                tracks
                    .iter()
                    .zip(start_tracks)
                    .all(|(now, before)| (now - before).abs() > 4.0),
                "直行履带累计里程大于 4 米",
            );
        }
        if options.scenario == Scenario::Drive && frame == 180 {
            let (state, _) = player_state(app.world_mut())?;
            record_check(
                report,
                &mut failures,
                state.rotation.angle_between(start.rotation) > 1.0,
                "转弯偏航变化大于 1 弧度",
            );
        }
        if options.scenario == Scenario::Combat && frame == 24 {
            let evidence = app.world().resource::<Evidence>();
            record_check(
                report,
                &mut failures,
                evidence.bullet_seen
                    && evidence.bullet_moved
                    && evidence.destroyed == 1
                    && evidence.explosion_seen,
                "子弹生成并飞行、单个敌人击毁及爆炸生成",
            );
        }
        if options.scenario == Scenario::Combat && frame == 120 {
            let world = app.world_mut();
            record_check(
                report,
                &mut failures,
                world
                    .query_filtered::<(), With<crate::enemy::EnemyTank>>()
                    .iter(world)
                    .count()
                    == crate::enemy::ENEMY_COUNT - 1,
                "单次命中只清理一辆敌人",
            );
            record_check(
                report,
                &mut failures,
                world
                    .query_filtered::<(), With<crate::explosion::Explosion>>()
                    .iter(world)
                    .count()
                    == 0,
                "爆炸清理完成",
            );
            record_check(
                report,
                &mut failures,
                world
                    .query_filtered::<(), With<crate::firing::Bullet>>()
                    .iter(world)
                    .count()
                    == 0,
                "命中子弹清理完成",
            );
        }
        if options.scenario.frames().contains(&frame) {
            let (state, tracks) = player_state(app.world_mut())?;
            report.push_str(&format!(
                "模拟帧：{frame}；位置：{:?}；旋转：{:?}；履带里程：{tracks:?}\n",
                state.translation, state.rotation
            ));
            capture(app, &options.image_path(frame))?;
            report.push_str(&format!("图片：{}\n", options.image_path(frame).display()));
        }
    }
    ensure(failures.is_empty(), &failures.join("；"))?;
    info!(
        "离屏验证通过：{:?}，输出：{}",
        options.scenario,
        options.output.display()
    );
    Ok(())
}

/// 请求一次截图并等待 observer 完成保存，绝不以请求已提交作为成功判据。
fn capture(app: &mut App, path: &Path) -> Result<(), String> {
    wait_render(app, false)?;
    app.world_mut().resource_mut::<CaptureResult>().0 = None;
    let target = app.world().resource::<Target>().0.clone();
    let path = path.to_owned();
    app.world_mut().spawn(Screenshot::image(target)).observe(
        move |event: On<ScreenshotCaptured>, mut result: ResMut<CaptureResult>| {
            result.0 = Some(save_image(&event.image, &path));
        },
    );
    loop {
        update(app, Duration::ZERO, None)?;
        if let Some(result) = app.world_mut().resource_mut::<CaptureResult>().0.take() {
            return result;
        }
    }
}

/// RGB 编码会丢弃 HDR 输出中用于亮度的 alpha，与 Bevy 官方截图行为一致。
fn save_image(image: &Image, path: &Path) -> Result<(), String> {
    let rgb = image
        .clone()
        .try_into_dynamic()
        .map_err(|error| format!("截图转换失败：{error}"))?
        .to_rgb8();
    ensure(
        rgb.width() == WIDTH && rgb.height() == HEIGHT,
        "截图尺寸不符",
    )?;
    validate_pixels(rgb.as_raw())?;
    let mut file = create_new(path)?;
    rgb.write_to(
        &mut file,
        ImageFormat::Png
            .as_image_crate_format()
            .ok_or("未启用 PNG 编码")?,
    )
    .map_err(|error| format!("PNG 编码或写入失败：{error}"))?;
    file.sync_all()
        .map_err(|error| format!("截图落盘失败：{error}"))?;
    Ok(())
}

/// 排除纯色/黑色等明显空图；具体模型和材质效果仍需读取关键帧人工检查。
fn validate_pixels(rgb: &[u8]) -> Result<(), String> {
    ensure(
        rgb.len() == (WIDTH * HEIGHT * 3) as usize,
        "截图像素长度不符",
    )?;
    let first = &rgb[..3];
    ensure(
        rgb.chunks_exact(3).filter(|pixel| *pixel != first).count()
            > (WIDTH * HEIGHT / 100) as usize,
        "截图缺少实际场景内容（接近纯色）",
    )
}

/// 将断言失败转换为可报告的运行错误，不依赖 panic 作为正常失败路径。
fn ensure(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

/// 行为断言失败仍收集后续关键帧，最终统一失败退出，保留完整诊断证据。
fn record_check(report: &mut String, failures: &mut Vec<String>, condition: bool, message: &str) {
    let status = if condition {
        "通过"
    } else {
        failures.push(message.into());
        "失败"
    };
    report.push_str(&format!("断言{status}：{message}\n"));
}

/// 发送端随运行结束而释放即取消计时；测试可使用短时限验证同一调度逻辑。
fn spawn_watchdog(
    timeout: Duration,
    on_timeout: impl FnOnce() + Send + 'static,
) -> mpsc::Sender<()> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        if receiver.recv_timeout(timeout) == Err(mpsc::RecvTimeoutError::Timeout) {
            on_timeout();
        }
    });
    sender
}

/// 排他创建产物；即使预检查后有其它进程写入，也不会覆盖已有文件。
fn create_new(path: &Path) -> Result<File, String> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("无法创建产物 {}：{error}", path.display()))
}

/// 报告也按排他创建、完整写入与刷新处理，不吞掉磁盘错误。
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = create_new(path)?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("写入报告失败：{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Option<Options>, String> {
        Options::parse(args.iter().map(Into::into).collect())
    }

    #[test]
    fn command_line_keeps_normal_game_and_accepts_documented_commands() {
        assert_eq!(parse(&[]).unwrap(), None);
        let capture = parse(&["--capture", "截图.png"]).unwrap().unwrap();
        assert!(capture.single_image);
        assert_eq!(capture.scenario, Scenario::Scene);
        for (name, scenario) in [
            ("scene", Scenario::Scene),
            ("drive", Scenario::Drive),
            ("combat", Scenario::Combat),
        ] {
            let options = parse(&["--offscreen-test", name, "--output", "out"])
                .unwrap()
                .unwrap();
            assert_eq!(options.scenario, scenario);
            assert!(!options.single_image);
        }
    }

    #[test]
    fn invalid_arguments_fail_before_starting_renderer() {
        for args in [
            vec!["--capture"],
            vec!["--capture", "a.jpg"],
            vec!["--capture", ""],
            vec!["--offscreen-test", "unknown", "--output", "out"],
            vec!["--offscreen-test", "scene"],
            vec!["--unknown"],
            vec!["--capture", "a.png", "extra"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }

    #[test]
    fn scenario_inputs_stop_at_boundaries_and_fire_only_once() {
        assert!(Scenario::Drive.keys(0).is_empty());
        assert_eq!(Scenario::Drive.keys(60), &[KeyCode::ArrowUp]);
        assert_eq!(
            Scenario::Drive.keys(61),
            &[KeyCode::ArrowUp, KeyCode::ArrowLeft]
        );
        assert!(Scenario::Drive.keys(181).is_empty());
        assert_eq!(Scenario::Combat.keys(1), &[KeyCode::Space]);
        assert!(Scenario::Combat.keys(2).is_empty());
        assert!(Scenario::Combat.keys(120).is_empty());
        assert!(Scenario::Scene.keys(1).is_empty());
    }

    #[test]
    fn output_creation_refuses_existing_files_and_reports_io_failures() {
        let directory = std::env::temp_dir().join(format!(
            "tanker-offscreen-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = directory.join("nested/capture.png");
        let options = Options {
            scenario: Scenario::Scene,
            output: path.clone(),
            single_image: true,
        };
        options.prepare().unwrap();
        write_new(&path, b"existing evidence").unwrap();
        assert!(options.prepare().is_err());
        assert!(write_new(&path, b"overwrite").is_err());
        assert_eq!(fs::read(&path).unwrap(), b"existing evidence");
        assert!(create_new(&directory).is_err());
        fs::remove_file(path).unwrap();
        fs::remove_dir(directory.join("nested")).unwrap();
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn empty_or_solid_images_and_failed_assertions_cannot_pass() {
        assert!(validate_pixels(&[]).is_err());
        assert!(validate_pixels(&vec![0; (WIDTH * HEIGHT * 3) as usize]).is_err());
        let mut pixels = vec![0; (WIDTH * HEIGHT * 3) as usize];
        pixels[..(WIDTH * HEIGHT * 3 / 2) as usize].fill(128);
        assert!(validate_pixels(&pixels).is_ok());
        assert!(ensure(false, "断言失败").is_err());
        let mut report = String::new();
        let mut failures = Vec::new();
        record_check(&mut report, &mut failures, false, "驾驶位移");
        record_check(&mut report, &mut failures, true, "相机跟随");
        assert_eq!(failures, ["驾驶位移"]);
        assert!(report.contains("断言失败：驾驶位移"));
        assert!(ensure(failures.is_empty(), &failures.join("；")).is_err());
    }

    #[test]
    fn watchdog_fires_on_timeout_and_cancels_when_runner_finishes() {
        let (expired, notification) = mpsc::channel();
        let guard = spawn_watchdog(Duration::from_millis(5), move || expired.send(()).unwrap());
        notification.recv_timeout(Duration::from_secs(1)).unwrap();
        drop(guard);
        let (expired, notification) = mpsc::channel();
        let guard = spawn_watchdog(Duration::from_secs(1), move || expired.send(()).unwrap());
        drop(guard);
        assert_eq!(
            notification.recv_timeout(Duration::from_secs(1)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        );
    }

    #[test]
    fn injected_input_drives_real_physics_for_every_fixed_step() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            bevy::input::InputPlugin,
            avian3d::prelude::PhysicsPlugins::default(),
            crate::driving::DrivingPlugin,
        ))
        .insert_resource(InputFrame(Scenario::Drive, None))
        .insert_resource(TimeUpdateStrategy::ManualDuration(STEP))
        .insert_resource(Time::<Fixed>::from_duration(STEP))
        .add_systems(PreUpdate, inject_input.after(InputSystems))
        .add_systems(Startup, |mut commands: Commands| {
            crate::ground::spawn_arena(&mut commands)
        });
        app.finish();
        app.cleanup();
        let player = app
            .world_mut()
            .spawn((
                crate::firing::Tank,
                crate::driving::tank_body(),
                Transform::from_xyz(0.0, 0.75, 0.0),
            ))
            .id();
        for _ in 0..120 {
            app.update();
        }
        for frame in 1..=180 {
            // Drive 场景从 61 帧起注入转向键；本测试验证的是持续直行，
            // 故把注入帧钳制在场景的直行区间内。
            app.world_mut().resource_mut::<InputFrame>().1 = Some(frame.min(60));
            app.update();
            if frame == 60 {
                let position = app.world().get::<Transform>(player).unwrap().translation;
                let elapsed = app.world().resource::<Time<Fixed>>().elapsed();
                app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
                app.world_mut().resource_mut::<InputFrame>().1 = None;
                for _ in 0..8 {
                    app.update();
                }
                assert_eq!(
                    app.world().get::<Transform>(player).unwrap().translation,
                    position
                );
                assert_eq!(app.world().resource::<Time<Fixed>>().elapsed(), elapsed);
                app.insert_resource(TimeUpdateStrategy::ManualDuration(STEP));
            }
        }
        let position = app.world().get::<Transform>(player).unwrap().translation;
        assert!(position.x < -17.0, "输入未持续生效：{position:?}");
    }

    #[test]
    fn scene_assertions_detect_penetration_against_actual_collider() {
        use avian3d::prelude::{Collider, PhysicsPlugins, RigidBody};
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            PhysicsPlugins::default(),
            crate::driving::DrivingPlugin,
        ))
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(STEP));
        app.finish();
        app.cleanup();
        app.world_mut().spawn((
            crate::ground::TerrainSurface,
            RigidBody::Static,
            Collider::cuboid(200.0, 1.0, 200.0),
            Transform::from_xyz(0.0, -0.5, 0.0),
        ));
        let player = app
            .world_mut()
            .spawn((
                crate::firing::Tank,
                crate::driving::tank_body(),
                Transform::from_xyz(0.0, 0.75, 0.0),
            ))
            .id();
        app.world_mut()
            .spawn((crate::driving::FollowCamera, Transform::default()));
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

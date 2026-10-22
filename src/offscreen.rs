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
pub(crate) const WIDTH: u32 = 1280;
pub(crate) const HEIGHT: u32 = 720;
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
    /// 所选车型轮组的侧面动画与三视点对照。
    Wheels,
    /// 主菜单首页：验证 UI 结构、键盘流转与到真实场景的过渡。
    Menu,
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
            Self::Combat => &[0, 6, 24, 48, 60, 120],
            Self::Wheels => &[
                0, 5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55, 60, 75, 90, 105, 120, 130, 240, 250,
            ],
            // 菜单关键帧按真实阶段动态计帧，固定列表只声明基准图。
            Self::Menu => &[0],
        }
    }

    /// 按固定模拟帧注入按键，经过游戏现有输入系统，而非直接调用业务逻辑。
    fn keys(self, frame: u32) -> &'static [KeyCode] {
        match (self, frame) {
            (Self::Drive, 1..=60) => &[KeyCode::ArrowUp],
            (Self::Drive, 61..=180) => &[KeyCode::ArrowUp, KeyCode::ArrowLeft],
            (Self::Combat, 1) => &[KeyCode::Space],
            (Self::Wheels, 1..=30) => &[KeyCode::ArrowUp],
            (Self::Wheels, 31..=60) => &[KeyCode::ArrowDown],
            (Self::Wheels, 61..=90) => &[KeyCode::ArrowLeft],
            (Self::Wheels, 91..=120) => &[KeyCode::ArrowRight],
            (Self::Menu, 1) => &[KeyCode::ArrowDown],
            (Self::Menu, 3) => &[KeyCode::Enter],
            (Self::Menu, 5) => &[KeyCode::Escape],
            (Self::Menu, 7) => &[KeyCode::Tab],
            (Self::Menu, 8) => &[KeyCode::Tab, KeyCode::ShiftLeft],
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
    /// 仅 wheels 接受 --model enemy；默认继续验证玩家。
    enemy_wheels: bool,
}

impl Options {
    /// 无参数返回普通游戏；其余参数必须完整匹配已公布的接口。
    pub(super) fn parse(args: Vec<OsString>) -> Result<Option<Self>, String> {
        let usage = "用法：--capture <图片.png> 或 --offscreen-test <scene|drive|combat|wheels|menu> --output <目录> [--model enemy]（仅 wheels）";
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
                enemy_wheels: false,
            }
        } else if (args.len() == 4 || args.len() == 6)
            && args[0] == "--offscreen-test"
            && args[2] == "--output"
        {
            let scenario = match args[1].to_str() {
                Some("scene") => Scenario::Scene,
                Some("drive") => Scenario::Drive,
                Some("combat") => Scenario::Combat,
                Some("wheels") => Scenario::Wheels,
                Some("menu") => Scenario::Menu,
                _ => return Err(usage.into()),
            };
            let enemy_wheels = args.len() == 6;
            if enemy_wheels
                && (scenario != Scenario::Wheels || args[4] != "--model" || args[5] != "enemy")
            {
                return Err(usage.into());
            }
            Self {
                scenario,
                output: PathBuf::from(&args[3]),
                single_image: false,
                enemy_wheels,
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
            if self.scenario == Scenario::Wheels {
                for name in ["side", "front", "rear", "bounds", "comparison"] {
                    paths.push(self.output.join(format!("{name}.png")));
                }
            }
        }
        paths
    }
}

/// 最终合成相机的离屏目标；内容纹理与此输出纹理始终分离。
#[derive(Resource)]
pub(crate) struct Target(pub(crate) Handle<Image>);

/// 下一次更新要注入的模拟帧；None 用于加载、预热和截图等待。
#[derive(Resource)]
struct InputFrame(Scenario, Option<u32>);

/// 下一帧菜单鼠标按下状态，与窗口输入清理时序一致。
#[derive(Resource, Default)]
struct MenuMouse(bool);

/// wheels 测试显式驱动的敌人；正常游戏不创建，不改变玩家标记。
#[derive(Resource)]
struct WheelTarget(Entity);

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

/// 幂等把游戏相机挂到离屏图像目标；相机可能随状态进入（菜单相机在首个
/// StateTransition 生成、跟随相机在进入 InGame 时生成），因此常驻 PreUpdate，
/// 只补挂尚未指向离屏图像的相机，已挂载的相机不重复插入。
fn setup_target(
    mut commands: Commands,
    target: Option<Res<Target>>,
    mut images: ResMut<Assets<Image>>,
    cameras: Query<(Entity, &RenderTarget), With<crate::transition::OutputCamera>>,
) {
    let image = match target {
        Some(target) => target.0.clone(),
        None => {
            let image = images.add(Image::new_target_texture(
                WIDTH,
                HEIGHT,
                TextureFormat::Rgba8UnormSrgb,
                None,
            ));
            commands.insert_resource(Target(image.clone()));
            image
        }
    };
    for (camera, render_target) in &cameras {
        if render_target.as_image() != Some(&image) {
            commands
                .entity(camera)
                .insert(RenderTarget::Image(image.clone().into()));
        }
    }
}

/// InputPlugin 清理本帧输入后再注入，FixedPostUpdate 驾驶和 PostUpdate 开火均能读到。
fn inject_input(
    frame: Res<InputFrame>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    target: Option<Res<WheelTarget>>,
    mut inputs: Query<&mut crate::driving::DrivingInput>,
    mouse_script: Option<Res<MenuMouse>>,
    mouse: Option<ResMut<ButtonInput<MouseButton>>>,
) {
    if let (Some(script), Some(mut mouse)) = (mouse_script, mouse) {
        if script.0 {
            mouse.press(MouseButton::Left);
        } else {
            mouse.release(MouseButton::Left);
        }
    }
    let mut desired = frame.1.map_or(&[][..], |index| frame.0.keys(index));
    if let Some(target) = target {
        if let Ok(mut input) = inputs.get_mut(target.0) {
            input.throttle = i32::from(desired.contains(&KeyCode::ArrowUp)) as f32
                - i32::from(desired.contains(&KeyCode::ArrowDown)) as f32;
            input.turn = i32::from(desired.contains(&KeyCode::ArrowLeft)) as f32
                - i32::from(desired.contains(&KeyCode::ArrowRight)) as f32;
        }
        desired = &[];
    }
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
        .init_resource::<MenuMouse>()
        .add_systems(PreUpdate, (setup_target, inject_input.after(InputSystems)))
        .add_systems(Last, collect_evidence);
    if options.scenario != Scenario::Menu {
        // 旧场景在首个 StateTransition 前直接请求 InGame 跳过菜单：
        // 启动期 StateTransition 应用 InGame 并运行 OnEnter(setup_scene)，
        // 引导消息与 Pending 取 .last() 后以 InGame 为准，不会落入菜单。
        app.world_mut()
            .resource_mut::<NextState<crate::GameState>>()
            .set(crate::GameState::InGame);
    }
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
        // 0.20 起 RenderAdapterInfo 内部字段私有化，经 Deref<AdapterInfo> 取名称与后端。
        report.push_str(&format!(
            "GPU/后端：{} ({:?})\n",
            adapter.name, adapter.backend
        ));
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
    let world = app.world_mut();
    if let Some(failure) = world
        .query::<&crate::track::TracksFailed>()
        .iter(world)
        .next()
    {
        return Err(failure.0.clone());
    }
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
    let mut pending = false;
    for pipeline in cache.pipelines() {
        if let CachedPipelineState::Err(error) = &pipeline.state {
            if shader_pending(error) {
                pending = true;
                continue;
            }
            return Err(format!("渲染管线失败：{error}"));
        }
    }
    Ok(!pending && cache.waiting_pipelines().next().is_none())
}

/// 与锁定版 PipelineCache 的重试规则一致；实际 shader 处理和模块创建错误仍失败。
fn shader_pending(error: &bevy::shader::ShaderCacheError) -> bool {
    matches!(
        error,
        bevy::shader::ShaderCacheError::ShaderNotLoaded(_)
            | bevy::shader::ShaderCacheError::ShaderImportNotYetAvailable
    )
}

/// 同时确认 CPU 场景、依赖、实例和 GPU 网格/纹理，避免仅凭加载日志提前截黑图。
fn scene_ready(app: &mut App) -> bool {
    let world = app.world_mut();
    if !world.contains_resource::<crate::ground::TerrainReady>()
        || !crate::firing::assets_ready(world)
        || !crate::explosion::assets_ready(world)
        || !crate::font::assets_ready(world)
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
    // 新敌人与玩家同高，在真实平地直接验证水平命中。
    let mut query = world.query_filtered::<Entity, With<crate::enemy::EnemyTank>>();
    let mut enemies: Vec<_> = query.iter(world).collect();
    enemies.sort_by_key(|entity| entity.to_bits());
    for (index, entity) in enemies.into_iter().enumerate() {
        let position = if index == 0 {
            Vec3::new(-12.0, 3.0, 0.0)
        } else {
            Vec3::new(30.0 + index as f32 * 10.0, 3.0, 30.0)
        };
        world.entity_mut(entity).insert((
            Transform::from_translation(position),
            Position(position),
            Rotation::default(),
            LinearVelocity::ZERO,
        ));
    }
}

/// 捕获当前验收对象状态；默认玩家，敌人 wheels 预设显式选择测试靶车。
fn subject_state(world: &mut World) -> Result<(Transform, [f32; 2]), String> {
    if let Some(target) = world.get_resource::<WheelTarget>() {
        let entity = world.entity(target.0);
        return entity
            .get::<Transform>()
            .zip(entity.get::<crate::track::TrackAnim>())
            .map(|(transform, tracks)| (*transform, tracks.distances()))
            .ok_or_else(|| "敌人轮组状态异常".into());
    }
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
    if options.scenario == Scenario::Menu {
        return run_menu_scenario(app, options, report);
    }
    wait_render(app, true)?;
    if options.scenario == Scenario::Combat {
        arrange_combat(app.world_mut());
        report.push_str("战斗布局：与玩家同高的靶车在平地直接承受水平射击，无台座。\n");
    }
    if options.enemy_wheels {
        let world = app.world_mut();
        let enemy = world
            .query_filtered::<Entity, With<crate::enemy::EnemyTank>>()
            .iter(world)
            .min_by_key(|e| e.to_bits())
            .ok_or("未找到敌人轮组目标")?;
        let position = Vec3::new(0.0, 3.0, 9.0);
        world.entity_mut(enemy).insert((
            Transform::from_translation(position),
            Position(position),
            Rotation::default(),
            LinearVelocity::ZERO,
        ));
        world.insert_resource(WheelTarget(enemy));
    }
    report.push_str(if options.enemy_wheels {
        "轮组目标：敌人4+2\n"
    } else {
        "轮组目标：玩家5+2\n"
    });
    for _ in 0..120 {
        update(app, STEP, None)?;
    }
    validate_scene(app.world_mut())?;
    let (start, start_tracks) = subject_state(app.world_mut())?;
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
    let mut previous_tracks = start_tracks;
    if options.scenario == Scenario::Wheels {
        capture_wheel_views(app, &options.output)?;
        capture_side(app, &options.image_path(0))?;
    } else {
        capture(app, &options.image_path(0))?;
    }
    report.push_str(&format!(
        "模拟帧：0；图片：{}\n",
        options.image_path(0).display()
    ));
    for frame in 1..=*options.scenario.frames().last().unwrap() {
        update(app, STEP, Some(frame))?;
        validate_scene(app.world_mut())?;
        if options.scenario == Scenario::Wheels && [30, 60, 90, 120, 240, 250].contains(&frame) {
            let (_, tracks) = subject_state(app.world_mut())?;
            let delta = [
                tracks[0] - previous_tracks[0],
                tracks[1] - previous_tracks[1],
            ];
            let passed = match frame {
                30 => delta.iter().all(|d| *d > 2.9),
                60 => delta.iter().all(|d| *d < -1.9),
                90 => delta[0] > 0.5 && delta[1] < -0.5,
                120 => delta[0] < -0.3 && delta[1] > 0.3,
                250 => delta.iter().all(|d| d.abs() < 0.0001),
                _ => true,
            };
            record_check(
                report,
                &mut failures,
                passed,
                &format!("轮组输入方向与停车：帧{frame}，左右里程变化{delta:?}"),
            );
            previous_tracks = tracks;
        }
        if options.scenario == Scenario::Drive && frame == 60 {
            let (state, tracks) = subject_state(app.world_mut())?;
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
            let (state, _) = subject_state(app.world_mut())?;
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
            let (state, tracks) = subject_state(app.world_mut())?;
            report.push_str(&format!(
                "模拟帧：{frame}；位置：{:?}；旋转：{:?}；履带里程：{tracks:?}\n",
                state.translation, state.rotation
            ));
            if options.scenario == Scenario::Wheels {
                capture_side(app, &options.image_path(frame))?;
            } else {
                capture(app, &options.image_path(frame))?;
            }
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

/// 菜单回归共用真实输入和渲染链路；全黑交接帧单独验收。
fn run_menu_scenario(app: &mut App, options: &Options, report: &mut String) -> Result<(), String> {
    use crate::transition::{Fade, Phase};
    wait_render(app, false)?;
    while !crate::transition::content_ready(app.world_mut()) {
        update(app, Duration::ZERO, None)?;
    }
    ensure(
        app.world()
            .resource::<crate::menu::MenuInput>()
            .focus
            .is_none(),
        "初始首页不应选中菜单项",
    )?;
    let orders: Vec<_> = app
        .world_mut()
        .query::<&crate::menu::MenuItem>()
        .iter(app.world())
        .map(|item| item.order)
        .collect();
    ensure(
        orders.len() == 2 && orders.contains(&0) && orders.contains(&1),
        "首页必须含两项可交互菜单",
    )?;
    capture(app, &options.image_path(0))?;
    report.push_str("初始首页：无焦点；两项菜单；已保存 frame-000000.png\n");
    // 保留两个相隔四分之一周期的漂浮关键帧，便于检查实际运动幅度。
    for frame in 1..=210 {
        update(app, STEP, None)?;
        if frame == 105 || frame == 210 {
            capture(app, &options.output.join(format!("drift-{frame:03}.png")))?;
        }
    }
    // UI 遮罩受控对照：移除后必须恢复，不能据历史失败认定引擎永久黑屏。
    let camera = app
        .world_mut()
        .query_filtered::<Entity, With<crate::menu::MenuCamera>>()
        .single(app.world())
        .map_err(|e| e.to_string())?;
    let overlay = app
        .world_mut()
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                position_type: PositionType::Absolute,
                ..default()
            },
            UiTargetCamera(camera),
            GlobalZIndex(100),
            BackgroundColor(Color::BLACK.with_alpha(0.5)),
        ))
        .id();
    capture(app, &options.output.join("control-ui-overlay.png"))?;
    app.world_mut().despawn(overlay);
    capture(app, &options.output.join("control-ui-removed.png"))?;
    report.push_str("受控对照：全屏 UI 遮罩添加与移除均正常渲染；无永久黑屏\n");
    for i in 0..2 {
        let l = *app.world().resource::<crate::menu::Layout>();
        app.world_mut()
            .resource_mut::<crate::menu::MenuInput>()
            .pointer = Some(Vec2::new(
            l.hero.x + 50.0,
            l.menu_y + 20.0 + l.row_height * i as f32,
        ));
        for _ in 0..20 {
            update(app, STEP, None)?;
        }
        ensure(
            app.world().resource::<crate::menu::MenuInput>().hover == Some(i),
            "鼠标未命中菜单项",
        )?;
        capture(app, &options.output.join(format!("hover-{}.png", i + 1)))?;
        if i == 1 {
            app.world_mut().resource_mut::<MenuMouse>().0 = true;
            update(app, STEP, None)?;
            app.world_mut().resource_mut::<MenuMouse>().0 = false;
            update(app, STEP, None)?;
            ensure(
                app.world().resource::<Fade>().phase == Phase::Idle,
                "Gallery 点击不得发起切换",
            )?;
        }
    }
    app.world_mut()
        .resource_mut::<crate::menu::MenuInput>()
        .pointer = None;
    for _ in 0..20 {
        update(app, STEP, None)?;
    }
    // Tab 到第一项、再到 Gallery，Enter 不触发业务；Shift+Tab 返回第一项。
    for key_frame in [7, 7, 3] {
        update(app, STEP, Some(key_frame))?;
        update(app, STEP, None)?;
    }
    ensure(
        app.world().resource::<Fade>().phase == Phase::Idle,
        "Gallery Enter 不得发起切换",
    )?;
    update(app, STEP, Some(8))?;
    update(app, STEP, None)?;
    ensure(
        app.world().resource::<crate::menu::MenuInput>().focus == Some(0),
        "Shift+Tab 应返回第一项",
    )?;
    for _ in 0..20 {
        update(app, STEP, None)?;
    }
    capture(app, &options.output.join("keyboard-focus.png"))?;
    for (width, height) in [
        (1920, 1080),
        (901, 720),
        (900, 720),
        (641, 720),
        (640, 720),
        (480, 800),
        (1280, 720),
    ] {
        let size = bevy::render::render_resource::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let handles = [
            app.world().resource::<Target>().0.clone(),
            app.world()
                .resource::<crate::transition::SceneTarget>()
                .0
                .clone(),
        ];
        for handle in handles {
            app.world_mut()
                .resource_mut::<Assets<Image>>()
                .get_mut(&handle)
                .unwrap()
                .resize(size);
        }
        capture(
            app,
            &options.output.join(format!("layout-{width}x{height}.png")),
        )?;
        report.push_str(&format!("整体缩放布局：{width}×{height}\n"));
    }
    let mut frame = 0u32;
    let mut session_instances = Vec::new();
    for cycle in 1..=3 {
        if cycle > 1 {
            update(app, STEP, Some(1))?;
            update(app, STEP, None)?;
        }
        for (direction, key_frame, target) in [
            ("enter", 3, crate::GameState::InGame),
            ("return", 5, crate::GameState::Menu),
        ] {
            update(app, STEP, Some(key_frame))?;
            let mut saved = [false; 3];
            let mut iterations = 0;
            loop {
                let phase = app.world().resource::<Fade>().phase;
                let alpha = app.world().resource::<Fade>().alpha;
                // 加载期间也推进正常墙钟步长，验证游戏自身冻结物理，而非靠测试停钟。
                update(app, STEP, None)?;
                frame += 1;
                let new_phase = app.world().resource::<Fade>().phase;
                let new_alpha = app.world().resource::<Fade>().alpha;
                let slot = if phase == Phase::Out && (0.45..=0.55).contains(&alpha) {
                    Some(0)
                } else if alpha == 1.0 && phase == Phase::Waiting {
                    Some(1)
                } else if phase == Phase::In && (0.45..=0.55).contains(&alpha) {
                    Some(2)
                } else {
                    None
                };
                if cycle == 1 {
                    if let Some(slot) = slot.filter(|&slot| !saved[slot]) {
                        saved[slot] = true;
                        let path = options.image_path(frame);
                        capture_checked(app, &path, slot == 1)?;
                        report.push_str(&format!(
                            "{}\n",
                            crate::transition::diagnostics(app.world_mut())
                        ));
                        report.push_str(&format!("第{cycle}轮 {direction}：阶段={phase:?} alpha={new_alpha:.4} frame={frame} 图片={}\n",path.display()));
                    }
                }
                iterations += 1;
                ensure(iterations < 20000, "页面过渡未能完成")?;
                if new_phase == Phase::Idle && new_alpha == 0.0 {
                    break;
                }
            }
            ensure(
                *app.world().resource::<State<crate::GameState>>().get() == target,
                "过渡未到达目标页面",
            )?;
            if cycle == 1 {
                ensure(saved.iter().all(|saved| *saved), "缺少双向过渡关键阶段截图")?;
            }
            for _ in 0..120 {
                update(app, STEP, None)?;
                frame += 1;
            }
            ensure(
                app.world().resource::<Fade>().phase == Phase::Idle
                    && app.world().resource::<Fade>().alpha == 0.0,
                "过渡结束后黑幕重新出现",
            )?;
            if target == crate::GameState::InGame {
                validate_scene(app.world_mut())?;
                session_instances = app
                    .world_mut()
                    .query::<&WorldInstance>()
                    .iter(app.world())
                    .map(|instance| **instance)
                    .collect();
                ensure(
                    app.world_mut()
                        .query_filtered::<(), With<crate::menu::MenuRoot>>()
                        .iter(app.world())
                        .count()
                        == 0,
                    "游戏中残留首页",
                )?;
                let follows = app
                    .world_mut()
                    .query_filtered::<(), With<crate::driving::FollowCamera>>()
                    .iter(app.world())
                    .count();
                ensure(follows == 1, "游戏相机重复生成")?;
                report.push_str(&format!(
                    "第{cycle}轮进入：资源、实例与 GPU 就绪；玩家、敌人、地形与相机断言通过\n"
                ));
            } else {
                let world = app.world_mut();
                ensure(
                    world
                        .query_filtered::<(), With<crate::transition::SessionEntity>>()
                        .iter(world)
                        .count()
                        == 0,
                    "返回首页后残留对局实体",
                )?;
                ensure(
                    world.query::<&WorldAssetRoot>().iter(world).count() == 0,
                    "返回首页后残留场景实例根",
                )?;
                ensure(
                    world
                        .query_filtered::<(), With<crate::menu::MenuCamera>>()
                        .iter(world)
                        .count()
                        == 1,
                    "首页相机重复生成",
                )?;
                ensure(
                    session_instances.iter().all(|&instance| {
                        !world
                            .resource::<WorldInstanceSpawner>()
                            .instance_is_ready(instance)
                    }),
                    "返回首页后场景实例未注销",
                )?;
                ensure(
                    !world.contains_resource::<crate::ground::TerrainReady>(),
                    "地形生成标志未清除",
                )?;
                report.push_str(&format!(
                    "第{cycle}轮返回：实体与场景根清零；首页相机唯一；生成标志已重置\n"
                ));
            }
            capture(app, &options.image_path(frame))?;
            report.push_str(&format!(
                "{}\n",
                crate::transition::diagnostics(app.world_mut())
            ));
        }
    }
    report.push_str("三轮双向过渡全部完成，持续120帧未重建黑幕\n");
    Ok(())
}

/// 临时取消跟随以拍摄固定的车体局部视点；截图等待仍不推进模拟。
fn capture_view(app: &mut App, path: &Path, offset: Vec3) -> Result<(), String> {
    let (player, _) = subject_state(app.world_mut())?;
    let world = app.world_mut();
    let (camera, saved) = world
        .query_filtered::<(Entity, &Transform), With<crate::driving::FollowCamera>>()
        .single(world)
        .map(|(entity, transform)| (entity, *transform))
        .map_err(|e| e.to_string())?;
    let target = player.translation + Vec3::Y * -0.15;
    world
        .entity_mut(camera)
        .remove::<crate::driving::FollowCamera>()
        .insert(
            Transform::from_translation(target + player.rotation * offset)
                .looking_at(target, Vec3::Y),
        );
    let result = capture(app, path);
    app.world_mut()
        .entity_mut(camera)
        .insert((crate::driving::FollowCamera, saved));
    result
}

/// 侧面与车体一起移动旋转，使逐帧比较只显示轮组自身动画。
fn capture_side(app: &mut App, path: &Path) -> Result<(), String> {
    capture_view(app, path, Vec3::new(0.0, 0.5, 10.0))
}

/// 相同侧面、前侧和后侧视点，并用细红线标示碰撞框外包围边界（不含炮管）。
fn capture_wheel_views(app: &mut App, output: &Path) -> Result<(), String> {
    capture_side(app, &output.join("side.png"))?;
    capture_view(app, &output.join("front.png"), Vec3::new(-9.0, 3.0, 9.0))?;
    capture_view(app, &output.join("rear.png"), Vec3::new(9.0, 3.0, 9.0))?;
    let (player, _) = subject_state(app.world_mut())?;
    let material = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.05, 0.02),
            unlit: true,
            ..default()
        });
    capture_view(
        app,
        &output.join("comparison.png"),
        Vec3::new(13.0, 10.0, 18.0),
    )?;
    let half = if app.world().contains_resource::<WheelTarget>() {
        crate::enemy::ENEMY_SIZE
    } else {
        crate::driving::TANK_SIZE
    } / 2.0;
    let mut edges = Vec::new();
    for axis in 0..3 {
        for a in [-1.0, 1.0] {
            for b in [-1.0, 1.0] {
                let mut size = Vec3::splat(0.025);
                size[axis] = half[axis] * 2.0;
                let mut center = Vec3::ZERO;
                center[(axis + 1) % 3] = half[(axis + 1) % 3] * a;
                center[(axis + 2) % 3] = half[(axis + 2) % 3] * b;
                let mesh = app
                    .world_mut()
                    .resource_mut::<Assets<Mesh>>()
                    .add(Cuboid::from_size(size));
                edges.push(
                    app.world_mut()
                        .spawn((
                            Mesh3d(mesh),
                            MeshMaterial3d(material.clone()),
                            Transform::from_translation(
                                player.translation + player.rotation * center,
                            )
                            .with_rotation(player.rotation),
                        ))
                        .id(),
                );
            }
        }
    }
    let result = capture_view(app, &output.join("bounds.png"), Vec3::new(-9.0, 3.0, 9.0));
    for edge in edges {
        app.world_mut().despawn(edge);
    }
    result
}

/// 请求一次截图并等待 observer 完成保存，绝不以请求已提交作为成功判据。
fn capture(app: &mut App, path: &Path) -> Result<(), String> {
    capture_checked(app, path, false)
}

/// 全黑交接专用像素断言，其余帧仍拒收空画面。
fn capture_checked(app: &mut App, path: &Path, black: bool) -> Result<(), String> {
    wait_render(app, false)?;
    app.world_mut().resource_mut::<CaptureResult>().0 = None;
    let target = app.world().resource::<Target>().0.clone();
    let size = app
        .world()
        .resource::<Assets<Image>>()
        .get(&target)
        .unwrap()
        .size();
    let path = path.to_owned();
    app.world_mut().spawn(Screenshot::image(target)).observe(
        move |event: On<ScreenshotCaptured>, mut result: ResMut<CaptureResult>| {
            result.0 = Some(save_image(&event.image, &path, size, black));
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
fn save_image(image: &Image, path: &Path, size: UVec2, black: bool) -> Result<(), String> {
    let rgb = image
        .clone()
        .try_into_dynamic()
        .map_err(|error| format!("截图转换失败：{error}"))?
        .to_rgb8();
    ensure(
        rgb.width() == size.x && rgb.height() == size.y,
        "截图尺寸不符",
    )?;
    // 临时诊断：像素校验失败也落盘 reject 文件，便于人工检查（验证后删除）。
    if let Err(message) = validate_capture_pixels(rgb.as_raw(), size, black) {
        let reject = path.with_extension("reject.png");
        let mut file = create_new(&reject)?;
        rgb.write_to(
            &mut file,
            ImageFormat::Png
                .as_image_crate_format()
                .ok_or("未启用 PNG 编码")?,
        )
        .map_err(|error| format!("PNG 编码或写入失败：{error}"))?;
        drop(file);
        return Err(format!("{message}；已保存诊断图：{}", reject.display()));
    }
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
#[cfg(test)]
fn validate_pixels(rgb: &[u8]) -> Result<(), String> {
    validate_capture_pixels(rgb, UVec2::new(WIDTH, HEIGHT), false)
}

/// 验证目标尺寸与阶段预期，避免把正常黑场与异常黑屏混为一谈。
fn validate_capture_pixels(rgb: &[u8], size: UVec2, black: bool) -> Result<(), String> {
    ensure(
        rgb.len() == (size.x * size.y * 3) as usize && !rgb.is_empty(),
        "截图像素长度不符",
    )?;
    if black {
        return ensure(
            rgb.iter().all(|&value| value <= 3),
            "全黑交接帧仍有可见内容",
        );
    }
    let first = &rgb[..3];
    ensure(
        rgb.chunks_exact(3).filter(|pixel| *pixel != first).count()
            > (size.x * size.y / 100) as usize,
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

    /// Bevy 0.20 会重试尚未到达渲染世界的 shader，不能将等待判为致命错误。
    #[test]
    fn shader_loading_waits_but_compilation_failure_still_fails() {
        use bevy::shader::ShaderCacheError;
        assert!(shader_pending(&ShaderCacheError::ShaderNotLoaded(
            Handle::<Shader>::default().id()
        )));
        assert!(shader_pending(
            &ShaderCacheError::ShaderImportNotYetAvailable
        ));
        assert!(!shader_pending(&ShaderCacheError::CreateShaderModule(
            "编译失败".into()
        )));
    }

    #[test]
    fn command_line_keeps_normal_game_and_accepts_documented_commands() {
        assert_eq!(parse(&[]).unwrap(), None);
        let capture = parse(&["--capture", "截图.png"]).unwrap().unwrap();
        let enemy = parse(&[
            "--offscreen-test",
            "wheels",
            "--output",
            "out",
            "--model",
            "enemy",
        ])
        .unwrap()
        .unwrap();
        assert!(enemy.enemy_wheels);
        assert!(capture.single_image);
        assert_eq!(capture.scenario, Scenario::Scene);
        for (name, scenario) in [
            ("scene", Scenario::Scene),
            ("drive", Scenario::Drive),
            ("combat", Scenario::Combat),
            ("wheels", Scenario::Wheels),
            ("menu", Scenario::Menu),
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
            vec![
                "--offscreen-test",
                "scene",
                "--output",
                "out",
                "--model",
                "enemy",
            ],
            vec![
                "--offscreen-test",
                "wheels",
                "--output",
                "out",
                "--model",
                "unknown",
            ],
            vec!["--capture", "a.png", "extra"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }

    /// 敌人 wheels 的脚本输入只驱动目标，不能让玩家或其它敌人一起移动。
    #[test]
    fn enemy_wheel_input_is_routed_only_to_selected_target() {
        let mut app = App::new();
        let target = app
            .world_mut()
            .spawn(crate::driving::DrivingInput::default())
            .id();
        let idle = app
            .world_mut()
            .spawn(crate::driving::DrivingInput::default())
            .id();
        app.insert_resource(WheelTarget(target))
            .insert_resource(InputFrame(Scenario::Wheels, None))
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, inject_input);
        for (frame, throttle, turn) in [
            (1, 1.0, 0.0),
            (31, -1.0, 0.0),
            (61, 0.0, 1.0),
            (91, 0.0, -1.0),
            (121, 0.0, 0.0),
        ] {
            app.world_mut().resource_mut::<InputFrame>().1 = Some(frame);
            app.update();
            let input = app
                .world()
                .get::<crate::driving::DrivingInput>(target)
                .unwrap();
            assert_eq!((input.throttle, input.turn), (throttle, turn));
            let input = app
                .world()
                .get::<crate::driving::DrivingInput>(idle)
                .unwrap();
            assert_eq!((input.throttle, input.turn), (0.0, 0.0));
            assert_eq!(
                app.world()
                    .resource::<ButtonInput<KeyCode>>()
                    .get_pressed()
                    .count(),
                0
            );
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
        assert_eq!(Scenario::Menu.keys(1), &[KeyCode::ArrowDown]);
        assert_eq!(Scenario::Menu.keys(3), &[KeyCode::Enter]);
        assert_eq!(Scenario::Menu.keys(5), &[KeyCode::Escape]);
        assert!(Scenario::Menu.keys(2).is_empty());
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
            enemy_wheels: false,
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

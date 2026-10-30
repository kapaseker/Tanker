//! 无窗口渲染配置、固定时钟、输入注入、就绪等待与 watchdog。

use bevy::{
    app::PluginsState,
    asset::UntypedAssetLoadFailedEvent,
    camera::RenderTarget,
    input::InputSystems,
    prelude::*,
    render::{
        RenderApp, RenderPlugin,
        mesh::{PrimitiveTopology, RenderMesh, RenderMeshBufferInfo},
        pipelined_rendering::PipelinedRenderingPlugin,
        render_asset::RenderAssets,
        render_resource::{CachedPipelineState, PipelineCache, PollType, TextureFormat},
        renderer::{RenderAdapterInfo, RenderDevice},
        texture::GpuImage,
    },
    time::TimeUpdateStrategy,
    window::ExitCondition,
    winit::WinitPlugin,
};
use std::sync::mpsc;

use super::*;
use super::{capture::write_new, scenarios::run_scenario};

/// 配置真正无窗口的渲染器；禁用并行渲染以便受控更新后检查 GPU 状态。
pub(crate) fn configure(app: &mut App) {
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
    cameras: Query<(Entity, &RenderTarget), With<crate::app::transition::OutputCamera>>,
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
    mut inputs: Query<&mut crate::tank::driving::DrivingInput>,
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
    mut expected: Option<ResMut<ExpectedAssetFailure>>,
    mut failures: MessageReader<UntypedAssetLoadFailedEvent>,
    mut destroyed: MessageReader<crate::gameplay::enemy::EnemyDestroyed>,
    bullets: Query<&Transform, With<crate::gameplay::combat::firing::Bullet>>,
    explosions: Query<(), With<crate::gameplay::combat::explosion::Explosion>>,
) {
    for failure in failures.read() {
        if let Some(expected) = expected.as_mut() {
            if failure.path.to_string() == expected.path {
                expected.seen = true;
                continue;
            }
        }
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
pub(crate) fn run(mut app: App, options: Options) -> Result<(), String> {
    app.insert_resource(options.clone())
        .insert_resource(InputFrame(options.scenario, None))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
        .insert_resource(Time::<Fixed>::from_duration(STEP))
        .init_resource::<Evidence>()
        .init_resource::<CaptureResult>()
        .init_resource::<MenuMouse>()
        .insert_resource(Perf {
            start: Some(std::time::Instant::now()),
            ..default()
        })
        .add_systems(PreUpdate, (setup_target, inject_input.after(InputSystems)))
        .add_systems(Last, collect_evidence);
    if !matches!(
        options.scenario,
        Scenario::Index | Scenario::Gallery | Scenario::Detail | Scenario::Pastoral
    ) {
        // 旧场景在首个 StateTransition 前直接请求 InGame 跳过菜单：
        // 启动期 StateTransition 应用 InGame 并运行 OnEnter(session::setup)，
        // 引导消息与 Pending 取 .last() 后以 InGame 为准，不会落入菜单。
        app.world_mut()
            .resource_mut::<NextState<crate::app::state::GameState>>()
            .set(crate::app::state::GameState::InGame);
    }
    if options.candidate_light {
        // 最小光照对照：只在本次离屏进程新增一盏无阴影方向光，其余相机、
        // 环境光、环境贴图与曝光保持不变，也不写入游戏默认光照。
        let direction = CANDIDATE_LIGHT_DIRECTION.normalize();
        app.world_mut().spawn((
            DirectionalLight {
                illuminance: CANDIDATE_LIGHT_ILLUMINANCE,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_rotation(Quat::from_rotation_arc(Vec3::NEG_Z, direction)),
        ));
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
    if options.candidate_light {
        // 报告与实际光源共用同一常量，避免两处手写漂移。
        report.push_str(&format!(
            "光照实验：仅新增 DirectionalLight {} lux、无阴影、方向 {:?}；其余不变\n",
            CANDIDATE_LIGHT_ILLUMINANCE, CANDIDATE_LIGHT_DIRECTION
        ));
    }
    if let Some(adapter) = app.world().get_resource::<RenderAdapterInfo>() {
        // 0.20 起 RenderAdapterInfo 内部字段私有化，经 Deref<AdapterInfo> 取名称与后端。
        report.push_str(&format!(
            "GPU/后端：{} ({:?})\n",
            adapter.name, adapter.backend
        ));
    }
    let result = run_scenario(&mut app, &options, &mut report);
    let perf = app.world().resource::<Perf>();
    if let Some(ready) = perf.ready_secs {
        report.push_str(&format!("性能：启动到渲染就绪墙钟 {:.3}s\n", ready));
    }
    if perf.mesh_instances > 0 {
        report.push_str(&format!(
            "性能：就绪网格实例 {}，唯一网格 {}，三角形 {}\n",
            perf.mesh_instances, perf.unique_meshes, perf.triangles
        ));
    }
    if perf.sim_updates > 0 {
        report.push_str(&format!(
            "性能：模拟更新 {} 次，累计墙钟 {:.3}s（平均 {:.2}ms/帧；含同步渲染、物理与每帧 1ms 让出，不含截图等待）\n",
            perf.sim_updates,
            perf.sim_wall_secs,
            perf.sim_wall_secs * 1000.0 / f64::from(perf.sim_updates)
        ));
    }
    report.push_str(&match &result {
        Ok(()) => "结果：通过\n".into(),
        Err(error) => format!("结果：失败\n原因：{error}\n"),
    });
    if !options.single_image {
        write_new(&options.output.join("report.txt"), report.as_bytes())?;
    }
    result
}

/// 离屏性能证据：启动到渲染就绪的墙钟、就绪时网格统计、模拟步累计墙钟。
/// 只写入 report.txt 供验收对比，不参与任何断言。
#[derive(Resource, Default)]
struct Perf {
    /// run() 入口时刻，用于计算加载墙钟。
    start: Option<std::time::Instant>,
    /// 首次 wait_render 稳定所需的墙钟秒数。
    ready_secs: Option<f64>,
    /// 模拟步（duration==STEP）的更新次数与累计墙钟秒数；
    /// 含 GPU 同步渲染、物理与每帧 1ms 让出，不含截图等待与断言。
    sim_updates: u32,
    sim_wall_secs: f64,
    /// 场景就绪时的网格统计：Mesh3d 实例数、唯一网格数、三角形总数。
    mesh_instances: usize,
    unique_meshes: usize,
    triangles: u64,
}

/// 每次更新都检查资源失败、退出请求和管线错误；等待时也让出 CPU 并轮询读回。
pub(super) fn update(app: &mut App, duration: Duration, frame: Option<u32>) -> Result<(), String> {
    app.world_mut()
        .insert_resource(TimeUpdateStrategy::ManualDuration(duration));
    app.world_mut().resource_mut::<InputFrame>().1 = frame;
    let tick = std::time::Instant::now();
    app.update();
    if duration == STEP {
        let mut perf = app.world_mut().resource_mut::<Perf>();
        perf.sim_updates += 1;
        perf.sim_wall_secs += tick.elapsed().as_secs_f64();
    }
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
        .query::<&crate::tank::track::TracksFailed>()
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
/// 业务就绪汇总复用 `gameplay::session::content_ready`，此处只补充验证特有的
/// 托管纹理加载与实际 GPU 上传检查。
fn scene_ready(app: &mut App) -> bool {
    let world = app.world_mut();
    if !crate::ui::font::assets_ready(world) || !crate::gameplay::session::content_ready(world) {
        return false;
    }
    let server = world.resource::<AssetServer>();
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
pub(super) fn wait_render(app: &mut App, require_scene: bool) -> Result<(), String> {
    let mut stable = 0;
    while stable < 8 {
        update(app, Duration::ZERO, None)?;
        if pipelines_ready(app)? && gpu_assets_ready(app) && (!require_scene || scene_ready(app)) {
            stable += 1;
        } else {
            stable = 0;
        }
    }
    record_perf(app, require_scene);
    Ok(())
}

/// 首次就绪时记录加载墙钟；游戏场景额外统计网格实例、唯一网格与三角形总数。
/// 纯证据采集，任何失败都不影响场景流程。
fn record_perf(app: &mut App, require_scene: bool) {
    // 先整体取出 Perf，避免后续 app.sub_app(RenderApp) 与主 world 可变借用冲突；函数末尾写回。
    let mut perf = std::mem::take(&mut *app.world_mut().resource_mut::<Perf>());
    if perf.ready_secs.is_none() {
        if let Some(start) = perf.start {
            perf.ready_secs = Some(start.elapsed().as_secs_f64());
        }
    }
    if require_scene && perf.mesh_instances == 0 {
        // 交付资产 retain_mesh_data=false，就绪后 CPU 端 Mesh 已被 extract 释放；
        // 三角形数改从渲染世界的 RenderMesh 缓冲信息读取。
        let world = app.world_mut();
        let ids: Vec<_> = world
            .query::<&Mesh3d>()
            .iter(world)
            .map(|mesh| mesh.0.id())
            .collect();
        perf.mesh_instances = ids.len();
        let render = app.sub_app(RenderApp).world();
        let gpu_meshes = render.resource::<RenderAssets<RenderMesh>>();
        let mut unique = std::collections::HashSet::new();
        for id in ids {
            if unique.insert(id) {
                if let Some(mesh) = gpu_meshes.get(id) {
                    perf.triangles += match (mesh.primitive_topology(), &mesh.buffer_info) {
                        (PrimitiveTopology::TriangleList, RenderMeshBufferInfo::Indexed { count, .. }) => {
                            (count / 3) as u64
                        }
                        (PrimitiveTopology::TriangleList, RenderMeshBufferInfo::NonIndexed) => {
                            (mesh.vertex_count / 3) as u64
                        }
                        _ => 0,
                    };
                }
            }
        }
        perf.unique_meshes = unique.len();
    }
    *app.world_mut().resource_mut::<Perf>() = perf;
}

/// 发送端随运行结束而释放即取消计时；测试可使用短时限验证同一调度逻辑。
pub(super) fn spawn_watchdog(
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

#[cfg(test)]
mod tests {
    use super::*;

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

    /// 敌人 wheels 的脚本输入只驱动目标，不能让玩家或其它敌人一起移动。
    #[test]
    fn enemy_wheel_input_is_routed_only_to_selected_target() {
        let mut app = App::new();
        let target = app
            .world_mut()
            .spawn(crate::tank::driving::DrivingInput::default())
            .id();
        let idle = app
            .world_mut()
            .spawn(crate::tank::driving::DrivingInput::default())
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
                .get::<crate::tank::driving::DrivingInput>(target)
                .unwrap();
            assert_eq!((input.throttle, input.turn), (throttle, turn));
            let input = app
                .world()
                .get::<crate::tank::driving::DrivingInput>(idle)
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
            crate::tank::driving::DrivingPlugin,
            crate::gameplay::player::PlayerPlugin,
        ))
        .insert_resource(InputFrame(Scenario::Drive, None))
        .insert_resource(TimeUpdateStrategy::ManualDuration(STEP))
        .insert_resource(Time::<Fixed>::from_duration(STEP))
        .add_systems(PreUpdate, inject_input.after(InputSystems))
        .add_systems(Startup, |mut commands: Commands| {
            crate::terrain::spawn_arena(&mut commands)
        });
        app.finish();
        app.cleanup();
        let player = app
            .world_mut()
            .spawn((
                crate::gameplay::player::Tank,
                crate::tank::driving::tank_body(),
                // 驾驶系统按履带装备读取速度，缺省组件会导致坦克不动。
                crate::tank::track::PLAYER_TRACKS,
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
}

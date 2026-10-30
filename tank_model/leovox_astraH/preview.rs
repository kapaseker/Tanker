//! 从交付 VOX 本身加载的独立无窗口预览，不调用游戏模型或生成器。
use bevy::{
    app::PluginsState,
    asset::UntypedAssetLoadFailedEvent,
    camera::{Exposure, RenderTarget, ScalingMode},
    light::DirectionalLightShadowMap,
    pbr::{ScreenSpaceAmbientOcclusion, ScreenSpaceAmbientOcclusionQualityLevel},
    prelude::*,
    render::{
        RenderPlugin,
        pipelined_rendering::PipelinedRenderingPlugin,
        render_resource::TextureFormat,
        renderer::RenderAdapterInfo,
        view::screenshot::{Screenshot, ScreenshotCaptured},
    },
    window::ExitCondition,
    winit::WinitPlugin,
};
use bevy_vox_scene::VoxScenePlugin;
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

/// 屏蔽地面，保证测量只统计实际加载模型。
#[derive(Component)]
struct Ground;

/// 异步截图写盘结果，主循环必须等到成功才进入下一视角。
#[derive(Resource, Default)]
struct CaptureResult(Option<Result<(), String>>);

/// 收集所有异步资源失败，任何一次失败均使预览返回非零。
#[derive(Resource, Default)]
struct Failures(Vec<String>);

fn collect_failures(
    mut events: MessageReader<UntypedAssetLoadFailedEvent>,
    mut failures: ResMut<Failures>,
) {
    for event in events.read() {
        failures
            .0
            .push(format!("资源加载失败 {}: {}", event.path, event.error));
    }
}

/// 不启动 winit；180 秒看门狗涵盖加载、GPU 管线与截图。
fn tick(app: &mut App, start: Instant) -> Result<(), String> {
    if start.elapsed() > Duration::from_secs(180) {
        return Err("离屏运行超过 180 秒".into());
    }
    app.update();
    if !app.world().resource::<Failures>().0.is_empty() {
        return Err(app.world().resource::<Failures>().0.join("\n"));
    }
    std::thread::sleep(Duration::from_millis(2));
    Ok(())
}

/// VOX 坐标 +X 前、+Y 左、+Z 上；插件转换为 Bevy (-X,Z,Y)。
fn engine(p: Vec3) -> Vec3 {
    Vec3::new(-p.x, p.z, p.y)
}

fn run(output: &Path) -> Result<(), String> {
    fs::create_dir_all(output).map_err(|e| e.to_string())?;
    let start = Instant::now();
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: env!("CARGO_MANIFEST_DIR").to_owned(),
                ..default()
            })
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
    app.add_plugins(VoxScenePlugin::default());
    app.init_resource::<CaptureResult>()
        .init_resource::<Failures>()
        .insert_resource(ClearColor(Color::srgb(0.19, 0.205, 0.22)))
        .insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.83, 0.89, 1.0),
            brightness: 420.0,
            ..default()
        })
        .insert_resource(DirectionalLightShadowMap { size: 4096 })
        .add_systems(Update, collect_failures);
    while app.plugins_state() == PluginsState::Adding {
        bevy::tasks::tick_global_task_pools_on_main_thread();
        std::thread::sleep(Duration::from_millis(1));
    }
    app.finish();
    app.cleanup();
    let model: Handle<WorldAsset> = app
        .world()
        .resource::<AssetServer>()
        .load("leopard_2a6.vox");
    app.world_mut().spawn(WorldAssetRoot(model.clone()));
    let plane = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Plane3d::default().mesh().size(150., 150.));
    let material = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            base_color: Color::srgb(0.30, 0.315, 0.33),
            perceptual_roughness: 1.0,
            ..default()
        });
    app.world_mut().spawn((
        Ground,
        Mesh3d(plane),
        MeshMaterial3d(material),
        Transform::from_xyz(-4., -0.025, 0.),
    ));
    app.world_mut().spawn((
        DirectionalLight {
            illuminance: 13000.,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(-8., 12., -6.).looking_at(Vec3::new(-3., 0., 0.), Vec3::Y),
    ));
    app.world_mut().spawn((
        DirectionalLight {
            illuminance: 3500.,
            color: Color::srgb(0.80, 0.88, 1.),
            ..default()
        },
        Transform::from_xyz(4., 6., 8.).looking_at(Vec3::new(-4., 0., 0.), Vec3::Y),
    ));
    let target = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::new_target_texture(
            1800,
            1200,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
    let camera = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            RenderTarget::Image(target.clone().into()),
            Projection::Orthographic(OrthographicProjection {
                scaling_mode: ScalingMode::FixedVertical {
                    viewport_height: 7.2,
                },
                ..OrthographicProjection::default_3d()
            }),
            Transform::from_xyz(-16., 8., -14.).looking_at(Vec3::new(-4.8, 1.6, 0.), Vec3::Y),
            Msaa::Off,
            Exposure { ev100: 10.7 },
            ScreenSpaceAmbientOcclusion {
                quality_level: ScreenSpaceAmbientOcclusionQualityLevel::Ultra,
                radius: 0.24,
                ..default()
            },
        ))
        .id();
    while !app
        .world()
        .resource::<AssetServer>()
        .is_loaded_with_dependencies(model.id())
    {
        tick(&mut app, start)?;
    }
    // 让场景实例化、材质上传和阴影管线全部产生可见帧。
    for _ in 0..60 {
        tick(&mut app, start)?;
    }
    let mut report = String::new();
    if let Some(gpu) = app.world().get_resource::<RenderAdapterInfo>() {
        report.push_str(&format!("GPU: {} / {:?}\n", gpu.name, gpu.backend));
        if gpu.name.to_lowercase().contains("llvmpipe") {
            return Err("检测到 llvmpipe，不能视为硬件渲染验收".into());
        }
    } else {
        return Err("无法读取 GPU 设备".into());
    }
    // 测量实际加载的网格顶点，而不是重复作者配置中的数字。
    let world = app.world_mut();
    let instances: Vec<_> = world
        .query_filtered::<(&Mesh3d, &GlobalTransform), Without<Ground>>()
        .iter(world)
        .map(|(mesh, t)| (mesh.0.clone(), *t))
        .collect();
    if instances.len() != 42 {
        return Err(format!("应加载 42 个网格，实际 {}", instances.len()));
    }
    let meshes = world.resource::<Assets<Mesh>>();
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    for (handle, transform) in &instances {
        let mesh = meshes.get(handle).ok_or("缺少 CPU 网格数据")?;
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(|p| p.as_float3())
            .ok_or("网格缺少顶点")?;
        for p in positions {
            let p = transform.transform_point(Vec3::from(*p));
            lo = lo.min(p);
            hi = hi.max(p);
        }
    }
    let size = hi - lo;
    if (size - Vec3::new(10.98, 4.32, 3.76)).abs().max_element() > 0.001 {
        return Err(format!("实际网格尺寸错误: {size:?}，边界 {lo:?} / {hi:?}"));
    }
    report.push_str(&format!(
        "VOX: leopard_2a6.vox\n网格数: {}\n实际尺寸 XYZ (m): {size:?}\n边界: {lo:?} / {hi:?}\n",
        instances.len()
    ));
    // 每个视角的目标与偏移都用 VOX 米制坐标书写。
    let views = [
        ("01-front-right", [4.8, 0., 1.65], [11., -14., 8.], 7.3),
        ("02-rear", [3.3, 0., 1.8], [-16., 0., 4.5], 5.7),
        ("03-front", [4.7, 0., 2.1], [17., 0., 2.], 6.6),
        ("04-top", [5.49, 0., 0.], [0., 0., 20.], 12.3),
        ("05-front-left", [4.8, 0., 1.65], [11., 14., 8.], 7.3),
        ("06-rear-left", [4.4, 0., 1.65], [-12., 14., 8.], 7.5),
        ("07-rear-right", [4.4, 0., 1.65], [-12., -14., 8.], 7.5),
        ("08-right-side", [5.49, 0., 1.9], [0., -20., 0.], 8.1),
        ("09-engine-detail", [0.9, 0., 1.15], [-4., -3., 5.], 3.8),
        ("10-turret-detail", [3.8, 0., 2.25], [5., -7., 6.], 4.3),
    ];
    for (name, center, offset, height) in views {
        let center = engine(Vec3::from(center));
        let offset = engine(Vec3::from(offset));
        let up = if name == "04-top" {
            Vec3::NEG_X
        } else {
            Vec3::Y
        };
        app.world_mut().entity_mut(camera).insert((
            Transform::from_translation(center + offset).looking_at(center, up),
            Projection::Orthographic(OrthographicProjection {
                scaling_mode: ScalingMode::FixedVertical {
                    viewport_height: height,
                },
                ..OrthographicProjection::default_3d()
            }),
        ));
        for _ in 0..12 {
            tick(&mut app, start)?;
        }
        let path = output.join(format!("{name}.png"));
        app.world_mut().resource_mut::<CaptureResult>().0 = None;
        app.world_mut()
            .spawn(Screenshot::image(target.clone()))
            .observe(
                move |event: On<ScreenshotCaptured>, mut result: ResMut<CaptureResult>| {
                    result.0 = Some((|| {
                        let image = event
                            .image
                            .clone()
                            .try_into_dynamic()
                            .map_err(|e| e.to_string())?;
                        let rgb = image.to_rgb8();
                        let bytes = rgb.as_raw();
                        if bytes.iter().max() == bytes.iter().min() {
                            return Err("截图是纯色".into());
                        }
                        let mut file = fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(&path)
                            .map_err(|e| e.to_string())?;
                        image
                            .write_to(
                                &mut file,
                                bevy::image::ImageFormat::Png
                                    .as_image_crate_format()
                                    .ok_or("PNG 编码器缺失")?,
                            )
                            .map_err(|e| e.to_string())?;
                        Ok(())
                    })());
                },
            );
        while app.world().resource::<CaptureResult>().0.is_none() {
            tick(&mut app, start)?;
        }
        app.world_mut()
            .resource_mut::<CaptureResult>()
            .0
            .take()
            .unwrap()?;
        report.push_str(&format!("截图已保存: {name}.png\n"));
        println!("截图完成: {name}");
    }
    report.push_str(&format!(
        "结果: 通过\n墙钟: {:.2}s\n",
        start.elapsed().as_secs_f32()
    ));
    fs::write(output.join("report.txt"), report).map_err(|e| e.to_string())?;
    Ok(())
}

fn main() {
    let output = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "leovox_astraH/renders".into());
    if let Err(error) = run(Path::new(&output)) {
        eprintln!("预览失败: {error}");
        std::process::exit(1);
    }
}

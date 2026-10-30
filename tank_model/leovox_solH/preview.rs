//! 独立厘米 VOX 的真实 Bevy 离屏预览；只加载本目录交付物，不创建窗口。

use bevy::{
    anti_alias::taa::TemporalAntiAliasing,
    app::PluginsState,
    asset::{LoadState, UntypedAssetLoadFailedEvent},
    camera::{RenderTarget, ScalingMode, primitives::Aabb},
    image::ImageFormat,
    pbr::{ScreenSpaceAmbientOcclusion, ScreenSpaceAmbientOcclusionQualityLevel},
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
    window::ExitCondition,
    winit::WinitPlugin,
    world_serialization::{WorldAsset, WorldAssetRoot},
};
use bevy_vox_scene::{VoxScenePlugin, VoxelModelInstance};
use std::{
    fs::OpenOptions,
    io::Write,
    path::Path,
    time::{Duration, Instant},
};

/// 图片分辨率；厘米细节在局部机位另外检查。
const WIDTH: u32 = 1600;
const HEIGHT: u32 = 1000;

/// 场景资源、离屏纹理与唯一相机。
#[derive(Resource)]
struct Subject {
    asset: Handle<WorldAsset>,
    target: Handle<Image>,
    camera: Entity,
}

/// 与源文件 PACK 数量相等，保证没有静默漏加载场景块。
#[derive(Resource)]
struct ExpectedChunks(usize);

/// 截图保存结果和资源失败，任何失败都阻止成功报告。
#[derive(Resource, Default)]
struct Evidence {
    saved: Option<Result<(), String>>,
    errors: Vec<String>,
}

/// 灰色摄影棚只提供比例和接触阴影，不把地面混入 VOX。
fn setup(
    mut commands: Commands,
    server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let asset = server.load("leopard_2a6.vox");
    // vox_bake 将 (X,Y,Z) 转为 (-X,Z,Y)，平移后以车体中点落在场景原点。
    commands.spawn((
        WorldAssetRoot(asset.clone()),
        Transform::from_xyz(3.83, 0.0, -1.88),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(200.0, 200.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.37, 0.36, 0.34),
            perceptual_roughness: 0.95,
            ..default()
        })),
        Transform::from_xyz(0.0, -0.012, 0.0),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 10500.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(-5.0, 10.0, -7.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 4500.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(6.0, 7.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.insert_resource(GlobalAmbientLight {
        brightness: 550.0,
        color: Color::srgb(0.90, 0.93, 1.0),
        ..default()
    });
    let target = images.add(Image::new_target_texture(
        WIDTH,
        HEIGHT,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    let camera = commands
        .spawn((
            Camera3d::default(),
            Msaa::Off,
            TemporalAntiAliasing::default(),
            ScreenSpaceAmbientOcclusion {
                quality_level: ScreenSpaceAmbientOcclusionQualityLevel::Ultra,
                radius: 0.32,
                constant_object_thickness: 0.12,
            },
            RenderTarget::Image(target.clone().into()),
            Projection::Orthographic(OrthographicProjection {
                scaling_mode: ScalingMode::FixedVertical {
                    viewport_height: 8.0,
                },
                ..OrthographicProjection::default_3d()
            }),
            Transform::from_xyz(-12.0, 8.0, -12.0).looking_at(Vec3::new(-1.0, 1.5, 0.0), Vec3::Y),
        ))
        .id();
    commands.insert_resource(Subject {
        asset,
        target,
        camera,
    });
}

/// 读取实际资源失败，不把尚未完成与已失败混在一起。
fn failures(
    mut events: MessageReader<UntypedAssetLoadFailedEvent>,
    mut evidence: ResMut<Evidence>,
) {
    for event in events.read() {
        evidence
            .errors
            .push(format!("{}：{}", event.path, event.error));
    }
}

/// 手动更新并检查 GPU、资源、窗口和超时；整个流程不调用 winit runner。
fn tick(app: &mut App, start: Instant) -> Result<(), String> {
    if start.elapsed().as_secs() > 180 {
        return Err("离屏过程超过 180 秒".into());
    }
    app.update();
    let world = app.world_mut();
    if world.query::<&Window>().iter(world).next().is_some() {
        return Err("离屏场景不应创建窗口".into());
    }
    if let Some(error) = world.resource::<Evidence>().errors.first() {
        return Err(error.clone());
    }
    app.world()
        .resource::<RenderDevice>()
        .wgpu_device()
        .poll(PollType::Poll)
        .map_err(|e| e.to_string())?;
    let cache = app.sub_app(RenderApp).world().resource::<PipelineCache>();
    for pipeline in cache.pipelines() {
        if let CachedPipelineState::Err(error) = &pipeline.state {
            return Err(format!("管线失败：{error}"));
        }
    }
    std::thread::sleep(Duration::from_millis(2));
    Ok(())
}

/// 同时检查源资产、实例网格和 GPU 纹理，连续稳定后才允许截图。
fn ready(app: &mut App) -> bool {
    let world = app.world_mut();
    let Some(subject) = world.get_resource::<Subject>() else {
        return false;
    };
    let server = world.resource::<AssetServer>();
    if !server.is_loaded_with_dependencies(subject.asset.id()) {
        return false;
    }
    let mesh_ids: Vec<_> = world
        .query::<&Mesh3d>()
        .iter(world)
        .map(|m| m.0.id())
        .collect();
    let expected = world.resource::<ExpectedChunks>().0;
    if mesh_ids.len() != expected + 1 {
        return false;
    }
    let image_ids: Vec<_> = world.resource::<Assets<Image>>().ids().collect();
    let render = app.sub_app(RenderApp).world();
    let meshes = render.resource::<RenderAssets<RenderMesh>>();
    let images = render.resource::<RenderAssets<GpuImage>>();
    mesh_ids.iter().all(|id| meshes.get(*id).is_some())
        && image_ids.iter().all(|id| images.get(*id).is_some())
        && render
            .resource::<PipelineCache>()
            .waiting_pipelines()
            .next()
            .is_none()
}

/// 文件排他创建，旧截图不会被重复验证覆盖。
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())
}

/// 截图 observer 保存成功后才继续；失败会留在报告中。
fn capture(app: &mut App, path: &Path, start: Instant) -> Result<(), String> {
    let target = app.world().resource::<Subject>().target.clone();
    let path = path.to_owned();
    app.world_mut().resource_mut::<Evidence>().saved = None;
    app.world_mut().spawn(Screenshot::image(target)).observe(
        move |event: On<ScreenshotCaptured>, mut evidence: ResMut<Evidence>| {
            evidence.saved = Some((|| {
                let rgb = event
                    .image
                    .clone()
                    .try_into_dynamic()
                    .map_err(|e| e.to_string())?
                    .to_rgb8();
                if rgb.width() != WIDTH || rgb.height() != HEIGHT {
                    return Err("截图尺寸不符".into());
                }
                let pixels = rgb.as_raw();
                let first = &pixels[..3];
                if pixels.chunks_exact(3).filter(|p| *p != first).count()
                    < (WIDTH * HEIGHT / 20) as usize
                {
                    return Err("截图缺少内容".into());
                }
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .map_err(|e| e.to_string())?;
                rgb.write_to(
                    &mut file,
                    ImageFormat::Png
                        .as_image_crate_format()
                        .ok_or("PNG 未启用")?,
                )
                .map_err(|e| e.to_string())?;
                file.sync_all().map_err(|e| e.to_string())
            })());
        },
    );
    loop {
        tick(app, start)?;
        if let Some(result) = app.world_mut().resource_mut::<Evidence>().saved.take() {
            return result;
        }
    }
}

/// 相同实体在七个机位和两个细节机位下保持结构一致。
fn render(app: &mut App, output: &Path, start: Instant, report: &mut String) -> Result<(), String> {
    let mut stable = 0;
    while stable < 8 {
        tick(app, start)?;
        if ready(app) {
            stable += 1;
        } else {
            stable = 0;
        }
        if let Some(subject) = app.world().get_resource::<Subject>() {
            if matches!(
                app.world()
                    .resource::<AssetServer>()
                    .get_load_state(subject.asset.id()),
                Some(LoadState::Failed(_))
            ) {
                return Err("VOX 加载失败".into());
            }
        }
    }
    let adapter = app.world().resource::<RenderAdapterInfo>();
    report.push_str(&format!(
        "GPU/后端：{} ({:?})\n就绪耗时：{:.2}s\n",
        adapter.name,
        adapter.backend,
        start.elapsed().as_secs_f64()
    ));
    // 对实际加载、变换后的网格再量一次，覆盖元数据单位、轴转换及分块 pivot。
    let world = app.world_mut();
    let mut minimum = Vec3::splat(f32::INFINITY);
    let mut maximum = Vec3::splat(f32::NEG_INFINITY);
    let mut count = 0;
    for (aabb, transform) in world
        .query_filtered::<(&Aabb, &GlobalTransform), With<VoxelModelInstance>>()
        .iter(world)
    {
        count += 1;
        for x in [-1.0, 1.0] {
            for y in [-1.0, 1.0] {
                for z in [-1.0, 1.0] {
                    let point = transform.transform_point(
                        Vec3::from(aabb.center)
                            + Vec3::from(aabb.half_extents) * Vec3::new(x, y, z),
                    );
                    minimum = minimum.min(point);
                    maximum = maximum.max(point);
                }
            }
        }
    }
    if count != world.resource::<ExpectedChunks>().0
        || !minimum.abs_diff_eq(Vec3::new(-7.14, 0.0, -1.88), 0.0001)
        || !maximum.abs_diff_eq(Vec3::new(3.83, 4.38, 1.88), 0.0001)
    {
        return Err(format!(
            "加载后尺寸不符：{count} 块，{minimum:?}～{maximum:?}"
        ));
    }
    report.push_str(&format!(
        "实际 VOX 网格：{count} 块；世界尺寸：{:?} m；最低点：{:.4} m\n",
        maximum - minimum,
        minimum.y
    ));
    let target = Vec3::new(-1.1, 1.85, 0.0);
    let views = [
        (
            "01-front-right",
            Vec3::new(-13.0, 8.0, -13.0),
            target,
            8.0,
            Vec3::Y,
        ),
        (
            "02-rear",
            Vec3::new(14.0, 5.0, 0.0),
            Vec3::new(0.8, 1.8, 0.0),
            6.4,
            Vec3::Y,
        ),
        ("03-front", Vec3::new(-17.0, 4.6, 0.0), target, 6.4, Vec3::Y),
        (
            "04-top",
            Vec3::new(-1.65, 20.0, 0.0),
            Vec3::new(-1.65, 0.0, 0.0),
            13.0,
            Vec3::NEG_X,
        ),
        (
            "05-front-left",
            Vec3::new(-13.0, 8.0, 13.0),
            target,
            8.0,
            Vec3::Y,
        ),
        (
            "06-rear-left",
            Vec3::new(13.0, 8.0, 13.0),
            target,
            8.0,
            Vec3::Y,
        ),
        (
            "07-rear-right",
            Vec3::new(13.0, 8.0, -13.0),
            target,
            8.0,
            Vec3::Y,
        ),
        (
            "08-roof-detail",
            Vec3::new(-5.0, 8.0, -7.0),
            Vec3::new(0.35, 2.6, 0.0),
            4.6,
            Vec3::Y,
        ),
        (
            "09-right-side",
            Vec3::new(-1.65, 2.0, -20.0),
            Vec3::new(-1.65, 2.0, 0.0),
            7.8,
            Vec3::Y,
        ),
    ];
    for (name, position, target, span, up) in views {
        let camera = app.world().resource::<Subject>().camera;
        app.world_mut().entity_mut(camera).insert((
            TemporalAntiAliasing { reset: true },
            Transform::from_translation(position).looking_at(target, up),
            Projection::Orthographic(OrthographicProjection {
                scaling_mode: ScalingMode::FixedVertical {
                    viewport_height: span,
                },
                ..OrthographicProjection::default_3d()
            }),
        ));
        for _ in 0..32 {
            tick(app, start)?;
        }
        capture(app, &output.join(format!("{name}.png")), start)?;
        report.push_str(&format!("已保存：{name}.png\n"));
    }
    Ok(())
}

/// 独立资源根与新输出目录，避免错误加载游戏资产或覆盖历史证据。
fn main() {
    let output = std::env::args().nth(1).expect("请指定新的输出目录");
    let output = Path::new(&output);
    assert!(!output.exists(), "输出目录已存在，请使用新路径");
    std::fs::create_dir_all(output).expect("无法创建输出目录");
    let mut app = App::new();
    let source = std::fs::read(format!(
        "{}/leopard_2a6.vox",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("无法读取 VOX");
    assert_eq!(&source[20..24], b"PACK", "缺少模型数量声明");
    app.insert_resource(ExpectedChunks(
        u32::from_le_bytes(source[32..36].try_into().unwrap()) as usize,
    ));
    app.insert_resource(ClearColor(Color::srgb(0.37, 0.36, 0.34)))
        .init_resource::<Evidence>()
        .add_plugins(
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
        )
        .add_plugins(VoxScenePlugin::default())
        .add_systems(Startup, setup)
        .add_systems(Last, failures);
    let start = Instant::now();
    while app.plugins_state() != PluginsState::Ready {
        std::thread::yield_now();
    }
    app.finish();
    app.cleanup();
    let mut report = "Leopard 2A6 solH / 0.01 m VOX / Bevy 0.20 / 无窗口摄影棚\n".to_owned();
    let result = render(&mut app, output, start, &mut report);
    report.push_str(&match &result {
        Ok(()) => "结果：通过\n".into(),
        Err(e) => format!("结果：失败\n原因：{e}\n"),
    });
    write_new(&output.join("report.txt"), report.as_bytes()).expect("无法保存报告");
    if let Err(error) = result {
        eprintln!("离屏失败：{error}");
        std::process::exit(1);
    }
}

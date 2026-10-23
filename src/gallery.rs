//! 真实 VOX 模型画廊：纯视觉预览与页面生命周期，不参与战斗和物理。
mod ui;

use crate::{
    app::{
        state::GameState,
        transition::{Fade, SceneTarget},
    },
    tank::{track, visual},
};
use bevy::{
    asset::LoadState,
    camera::{RenderTarget, ScalingMode, primitives::MeshAabb, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    math::Affine3A,
    prelude::*,
    render::render_resource::TextureFormat,
    world_serialization::{WorldInstance, WorldInstanceSpawner},
};

/// 分类索引与页面顺序一致；整车显示其车身型号。
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Entry {
    pub category: usize,
    pub path: &'static str,
    pub enemy: bool,
}
impl Entry {
    /// 型号严格取主资产的父目录，排除扩展名和车型目录。
    pub fn name(self) -> &'static str {
        self.path.rsplit('/').nth(1).unwrap()
    }
}

/// 当前已接入的真实资源，不枚举 ADR 中的预留型号或炮管附属子弹。
pub(crate) const ENTRIES: [Entry; 8] = [
    Entry {
        category: 0,
        path: visual::PLAYER_BARREL,
        enemy: false,
    },
    Entry {
        category: 0,
        path: visual::ENEMY_BARREL,
        enemy: true,
    },
    Entry {
        category: 1,
        path: "tank/tracks/player/cleat/track.vox",
        enemy: false,
    },
    Entry {
        category: 1,
        path: "tank/tracks/enemy/crawl/track.vox",
        enemy: true,
    },
    Entry {
        category: 2,
        path: "tank/bodies/player/anvil/body.vox",
        enemy: false,
    },
    Entry {
        category: 2,
        path: "tank/bodies/enemy/carapace/body.vox",
        enemy: true,
    },
    Entry {
        category: 3,
        path: "tank/bodies/player/anvil/body.vox",
        enemy: false,
    },
    Entry {
        category: 3,
        path: "tank/bodies/enemy/carapace/body.vox",
        enemy: true,
    },
];

/// 页面相机复用菜单的纹理缩放规则，不参与模型预览层。
#[derive(Component)]
pub(crate) struct GalleryCamera;
/// 画廊持有的根实体，离开时递归清理。
#[derive(Component)]
pub(crate) struct GalleryEntity;

/// 一张卡片独占的模型、相机与纹理；尺寸在未旋转的模型空间测量。
pub(crate) struct Preview {
    pub(crate) root: Entity,
    content: Entity,
    camera: Entity,
    pub image: Handle<Image>,
    pub size: Option<Vec3>,
    pub error: Option<String>,
    /// 场景根就绪后仍等待网格后代稳定，避免插件延迟生成的节点漏分层或漏计尺寸。
    mesh_count: usize,
    stable_frames: u8,
}
/// 页面持有强句柄，退出后显式释放动态纹理。
#[derive(Resource)]
pub(crate) struct Previews {
    pub items: Vec<Preview>,
    started: std::time::Instant,
}

/// 安装页面及纯视觉预览系统。
pub(crate) struct GalleryPlugin;
impl Plugin for GalleryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ui::View>()
            .add_systems(
                OnEnter(GameState::Gallery),
                (spawn_previews, ui::spawn).chain(),
            )
            .add_systems(Update, (prepare_previews, spin).chain().run_if(in_gallery))
            .add_systems(Update, (ui::input, ui::update).chain().run_if(in_gallery))
            .add_systems(OnExit(GameState::GalleryToMenu), cleanup);
    }
}

/// 渐出时保留最后一页内容与动画，黑场交接后才销毁。
fn in_gallery(state: Res<State<GameState>>) -> bool {
    matches!(state.get(), GameState::Gallery | GameState::GalleryToMenu)
}

/// 每项独占渲染层；整车只添加视觉子树，不使用 Tank/EnemyTank 标记。
fn spawn_previews(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut items = Vec::new();
    for (index, entry) in ENTRIES.iter().enumerate() {
        let layer = RenderLayers::layer(index + 1);
        let root = commands
            .spawn((GalleryEntity, Transform::default(), Visibility::Inherited))
            .id();
        let content = commands
            .spawn((Transform::default(), Visibility::Inherited))
            .id();
        commands.entity(root).add_child(content);
        if entry.category != 1 {
            commands
                .entity(content)
                .insert(WorldAssetRoot(assets.load(entry.path)));
        }
        if entry.category == 1 || entry.category == 3 {
            commands.entity(content).insert(if entry.enemy {
                track::ENEMY_TRACKS
            } else {
                track::PLAYER_TRACKS
            });
        }
        if entry.category == 3 {
            visual::mount_barrel(
                &mut commands,
                &assets,
                content,
                if entry.enemy {
                    visual::ENEMY_BARREL
                } else {
                    visual::PLAYER_BARREL
                },
            );
        }
        let image = images.add(Image::new_target_texture(
            460,
            360,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
        let camera = commands
            .spawn((
                GalleryEntity,
                Camera3d::default(),
                Camera {
                    order: -3,
                    clear_color: ClearColorConfig::Custom(
                        crate::ui_resources::colors::gallery::SURFACE,
                    ),
                    ..default()
                },
                RenderTarget::Image(image.clone().into()),
                layer,
                Projection::Orthographic(OrthographicProjection {
                    scaling_mode: ScalingMode::FixedVertical {
                        viewport_height: 16.0,
                    },
                    ..OrthographicProjection::default_3d()
                }),
                Transform::from_xyz(-18.0, 10.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
                Tonemapping::BlenderFilmic,
                Msaa::Off,
                EnvironmentMapLight {
                    diffuse_map: assets.load("pisa_diffuse.ktx2"),
                    specular_map: assets.load("pisa_specular.ktx2"),
                    intensity: 500.0,
                    ..default()
                },
            ))
            .id();
        items.push(Preview {
            root,
            content,
            camera,
            image,
            size: None,
            error: None,
            mesh_count: 0,
            stable_frames: 0,
        });
    }
    commands.insert_resource(Previews {
        items,
        started: std::time::Instant::now(),
    });
}

/// 层级展开时累计局部变换，显隐沿父链传播；只显示的网格参与尺寸计算。
fn model_nodes(world: &World, content: Entity) -> Vec<(Entity, Affine3A, bool)> {
    let mut result = Vec::new();
    let mut pending = vec![(content, Affine3A::IDENTITY, true)];
    while let Some((entity, parent, visible)) = pending.pop() {
        let transform = world
            .get::<Transform>(entity)
            .map_or(Affine3A::IDENTITY, Transform::compute_affine);
        let affine = parent * transform;
        let visible = visible && world.get::<Visibility>(entity) != Some(&Visibility::Hidden);
        result.push((entity, affine, visible));
        if let Some(children) = world.get::<Children>(entity) {
            pending.extend(children.iter().map(|child| (child, affine, visible)));
        }
    }
    result
}

/// 等待真正的依赖及场景实例；初次就绪时设置所有后代的层并测量几何。
fn prepare_previews(world: &mut World) {
    let Some(mut previews) = world.remove_resource::<Previews>() else {
        return;
    };
    for (index, preview) in previews.items.iter_mut().enumerate() {
        let nodes = model_nodes(world, preview.content);
        // VOX 插件可能在实例就绪后继续展开后代；渲染层不能只在首帧设置。
        for (entity, _, _) in &nodes {
            if world.get::<RenderLayers>(*entity) != Some(&RenderLayers::layer(index + 1)) {
                world
                    .entity_mut(*entity)
                    .insert(RenderLayers::layer(index + 1));
            }
        }
        if preview.size.is_some() || preview.error.is_some() {
            continue;
        }
        let server = world.resource::<AssetServer>();
        let spawner = world.resource::<WorldInstanceSpawner>();
        let mut ready = !nodes.is_empty();
        let mut error = None;
        for (entity, _, _) in &nodes {
            if let Some(root) = world.get::<WorldAssetRoot>(*entity) {
                if let Some(LoadState::Failed(reason)) = server.get_load_state(&root.0) {
                    error = Some(reason.to_string());
                }
                ready &= server.is_loaded_with_dependencies(&root.0)
                    && world
                        .get::<WorldInstance>(*entity)
                        .is_some_and(|instance| spawner.instance_is_ready(**instance));
            }
            if let Some(failure) = world.get::<track::TracksFailed>(*entity) {
                error = Some(failure.0.clone());
            }
        }
        if ENTRIES[index].category == 1 || ENTRIES[index].category == 3 {
            ready &= world
                .get::<track::TracksValidated>(preview.content)
                .is_some();
        }
        if previews.started.elapsed().as_secs() > 30 {
            error.get_or_insert("模型加载超过30秒".into());
        }
        if let Some(error) = error {
            error!("画廊模型加载失败：{}：{error}", ENTRIES[index].path);
            preview.error = Some(error);
            continue;
        }
        if !ready {
            continue;
        }
        let mesh_count = nodes
            .iter()
            .filter(|(entity, _, _)| world.get::<Mesh3d>(*entity).is_some())
            .count();
        preview.stable_frames = if mesh_count > 0 && mesh_count == preview.mesh_count {
            preview.stable_frames + 1
        } else {
            0
        };
        preview.mesh_count = mesh_count;
        if preview.stable_frames < 8 {
            continue;
        }
        let mut low = Vec3::splat(f32::INFINITY);
        let mut high = Vec3::splat(f32::NEG_INFINITY);
        for (entity, affine, visible) in &nodes {
            world
                .entity_mut(*entity)
                .insert(RenderLayers::layer(index + 1));
            if !visible {
                continue;
            }
            let Some(mesh) = world.get::<Mesh3d>(*entity) else {
                continue;
            };
            let Some(aabb) = world
                .resource::<Assets<Mesh>>()
                .get(&mesh.0)
                .and_then(MeshAabb::get_aabb)
            else {
                continue;
            };
            let center = Vec3::from(aabb.center);
            let half = Vec3::from(aabb.half_extents);
            for x in [-1.0, 1.0] {
                for y in [-1.0, 1.0] {
                    for z in [-1.0, 1.0] {
                        let point = affine.transform_point3(center + half * Vec3::new(x, y, z));
                        low = low.min(point);
                        high = high.max(point);
                    }
                }
            }
        }
        if !low.is_finite() || !high.is_finite() {
            preview.error = Some("模型没有可见网格".into());
            error!("画廊模型没有可见网格：{}", ENTRIES[index].path);
            continue;
        }
        let size = high - low;
        // 整车保留与游戏一致的 body 根原点，炮管不能把旋转轴拉向车头。
        let pivot = if ENTRIES[index].category == 3 {
            Vec3::ZERO
        } else {
            (low + high) * 0.5
        };
        world
            .get_mut::<Transform>(preview.content)
            .unwrap()
            .translation = -pivot;
        // 以实际旋转轴为球心，取最远边界的半径；非对称炮管也不会在自转中裁切。
        let radius = (low - pivot).abs().max((high - pivot).abs()).length();
        if let Some(mut projection) = world.get_mut::<Projection>(preview.camera) {
            if let Projection::Orthographic(ref mut ortho) = *projection {
                ortho.scaling_mode = ScalingMode::FixedVertical {
                    viewport_height: radius * 2.0 * 1.15,
                };
            }
        }
        preview.size = Some(size);
        info!(
            "画廊模型就绪：{}，长宽高 {:.1}×{:.1}×{:.1} 米",
            ENTRIES[index].name(),
            size.x,
            size.z,
            size.y
        );
    }
    world.insert_resource(previews);
}

/// 相位只由时间决定，各卡片错开，履带轮组保持静止安装姿态。
fn spin(time: Res<Time>, previews: Res<Previews>, mut transforms: Query<&mut Transform>) {
    for (index, preview) in previews.items.iter().enumerate() {
        if preview.size.is_some() {
            if let Ok(mut transform) = transforms.get_mut(preview.root) {
                transform.rotation = Quat::from_rotation_y(
                    (time.elapsed_secs() + 3.0 + index as f32 * 2.6) * std::f32::consts::TAU / 16.0,
                );
            }
        }
    }
}

/// 错误卡片也完成等待，用户仍可返回首页；错误细节写入日志与离屏报告。
pub(crate) fn assets_ready(world: &World) -> bool {
    world.get_resource::<Previews>().is_some_and(|previews| {
        previews
            .items
            .iter()
            .all(|p| p.size.is_some() || p.error.is_some())
    })
}

/// 离屏验收读取真实子树，检查轮组、相位、渲染隔离和行为隔离。
pub(crate) fn validate(world: &World) -> Result<String, String> {
    let previews = world.resource::<Previews>();
    let mut report = String::new();
    for (index, preview) in previews.items.iter().enumerate() {
        if let Some(error) = &preview.error {
            return Err(format!("{}：{error}", ENTRIES[index].path));
        }
        let size = preview.size.ok_or("预览尺寸尚未就绪")?;
        let nodes = model_nodes(world, preview.content);
        let mut visible_phases = 0;
        let mut wheels = 0;
        for (entity, _, visible) in nodes {
            if world.get::<crate::gameplay::player::Tank>(entity).is_some()
                || world.get::<crate::gameplay::enemy::EnemyTank>(entity).is_some()
                || world.get::<avian3d::prelude::RigidBody>(entity).is_some()
            {
                return Err("预览混入游戏行为或物理组件".into());
            }
            if world.get::<Mesh3d>(entity).is_some()
                && world.get::<RenderLayers>(entity) != Some(&RenderLayers::layer(index + 1))
            {
                return Err(format!(
                    "模型网格渲染层不匹配：{} {entity:?}",
                    ENTRIES[index].path
                ));
            }
            if let Some(name) = world.get::<Name>(entity) {
                visible_phases += usize::from(
                    visible
                        && name.as_str().starts_with("track_")
                        && world.get::<WorldAssetRoot>(entity).is_some(),
                );
                wheels += usize::from(
                    name.as_str().starts_with("wheel_")
                        && world.get::<WorldAssetRoot>(entity).is_some(),
                );
            }
        }
        if matches!(ENTRIES[index].category, 1 | 3)
            && (visible_phases != 2 || wheels != if ENTRIES[index].enemy { 12 } else { 14 })
        {
            return Err(format!("履带可见相位/轮组异常：{visible_phases}/{wheels}"));
        }
        if ENTRIES[index].category == 3 {
            let expected = if ENTRIES[index].enemy { 9.0 } else { 10.5 };
            if (size.x - expected).abs() > 0.01 {
                return Err(format!("整车视觉长度异常：{}", size.x));
            }
            if world.get::<Transform>(preview.content).unwrap().translation != Vec3::ZERO {
                return Err("整车旋转轴偏离 body 根节点原点".into());
            }
            report.push_str("整车旋转轴：body 根节点原点；不按组合包围盒重新居中\n");
        }
        report.push_str(&format!(
            "{}：{}，长宽高 {:.1}×{:.1}×{:.1} 米；可见相位 {visible_phases}；轮 {wheels}\n",
            ENTRIES[index].path,
            ENTRIES[index].name(),
            size.x,
            size.z,
            size.y
        ));
    }
    Ok(report)
}

/// 供离屏验证操作同一指针与滚动状态，不绕过页面输入处理。
pub(crate) fn view_mut(world: &mut World) -> Mut<'_, ui::View> {
    world.resource_mut::<ui::View>()
}

/// 黑场交接时删除全部页面根，并释放预览专用纹理。
fn cleanup(
    mut commands: Commands,
    roots: Query<Entity, With<GalleryEntity>>,
    previews: Res<Previews>,
    mut images: ResMut<Assets<Image>>,
) {
    for root in &roots {
        commands.entity(root).despawn();
    }
    for preview in &previews.items {
        images.remove(preview.image.id());
    }
    commands.remove_resource::<Previews>();
}

#[cfg(test)]
mod tests {
    use super::*;
    /// 清单只包含当前真实型号，整车型号来自车身目录。
    #[test]
    fn catalog_uses_real_variant_directories() {
        assert_eq!(
            ENTRIES.map(Entry::name),
            [
                "lance", "spit", "cleat", "crawl", "anvil", "carapace", "anvil", "carapace"
            ]
        );
        for category in 0..4 {
            assert_eq!(ENTRIES.iter().filter(|e| e.category == category).count(), 2);
        }
        for entry in ENTRIES {
            assert!(std::path::Path::new("assets").join(entry.path).is_file());
        }
    }

    /// 无 GPU 和窗口，使用真实加载器验收八种预览的几何、轮组及资源清理。
    #[test]
    fn real_previews_measure_visible_geometry_and_clean_instances() {
        use bevy::{
            asset::AssetApp, camera::visibility::VisibilityClass, ecs::system::RunSystemOnce,
            light::FogVolume, world_serialization::WorldSerializationPlugin,
        };
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            ImagePlugin::default(),
            WorldSerializationPlugin,
            TransformPlugin,
            bevy_vox_scene::VoxScenePlugin {
                global_settings: Some(bevy_vox_scene::VoxLoaderSettings {
                    voxel_size: crate::tank::model::VOXEL_SIZE,
                    ..default()
                }),
            },
            track::TrackAnimationPlugin,
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
        .add_systems(Startup, spawn_previews)
        .add_systems(Update, prepare_previews);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            app.update();
            if assets_ready(app.world()) {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "真实预览加载超时");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        validate(app.world()).unwrap();
        // X/Y/Z 为长/高/宽；这些边界来自真实体素，不能替换为共同画布或碰撞盒。
        let expected = [
            Vec3::new(4.5, 0.4, 0.4),
            Vec3::new(3.8, 0.4, 0.4),
            Vec3::new(7.5, 1.3, 3.6),
            Vec3::new(6.6, 1.4, 3.4),
            Vec3::new(7.5, 2.2, 3.4),
            Vec3::new(6.5, 2.1, 3.2),
            Vec3::new(10.5, 2.6, 3.6),
            Vec3::new(9.0, 2.6, 3.4),
        ];
        for (index, (preview, size)) in app
            .world()
            .resource::<Previews>()
            .items
            .iter()
            .zip(expected)
            .enumerate()
        {
            assert!(
                preview.size.unwrap().abs_diff_eq(size, 0.001),
                "{:?} != {size:?}",
                preview.size
            );
            if ENTRIES[index].category == 3 {
                assert_eq!(
                    app.world()
                        .get::<Transform>(preview.content)
                        .unwrap()
                        .translation,
                    Vec3::ZERO,
                    "整车必须绕 body 根节点原点旋转，不能按包含炮管的包围盒重新居中"
                );
                let camera = app.world().get::<Transform>(preview.camera).unwrap();
                let Projection::Orthographic(projection) =
                    app.world().get::<Projection>(preview.camera).unwrap()
                else {
                    panic!("预览应使用正交相机");
                };
                let ScalingMode::FixedVertical { viewport_height } = projection.scaling_mode else {
                    panic!("预览应按竖直视野适配");
                };
                let image_size = app
                    .world()
                    .resource::<Assets<Image>>()
                    .get(&preview.image)
                    .unwrap()
                    .size()
                    .as_vec2();
                let half_height = viewport_height * 0.5;
                let half_width = half_height * image_size.x / image_size.y;
                // 真实网格边界转过完整一周，在相机空间检查四边，覆盖长炮管的扫掠范围。
                for (entity, affine, visible) in model_nodes(app.world(), preview.content) {
                    if !visible {
                        continue;
                    }
                    let Some(mesh) = app.world().get::<Mesh3d>(entity) else {
                        continue;
                    };
                    let aabb = app
                        .world()
                        .resource::<Assets<Mesh>>()
                        .get(&mesh.0)
                        .unwrap()
                        .get_aabb()
                        .unwrap();
                    for x in [-1.0, 1.0] {
                        for y in [-1.0, 1.0] {
                            for z in [-1.0, 1.0] {
                                let point = affine.transform_point3(
                                    Vec3::from(aabb.center)
                                        + Vec3::from(aabb.half_extents) * Vec3::new(x, y, z),
                                );
                                for degrees in (0..360).step_by(15) {
                                    let rotated =
                                        Quat::from_rotation_y((degrees as f32).to_radians())
                                            * point;
                                    let view_point =
                                        camera.rotation.inverse() * (rotated - camera.translation);
                                    assert!(
                                        view_point.x.abs() < half_width
                                            && view_point.y.abs() < half_height
                                            && view_point.z < 0.0,
                                        "{} 自转时裁切：{degrees}° {view_point:?}",
                                        ENTRIES[index].name()
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        let instances: Vec<_> = app
            .world_mut()
            .query::<&WorldInstance>()
            .iter(app.world())
            .map(|i| **i)
            .collect();
        assert_eq!(instances.len(), 84);
        // 模拟真实文件缺失，不修改任何资产；失败项必须结束等待并保留返回能力。
        let missing = app
            .world()
            .resource::<AssetServer>()
            .load("tank/barrels/player/__missing_gallery_test__/barrel.vox");
        let content = app.world().resource::<Previews>().items[0].content;
        app.world_mut().resource_mut::<Previews>().items[0].size = None;
        app.world_mut()
            .entity_mut(content)
            .insert(WorldAssetRoot(missing));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.world().resource::<Previews>().items[0].error.is_none() {
            app.update();
            assert!(std::time::Instant::now() < deadline, "失败资产没有结束等待");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(assets_ready(app.world()));
        assert!(validate(app.world()).is_err());
        app.world_mut().run_system_once(cleanup).unwrap();
        app.update();
        assert!(!app.world().contains_resource::<Previews>());
        assert!(instances.iter().all(|id| {
            !app.world()
                .resource::<WorldInstanceSpawner>()
                .instance_is_ready(*id)
        }));
    }
}

//! 画廊模型预览：每张卡片独占的模型、相机与离屏纹理，只做纯视觉组装。
use super::{ENTRIES, GalleryEntity};
use crate::{tank::{track, visual}, ui::resources::colors};
use bevy::{
    asset::LoadState,
    camera::{RenderTarget, ScalingMode, primitives::MeshAabb, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    math::Affine3A,
    prelude::*,
    render::render_resource::TextureFormat,
    world_serialization::{WorldInstance, WorldInstanceSpawner},
};

/// 一张卡片独占的模型、相机与纹理；尺寸在未旋转的模型空间测量。
pub(crate) struct Preview {
    pub(crate) root: Entity,
    pub(super) content: Entity,
    pub(super) camera: Entity,
    pub(crate) image: Handle<Image>,
    pub(super) size: Option<Vec3>,
    pub(super) error: Option<String>,
    /// 场景根就绪后仍等待网格后代稳定，避免插件延迟生成的节点漏分层或漏计尺寸。
    mesh_count: usize,
    stable_frames: u8,
}
/// 页面持有强句柄，退出后显式释放动态纹理。
#[derive(Resource)]
pub(crate) struct Previews {
    pub(crate) items: Vec<Preview>,
    started: std::time::Instant,
}

/// 每项独占渲染层；整车只添加视觉子树，不使用 Tank/EnemyTank 标记。
pub(super) fn spawn_previews(
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
                    clear_color: ClearColorConfig::Custom(colors::gallery::SURFACE),
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
pub(super) fn model_nodes(world: &World, content: Entity) -> Vec<(Entity, Affine3A, bool)> {
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
pub(super) fn prepare_previews(world: &mut World) {
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
pub(super) fn spin(time: Res<Time>, previews: Res<Previews>, mut transforms: Query<&mut Transform>) {
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

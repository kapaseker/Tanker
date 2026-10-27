//! 画廊与详情共用的真实模型目录、视觉组装、离屏纹理和几何测量。
use crate::tank::visual;
use crate::{tank::track, ui::resources::colors};
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
pub(crate) const ENTRIES: [Entry; 9] = [
    Entry {
        category: 0,
        path: visual::PLAYER_BARREL.path,
        enemy: false,
    },
    Entry {
        category: 0,
        path: visual::ENEMY_BARREL.path,
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
    Entry {
        category: 4,
        path: "obstacles/concrete/obstacle.vox",
        enemy: false,
    },
];

/// 预览专用实体；页面退出时统一清理，避免混入玩法。
#[derive(Component)]
pub(crate) struct PreviewEntity;

/// 伴随子弹子树标记（仅 lance 配置）：只作"子弹与炮管组合"示意，
/// 不参与包围盒测量、自转中心与详情尺寸轴（测量时按本标记整树排除）。
#[derive(Component)]
pub(crate) struct BulletCompanion;

/// 一张卡片独占的模型、相机与纹理；尺寸在未旋转的模型空间测量。
pub(crate) struct Preview {
    pub(crate) root: Entity,
    /// 实际资源目录项，详情单项不依赖画廊中的纹理下标。
    pub(crate) entry: Entry,
    /// 适配完整旋转包络所需的球半径，供详情按视口纵横比取景。
    pub(crate) radius: f32,
    pub(crate) content: Entity,
    pub(crate) camera: Entity,
    pub(crate) image: Handle<Image>,
    pub(crate) size: Option<Vec3>,
    /// 测得的可见几何包围盒（content 局部空间，米；不含伴随子弹），
    /// 供详情页尺寸轴贴合模型边沿取位。
    pub(crate) bounds: Option<(Vec3, Vec3)>,
    /// 自转轴（content 局部空间）；content 平移取其负值，详情投影复用。
    pub(crate) pivot: Vec3,
    pub(crate) error: Option<String>,
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
pub(crate) fn spawn_previews(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
) {
    spawn(&mut commands, &assets, &mut images, &ENTRIES);
}

/// 画廊传完整清单，详情只传当前项；两者共用组装与就绪协议。
pub(crate) fn spawn(
    commands: &mut Commands,
    assets: &AssetServer,
    images: &mut Assets<Image>,
    entries: &[Entry],
) {
    let mut items = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let layer = RenderLayers::layer(index + 1);
        let root = commands
            .spawn((PreviewEntity, Transform::default(), Visibility::Inherited))
            .id();
        let content = commands
            .spawn((
                // 墩体资产厚度沿 X；预览转向后统一按长、宽、高展示尺寸。
                Transform::from_rotation(if entry.category == 4 {
                    Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
                } else {
                    Quat::IDENTITY
                }),
                Visibility::Inherited,
            ))
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
                commands,
                assets,
                content,
                if entry.enemy {
                    visual::ENEMY_BARREL.path
                } else {
                    visual::PLAYER_BARREL.path
                },
            );
        }
        // 伴随子弹：炮管 lance（唯一配置）在正上方叠加，表达与炮管的组合关系；
        // 不改变模型中心，也不参与尺寸与包围盒（测量时按 BulletCompanion 排除）。
        if entry.path == visual::PLAYER_BARREL.path {
            let bullet = commands
                .spawn((
                    BulletCompanion,
                    Transform::default(),
                    Visibility::Inherited,
                    WorldAssetRoot(assets.load(visual::LANCE_BULLET)),
                ))
                .id();
            commands.entity(content).add_child(bullet);
        }
        let image = images.add(Image::new_target_texture(
            460,
            360,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
        let camera = commands
            .spawn((
                PreviewEntity,
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
            entry: *entry,
            radius: 0.0,
            content,
            camera,
            image,
            size: None,
            bounds: None,
            pivot: Vec3::ZERO,
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

/// 层级展开时累计局部变换，显隐与伴随子弹排除沿父链传播；只显示的网格参与尺寸计算。
/// 返回元组为 (实体, 局部仿射, 是否可见, 是否位于伴随子弹子树内)。
pub(crate) fn model_nodes(world: &World, content: Entity) -> Vec<(Entity, Affine3A, bool, bool)> {
    let mut result = Vec::new();
    let mut pending = vec![(content, Affine3A::IDENTITY, true, false)];
    while let Some((entity, parent, visible, bullet)) = pending.pop() {
        let transform = world
            .get::<Transform>(entity)
            .map_or(Affine3A::IDENTITY, Transform::compute_affine);
        let affine = parent * transform;
        let visible = visible && world.get::<Visibility>(entity) != Some(&Visibility::Hidden);
        let bullet = bullet || world.get::<BulletCompanion>(entity).is_some();
        result.push((entity, affine, visible, bullet));
        if let Some(children) = world.get::<Children>(entity) {
            pending.extend(children.iter().map(|child| (child, affine, visible, bullet)));
        }
    }
    result
}

/// 等待真正的依赖及场景实例；初次就绪时设置所有后代的层并测量几何。
pub(crate) fn prepare_previews(world: &mut World) {
    let Some(mut previews) = world.remove_resource::<Previews>() else {
        return;
    };
    for (index, preview) in previews.items.iter_mut().enumerate() {
        let nodes = model_nodes(world, preview.content);
        // VOX 插件可能在实例就绪后继续展开后代；渲染层不能只在首帧设置。
        for (entity, _, _, _) in &nodes {
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
        for (entity, _, _, _) in &nodes {
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
        if preview.entry.category == 1 || preview.entry.category == 3 {
            ready &= world
                .get::<track::TracksValidated>(preview.content)
                .is_some();
        }
        if previews.started.elapsed().as_secs() > 30 {
            error.get_or_insert("模型加载超过30秒".into());
        }
        if let Some(error) = error {
            error!("模型预览加载失败：{}：{error}", preview.entry.path);
            preview.error = Some(error);
            continue;
        }
        if !ready {
            continue;
        }
        let mesh_count = nodes
            .iter()
            .filter(|(entity, _, _, _)| world.get::<Mesh3d>(*entity).is_some())
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
        // 主体包围盒与子弹本地包围盒分别累计；子弹不计入模型尺寸与自转中心。
        let mut low = Vec3::splat(f32::INFINITY);
        let mut high = Vec3::splat(f32::NEG_INFINITY);
        let mut bullet_low = Vec3::splat(f32::INFINITY);
        let mut bullet_high = Vec3::splat(f32::NEG_INFINITY);
        for (entity, affine, visible, bullet) in &nodes {
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
            let (target_low, target_high) = if *bullet {
                (&mut bullet_low, &mut bullet_high)
            } else {
                (&mut low, &mut high)
            };
            for x in [-1.0, 1.0] {
                for y in [-1.0, 1.0] {
                    for z in [-1.0, 1.0] {
                        let point = affine.transform_point3(center + half * Vec3::new(x, y, z));
                        *target_low = target_low.min(point);
                        *target_high = target_high.max(point);
                    }
                }
            }
        }
        if !low.is_finite() || !high.is_finite() {
            preview.error = Some("模型没有可见网格".into());
            error!("模型预览没有可见网格：{}", preview.entry.path);
            continue;
        }
        let size = high - low;
        // 整车保留与游戏一致的 body 根原点，炮管不能把旋转轴拉向车头。
        let pivot = if preview.entry.category == 3 {
            Vec3::ZERO
        } else {
            (low + high) * 0.5
        };
        // 伴随子弹就位：模型上缘上方 0.8 米（design.md 标准间距），XZ 对中自转轴；
        // content 平移为 -pivot，故本地目标 = pivot + 上方偏移；子弹本地包围盒
        // 在其根尚未平移时测得，平移量按该包围盒换算。
        if let Some((bullet_root, _, _, _)) =
            nodes.iter().find(|(e, _, _, _)| world.get::<BulletCompanion>(*e).is_some()).copied()
            .filter(|_| bullet_low.is_finite())
        {
            let bullet_center = (bullet_low + bullet_high) * 0.5;
            world
                .get_mut::<Transform>(bullet_root)
                .unwrap()
                .translation = Vec3::new(
                pivot.x - bullet_center.x,
                high.y + crate::ui::resources::dimens::BULLET_GAP - bullet_low.y,
                pivot.z - bullet_center.z,
            );
        }
        world
            .get_mut::<Transform>(preview.content)
            .unwrap()
            .translation = -pivot;
        preview.pivot = pivot;
        // 以实际旋转轴为球心，取最远边界的半径；非对称炮管也不会在自转中裁切。
        let radius = (low - pivot).abs().max((high - pivot).abs()).length();
        if let Some(mut projection) = world.get_mut::<Projection>(preview.camera) {
            if let Projection::Orthographic(ref mut ortho) = *projection {
                ortho.scaling_mode = ScalingMode::FixedVertical {
                    viewport_height: radius * 2.0 * 1.15,
                };
            }
        }
        preview.radius = radius;
        preview.size = Some(size);
        preview.bounds = Some((low, high));
        info!(
            "模型预览就绪：{}，长宽高 {:.1}×{:.1}×{:.1} 米",
            preview.entry.name(),
            size.x,
            size.z,
            size.y
        );
    }
    world.insert_resource(previews);
}

/// 相位只由时间决定，各卡片错开，履带轮组保持静止安装姿态。
pub(crate) fn spin(
    time: Res<Time>,
    previews: Res<Previews>,
    mut transforms: Query<&mut Transform>,
) {
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

/// 离屏验收读取真实子树，检查轮组、相位、渲染隔离和行为隔离。
pub(crate) fn validate(world: &World) -> Result<String, String> {
    let previews = world.resource::<Previews>();
    let mut report = String::new();
    for (index, preview) in previews.items.iter().enumerate() {
        if let Some(error) = &preview.error {
            return Err(format!("{}：{error}", preview.entry.path));
        }
        let size = preview.size.ok_or("预览尺寸尚未就绪")?;
        let nodes = model_nodes(world, preview.content);
        let mut visible_phases = 0;
        let mut wheels = 0;
        for (entity, _, visible, _) in nodes {
            if world.get::<crate::gameplay::player::Tank>(entity).is_some()
                || world
                    .get::<crate::gameplay::enemy::EnemyTank>(entity)
                    .is_some()
                || world.get::<avian3d::prelude::RigidBody>(entity).is_some()
            {
                return Err("预览混入游戏行为或物理组件".into());
            }
            if world.get::<Mesh3d>(entity).is_some()
                && world.get::<RenderLayers>(entity) != Some(&RenderLayers::layer(index + 1))
            {
                return Err(format!(
                    "模型网格渲染层不匹配：{} {entity:?}",
                    preview.entry.path
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
        if matches!(preview.entry.category, 1 | 3)
            && (visible_phases != 2 || wheels != if preview.entry.enemy { 12 } else { 14 })
        {
            return Err(format!("履带可见相位/轮组异常：{visible_phases}/{wheels}"));
        }
        if preview.entry.category == 3 {
            let expected = if preview.entry.enemy { 9.0 } else { 10.5 };
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
            preview.entry.path,
            preview.entry.name(),
            size.x,
            size.z,
            size.y
        ));
    }
    Ok(report)
}

/// 错误项也结束等待，页面显示失败并仍可返回。
pub(crate) fn assets_ready(world: &World) -> bool {
    world.get_resource::<Previews>().is_some_and(|previews| {
        previews
            .items
            .iter()
            .all(|p| p.size.is_some() || p.error.is_some())
    })
}

/// 页面黑场交接时释放预览实体与专用动态纹理。
pub(crate) fn cleanup(
    mut commands: Commands,
    roots: Query<Entity, With<PreviewEntity>>,
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

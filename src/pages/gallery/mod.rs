//! 真实 VOX 模型画廊：纯视觉预览与页面生命周期，不参与战斗和物理。
mod preview;
mod ui;

use crate::{
    app::{
        state::GameState,
        transition::{Fade, SceneTarget},
    },
    tank::visual,
};
use bevy::{camera::visibility::RenderLayers, prelude::*};

pub(crate) use preview::Previews;
use preview::{model_nodes, prepare_previews, spawn_previews, spin};

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

/// 安装页面及纯视觉预览系统。
pub(crate) struct GalleryPlugin;
impl Plugin for GalleryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ui::View>()
            .add_systems(
                OnEnter(GameState::Gallery),
                (spawn_previews, ui::spawn).chain(),
            )
            .add_systems(
                Update,
                (prepare_previews, spin).chain().run_if(in_gallery),
            )
            .add_systems(Update, (ui::input, ui::update).chain().run_if(in_gallery))
            .add_systems(OnExit(GameState::GalleryToMenu), cleanup);
    }
}

/// 渐出时保留最后一页内容与动画，黑场交接后才销毁。
fn in_gallery(state: Res<State<GameState>>) -> bool {
    matches!(state.get(), GameState::Gallery | GameState::GalleryToMenu)
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
            asset::AssetApp,
            camera::ScalingMode,
            camera::primitives::MeshAabb,
            camera::visibility::VisibilityClass,
            ecs::system::RunSystemOnce,
            light::FogVolume,
            world_serialization::{WorldInstance, WorldSerializationPlugin},
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
            crate::tank::track::TrackAnimationPlugin,
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

//! 地形统一入口：登记地图资产、资源就绪后编译并铺设地形、退出对局时回收本局地形。
//!
//! 地形由 `scripts/generate_ground.py` 生成高度图与土色图，
//! 存放在 `assets/terrain/<地形名>/` 下：`.ht` 为高度（1200×1200 字节矩阵），
//! `.abd` 为土色（`[调色板数量 u8][数量×3 字节 RGB][1200×1200 索引字节]`，
//! 索引指向文件自带的调色板）。
//! 职责按子模块拆分：`map` 负责两张图的加载与解析，`mesh` 编译体素风格视觉网格，
//! `collider` 生成体素台阶复合碰撞体与边界墙；本模块保留统一入口与共享场地常量。
//! 视觉网格与碰撞体同源：都由同一份量化档位编译，
//! 碰撞体顶面即台阶顶、侧面即竖直坎，
//! 不存在高度场式相邻采样插值产生的隐形斜坡。

mod collider;
mod map;
mod mesh;

// 平坦竞技场仅存在于测试构建，重导出供各模块的物理测试按 `terrain::spawn_arena` 调用。
#[cfg(test)]
pub(crate) use collider::spawn_arena;

use avian3d::prelude::*;
use bevy::{asset::LoadState, prelude::*};

use collider::{collider_cuboids, spawn_walls};
use map::{parse_albedo, terrain_texture, GroundMap, GroundMapLoader};
use mesh::{build_chunk, ChunkSource};

/// 正方形场地总面积，单位：平方米。
const AREA: f32 = 360_000.0;
/// 场地边长，单位：米（即 [`AREA`] 的平方根；不用 `sqrt` 是为了能写成常量）。
pub(crate) const FIELD_SIDE: f32 = 600.0;
/// 地形图的作者化采样间距，单位：米；每个采样格渲染成一个方块。
pub(super) const AUTHOR_RES: f32 = 0.5;
/// 地形图单边采样数（[`FIELD_SIDE`] ÷ [`AUTHOR_RES`]）。
pub(super) const MAP_SIDE: usize = 1200;
/// 地形资产根目录（相对 assets/）；每个子目录是一个地形，含 `ground.ht` / `ground.abd`。
const TERRAIN_ROOT: &str = "terrain";
/// 当前启用的地形目录名；将来多地形时改为按清单或参数选择。
const TERRAIN_NAME: &str = "ground";
/// 高度字节中的平地基准值，即 0 米。
pub(super) const FLAT: u8 = 10;
/// 地形量化粒度，单位：米；也是台阶高度与贴图色块的对齐网格。
pub(super) const TERRAIN_STEP: f32 = 0.1;
/// 地形起伏幅度上限，单位：米；对应高度字节区间 [FLAT − 10, FLAT + 10]。
pub(super) const TERRAIN_HALF_RANGE: f32 = 1.0;
/// 地形网格分块边长，单位：米。
const CHUNK_SIDE: f32 = 60.0;
/// 每块单边采样数（[`CHUNK_SIDE`] ÷ [`AUTHOR_RES`]）。
const CHUNK_CELLS: usize = 120;
/// 场地单边的分块数（[`FIELD_SIDE`] ÷ [`CHUNK_SIDE`]）。
const CHUNK_COUNT: usize = 10;

/// 标记程序化地形的一块视觉网格，供测试与后续系统识别。
#[derive(Component)]
pub(crate) struct TerrainChunk;

/// 标记承载地形高度场的静态物理表面。
#[derive(Component)]
pub(crate) struct TerrainSurface;

/// 地形自有的根实体标记（视觉块、碰撞面与边界墙）：地形实体由本模块的
/// [`reset_session`] 回收，不依赖对局或页面状态的标记。
#[derive(Component)]
pub(super) struct TerrainEntity;

/// 标记地形网格与高度场碰撞体已铺设完成，供依赖地形的生成逻辑（敌人）等待。
#[derive(Resource)]
pub(crate) struct TerrainReady;

/// 两张地形图句柄：驱动汇报与一次性地形编译。
#[derive(Resource)]
struct GroundAssets {
    heights: Handle<GroundMap>,
    colors: Handle<GroundMap>,
}

/// 返回首页时清除本局地形：回收地形自有标记的实体与生成完成标志，
/// 允许下一局重新铺设；预加载的共享资产继续复用。
pub(crate) fn reset_session(world: &mut World) {
    let entities: Vec<_> = world
        .query_filtered::<Entity, With<TerrainEntity>>()
        .iter(world)
        .collect();
    for entity in entities {
        world.despawn(entity);
    }
    world.remove_resource::<GroundAssets>();
    world.remove_resource::<TerrainReady>();
}

/// 注册地形图资产、编译地形网格并铺设物理表面。
pub(crate) struct GroundPlugin;

impl Plugin for GroundPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<GroundMap>()
            .init_asset_loader::<GroundMapLoader>()
            // 地形是战斗场景的一部分：进入 InGame 才铺设，主菜单期保持暗底。
            .add_systems(Update, (spawn_terrain, report_ground));
    }
}

/// 进入 InGame 时登记两张地形图，实际铺设等资源就绪后由 [`spawn_terrain`] 完成。
/// 由 `gameplay::session` 在对局生成时调用；接收借用参数以便作为普通函数调用。
pub(crate) fn load_ground(commands: &mut Commands, assets: &AssetServer) {
    // 路径形如 "terrain/ground/ground.ht"；Bevy 资产路径固定用正斜杠。
    commands.insert_resource(GroundAssets {
        heights: assets.load(format!("{TERRAIN_ROOT}/{TERRAIN_NAME}/ground.ht")),
        colors: assets.load(format!("{TERRAIN_ROOT}/{TERRAIN_NAME}/ground.abd")),
    });
}

/// 地形图就绪后编译与铺设地形：按 60 米分块生成 100 块视觉网格，
/// 并在同一实体上挂高度场碰撞体。
fn spawn_terrain(
    mut commands: Commands,
    // 主菜单期地形图尚未登记，资源缺失时静默等待。
    ground: Option<Res<GroundAssets>>,
    built: Option<Res<TerrainReady>>,
    maps: Res<Assets<GroundMap>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    if built.is_some() {
        return;
    }
    let Some(ground) = ground else { return; };
    let (Some(heights), Some(colors)) = (maps.get(&ground.heights), maps.get(&ground.colors)) else {
        return;
    };
    // 资产坏文件即死：主线程 panic，原因即错误日志，不带病运行。
    if heights.0.len() != MAP_SIDE * MAP_SIDE {
        panic!(
            "地形高度图不合规：应为 {} 字节，实际 {} 字节；请重新运行 scripts/generate_ground.py",
            MAP_SIDE * MAP_SIDE,
            heights.0.len()
        );
    }
    let (palette, indices) = parse_albedo(&colors.0);

    let material = materials.add(StandardMaterial {
        base_color_texture: Some(images.add(terrain_texture(indices, &palette))),
        // 与 VOX 场景地板一致的哑光土质：粗糙度 1、非金属。
        perceptual_roughness: 1.0,
        metallic: 0.0,
        // 台阶竖直面可能背向相机掠射观察，关闭背面剔除避免整片剖面被裁掉。
        cull_mode: None,
        ..default()
    });
    // 高度字节转成以 0 米为基准的量化档位，供网格与高度场共用。
    let levels: Vec<i32> = heights
        .0
        .iter()
        .map(|value| *value as i32 - FLAT as i32)
        .collect();
    // 碰撞体 cuboid 总数，用于汇报合并效果、发现退化。
    let mut cuboid_count = 0usize;
    for row in 0..CHUNK_COUNT {
        for col in 0..CHUNK_COUNT {
            let start = (row * CHUNK_CELLS, col * CHUNK_CELLS);
            let origin = (
                row as f32 * CHUNK_SIDE - FIELD_SIDE / 2.0,
                col as f32 * CHUNK_SIDE - FIELD_SIDE / 2.0,
            );
            let chunk = build_chunk(&ChunkSource {
                levels: &levels,
                colors: indices,
                palette: &palette,
                side: MAP_SIDE,
                start,
                cells: CHUNK_CELLS,
                origin,
            });
            commands.spawn((
                TerrainChunk,
                TerrainEntity,
                Mesh3d(meshes.add(chunk)),
                MeshMaterial3d(material.clone()),
            ));
            // 物理与视觉同源：同一份量化档位编译出体素台阶复合碰撞体。
            let cuboids = collider_cuboids(&levels, MAP_SIDE, start, CHUNK_CELLS, origin);
            cuboid_count += cuboids.len();
            commands.spawn((
                TerrainSurface,
                TerrainEntity,
                RigidBody::Static,
                Collider::compound(
                    cuboids
                        .into_iter()
                        .map(|(center, size)| (center, Rotation::IDENTITY, Collider::cuboid(size.x, size.y, size.z)))
                        .collect(),
                ),
                Friction::ZERO,
            ));
        }
    }
    spawn_walls(&mut commands);
    // 铺设完成后发就绪信号，敌人等依赖地形的生成逻辑据此确定时机。
    commands.insert_resource(TerrainReady);
    info!("地形体素碰撞体铺设完成：{CHUNK_COUNT}×{CHUNK_COUNT} 块共 {cuboid_count} 个 cuboid");
}

/// 加载失败与完成各汇报一次，避免只检查主资产掩盖另一张图的问题。
fn report_ground(
    // 主菜单期地形图尚未登记，资源缺失时静默等待。
    ground: Option<Res<GroundAssets>>,
    assets: Res<AssetServer>,
    mut reported: Local<bool>,
) {
    let Some(ground) = ground else { return; };
    if *reported {
        return;
    }
    for (name, handle) in [
        ("高度图", &ground.heights),
        ("土色图", &ground.colors),
    ] {
        match assets.get_load_state(handle) {
            Some(LoadState::Failed(error)) => {
                error!("地形{name}加载失败：{error}");
                *reported = true;
                return;
            }
            _ if assets.is_loaded_with_dependencies(handle) => {}
            _ => return,
        }
    }
    info!(
        "地形地图加载完成：{} 米见方 / {:.0} 平方米（高度图 {MAP_SIDE}×{MAP_SIDE} @{AUTHOR_RES} 米，\
         渲染台阶 {TERRAIN_STEP} 米，起伏 ±{TERRAIN_HALF_RANGE} 米，{CHUNK_COUNT}×{CHUNK_COUNT} 块）",
        FIELD_SIDE, AREA
    );
    *reported = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 场地常量必须互相对齐，防止面积、边长、采样数与分块数脱节。
    #[test]
    fn field_constants_stay_consistent() {
        assert!((FIELD_SIDE * FIELD_SIDE - AREA).abs() < 0.001);
        assert!((MAP_SIDE as f32 * AUTHOR_RES - FIELD_SIDE).abs() < 0.001);
        assert_eq!(CHUNK_CELLS * CHUNK_COUNT, MAP_SIDE);
        assert!((CHUNK_SIDE * CHUNK_COUNT as f32 - FIELD_SIDE).abs() < 0.001);
        assert_eq!(TERRAIN_HALF_RANGE / TERRAIN_STEP, FLAT as f32);
        // 首块与末块的世界原点把场地正好铺满 [−300, 300]。
        assert_eq!(0.0 * CHUNK_SIDE - FIELD_SIDE / 2.0, -300.0);
        assert_eq!(9.0 * CHUNK_SIDE - FIELD_SIDE / 2.0 + CHUNK_SIDE, 300.0);
    }
}

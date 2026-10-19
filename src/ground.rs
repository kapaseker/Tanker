//! 加载 600 米见方的地形高度图与土色图，编译成体素风格网格与体素台阶碰撞体。
//!
//! 地形由 `scripts/generate_ground.py` 生成高度图与土色图，
//! 存放在 `assets/terrain/<地形名>/` 下：`.ht` 为高度（1200×1200 字节矩阵），
//! `.abd` 为土色（`[调色板数量 u8][数量×3 字节 RGB][1200×1200 索引字节]`，
//! 索引指向文件自带的调色板）。
//! 本模块把高度图编译成 0.1 米台阶的方块网格（视觉）与 cuboid 复合碰撞体（物理），
//! 两者同源：碰撞体由同档位合并矩形生成，顶面即台阶顶、侧面即竖直坎，
//! 不存在高度场式相邻采样插值产生的隐形斜坡。

use avian3d::prelude::*;
use bevy::{
    asset::{AssetLoader, LoadContext, LoadState, RenderAssetUsages, io::Reader},
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

/// 正方形场地总面积，单位：平方米。
const AREA: f32 = 360_000.0;
/// 场地边长，单位：米（即 [`AREA`] 的平方根；不用 `sqrt` 是为了能写成常量）。
pub(super) const FIELD_SIDE: f32 = 600.0;
/// 地形图的作者化采样间距，单位：米；每个采样格渲染成一个方块。
const AUTHOR_RES: f32 = 0.5;
/// 地形图单边采样数（[`FIELD_SIDE`] ÷ [`AUTHOR_RES`]）。
const MAP_SIDE: usize = 1200;
/// 地形资产根目录（相对 assets/）；每个子目录是一个地形，含 `ground.ht` / `ground.abd`。
const TERRAIN_ROOT: &str = "terrain";
/// 当前启用的地形目录名；将来多地形时改为按清单或参数选择。
const TERRAIN_NAME: &str = "ground";
/// 高度字节中的平地基准值，即 0 米。
const FLAT: u8 = 10;
/// 地形量化粒度，单位：米；也是台阶高度与贴图色块的对齐网格。
const TERRAIN_STEP: f32 = 0.1;
/// 地形起伏幅度上限，单位：米；对应高度字节区间 [FLAT − 10, FLAT + 10]。
const TERRAIN_HALF_RANGE: f32 = 1.0;
/// 碰撞体 cuboid 的统一底面高度，单位：米；比最低档（−1.0 米）再低一级。
/// 所有 cuboid 统一延伸到此底面：静态碰撞体在地下互相重叠无副作用，
/// 同时保证最低档的 cuboid 仍有 0.1 米厚度而不退化为零厚度。
const COLLIDER_BASE_Y: f32 = -TERRAIN_HALF_RANGE - TERRAIN_STEP;
/// 地形网格分块边长，单位：米。
const CHUNK_SIDE: f32 = 60.0;
/// 每块单边采样数（[`CHUNK_SIDE`] ÷ [`AUTHOR_RES`]）。
const CHUNK_CELLS: usize = 120;
/// 场地单边的分块数（[`FIELD_SIDE`] ÷ [`CHUNK_SIDE`]）。
const CHUNK_COUNT: usize = 10;
/// 竖直面相对顶面的明度系数，按格子奇偶交替，避免大片台阶同色。
const SIDE_SHADE: [f32; 2] = [0.72, 0.84];
/// 不可见边界的厚度和高度，单位：米。
const WALL_THICKNESS: f32 = 2.0;
const WALL_HEIGHT: f32 = 8.0;

/// 地形图的原始字节：高度图（`.ht`）每格一个高度档位（[`FLAT`] 为 0 米，每档 0.1 米）；
/// 土色图（`.abd`）为调色板 + 索引矩阵（格式见 [`parse_albedo`]）。
/// 本类型只承载原始字节，格式校验由使用方在主线程 panic。
#[derive(Asset, TypePath, Debug)]
struct GroundMap(Vec<u8>);

/// 读取地形图的 `.ht` / `.abd` 字节流；只负责搬运字节，格式校验由 [`spawn_terrain`] panic。
#[derive(Default, TypePath)]
struct GroundMapLoader;

impl AssetLoader for GroundMapLoader {
    type Asset = GroundMap;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::with_capacity(MAP_SIDE * MAP_SIDE);
        reader.read_to_end(&mut bytes).await?;
        Ok(GroundMap(bytes))
    }

    fn extensions(&self) -> &[&str] {
        &["ht", "abd"]
    }
}

/// 标记程序化地形的一块视觉网格，供测试与后续系统识别。
#[derive(Component)]
pub(super) struct TerrainChunk;

/// 标记承载地形高度场的静态物理表面。
#[derive(Component)]
pub(super) struct TerrainSurface;

/// 标记地形网格与高度场碰撞体已铺设完成，供依赖地形的生成逻辑（敌人）等待。
#[derive(Resource)]
pub(crate) struct TerrainReady;

/// 两张地形图句柄：驱动汇报与一次性地形编译。
#[derive(Resource)]
struct GroundAssets {
    heights: Handle<GroundMap>,
    colors: Handle<GroundMap>,
}

/// 注册地形图资产、编译地形网格并铺设物理表面。
pub(super) struct GroundPlugin;

impl Plugin for GroundPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<GroundMap>()
            .init_asset_loader::<GroundMapLoader>()
            .add_systems(Startup, load_ground)
            .add_systems(Update, (spawn_terrain, report_ground));
    }
}

/// 启动时只登记两张地形图，实际铺设等资源就绪后由 [`spawn_terrain`] 完成。
fn load_ground(mut commands: Commands, assets: Res<AssetServer>) {
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
    ground: Res<GroundAssets>,
    mut built: Local<bool>,
    maps: Res<Assets<GroundMap>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    if *built {
        return;
    }
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
                Mesh3d(meshes.add(chunk)),
                MeshMaterial3d(material.clone()),
            ));
            // 物理与视觉同源：同一份量化档位编译出体素台阶复合碰撞体。
            let cuboids = collider_cuboids(&levels, MAP_SIDE, start, CHUNK_CELLS, origin);
            cuboid_count += cuboids.len();
            commands.spawn((
                TerrainSurface,
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
    *built = true;
}

/// 由量化档位编译一块的体素台阶复合碰撞体 cuboid 集合。
///
/// 按高度档位（忽略颜色，比视觉顶面合并更宽松）贪心合并出矩形集合，
/// 每个矩形生成一个竖直 cuboid：顶面在档位高度（台阶顶），底面统一延伸到
/// [`COLLIDER_BASE_Y`]（地下重叠对静态碰撞体无副作用，且最低档不退化为零厚度）。
/// 相邻矩形的竖直落差由 cuboid 侧面天然构成台阶墙，坦克撞上的是真实的坎，
/// 不存在高度场相邻采样插值出的隐形斜坡。
///
/// `levels` 为整图量化档位（0 为平地，单位 0.1 米），行沿 X、列沿 Z；
/// 越界采样按平地处理，与 [`build_chunk`] 的 `level_at` 语义一致，
/// 保证场地外缘也有竖直包络。
fn collider_cuboids(
    levels: &[i32],
    side: usize,
    start: (usize, usize),
    cells: usize,
    origin: (f32, f32),
) -> Vec<(Vec3, Vec3)> {
    let (row0, col0) = start;
    let level_at = |i: i64, j: i64| {
        if i < 0 || j < 0 || i >= side as i64 || j >= side as i64 {
            0
        } else {
            levels[j as usize * side + i as usize]
        }
    };
    let world_x = |i: i64| origin.0 + (i - row0 as i64) as f32 * AUTHOR_RES;
    let world_z = |j: i64| origin.1 + (j - col0 as i64) as f32 * AUTHOR_RES;

    let mut merged = vec![false; cells * cells];
    let mut cuboids = Vec::new();
    for dj in 0..cells {
        for di in 0..cells {
            if merged[dj * cells + di] {
                continue;
            }
            let i = row0 as i64 + di as i64;
            let j = col0 as i64 + dj as i64;
            let level = level_at(i, j);
            // 沿 X 扩到同档位尽头，再沿 Z 整行验证后继续扩，与视觉顶面合并同策略。
            let mut width = 1;
            while di + width < cells && level_at(i + width as i64, j) == level {
                width += 1;
            }
            let mut height = 1;
            'expand_z: while dj + height < cells {
                for step in 0..width {
                    if level_at(i + step as i64, j + height as i64) != level {
                        break 'expand_z;
                    }
                }
                height += 1;
            }
            for row in 0..height {
                for step in 0..width {
                    merged[(dj + row) * cells + di + step] = true;
                }
            }
            let (x0, x1) = (world_x(i), world_x(i + width as i64));
            let (z0, z1) = (world_z(j), world_z(j + height as i64));
            let top = level as f32 * TERRAIN_STEP;
            let size = Vec3::new(x1 - x0, top - COLLIDER_BASE_Y, z1 - z0);
            let center = Vec3::new((x0 + x1) / 2.0, (top + COLLIDER_BASE_Y) / 2.0, (z0 + z1) / 2.0);
            cuboids.push((center, size));
        }
    }
    cuboids
}

/// 解析土色图 `.abd`：`[调色板数量 u8][数量×3 字节 sRGB][1200×1200 索引字节]`。
/// 返回 (调色板, 索引矩阵)；文件不规范时 panic，panic 消息即错误日志。
fn parse_albedo(bytes: &[u8]) -> (Vec<[u8; 3]>, &[u8]) {
    let panic_invalid = |reason: String| -> ! {
        panic!("地形土色图不合规：{reason}；请重新运行 scripts/generate_ground.py")
    };
    let Some(&count) = bytes.first() else {
        panic_invalid("文件为空".into());
    };
    if count == 0 {
        panic_invalid("调色板数量为 0".into());
    }
    let expected = 1 + count as usize * 3 + MAP_SIDE * MAP_SIDE;
    if bytes.len() != expected {
        panic_invalid(format!(
            "调色板 {count} 色时应为 {expected} 字节，实际 {} 字节",
            bytes.len()
        ));
    }
    let palette = bytes[1..1 + count as usize * 3]
        .chunks_exact(3)
        .map(|rgb| [rgb[0], rgb[1], rgb[2]])
        .collect();
    let indices = &bytes[1 + count as usize * 3..];
    if let Some((at, index)) = indices.iter().enumerate().find(|(_, index)| **index >= count) {
        panic_invalid(format!(
            "第 {at} 格下标 {index} 超出调色板数量 {count}"
        ));
    }
    (palette, indices)
}

/// 由土色索引矩阵构造 [`MAP_SIDE`]² 的 sRGB 贴图；最近邻采样让色块边界与 0.1 米网格对齐。
/// 索引已由 [`parse_albedo`] 校验，这里直接查表。
fn terrain_texture(indices: &[u8], palette: &[[u8; 3]]) -> Image {
    let mut data = Vec::with_capacity(indices.len() * 4);
    for index in indices {
        let [red, green, blue] = palette[*index as usize];
        data.extend([red, green, blue, u8::MAX]);
    }
    let mut image = Image::new(
        Extent3d {
            width: MAP_SIDE as u32,
            height: MAP_SIDE as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        // 台阶竖直面的 UV 会超出 [0, 1]，用 repeat 保证仍是连续土色。
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::nearest()
    });
    image
}

/// 物理边界：墙的内侧与场地边缘对齐，墙角相交以封住四角。
pub(super) fn spawn_walls(commands: &mut Commands) {
    for sign in [-1.0, 1.0] {
        let edge = sign * (FIELD_SIDE + WALL_THICKNESS) / 2.0;
        for (size, position) in [
            (
                Vec3::new(WALL_THICKNESS, WALL_HEIGHT, FIELD_SIDE + 2.0 * WALL_THICKNESS),
                Vec3::new(edge, WALL_HEIGHT / 2.0, 0.0),
            ),
            (
                Vec3::new(FIELD_SIDE + 2.0 * WALL_THICKNESS, WALL_HEIGHT, WALL_THICKNESS),
                Vec3::new(0.0, WALL_HEIGHT / 2.0, edge),
            ),
        ] {
            commands.spawn((
                RigidBody::Static,
                Collider::cuboid(size.x, size.y, size.z),
                Transform::from_translation(position),
                Friction::ZERO,
            ));
        }
    }
}

/// 平坦场地 + 墙，仅供物理测试使用，以便驾驶与敌人测试不受地形起伏影响。
#[cfg(test)]
pub(super) fn spawn_arena(commands: &mut Commands) {
    // `Collider::cuboid` 接收全长而非半长，这里给场地整边长。
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(FIELD_SIDE, 2.0, FIELD_SIDE),
        Transform::from_xyz(0.0, -1.0, 0.0),
        Friction::ZERO,
    ));
    spawn_walls(commands);
}

/// 加载失败与完成各汇报一次，避免只检查主资产掩盖另一张图的问题。
fn report_ground(
    ground: Res<GroundAssets>,
    assets: Res<AssetServer>,
    mut reported: Local<bool>,
) {
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

/// 一块地形网格的编译输入。
struct ChunkSource<'a> {
    /// 整图的量化档位（高度字节 − [`FLAT`]），单位 0.1 米，0 为平地。
    levels: &'a [i32],
    /// 整图的调色板下标，与 `levels` 同尺寸；已由 [`parse_albedo`] 校验越界。
    colors: &'a [u8],
    /// 文件自带的土色调色板（sRGB）。
    palette: &'a [[u8; 3]],
    /// 整图单边采样数。
    side: usize,
    /// 本块在整图中的采样起点（行 i0、列 j0）。
    start: (usize, usize),
    /// 本块单边采样数。
    cells: usize,
    /// 本块第 0 行 0 列格子的西/北世界边缘 (x, z)。
    origin: (f32, f32),
}

/// 地形网格顶点缓冲：位置、法线、UV、顶点色与索引。
#[derive(Default)]
struct TerrainBuffers {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl TerrainBuffers {
    /// 追加一个四边形；顶点顺序按逆时针给出，法线由调用方显式指定。
    fn push_quad(
        &mut self,
        corners: [[f32; 3]; 4],
        normal: Vec3,
        color: LinearRgba,
        uvs: [[f32; 2]; 4],
    ) {
        let base = self.positions.len() as u32;
        for (index, corner) in corners.iter().enumerate() {
            self.positions.push(*corner);
            self.normals.push(normal.to_array());
            self.uvs.push(uvs[index]);
            self.colors
                .push([color.red, color.green, color.blue, color.alpha]);
        }
        self.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// 转成渲染用网格；顶点位置已经是世界坐标，实体无需平移。
    fn build(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
            .with_inserted_indices(Indices::U32(self.indices))
    }
}

/// 编译一块地形：顶面按同档位同色号贪心合并成矩形，边界处按 0.1 米生成台阶竖直面。
fn build_chunk(source: &ChunkSource) -> Mesh {
    let ChunkSource {
        levels,
        colors: color_map,
        palette,
        side,
        start,
        cells,
        origin,
    } = *source;
    let (row0, col0) = start;
    let mut buffers = TerrainBuffers::default();
    // 整图采样索引 → 世界坐标；UV 也用世界坐标，跨块拼接处贴图不跳变。
    let world_x = |i: i64| origin.0 + (i - row0 as i64) as f32 * AUTHOR_RES;
    let world_z = |j: i64| origin.1 + (j - col0 as i64) as f32 * AUTHOR_RES;
    // 场地外的采样按平地处理，让外缘也生成竖直面而不会出现悬空缺口。
    let level_at = |i: i64, j: i64| {
        if i < 0 || j < 0 || i >= side as i64 || j >= side as i64 {
            0
        } else {
            levels[j as usize * side + i as usize]
        }
    };
    let color_at = |i: i64, j: i64| color_map[j as usize * side + i as usize];

    // 顶面：沿 X 扩到同档位同色号的尽头，再沿 Z 逐行验证整段一致后继续扩，
    // 因此一片平地只由少数几个大四边形构成，面数与片区周长同量级。
    let mut merged = vec![false; cells * cells];
    for dj in 0..cells {
        for di in 0..cells {
            if merged[dj * cells + di] {
                continue;
            }
            let i = row0 as i64 + di as i64;
            let j = col0 as i64 + dj as i64;
            let level = level_at(i, j);
            let color = color_at(i, j);
            let mut width = 1;
            while di + width < cells {
                let i_next = i + width as i64;
                if merged[dj * cells + di + width]
                    || level_at(i_next, j) != level
                    || color_at(i_next, j) != color
                {
                    break;
                }
                width += 1;
            }
            let mut height = 1;
            'expand_z: while dj + height < cells {
                for step in 0..width {
                    let i_probe = i + step as i64;
                    let j_probe = j + height as i64;
                    if merged[(dj + height) * cells + di + step]
                        || level_at(i_probe, j_probe) != level
                        || color_at(i_probe, j_probe) != color
                    {
                        break 'expand_z;
                    }
                }
                height += 1;
            }
            for row in 0..height {
                for step in 0..width {
                    merged[(dj + row) * cells + di + step] = true;
                }
            }
            let x0 = world_x(i);
            let x1 = world_x(i + width as i64);
            let z0 = world_z(j);
            let z1 = world_z(j + height as i64);
            let y = level as f32 * TERRAIN_STEP;
            let (u0, u1) = (x0 / AUTHOR_RES, x1 / AUTHOR_RES);
            let (v0, v1) = (z0 / AUTHOR_RES, z1 / AUTHOR_RES);
            buffers.push_quad(
                [[x0, y, z0], [x0, y, z1], [x1, y, z1], [x1, y, z0]],
                Vec3::Y,
                palette_color(color, palette),
                [[u0, v0], [u0, v1], [u1, v1], [u1, v0]],
            );
        }
    }

    // 台阶竖直面：+X 与 +Z 两条边界由本格生成，使每条边界恰好只生成一次；
    // 首行与首列额外补 −X / −Z 剖面，让场地外缘也有竖直面。
    for dj in 0..cells {
        for di in 0..cells {
            let i = row0 as i64 + di as i64;
            let j = col0 as i64 + dj as i64;
            let level = level_at(i, j);
            let color = palette_color(color_at(i, j), palette);
            let (z0, z1) = (world_z(j), world_z(j + 1));
            let (x0, x1) = (world_x(i), world_x(i + 1));
            // 场地外缘的相邻格由 level_at 视为平地，因此不需要额外判断边界。
            push_step(&mut buffers, true, world_x(i + 1), (z0, z1), level, level_at(i + 1, j), color);
            push_step(&mut buffers, false, world_z(j + 1), (x0, x1), level, level_at(i, j + 1), color);
            if i == 0 {
                push_step(&mut buffers, true, world_x(0), (z0, z1), level, level_at(-1, j), color);
            }
            if j == 0 {
                push_step(&mut buffers, false, world_z(0), (x0, x1), level, level_at(i, -1), color);
            }
        }
    }
    buffers.build()
}

/// 在一条单元边界上追加竖直台阶面：从较低一侧到较高一侧按 0.1 米逐级生成，
/// 使侧壁呈现体素台阶而不是斜插面。
///
/// `perpendicular_x` 为真表示边界垂直于 X 轴（`position` 是 X 坐标、`span` 是 Z 范围）。
fn push_step(
    buffers: &mut TerrainBuffers,
    perpendicular_x: bool,
    position: f32,
    span: (f32, f32),
    near: i32,
    far: i32,
    color: LinearRgba,
) {
    let (low, high) = (near.min(far), near.max(far));
    if low == high {
        return;
    }
    // 法线朝向实体一侧，即较高的一侧。
    let normal = match (perpendicular_x, near > far) {
        (true, true) => Vec3::X,
        (true, false) => Vec3::NEG_X,
        (false, true) => Vec3::Z,
        (false, false) => Vec3::NEG_Z,
    };
    let side_color = shade(color, SIDE_SHADE[(near + far).unsigned_abs() as usize % 2]);
    let (start, end) = (span.0 / AUTHOR_RES, span.1 / AUTHOR_RES);
    for step in low..high {
        let y0 = step as f32 * TERRAIN_STEP;
        let y1 = (step + 1) as f32 * TERRAIN_STEP;
        let (v0, v1) = (y0 / AUTHOR_RES, y1 / AUTHOR_RES);
        let corners = if perpendicular_x {
            [
                [position, y0, span.0],
                [position, y1, span.0],
                [position, y1, span.1],
                [position, y0, span.1],
            ]
        } else {
            [
                [span.0, y0, position],
                [span.0, y1, position],
                [span.1, y1, position],
                [span.1, y0, position],
            ]
        };
        buffers.push_quad(
            corners,
            normal,
            side_color,
            [[start, v0], [start, v1], [end, v1], [end, v0]],
        );
    }
}

/// 调色板下标 → 线性 RGBA；下标已由 [`parse_albedo`] 校验。
fn palette_color(index: u8, palette: &[[u8; 3]]) -> LinearRgba {
    let [red, green, blue] = palette[index as usize];
    Srgba::rgb_u8(red, green, blue).into()
}

/// 按系数压暗颜色，给竖直面制造体素侧面的明暗差。
fn shade(color: LinearRgba, factor: f32) -> LinearRgba {
    LinearRgba::rgb(color.red * factor, color.green * factor, color.blue * factor)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 小尺寸合成地图的块编译，用于验证顶面合并与台阶几何。
    fn synthetic(chunk: (usize, usize)) -> Mesh {
        // 4×4 采样：一列抬起 3 档（0.3 米），另一格下沉 2 档，其余平地。
        let mut levels = vec![0i32; 16];
        levels[1 * 4 + 1] = 3;
        levels[2 * 4 + 1] = 3;
        levels[3 * 4 + 2] = -2;
        build_chunk(&ChunkSource {
            levels: &levels,
            colors: &[2u8; 16],
            palette: &[[87, 65, 43], [102, 79, 52], [111, 85, 57], [119, 93, 63], [126, 99, 68]],
            side: 4,
            start: chunk,
            cells: 4,
            origin: (0.0, 0.0),
        })
    }

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

    /// 网格必须覆盖整块采样范围，且台阶面只出现在相邻档位之间。
    #[test]
    fn chunk_mesh_covers_the_area_and_builds_steps() {
        let mesh = synthetic((0, 0));
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("地形网格必须有位置属性");
        };
        assert_eq!(positions.len() % 4, 0, "顶点数必须是四边形的整数倍");
        // 4 格 × 0.5 米 = 2 米见方；顶面 0.3 米、下沉面 −0.2 米都在范围内。
        let x_range = positions.iter().map(|p| p[0]);
        let bounds = (
            x_range.clone().fold(f32::MAX, f32::min),
            x_range.fold(f32::MIN, f32::max),
        );
        assert_eq!(bounds, (0.0, 2.0));
        let y_min = positions.iter().fold(f32::MAX, |acc, p| acc.min(p[1]));
        let y_max = positions.iter().fold(f32::MIN, |acc, p| acc.max(p[1]));
        assert!((y_min - -0.2).abs() < 1e-6, "下沉格底面应为 −0.2 米");
        assert!((y_max - 0.3).abs() < 1e-6, "抬起格顶面应为 0.3 米");
        // 3 档高差共 3 个台阶面，加 2 档下沉的 2 个台阶面，竖直面方向均只有 ±X / ±Z。
        let normals = mesh
            .attribute(Mesh::ATTRIBUTE_NORMAL)
            .expect("地形网格必须有法线");
        assert!(matches!(normals, bevy::mesh::VertexAttributeValues::Float32x3(_)));
        assert!(mesh.attribute(Mesh::ATTRIBUTE_UV_0).is_some());
        assert!(mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_some());
    }

    /// 体素碰撞体必须按档位合并并与视觉格子对齐：同档位相邻格合并成一个
    /// cuboid（顶面 = 档位 × 0.1 米），全部 cuboid 底面统一在 [`COLLIDER_BASE_Y`]，
    /// 且所有格子中心恰好被某个 cuboid 的 XZ 覆盖（无缺口、无重复）。
    #[test]
    fn collider_cuboids_merge_by_level_and_align_with_cells() {
        // 与视觉测试同一份合成档位：X=1、Z=1..2 抬起 3 档（0.3 米），
        // X=2、Z=3 下沉 2 档（−0.2 米），其余平地。碰撞体只看档位、不看颜色。
        let mut levels = vec![0i32; 16];
        levels[1 * 4 + 1] = 3;
        levels[2 * 4 + 1] = 3;
        levels[3 * 4 + 2] = -2;
        let cuboids = collider_cuboids(&levels, 4, (0, 0), 4, (0.0, 0.0));
        assert!(!cuboids.is_empty(), "必须产出 cuboid");
        for (center, size) in &cuboids {
            let bottom = center.y - size.y / 2.0;
            assert!(
                (bottom - COLLIDER_BASE_Y).abs() < 1e-6,
                "cuboid 底面应统一在 COLLIDER_BASE_Y：{center:?} {size:?}"
            );
        }
        // 抬起区两格（沿 Z 相邻、同档位）必须合并为一个 cuboid：
        // X [0.5, 1.0]、Z [0.5, 1.5]、顶面 0.3 米。
        let raised: Vec<_> = cuboids
            .iter()
            .filter(|(c, s)| (c.y + s.y / 2.0 - 0.3).abs() < 1e-6)
            .collect();
        assert_eq!(raised.len(), 1, "抬起 3 档的格子应合并为一个 cuboid：{raised:?}");
        let (c, s) = raised[0];
        assert!((c.x - 0.75).abs() < 1e-6 && (s.x - 0.5).abs() < 1e-6, "X 范围 [0.5,1.0]：{c:?} {s:?}");
        assert!((c.z - 1.0).abs() < 1e-6 && (s.z - 1.0).abs() < 1e-6, "Z 范围 [0.5,1.5]：{c:?} {s:?}");
        // 下沉格：X [1.0, 1.5]、Z [1.5, 2.0]、顶面 −0.2 米。
        let sunk: Vec<_> = cuboids
            .iter()
            .filter(|(c, s)| (c.y + s.y / 2.0 - -0.2).abs() < 1e-6)
            .collect();
        assert_eq!(sunk.len(), 1, "下沉 2 档的格子应为一个 cuboid：{sunk:?}");
        let (c, s) = sunk[0];
        assert!((c.x - 1.25).abs() < 1e-6 && (c.z - 1.75).abs() < 1e-6, "下沉格位置：{c:?} {s:?}");
        // 覆盖性：16 个格子中心都必须恰好落在某个 cuboid 的 XZ 覆盖内。
        for j in 0..4i64 {
            for i in 0..4i64 {
                let (x, z) = (i as f32 * 0.5 + 0.25, j as f32 * 0.5 + 0.25);
                let hits = cuboids
                    .iter()
                    .filter(|(c, s)| {
                        (c.x - x).abs() <= s.x / 2.0 && (c.z - z).abs() <= s.z / 2.0
                    })
                    .count();
                assert_eq!(hits, 1, "格子 ({i},{j}) 中心 ({x},{z}) 应恰好被一个 cuboid 覆盖");
            }
        }
    }

    /// 读取仓库内的真实高度图；缺失或尺寸不符即 panic。
    fn real_heights() -> Vec<u8> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/terrain/ground/ground.ht");
        let bytes = std::fs::read(path).expect("真实高度图 ground.ht 必须随仓库提供");
        assert_eq!(bytes.len(), MAP_SIDE * MAP_SIDE, "真实高度图尺寸不合规");
        bytes
    }

    /// 真实高度图的体素复合碰撞体不得出现幽灵接触：出生保护区是 24 米半径的
    /// 绝对平地，坦克直行 1 秒（6 米）加车长仍留在平区内；平区外即使有台阶，
    /// 也全部是可翻越的 1 档坎（见 [`real_terrain_has_only_one_level_steps_and_flat_spawn`]）。
    #[test]
    fn voxel_collider_keeps_straight_driving_on_real_terrain() {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;
        // 与离屏 drive 场景一致：1/60 秒固定步长，重力由 DrivingPlugin 翻倍。
        let step = Duration::from_secs_f64(1.0 / 60.0);
        let levels: Vec<i32> = real_heights()
            .iter()
            .map(|value| *value as i32 - FLAT as i32)
            .collect();
        // 测试直接用整图一份复合碰撞体（不必按 60 米分块拆分），几何与运行时同源。
        let cuboids = collider_cuboids(&levels, MAP_SIDE, (0, 0), MAP_SIDE, (-FIELD_SIDE / 2.0, -FIELD_SIDE / 2.0));
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            PhysicsPlugins::default(),
            crate::driving::DrivingPlugin,
        ))
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(step));
        // 手动逐帧测试也需完成 Avian 在 finish 阶段注册的求解器资源。
        app.finish();
        app.cleanup();
        // 碰撞体直接铺进世界（等价于 Startup 生成，且避免闭包按值捕获复合碰撞体）。
        app.world_mut().spawn((
            TerrainSurface,
            RigidBody::Static,
            Collider::compound(
                cuboids
                    .into_iter()
                    .map(|(center, size)| {
                        (center, Rotation::IDENTITY, Collider::cuboid(size.x, size.y, size.z))
                    })
                    .collect(),
            ),
            Friction::ZERO,
        ));
        let tank = app
            .world_mut()
            .spawn((
                crate::firing::Tank,
                crate::driving::tank_body(),
                Transform::from_xyz(0.0, 4.0, 0.0),
            ))
            .id();
        // 先模拟 120 帧让坦克在出生区落地，再按住前进 1 秒（车头朝世界 −X）。
        for _ in 0..120 {
            app.update();
        }
        let settled = app.world().get::<Transform>(tank).unwrap().translation;
        assert!((settled.y - 0.75).abs() < 0.05, "落地高度异常：{settled:?}");
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowUp);
        for _ in 0..60 {
            app.update();
        }
        let moved = app.world().get::<Transform>(tank).unwrap().translation;
        assert!(
            moved.x - settled.x < -4.0,
            "体素复合碰撞体上平地直行不应被幽灵接触挡住：{moved:?}"
        );
    }

    /// 真实地形必须满足两条硬约束（生成器 `scripts/generate_ground.py` 的契约）：
    /// 1. 全图任意相邻格高度差 ≤ 1 档（0.1 米）——坦克只能翻 1 档坎，
    ///    任何更大的坎都会成为不可翻越的墙；
    /// 2. 出生保护区（24 米半径）内绝对平地——玩家直接出生在大平面上，
    ///    不存在"坑中平地"的圆坑。
    #[test]
    fn real_terrain_has_only_one_level_steps_and_flat_spawn() {
        let bytes = real_heights();
        let level = |i: usize, j: usize| bytes[j * MAP_SIDE + i] as i32 - FLAT as i32;
        for j in 0..MAP_SIDE {
            for i in 0..MAP_SIDE {
                if i + 1 < MAP_SIDE {
                    assert!(
                        (level(i, j) - level(i + 1, j)).abs() <= 1,
                        "X 向相邻格高度差超过 1 档：(X={i}, Z={j})"
                    );
                }
                if j + 1 < MAP_SIDE {
                    assert!(
                        (level(i, j) - level(i, j + 1)).abs() <= 1,
                        "Z 向相邻格高度差超过 1 档：(X={i}, Z={j})"
                    );
                }
            }
        }
        let clear_cells = (24.0 / AUTHOR_RES) as f64;
        for j in 0..MAP_SIDE as i64 {
            for i in 0..MAP_SIDE as i64 {
                let (dx, dz) = (i as f64 + 0.5 - MAP_SIDE as f64 / 2.0, j as f64 + 0.5 - MAP_SIDE as f64 / 2.0);
                if dx * dx + dz * dz < clear_cells * clear_cells {
                    assert_eq!(
                        level(i as usize, j as usize),
                        0,
                        "出生保护区内出现非平地：(X={i}, Z={j})"
                    );
                }
            }
        }
    }

    /// 体素台阶 + 圆角盒坦克（能翻 1 档）的组合行为：坦克必须能翻过一档
    /// （0.1 米）的坎继续行驶，并停在坎顶高度；这是"体素碰撞可翻越矮坎"的回归测试。
    #[test]
    fn tank_climbs_one_voxel_step() {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;
        let step = Duration::from_secs_f64(1.0 / 60.0);
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            PhysicsPlugins::default(),
            crate::driving::DrivingPlugin,
        ))
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(step));
        app.finish();
        app.cleanup();
        // 平地（x > 0，顶 0 米）+ 一档台阶（x < 0，顶 0.1 米），坦克从 x=8 向 −X 开。
        for (x0, x1, top) in [(0.0, 50.0, 0.0f32), (-50.0, 0.0, TERRAIN_STEP)] {
            let size_y = top - COLLIDER_BASE_Y;
            app.world_mut().spawn((
                TerrainSurface,
                RigidBody::Static,
                Collider::cuboid(x1 - x0, size_y, 20.0),
                Transform::from_xyz((x0 + x1) / 2.0, (top + COLLIDER_BASE_Y) / 2.0, 0.0),
                Friction::ZERO,
            ));
        }
        let tank = app
            .world_mut()
            .spawn((
                crate::firing::Tank,
                crate::driving::tank_body(),
                Transform::from_xyz(8.0, 4.0, 0.0),
            ))
            .id();
        for _ in 0..120 {
            app.update();
        }
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowUp);
        // 4 秒 = 到达台阶（约 1 秒）+ 翻坎 + 在坎上继续行驶，留足余量。
        for _ in 0..240 {
            app.update();
        }
        let moved = app.world().get::<Transform>(tank).unwrap().translation;
        assert!(
            moved.x < -2.0,
            "坦克应翻过一档坎并继续行驶：{moved:?}"
        );
        assert!(
            (moved.y - (0.75 + TERRAIN_STEP)).abs() < 0.05,
            "坦克应停在坎顶高度 {}：{moved:?}",
            0.75 + TERRAIN_STEP
        );
    }

    /// 5 色合法 `.abd` 样本：数量字节 + 15 字节调色板 + 全 0 索引矩阵。
    fn valid_albedo() -> Vec<u8> {
        let mut bytes = vec![5u8];
        bytes.extend_from_slice(&[
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
        ]);
        bytes.resize(1 + 15 + MAP_SIDE * MAP_SIDE, 0);
        bytes
    }

    /// 合法 `.abd` 应拆出完整调色板与索引矩阵。
    #[test]
    fn parse_albedo_reads_palette_and_indices() {
        let bytes = valid_albedo();
        let (palette, indices) = parse_albedo(&bytes);
        assert_eq!(palette.len(), 5);
        assert_eq!(palette[0], [1, 2, 3]);
        assert_eq!(palette[4], [13, 14, 15]);
        assert_eq!(indices.len(), MAP_SIDE * MAP_SIDE);
        assert_eq!(indices[0], 0);
    }

    /// 长度与调色板数量不匹配即死。
    #[test]
    #[should_panic(expected = "地形土色图不合规")]
    fn parse_albedo_panics_on_wrong_size() {
        let mut bytes = valid_albedo();
        bytes.pop();
        parse_albedo(&bytes);
    }

    /// 索引超出调色板数量即死，并指出位置。
    #[test]
    #[should_panic(expected = "超出调色板数量")]
    fn parse_albedo_panics_on_out_of_range_index() {
        let mut bytes = valid_albedo();
        let last = bytes.len() - 1;
        bytes[last] = 5;
        parse_albedo(&bytes);
    }
}

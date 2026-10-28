//! 工单 01 小样：证明 bevy_vox_scene 0.23 的原生 `.vox.meta` 每文件单位配置
//! 能作用于网格、`#命名节点` 子资产与场景平移。
//!
//! 使用锁定版本的真实 AssetServer + VoxScenePlugin（`global_settings=None`，
//! 否则插件全局配置会覆盖 meta），资产放在内存 [`Dir`] 的自定义协议源
//! `voxfix://` 下加载，不触碰 `assets/`、不需要 GPU。
//!
//! 小样设计：`ten.vox`（1×1×10 格，meta voxel_size=0.1）与
//! `fifty.vox`（1×1×50 格，meta voxel_size=0.02）同场均应为 1 米高；
//! 命名节点 `block` 带非零平移帧 `_t="1 2 3"`，按烘焙约定映射为
//! `Vec3::new(-x, z, y) * voxel_size`（米）。

use std::path::Path;
use std::time::{Duration, Instant};

use bevy::{
    asset::{
        AssetApp, AssetLoader, AssetServer, LoadContext, LoadState,
        io::{AssetSourceBuilder, memory::Dir},
        meta::{AssetAction, AssetMeta, AssetMetaDyn},
    },
    camera::{
        primitives::{Aabb, MeshAabb},
        visibility::{VisibilityClass, VisibilityPlugin},
    },
    light::FogVolume,
    mesh::skinning::SkinnedMeshInverseBindposes,
    prelude::*,
    reflect::TypePath,
    time::TimeUpdateStrategy,
    world_serialization::{WorldAsset, WorldAssetRoot, WorldInstance, WorldInstanceSpawner},
};
use bevy_vox_scene::{VoxLoaderSettings, VoxScenePlugin};

/// 真实加载器类型名（meta 的 `loader` 字段），由 TypePath 冻结于插件源码。
const VOX_LOADER_TYPE_PATH: &str = "bevy_vox_scene::load::VoxSceneLoader";

/// 仅用于按 serde 结构生成 meta RON 的探针加载器：Settings 与真实
/// `VoxSceneLoader` 完全一致（`VoxLoaderSettings`），序列化格式即等价；
/// 从不被调用，也不注册进 AssetServer。
#[derive(TypePath)]
struct MetaProbe;

impl AssetLoader for MetaProbe {
    type Asset = ();
    type Settings = VoxLoaderSettings;
    type Error = std::io::Error;

    async fn load(
        &self,
        _reader: &mut dyn bevy::asset::io::Reader,
        _settings: &Self::Settings,
        _context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        unreachable!("MetaProbe 只用于序列化 meta，永不加载资产")
    }
}

/// 生成一份完整生效的 meta 字节：除 `voxel_size` 外全部取默认值。
fn meta_bytes(voxel_size: f32) -> Vec<u8> {
    let settings = VoxLoaderSettings {
        voxel_size,
        ..Default::default()
    };
    let meta = AssetMeta::<MetaProbe, ()>::new(AssetAction::Load {
        loader: VOX_LOADER_TYPE_PATH.to_string(),
        settings,
    });
    AssetMetaDyn::serialize(&meta)
}

// ---- .vox 字节生成（布局照抄 scripts/generate_concrete.py，dot_vox 可解析） ----

/// VOX 分块：4 字节标记 + 内容长度 u32 + 子块长度 u32 + 内容 + 子块。
fn chunk(id: &[u8; 4], content: &[u8], children: &[u8]) -> Vec<u8> {
    let mut out = id.to_vec();
    out.extend_from_slice(&(content.len() as u32).to_le_bytes());
    out.extend_from_slice(&(children.len() as u32).to_le_bytes());
    out.extend_from_slice(content);
    out.extend_from_slice(children);
    out
}

/// 节点字典：数量 u32 + 每项（键长 u32 + 键 + 值长 u32 + 值）。
fn dict(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut out = (entries.len() as u32).to_le_bytes().to_vec();
    for (key, value) in entries {
        out.extend_from_slice(&(key.len() as u32).to_le_bytes());
        out.extend_from_slice(key.as_bytes());
        out.extend_from_slice(&(value.len() as u32).to_le_bytes());
        out.extend_from_slice(value.as_bytes());
    }
    out
}

fn i32b(value: i32) -> [u8; 4] {
    value.to_le_bytes()
}

/// nTRN：节点 id + 内层字典（可含 `_name`）+ 子节点 + reserved(-1)
/// + layer(-1) + 帧数(1) + 帧字典（可含 `_t` 平移，单位：体素格）。
fn ntrn(node: i32, name: Option<&str>, translation: Option<&str>, child: i32) -> Vec<u8> {
    let inner: Vec<(&str, &str)> = name.map(|n| vec![("_name", n)]).unwrap_or_default();
    let frame: Vec<(&str, &str)> = translation.map(|t| vec![("_t", t)]).unwrap_or_default();
    let mut content = i32b(node).to_vec();
    content.extend_from_slice(&dict(&inner));
    content.extend_from_slice(&i32b(child));
    content.extend_from_slice(&i32b(-1));
    content.extend_from_slice(&i32b(-1));
    content.extend_from_slice(&i32b(1));
    content.extend_from_slice(&dict(&frame));
    chunk(b"nTRN", &content, &[])
}

/// nSHP：节点 id + 字典 + 模型数(1) + 模型 id(0) + 字典。
fn nshp(node: i32) -> Vec<u8> {
    let mut content = i32b(node).to_vec();
    content.extend_from_slice(&dict(&[]));
    content.extend_from_slice(&i32b(1));
    content.extend_from_slice(&i32b(0));
    content.extend_from_slice(&dict(&[]));
    chunk(b"nSHP", &content, &[])
}

/// 生成 1×1×`count` 竖直柱体：根 nTRN → nTRN(`_name="block"`, `_t="1 2 3"`) → nSHP。
/// 节点 id 按**节点块**出现顺序编号（SIZE/XYZI/RGBA/MATL 不占号）：
/// nTRN=0、nTRN=1、nSHP=2，与 scripts/generate_concrete.py 的编号规则一致。
fn build_vox(count: u32) -> Vec<u8> {
    let mut cells = Vec::new();
    for z in 0..count {
        cells.extend_from_slice(&[0u8, 0, z as u8, 1]);
    }
    let mut children = Vec::new();
    let mut size = Vec::new();
    size.extend_from_slice(&1u32.to_le_bytes());
    size.extend_from_slice(&1u32.to_le_bytes());
    size.extend_from_slice(&count.to_le_bytes());
    children.extend_from_slice(&chunk(b"SIZE", &size, &[]));
    let mut xyzi = count.to_le_bytes().to_vec();
    xyzi.extend_from_slice(&cells);
    children.extend_from_slice(&chunk(b"XYZI", &xyzi, &[]));
    children.extend_from_slice(&ntrn(0, None, None, 1));
    children.extend_from_slice(&ntrn(1, Some("block"), Some("1 2 3"), 2));
    children.extend_from_slice(&nshp(2));
    let mut rgba = vec![0u8; 256 * 4];
    rgba[..4].copy_from_slice(&[255, 0, 0, 255]);
    for pixel in rgba.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
    children.extend_from_slice(&chunk(b"RGBA", &rgba, &[]));
    let mut matl = i32b(1).to_vec();
    matl.extend_from_slice(&dict(&[("_type", "_diffuse"), ("_rough", "1")]));
    children.extend_from_slice(&chunk(b"MATL", &matl, &[]));
    let mut out = b"VOX ".to_vec();
    out.extend_from_slice(&150u32.to_le_bytes());
    out.extend_from_slice(&chunk(b"MAIN", &[], &children));
    out
}

// ---- 测试装配（复用 src/tank/track.rs 的最小化真实加载组装） ----

/// 组装带 `voxfix://` 内存源的应用；`VoxScenePlugin::default()` 保证走 meta。
fn app_with(dir: Dir) -> App {
    let mut app = App::new();
    app.register_asset_source(
        "voxfix",
        AssetSourceBuilder::new(move || {
            Box::new(bevy::asset::io::memory::MemoryAssetReader { root: dir.clone() })
        }),
    );
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ImagePlugin::default(),
        bevy::world_serialization::WorldSerializationPlugin,
        TransformPlugin,
        VisibilityPlugin,
        VoxScenePlugin::default(),
    ))
    .init_asset::<Mesh>()
    .init_asset::<StandardMaterial>()
    .init_asset::<SkinnedMeshInverseBindposes>()
    .register_type::<Visibility>()
    .register_type::<ViewVisibility>()
    .register_type::<InheritedVisibility>()
    .register_type::<VisibilityClass>()
    .register_type::<Transform>()
    .register_type::<GlobalTransform>()
    .register_type::<TransformTreeChanged>()
    .register_type::<Mesh3d>()
    .register_type::<MeshMaterial3d<StandardMaterial>>()
    .register_type::<FogVolume>();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        50,
    )));
    app
}

/// 生成并挂载 `path` 的 WorldAssetRoot，等待实例就绪；加载失败返回错误文本。
fn spawn_and_wait(app: &mut App, path: &str) -> Result<Entity, String> {
    let handle: Handle<WorldAsset> = app
        .world()
        .resource::<AssetServer>()
        .load(path.to_owned());
    let root = app.world_mut().spawn(WorldAssetRoot(handle.clone())).id();
    // 异步加载用墙钟限时；游戏时间固定步进，不依赖机器帧率。
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        if let Some(LoadState::Failed(error)) = app
            .world()
            .resource::<AssetServer>()
            .get_load_state(handle.id())
        {
            return Err(error.to_string());
        }
        let loaded = app
            .world()
            .resource::<AssetServer>()
            .is_loaded_with_dependencies(&handle);
        let ready = app.world().get::<WorldInstance>(root).is_some_and(|instance| {
            app.world()
                .resource::<WorldInstanceSpawner>()
                .instance_is_ready(**instance)
        });
        if loaded && ready {
            return Ok(root);
        }
        assert!(Instant::now() < deadline, "加载超时：{path}");
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// 收集子树全部网格实体及其模型空间 AABB（米制，CPU 侧可读）。
fn subtree_meshes(app: &App, root: Entity) -> Vec<(Entity, Aabb)> {
    let mut pending = vec![root];
    let mut out = Vec::new();
    let world = app.world();
    let assets = world.resource::<Assets<Mesh>>();
    while let Some(entity) = pending.pop() {
        if let Some(kids) = world.get::<Children>(entity) {
            pending.extend(kids.iter());
        }
        if let Some(mesh) = world.get::<Mesh3d>(entity)
            && let Some(aabb) = assets.get(&mesh.0).and_then(|m| m.get_aabb())
        {
            out.push((entity, aabb));
        }
    }
    out
}

/// 断言子树内没有任何缩放（禁止用根缩放作弊，也证明加载器不走缩放路径）。
fn assert_no_scaling(app: &App, root: Entity) {
    let mut pending = vec![root];
    let world = app.world();
    while let Some(entity) = pending.pop() {
        if let Some(kids) = world.get::<Children>(entity) {
            pending.extend(kids.iter());
        }
        if let Some(transform) = world.get::<Transform>(entity) {
            assert_eq!(
                transform.scale,
                Vec3::ONE,
                "{entity:?} 出现缩放，测试或加载器使用了缩放作弊路径"
            );
        }
    }
}

/// 子树中是否存在名为 `name` 的节点。
fn has_named(app: &App, root: Entity, name: &str) -> bool {
    let mut pending = vec![root];
    let world = app.world();
    while let Some(entity) = pending.pop() {
        if let Some(kids) = world.get::<Children>(entity) {
            pending.extend(kids.iter());
        }
        if let Some(node) = world.get::<Name>(entity)
            && node.as_str() == name
        {
            return true;
        }
    }
    false
}

/// 核心证明：0.1 米的 10 格与 0.02 米的 50 格同场均约 1 米高，
/// 网格横截面按各自 meta，`block` 命名节点存在，非零平移按各自 meta 烘焙。
#[test]
fn per_file_meta_sizes_both_fixtures_to_one_metre() {
    let dir = Dir::default();
    dir.insert_asset(Path::new("ten.vox"), build_vox(10));
    dir.insert_meta(Path::new("ten.vox"), meta_bytes(0.1));
    dir.insert_asset(Path::new("fifty.vox"), build_vox(50));
    dir.insert_meta(Path::new("fifty.vox"), meta_bytes(0.02));
    let mut app = app_with(dir);
    let ten = spawn_and_wait(&mut app, "voxfix://ten.vox").unwrap();
    let fifty = spawn_and_wait(&mut app, "voxfix://fifty.vox").unwrap();
    for (root, voxel_size) in [(ten, 0.1f32), (fifty, 0.02)] {
        let meshes = subtree_meshes(&app, root);
        assert_eq!(meshes.len(), 1, "小样应恰好生成一个网格");
        let (entity, aabb) = meshes[0];
        let size = Vec3::from(aabb.half_extents) * 2.0;
        assert!(
            (size.y - 1.0).abs() < 1e-4,
            "voxel_size={voxel_size} 的柱体高度应恒为 1 米，实际 {size:?}"
        );
        assert!(
            (size.x - voxel_size).abs() < 1e-4 && (size.z - voxel_size).abs() < 1e-4,
            "横截面应等于各自 voxel_size，实际 {size:?}"
        );
        assert!(has_named(&app, root, "block"), "命名节点 block 应存在");
        // _t="1 2 3" 按烘焙约定映射为 (-x, z, y) * voxel_size，单位：米。
        let expected = Vec3::new(-voxel_size, 3.0 * voxel_size, 2.0 * voxel_size);
        let got = app.world().get::<GlobalTransform>(entity).unwrap().translation();
        assert!(
            got.abs_diff_eq(expected, 1e-4),
            "voxel_size={voxel_size} 的平移应烘焙为 {expected:?}，实际 {got:?}"
        );
        assert_no_scaling(&app, root);
    }
}

/// `#block` 命名子资产独立加载：几何按该文件的 meta 烘焙。
///
/// 技术结论（bevy_vox_scene 0.23 原生行为，parse_scene.rs L75）：命名节点的
/// 子资产 scope 根实体固定为 `Transform::IDENTITY`，节点**自身**的 `_t` 平移
/// 不会烘焙进子资产根，须由调用方外部放置（本工程 `wheel_transform` 即此用法）；
/// scope **内部**其余节点的平移仍按 meta 烘焙（由整场景测试覆盖）。
#[test]
fn named_subasset_loads_with_own_meta() {
    let dir = Dir::default();
    dir.insert_asset(Path::new("ten.vox"), build_vox(10));
    dir.insert_meta(Path::new("ten.vox"), meta_bytes(0.1));
    let mut app = app_with(dir);
    let root = spawn_and_wait(&mut app, "voxfix://ten.vox#block").unwrap();
    let meshes = subtree_meshes(&app, root);
    assert_eq!(meshes.len(), 1, "#block 子资产应恰好生成一个网格");
    let (entity, aabb) = meshes[0];
    let size = Vec3::from(aabb.half_extents) * 2.0;
    assert!((size.y - 1.0).abs() < 1e-4, "#block 高度应为 1 米，实际 {size:?}");
    let got = app.world().get::<GlobalTransform>(entity).unwrap().translation();
    assert!(
        got.abs_diff_eq(Vec3::ZERO, 1e-6),
        "scope 根应为 identity（节点自身平移由调用方外部放置），实际 {got:?}"
    );
    assert_no_scaling(&app, root);
}

/// 缺失 meta 的行为记录：加载器静默回退 `voxel_size=1.0`（无报错）。
#[test]
fn missing_meta_falls_back_to_unit_voxel_size() {
    let dir = Dir::default();
    dir.insert_asset(Path::new("ten.vox"), build_vox(10));
    let mut app = app_with(dir);
    let root = spawn_and_wait(&mut app, "voxfix://ten.vox").unwrap();
    let meshes = subtree_meshes(&app, root);
    let (entity, aabb) = meshes[0];
    let size = Vec3::from(aabb.half_extents) * 2.0;
    assert!(
        (size.y - 10.0).abs() < 1e-3,
        "缺 meta 应静默按 1.0 米/格烘焙为 10 米，实际 {size:?}"
    );
    let got = app.world().get::<GlobalTransform>(entity).unwrap().translation();
    assert!(
        got.abs_diff_eq(Vec3::new(-1.0, 3.0, 2.0), 1e-4),
        "缺 meta 时平移也应回退 1.0 米/格，实际 {got:?}"
    );
}

/// 损坏 meta 的行为记录：明确失败且错误可定位到路径。
#[test]
fn corrupt_meta_fails_diagnosably() {
    let dir = Dir::default();
    dir.insert_asset(Path::new("ten.vox"), build_vox(10));
    dir.insert_meta(Path::new("ten.vox"), b"this is not ron");
    let mut app = app_with(dir);
    let error = spawn_and_wait(&mut app, "voxfix://ten.vox")
        .expect_err("损坏 meta 必须加载失败，不能静默回退");
    assert!(
        error.contains("ten.vox") || error.to_lowercase().contains("meta"),
        "错误应可定位到资产路径或 meta 反序列化，实际：{error}"
    );
}

/// 非正/非有限 voxel_size 的行为记录：0.0 与 -1.0 是否被校验、NaN 是否扩散。
#[test]
fn non_positive_voxel_size_behaviour_is_recorded() {
    let dir = Dir::default();
    for (name, size) in [("zero.vox", 0.0f32), ("neg.vox", -1.0), ("nan.vox", f32::NAN)] {
        dir.insert_asset(Path::new(name), build_vox(10));
        dir.insert_meta(Path::new(name), meta_bytes(size));
    }
    let mut app = app_with(dir);
    let zero = spawn_and_wait(&mut app, "voxfix://zero.vox").unwrap();
    let size = Vec3::from(subtree_meshes(&app, zero)[0].1.half_extents) * 2.0;
    assert!(
        size.y.abs() < 1e-6,
        "voxel_size=0 应产生零尺寸网格（当前加载器无校验），实际 {size:?}"
    );
    let neg = spawn_and_wait(&mut app, "voxfix://neg.vox").unwrap();
    let size = Vec3::from(subtree_meshes(&app, neg)[0].1.half_extents) * 2.0;
    assert!(
        (size.y - 10.0).abs() < 1e-3,
        "voxel_size=-1 未报错、按幅值 10 米烘焙（记录：无正值校验），实际 {size:?}"
    );
    let nan = spawn_and_wait(&mut app, "voxfix://nan.vox").unwrap();
    let size = Vec3::from(subtree_meshes(&app, nan)[0].1.half_extents) * 2.0;
    assert!(
        !size.y.is_finite(),
        "voxel_size=NaN 应扩散为非法几何（记录：无有限性校验），实际 {size:?}"
    );
}

//! 工单 03 集成测试：用锁定版本的真实 AssetServer + VoxScenePlugin 验证
//! `scripts/subdivide_vox.py` 产出的分块 VOX 小样。
//!
//! 流程：Rust 侧生成粗样（0.1 米/格）→ 调用真实 Python 转换器细分（0.02 米/格）
//! → 把粗样与细分产物放入内存资产源加载 → 断言：
//! 1. 超过 256 格的轴被分成计划中的块（网格数 = 块数，块尺寸精确）；
//! 2. `#节点` 子资产返回完整分块子树，世界 AABB 与粗样逐轴一致（1e-4）；
//! 3. 整文件作用域下命名节点帧平移（含负值）经细分后同样保形；
//! 4. 子树内无任何缩放（证明不走缩放作弊路径）。
//!
//! 前置：PATH 中需有 python（运行仓库内转换器脚本）。

use std::{
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

use bevy::{
    asset::{
        AssetApp, AssetServer, LoadState,
        io::{AssetSourceBuilder, memory::Dir},
    },
    camera::primitives::{Aabb, MeshAabb},
    mesh::skinning::SkinnedMeshInverseBindposes,
    prelude::*,
    time::TimeUpdateStrategy,
    world_serialization::{WorldAsset, WorldAssetRoot, WorldInstance, WorldInstanceSpawner},
};
use bevy_vox_scene::VoxScenePlugin;

// ---- .vox 字节生成（与 tests/vox_meta_settings.rs 同布局，dot_vox 可解析） ----

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

/// nTRN：节点 id + 字典 + 子节点 + reserved(-1) + layer + 帧数 + 各帧字典。
/// `translation` 为帧 `_t`（体素格）；`rotation` 为帧 `_r`（可选）。
fn ntrn(
    node: i32,
    name: Option<&str>,
    translation: Option<&str>,
    rotation: Option<&str>,
    child: i32,
) -> Vec<u8> {
    let mut inner: Vec<(&str, &str)> = Vec::new();
    if let Some(name) = name {
        inner.push(("_name", name));
    }
    let mut frame: Vec<(&str, &str)> = Vec::new();
    if let Some(translation) = translation {
        frame.push(("_t", translation));
    }
    if let Some(rotation) = rotation {
        frame.push(("_r", rotation));
    }
    let mut content = i32b(node).to_vec();
    content.extend_from_slice(&dict(&inner));
    content.extend_from_slice(&i32b(child));
    content.extend_from_slice(&i32b(-1));
    content.extend_from_slice(&i32b(-1));
    content.extend_from_slice(&i32b(1));
    content.extend_from_slice(&dict(&frame));
    chunk(b"nTRN", &content, &[])
}

/// nGRP：节点 id + 字典 + 子数量 + 子节点列表。
fn ngrp(node: i32, children: &[i32]) -> Vec<u8> {
    let mut content = i32b(node).to_vec();
    content.extend_from_slice(&dict(&[]));
    content.extend_from_slice(&i32b(children.len() as i32));
    for child in children {
        content.extend_from_slice(&i32b(*child));
    }
    chunk(b"nGRP", &content, &[])
}

/// nSHP：节点 id + 字典 + 模型数(1) + 模型 id + 字典。
fn nshp(node: i32, model: i32) -> Vec<u8> {
    let mut content = i32b(node).to_vec();
    content.extend_from_slice(&dict(&[]));
    content.extend_from_slice(&i32b(1));
    content.extend_from_slice(&i32b(model));
    content.extend_from_slice(&dict(&[]));
    chunk(b"nSHP", &content, &[])
}

/// 根 nTRN(0) → nGRP(1) → nTRN(2, 命名 `body`，可带 `_t`/`_r`) → nSHP(3, 模型 0)。
/// 节点 id 按节点块出现顺序编号（SIZE/XYZI/RGBA/MATL 不占号）。
fn build_vox(size: [u32; 3], voxels: &[[u8; 4]], translation: Option<&str>) -> Vec<u8> {
    let mut children = Vec::new();
    let mut size_bytes = Vec::new();
    for axis in size {
        size_bytes.extend_from_slice(&axis.to_le_bytes());
    }
    children.extend_from_slice(&chunk(b"SIZE", &size_bytes, &[]));
    let mut xyzi = (voxels.len() as u32).to_le_bytes().to_vec();
    for voxel in voxels {
        xyzi.extend_from_slice(voxel);
    }
    children.extend_from_slice(&chunk(b"XYZI", &xyzi, &[]));
    children.extend_from_slice(&ntrn(0, None, None, None, 1));
    children.extend_from_slice(&ngrp(1, &[2]));
    children.extend_from_slice(&ntrn(2, Some("body"), translation, None, 3));
    children.extend_from_slice(&nshp(3, 0));
    let mut rgba = vec![0u8; 256 * 4];
    rgba[..4].copy_from_slice(&[255, 0, 0, 255]);
    rgba[4..8].copy_from_slice(&[0, 255, 0, 255]);
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

// ---- 磁盘小样：粗样 + 真实转换器 staging ----

/// 转换器读取的源 meta 只需含 `voxel_size`；其余字段与单位校验无关。
const SOURCE_META: &str = "( settings: ( voxel_size: 0.1, ) )\n";

/// 生成隔离临时目录（进程号 + 用例名区分；已存在则先清空）。
fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tanker-vox-subdiv-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 运行真实转换器：把 `src.vox`（旁有 `voxel_size: 0.1` 的 meta）细分为 0.02，
/// 产物写入 `staging`。任一步失败即 panic，测试不静默降级。
fn run_converter(src: &Path, staging: &Path) {
    for program in ["python", "python3"] {
        let output = Command::new(program)
            .arg("-X")
            .arg("utf8")
            .arg("scripts/subdivide_vox.py")
            .arg(src)
            .arg("--staging")
            .arg(staging)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output();
        match output {
            Ok(out) if out.status.success() => return,
            Ok(out) => panic!(
                "转换器失败（{program}）：{}",
                String::from_utf8_lossy(&out.stderr)
            ),
            // 解释器不存在时尝试下一个名字。
            Err(_) => continue,
        }
    }
    panic!("未找到 python 解释器：工单 03 集成测试需要运行 scripts/subdivide_vox.py");
}

/// 写粗样与源 meta，运行转换器，返回（粗样字节, 细分产物字节）。
fn coarse_and_staged(tag: &str, size: [u32; 3], voxels: &[[u8; 4]], translation: Option<&str>) -> (Vec<u8>, Vec<u8>) {
    let dir = scratch_dir(tag);
    let src = dir.join("coarse.vox");
    std::fs::write(&src, build_vox(size, voxels, translation)).unwrap();
    std::fs::write(dir.join("coarse.vox.meta"), SOURCE_META).unwrap();
    let staging = dir.join("staged");
    run_converter(&src, &staging);
    let staged = std::fs::read(staging.join("coarse.vox")).unwrap();
    (std::fs::read(&src).unwrap(), staged)
}

// ---- 测试装配（与 tests/vox_meta_settings.rs 相同的最小真实加载组装） ----

/// 组装带 `voxsub://` 内存源的应用；`VoxScenePlugin::default()` 走 meta 路径。
fn app_with(dir: Dir) -> App {
    let mut app = App::new();
    app.register_asset_source(
        "voxsub",
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
        bevy::camera::visibility::VisibilityPlugin,
        VoxScenePlugin::default(),
    ))
    .init_asset::<Mesh>()
    .init_asset::<StandardMaterial>()
    .init_asset::<SkinnedMeshInverseBindposes>()
    .register_type::<Visibility>()
    .register_type::<ViewVisibility>()
    .register_type::<InheritedVisibility>()
    .register_type::<Transform>()
    .register_type::<GlobalTransform>()
    .register_type::<Mesh3d>()
    .register_type::<MeshMaterial3d<StandardMaterial>>();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(50)));
    app
}

/// 生成并挂载 `path` 的 WorldAssetRoot，等待实例就绪；加载失败返回错误文本。
fn spawn_and_wait(app: &mut App, path: &str) -> Result<Entity, String> {
    let handle: Handle<WorldAsset> = app.world().resource::<AssetServer>().load(path.to_owned());
    let root = app.world_mut().spawn(WorldAssetRoot(handle.clone())).id();
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

/// 收集子树全部网格实体的（模型空间 AABB, 世界变换）。
fn subtree_meshes(app: &App, root: Entity) -> Vec<(Aabb, GlobalTransform)> {
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
            let transform = world.get::<GlobalTransform>(entity).copied().unwrap_or_default();
            out.push((aabb, transform));
        }
    }
    out
}

/// 子树中所有实体的 Transform.scale 必须为 1（禁止缩放作弊路径）。
fn assert_no_scaling(app: &App, root: Entity) {
    let mut pending = vec![root];
    let world = app.world();
    while let Some(entity) = pending.pop() {
        if let Some(kids) = world.get::<Children>(entity) {
            pending.extend(kids.iter());
        }
        if let Some(transform) = world.get::<Transform>(entity) {
            assert_eq!(transform.scale, Vec3::ONE, "{entity:?} 出现缩放");
        }
    }
}

/// 子树世界 AABB 并集：逐网格变换 8 角点后取包络。
fn subtree_world_aabb(app: &App, root: Entity) -> Aabb {
    let mut min = Vec3::MAX;
    let mut max = Vec3::MIN;
    let meshes = subtree_meshes(app, root);
    assert!(!meshes.is_empty(), "子树中没有任何网格");
    for (aabb, transform) in meshes {
        let center = Vec3::from(aabb.center);
        let half = Vec3::from(aabb.half_extents);
        for sx in [-1.0, 1.0] {
            for sy in [-1.0, 1.0] {
                for sz in [-1.0, 1.0] {
                    let corner = transform
                        .transform_point(center + Vec3::new(sx * half.x, sy * half.y, sz * half.z));
                    min = min.min(corner);
                    max = max.max(corner);
                }
            }
        }
    }
    Aabb {
        center: ((min + max) / 2.0).into(),
        half_extents: ((max - min) / 2.0).into(),
    }
}

/// 断言两个 AABB 的中心与半尺寸逐分量接近（米制容差 1e-4）。
fn assert_aabb_close(actual: Aabb, expected: Aabb, label: &str) {
    let center_diff = (Vec3::from(actual.center) - Vec3::from(expected.center)).abs();
    let extent_diff = (Vec3::from(actual.half_extents) - Vec3::from(expected.half_extents)).abs();
    assert!(
        center_diff.max_element() < 1e-4 && extent_diff.max_element() < 1e-4,
        "{label}：中心差 {center_diff:?}，半尺寸差 {extent_diff:?}"
    );
}

// ---- 用例 ----

/// 构建粗样 → 真实转换 → 内存加载 `#body`，返回（应用, 粗样根, 细分根）。
fn setup(tag: &str, canvas: [u32; 3], voxels: &[[u8; 4]]) -> (App, Entity, Entity) {
    let (coarse, staged) = coarse_and_staged(tag, canvas, voxels, None);
    let dir = Dir::default();
    dir.insert_asset(Path::new("coarse.vox"), coarse);
    dir.insert_meta(Path::new("coarse.vox"), bevy_meta(0.1));
    dir.insert_asset(Path::new("fine.vox"), staged);
    dir.insert_meta(Path::new("fine.vox"), bevy_meta(0.02));
    let mut app = app_with(dir);
    let coarse_root = spawn_and_wait(&mut app, "voxsub://coarse.vox#body").unwrap();
    let fine_root = spawn_and_wait(&mut app, "voxsub://fine.vox#body").unwrap();
    (app, coarse_root, fine_root)
}

/// 分块小样共用断言：网格数、逐块尺寸（X 降序）、世界 AABB 并集、无缩放。
/// `expected` 为每块世界尺寸 (x,y,z)，按 X 降序排列；容差 1e-4 米。
fn assert_chunk_parity(
    app: &App,
    coarse_root: Entity,
    fine_root: Entity,
    expected: &[(f32, f32, f32)],
    label: &str,
) {
    let coarse_meshes = subtree_meshes(app, coarse_root);
    let fine_meshes = subtree_meshes(app, fine_root);
    assert_eq!(coarse_meshes.len(), 1, "{label}：粗样应恰好一个网格");
    assert_eq!(fine_meshes.len(), expected.len(), "{label}：块数不符");

    let coarse_aabb = subtree_world_aabb(app, coarse_root);
    let fine_aabb = subtree_world_aabb(app, fine_root);
    assert_aabb_close(fine_aabb, coarse_aabb, label);

    let mut sizes: Vec<Vec3> = fine_meshes
        .iter()
        .map(|(aabb, _)| Vec3::from(aabb.half_extents) * 2.0)
        .collect();
    sizes.sort_by(|a, b| b.x.total_cmp(&a.x));
    let tol = 1e-4;
    for (got, want) in sizes.iter().zip(expected) {
        let want = Vec3::new(want.0, want.1, want.2);
        assert!(
            (got - want).max_element() < tol,
            "{label}：块尺寸 {got:?} ≠ 期望 {want:?}"
        );
    }
    assert_no_scaling(app, fine_root);
}

/// 300 格细分画布（源 60 格）：X 轴分 (256,44) 两块；体素 x=51 的子格
/// [255,260) 横跨 256 边界，覆盖跨界拆分路径。
#[test]
fn staged_chunk_subtree_matches_coarse_geometry() {
    let canvas = [60u32, 3, 2];
    let voxels: &[[u8; 4]] = &[
        [0, 0, 0, 1],
        [30, 1, 1, 2],
        [51, 2, 0, 1],
        [59, 0, 1, 2],
    ];
    let (app, coarse_root, fine_root) = setup("chunks", canvas, voxels);
    // 块尺寸：chunk0 子格 [0,256) → 5.12 米，chunk1 [256,300) → 0.88 米；
    // Y 行 0..3 → 世界 Z 0.3 米，Z 行 0..2 → 世界 Y 0.2 米（烘焙 −x,z,y）。
    assert_chunk_parity(
        &app,
        coarse_root,
        fine_root,
        &[(5.12, 0.2, 0.3), (0.88, 0.2, 0.3)],
        "300 格小样",
    );
}

/// 675 格奇数画布（源 135 格）：奇画布取 K=255，三块 (255,255,165)；
/// 公式 t = 起点 + (C−F)/2 给出平移 (−210,45,255)（体素级已在 Python 验证）。
#[test]
fn staged_odd_canvas_675_chunked() {
    let canvas = [135u32, 3, 2];
    let voxels: &[[u8; 4]] = &[
        [0, 1, 0, 1],    // chunk0（子格 0..4）
        [50, 1, 0, 1],   // chunk0（子格 250..254）
        [51, 1, 0, 2],   // chunk1（子格 255..259）
        [100, 1, 0, 2],  // chunk1（子格 500..504）
        [134, 1, 0, 1],  // chunk2（子格 670..674）
    ];
    let (app, coarse_root, fine_root) = setup("odd675", canvas, voxels);
    // 块尺寸：[0,255)→5.1 米、[255,505)→5.0 米、[670,675)→0.1 米；
    // 单 Y 行/Z 行 → 世界 Z/Y 各 0.1 米。
    assert_chunk_parity(
        &app,
        coarse_root,
        fine_root,
        &[(5.1, 0.1, 0.1), (5.0, 0.1, 0.1), (0.1, 0.1, 0.1)],
        "675 奇数画布",
    );
}

/// 570 格偶数画布（源 114 格）：偶画布取 K=256，三块 (256,256,58)；
/// 体素 x=51 子格 [255,259) 横跨 256 边界，块平移含负值 (−157,99,256)。
#[test]
fn staged_even_canvas_570_chunked() {
    let canvas = [114u32, 3, 2];
    let voxels: &[[u8; 4]] = &[
        [0, 1, 0, 1],    // chunk0（子格 0..4）
        [51, 1, 0, 2],   // 跨界：子格 255..259 分属 chunk0/chunk1
        [100, 1, 0, 2],  // chunk1（子格 500..504）
        [113, 1, 0, 1],  // chunk2（子格 565..569）
    ];
    let (app, coarse_root, fine_root) = setup("even570", canvas, voxels);
    // 块尺寸：[0,256)→5.12 米、[256,505)→4.98 米、[565,570)→0.1 米。
    assert_chunk_parity(
        &app,
        coarse_root,
        fine_root,
        &[(5.12, 0.1, 0.1), (4.98, 0.1, 0.1), (0.1, 0.1, 0.1)],
        "570 偶数画布",
    );
}

#[test]
fn named_node_negative_translation_survives_subdivision() {
    // 命名节点帧 _t="-2 -3 -1"：粗样与细分产物都走整文件作用域，
    // 验证节点平移 ×5 后经真实加载器仍保形（含负值）。
    let (coarse, staged) = coarse_and_staged(
        "negtr",
        [60u32, 3, 2],
        &[[0, 0, 0, 1], [30, 1, 1, 2], [51, 2, 0, 1], [59, 0, 1, 2]],
        Some("-2 -3 -1"),
    );
    let dir = Dir::default();
    dir.insert_asset(Path::new("coarse.vox"), coarse);
    dir.insert_meta(Path::new("coarse.vox"), bevy_meta(0.1));
    dir.insert_asset(Path::new("fine.vox"), staged);
    dir.insert_meta(Path::new("fine.vox"), bevy_meta(0.02));
    let mut app = app_with(dir);

    let coarse_root = spawn_and_wait(&mut app, "voxsub://coarse.vox").unwrap();
    let fine_root = spawn_and_wait(&mut app, "voxsub://fine.vox").unwrap();

    let coarse_aabb = subtree_world_aabb(&app, coarse_root);
    let fine_aabb = subtree_world_aabb(&app, fine_root);
    assert_aabb_close(fine_aabb, coarse_aabb, "负平移节点细分后世界 AABB 不一致");
    assert_no_scaling(&app, fine_root);
}

/// 生成一份与真实 `.vox.meta` 同构的 meta 字节（全字段，仅 voxel_size 可调）；
/// VoxLoaderSettings 无顶层 serde default，缺字段会反序列化失败。
fn bevy_meta(voxel_size: f32) -> Vec<u8> {
    format!(
        r#"(
    meta_format_version: "1.0",
    asset: Load(
        loader: "bevy_vox_scene::load::VoxSceneLoader",
        settings: (
            animation_frames_per_second: 8.0,
            voxel_size: {voxel_size},
            mesh_outer_faces: true,
            mesh_offset: ((0.5, 0.5, 0.5)),
            emission_strength: 20.0,
            uses_srgb: true,
            diffuse_roughness: 0.8,
            retain_voxel_data: false,
            retain_mesh_data: false,
            texture_mode: Packed((
                padding: 1,
                alignment: 4,
                max_dimension: 8192,
                mip_policy: Disabled,
                ambient_occlusion: None,
                log_diagnostics: false,
            )),
        ),
    ),
)
"#
    )
    .into_bytes()
}

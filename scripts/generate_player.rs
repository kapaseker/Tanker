//! 提供玩家作者化包的命名画布、材质与独立轮粗源；细网格主体见 player_a6.py。
//! 日常发布请走统一入口：python -X utf8 scripts/publish_player.py
//! （生成命名画布/轮粗源 → 细分 → 直接作者化主体 → 验证后发布）；直接运行本生成器仅在
//! 交付 meta 仍是参考网格 0.1 时允许，细分迁移后会被拒绝，防止粗几何写回细配置。
//! rustc --edition 2024 -O scripts/generate_player.rs -o target/generate_player.exe
//! 车体与履带共享画布；局部轮模型以轴心居中，运行时按共享契约安装。

use std::{collections::BTreeMap, env, fs, path::PathBuf};
// 生成器只消费契约的一部分；未用到的车型数据在主工程运行时使用，不在此告警。
#[allow(dead_code)]
#[path = "../src/tank/model/mod.rs"]
mod model;
use model::player::CANVAS as SIZE;

/// 固定调色板索引：履带深灰和轮组浅灰。
const C_DARK: u8 = 4;
/// 独立浅色辐条，保留原履带灰色不变。
const C_SPOKE: u8 = 6;
/// 有序体素集合：后写的装甲细节覆盖底色，不输出重复体素。
type Cells = BTreeMap<(i32, i32, i32), u8>;

/// 单体素，坐标为所有包共用的 VOX 全局格坐标。
#[derive(Clone, Copy)]
struct Voxel {
    x: i32,
    y: i32,
    z: i32,
    c: u8,
}

/// 转换有序体素，所有坐标必须落在统一画布内。
fn voxels(cells: Cells) -> Vec<Voxel> {
    cells
        .into_iter()
        .map(|((x, y, z), c)| {
            assert!(
                x >= 0
                    && x < SIZE.0 as i32
                    && y >= 0
                    && y < SIZE.1 as i32
                    && z >= 0
                    && z < SIZE.2 as i32
            );
            Voxel { x, y, z, c }
        })
        .collect()
}

/// 车身和炮管只提供调色板、命名根与共同画布，几何由 player_a6.py 直接作者化。
fn authoring_canvas() -> Vec<Voxel> {
    vec![Voxel { x: 0, y: 0, z: 0, c: 1 }]
}

/// 六个履带命名画布加独立轮；闭环在细网格作者源中生成，不再依赖粗几何。
fn tracks() -> Vec<(String, Vec<Voxel>)> {
    let mut models = Vec::new();
    for side in ["l", "r"] {
        for phase in 0..3 {
            models.push((format!("track_{side}_{phase}"), authoring_canvas()));
        }
    }
    for spec in model::player::WHEELS.iter().flatten() {
        models.push((spec.name.into(), wheel_model(spec)));
    }
    models
}

/// 仅保留四角都在旋转包络内的体素；轴心等于局部 SIZE 中心。
fn wheel_model(spec: &model::WheelSpec) -> Vec<Voxel> {
    let mut cells = Cells::new();
    let half = spec.size.0 as f32 / 2.0;
    for x in 0..spec.size.0 as i32 {
        for z in 0..spec.size.2 as i32 {
            let far_x = (x as f32 - half).abs().max((x as f32 + 1.0 - half).abs());
            let far_z = (z as f32 - half).abs().max((z as f32 + 1.0 - half).abs());
            if far_x.hypot(far_z) > spec.envelope {
                continue;
            }
            let dx = x as f32 + 0.5 - half;
            let dz = z as f32 + 0.5 - half;
            let r = dx.hypot(dz);
            // 三条不对称辐条采用不同角度，避免三等分造成低速转动混淆。
            let spoke = [0.0f32, 1.7, 4.0].iter().any(|angle| {
                let along = dx * angle.cos() + dz * angle.sin();
                let across = -dx * angle.sin() + dz * angle.cos();
                along > 0.0 && across.abs() < 0.55
            });
            let color = if r > spec.radius - 1.2 {
                C_DARK
            } else if spoke {
                C_SPOKE
            } else {
                1
            };
            for y in 0..spec.size.1 as i32 {
                cells.insert((x, y, z), color);
            }
        }
    }
    voxels(cells)
}

/// 写出带自有调色板和哑光材质的场景包；节点序号沿用现有加载器契约。
fn package(models: &[(String, Vec<Voxel>)]) -> Vec<u8> {
    let mut children = Vec::new();
    for (name, m) in models {
        let size = model::player::WHEELS
            .iter()
            .flatten()
            .find(|wheel| wheel.name == name)
            .map_or(SIZE, |wheel| wheel.size);
        push_model(&mut children, m, size);
    }
    let colors = [
        [83, 99, 55, 255],
        [108, 121, 72, 255],
        [58, 70, 40, 255],
        [32, 35, 33, 255],
        [58, 62, 56, 255],
        [174, 181, 151, 255],
    ];
    let mut palette = vec![0; 1024];
    for (i, color) in colors.iter().enumerate() {
        palette[i * 4..i * 4 + 4].copy_from_slice(color);
    }
    push_chunk(&mut children, b"RGBA", &palette);
    for index in 1..=colors.len() {
        let mut material = (index as u32).to_le_bytes().to_vec();
        push_dict(&mut material, &[("_type", "_diffuse"), ("_rough", "1")]);
        push_chunk(&mut children, b"MATL", &material);
    }
    push_root_trn(&mut children, 1);
    let ids: Vec<u32> = (0..models.len() as u32).map(|i| 2 + 2 * i).collect();
    push_grp(&mut children, 1, &ids);
    for (i, (name, _)) in models.iter().enumerate() {
        push_trn_shp(&mut children, 2 + 2 * i as u32, name, i as u32);
    }
    let mut out = b"VOX ".to_vec();
    out.extend_from_slice(&150u32.to_le_bytes());
    push_main(&mut out, &children);
    out
}

/// 粗资产的参考网格单位（米/格），与运行时 `REFERENCE_VOXEL_SIZE` 一致。
const COARSE_UNIT: f32 = 0.1;

/// 配对 meta 已存在且单位不是 0.1 时拒绝直写：细分迁移后粗几何不得落到
/// 0.02 配置下（工单 04 验收项），必须改走 scripts/publish_player.py。
/// meta 不存在（初始生成或发布入口写临时目录）时放行。
fn refuse_if_migrated(path: &std::path::Path) {
    let mut meta_os = path.as_os_str().to_os_string();
    meta_os.push(".meta");
    let Ok(text) = fs::read_to_string(std::path::PathBuf::from(meta_os)) else {
        return;
    };
    let parse = |label: &str| -> f32 {
        let after = text
            .find("voxel_size")
            .unwrap_or_else(|| panic!("{label}：meta 缺少 voxel_size 字段，无法判定单位，拒绝直写粗资产"));
        let after = &text[after + "voxel_size".len()..];
        let colon = after.find(':').unwrap_or_else(|| panic!("{label}：voxel_size 字段格式异常，拒绝直写"));
        let tail = after[colon + 1..].trim_start();
        let end = tail
            .find(|c: char| c == ',' || c == ')' || c.is_whitespace())
            .unwrap_or(tail.len());
        tail[..end]
            .parse()
            .unwrap_or_else(|_| panic!("{label}：voxel_size 值 {:?} 无法解析，拒绝直写", &tail[..end]))
    };
    let path_label = path.display().to_string();
    let unit = parse(&path_label);
    if (unit - COARSE_UNIT).abs() > 1e-6 {
        panic!(
            "{path_label} 已配置为 {unit} 米/格（细分资产），生成器的粗输出不得覆盖；\n请运行：python -X utf8 scripts/publish_player.py",
        );
    }
}

/// 输出三个临时作者化包，可传工程根目录；不读取旧模型或敌人资源。
/// 细分迁移后 body/barrel 会被配对 meta 守卫拒绝，改走 publish_player.py。
fn main() {
    let root = PathBuf::from(env::args().nth(1).unwrap_or_else(|| ".".into()));
    for (path, models) in [
        (
            "assets/tank/bodies/player/anvil/body.vox",
            vec![("body".into(), authoring_canvas())],
        ),
        (
            "assets/tank/barrels/player/lance/barrel.vox",
            vec![("barrel".into(), authoring_canvas())],
        ),
        ("assets/tank/tracks/player/cleat/track.vox", tracks()),
    ] {
        let path = root.join(path);
        refuse_if_migrated(&path);
        fs::create_dir_all(path.parent().unwrap()).expect("创建模型目录失败");
        let data = package(&models);
        fs::write(&path, &data).expect("写出玩家模型失败");
        println!("已生成 {}（{} 字节）", path.display(), data.len());
    }
}

/// 写一个 model：全局部件用统一画布，轮子使用以轴心居中的局部 SIZE。
fn push_model(out: &mut Vec<u8>, m: &[Voxel], size: (u32, u32, u32)) {
    push_chunk(out, b"SIZE", &{
        let mut c = Vec::new();
        c.extend_from_slice(&size.0.to_le_bytes());
        c.extend_from_slice(&size.1.to_le_bytes());
        c.extend_from_slice(&size.2.to_le_bytes());
        c
    });
    push_chunk(out, b"XYZI", &{
        let mut c = Vec::new();
        c.extend_from_slice(&(m.len() as u32).to_le_bytes());
        for v in m {
            c.extend_from_slice(&[v.x as u8, v.y as u8, v.z as u8, v.c]);
        }
        c
    });
}

/// 写根 nTRN：node_id=0，child 指向 nGRP 的位置（dot_vox 用 graph 位置索引引用）
fn push_root_trn(out: &mut Vec<u8>, grp_pos: u32) {
    let mut c = Vec::new();
    c.extend_from_slice(&0u32.to_le_bytes());
    push_dict(&mut c, &[]);
    c.extend_from_slice(&grp_pos.to_le_bytes());
    c.extend_from_slice(&(-1i32).to_le_bytes()); // reserved
    c.extend_from_slice(&(-1i32).to_le_bytes()); // layer_id: 不属于任何层
    c.extend_from_slice(&1u32.to_le_bytes()); // frames: 1
    push_dict(&mut c, &[]); // frame dict
    push_chunk(out, b"nTRN", &c);
}

/// 写 nGRP：node_id=pos，children 为各子节点位置
fn push_grp(out: &mut Vec<u8>, pos: u32, children_pos: &[u32]) {
    let mut c = Vec::new();
    c.extend_from_slice(&pos.to_le_bytes());
    push_dict(&mut c, &[]);
    c.extend_from_slice(&(children_pos.len() as u32).to_le_bytes());
    for p in children_pos {
        c.extend_from_slice(&p.to_le_bytes());
    }
    push_chunk(out, b"nGRP", &c);
}

/// 写一对 nTRN(命名) + nSHP(引用 model_id)；显隐只由运行时控制。
fn push_trn_shp(out: &mut Vec<u8>, trn_pos: u32, name: &str, model_id: u32) {
    let shp_pos = trn_pos + 1;
    let mut c = Vec::new();
    c.extend_from_slice(&trn_pos.to_le_bytes());
    push_dict(&mut c, &[("_name", name)]);
    c.extend_from_slice(&shp_pos.to_le_bytes());
    c.extend_from_slice(&(-1i32).to_le_bytes()); // reserved
    c.extend_from_slice(&(-1i32).to_le_bytes()); // layer_id: 不属于任何层
    c.extend_from_slice(&1u32.to_le_bytes()); // frames: 1
    push_dict(&mut c, &[]); // frame dict
    push_chunk(out, b"nTRN", &c);
    let mut c = Vec::new();
    c.extend_from_slice(&shp_pos.to_le_bytes());
    push_dict(&mut c, &[]);
    c.extend_from_slice(&1u32.to_le_bytes()); // models: 1
    c.extend_from_slice(&model_id.to_le_bytes());
    push_dict(&mut c, &[]);
    push_chunk(out, b"nSHP", &c);
}

/// 小端读 u32
#[cfg(test)]
fn le_u32(b: &[u8], pos: usize) -> u32 {
    u32::from_le_bytes([b[pos], b[pos + 1], b[pos + 2], b[pos + 3]])
}

/// 写一个无子 chunk：id + content_size + children_size(0) + content
fn push_chunk(out: &mut Vec<u8>, id: &[u8; 4], content: &[u8]) {
    out.extend_from_slice(id);
    out.extend_from_slice(&(content.len() as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(content);
}

/// 写 MAIN chunk：content 为空、children 为全部子 chunk
fn push_main(out: &mut Vec<u8>, children: &[u8]) {
    out.extend_from_slice(b"MAIN");
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(children.len() as u32).to_le_bytes());
    out.extend_from_slice(children);
}

/// 写字典：count + 每项 (keyLen + key + valLen + val)
fn push_dict(out: &mut Vec<u8>, entries: &[(&str, &str)]) {
    out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for (k, v) in entries {
        out.extend_from_slice(&(k.len() as u32).to_le_bytes());
        out.extend_from_slice(k.as_bytes());
        out.extend_from_slice(&(v.len() as u32).to_le_bytes());
        out.extend_from_slice(v.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 轮子独立画布不能直接并入全局边界；按安装轴心核对整个旋转包络。
    #[test]
    fn wheel_envelopes_fit_collision_and_track_clearance() {
        for spec in model::player::WHEELS.iter().flatten() {
            for axis in 0..3 {
                let extent = if axis == 1 {
                    spec.size.1 as f32 / 2.0
                } else {
                    spec.envelope
                };
                assert!(spec.center[axis] - extent >= model::player::COLLISION_MIN[axis] as f32);
                assert!(spec.center[axis] + extent <= model::player::COLLISION_MAX[axis] as f32);
            }
            assert!(spec.center[2] - spec.envelope > model::player::WHEEL_CLEARANCE.0);
            assert!(spec.center[2] + spec.envelope < model::player::WHEEL_CLEARANCE.1);
        }
    }

    /// nTRN 尾部必须依次写 reserved、layer_id、帧数、帧字典，不能错位为零帧。
    #[test]
    fn transform_nodes_have_one_identity_frame() {
        let mut root = Vec::new();
        push_root_trn(&mut root, 1);
        let mut named = Vec::new();
        push_trn_shp(&mut named, 2, "track_l_1", 0);
        let expected: Vec<u8> = [-1i32, -1, 1, 0]
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect();
        for chunk in [root, named] {
            let end = 12 + le_u32(&chunk, 4) as usize;
            assert_eq!(&chunk[end - 16..end], expected.as_slice());
            assert!(!chunk.windows(7).any(|part| part == b"_hidden"));
        }
    }
}

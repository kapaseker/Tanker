//! 把原始单 shape 坦克 vox 拆分为车体 + 独立履带包，可重复生成相同资产。
//!
//! 在工程根目录编译：rustc --edition 2024 -O scripts/generate_player.rs -o target/generate_player.exe
//! 运行：target/generate_player.exe（可传入工程根目录作为首个参数）。
//! 编译运行均无外部依赖（纯 std）。
//!
//! 资产布局（底盘 ID 分组，履带路径第一层 = 适配的底盘）：
//! - assets/tank/player/default/body.vox                 车体（单节点 "body"）
//! - assets/tank/tracks/player/lugged/default.vox        履带包（6 个相位变体节点）
//!
//! 对齐约定：两个文件所有 model 统一 SIZE(32,26,15) 且体素用全局坐标，
//! bevy_vox_scene 默认 UnitOffset::CENTER 按 model 中心对齐 → 车体与履带天然重合，
//! 运行时 Transform 偏移为零。更换履带包时必须沿用统一 SIZE 与全局坐标。
//!
//! 履带包节点命名：track_l_0..2 / track_r_0..2；全部不带 `_hidden`。
//! 初始与动画显隐由运行时外层实体统一控制，避免内部隐藏阻止相位显示。

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;

/// 齿距（体素格）：相位图案周期，3 帧 × 1 格错位
const PITCH: i32 = 3;
/// 履带深灰 / 浅灰的调色板索引（沿用原模型）
const C_DARK: u8 = 4;
const C_LIGHT: u8 = 5;
/// 原始模型履带的 y 区间（左右两条）
const TRACK_L_Y: (i32, i32) = (0, 3);
const TRACK_R_Y: (i32, i32) = (20, 23);
/// 拆分后所有 model 共用的统一尺寸（原 32×24×15，y 扩 2 格容纳侧面凸齿）
const SIZE: (u32, u32, u32) = (32, 26, 15);

/// 单个体素：坐标 + 调色板索引
#[derive(Clone, Copy)]
struct Voxel {
    x: i32,
    y: i32,
    z: i32,
    c: u8,
}

/// 沿 VOX 车头 +X 顺序闭环：上段向前、车头下绕、下段向后、车尾上绕。
/// 下方两角各削一格，66 个轮廓点恰好整除齿距，三相位回绕时接缝连续。
fn track_loop() -> Vec<(i32, i32)> {
    (0..=29)
        .map(|x| (x, 5))
        .chain((1..5).rev().map(|z| (29, z)))
        .chain((1..=28).rev().map(|x| (x, 0)))
        .chain((1..5).map(|z| (0, z)))
        .collect()
}

fn main() {
    // 原始模型随脚本保存，生成过程不依赖本机 .scratch 备份。
    let root = PathBuf::from(env::args().nth(1).unwrap_or_else(|| ".".to_string()));
    let orig_path = root.join("scripts/fixtures/player_original.vox");
    let body_path = root.join("assets/tank/player/default/body.vox");
    let tracks_path = root.join("assets/tank/tracks/player/lugged/default.vox");

    let raw = fs::read(&orig_path).expect("读取原始 vox 备份失败");
    assert_eq!(&raw[0..4], b"VOX ", "备份文件不是 vox 格式");

    // ---- 遍历 MAIN children，解析体素并保留 RGBA/MATL ----
    let (main_csize, main_ksize) = (le_u32(&raw, 12) as usize, le_u32(&raw, 16) as usize);
    let mut pos = 20 + main_csize;
    let end = pos + main_ksize;
    let mut voxels: Vec<Voxel> = Vec::new();
    let mut palette_chunk: Vec<u8> = Vec::new();
    let mut material_chunks: Vec<Vec<u8>> = Vec::new();
    while pos < end {
        let id = &raw[pos..pos + 4];
        let csize = le_u32(&raw, pos + 4) as usize;
        let ksize = le_u32(&raw, pos + 8) as usize;
        let cstart = pos + 12;
        match id {
            b"XYZI" => {
                let n = le_u32(&raw, cstart) as usize;
                for i in 0..n {
                    let o = cstart + 4 + i * 4;
                    voxels.push(Voxel {
                        x: raw[o] as i32,
                        y: raw[o + 1] as i32,
                        z: raw[o + 2] as i32,
                        c: raw[o + 3],
                    });
                }
            }
            b"RGBA" => palette_chunk = raw[pos..cstart + csize + ksize].to_vec(),
            b"MATL" => material_chunks.push(raw[pos..cstart + csize + ksize].to_vec()),
            _ => {}
        }
        pos = cstart + csize + ksize;
    }
    println!(
        "原始模型 {} 体素，{} 材质",
        voxels.len(),
        material_chunks.len()
    );

    // ---- 拆分车体 / 履带，全部 y+1（侧面凸齿向外各扩 1 格） ----
    let in_track = |y: i32| {
        (TRACK_L_Y.0..=TRACK_L_Y.1).contains(&y) || (TRACK_R_Y.0..=TRACK_R_Y.1).contains(&y)
    };
    let shift_y = |v: Voxel| Voxel { y: v.y + 1, ..v };
    let body: Vec<Voxel> = voxels
        .iter()
        .filter(|v| !in_track(v.y))
        .map(|&v| shift_y(v))
        .collect();
    let tracks: Vec<Voxel> = voxels
        .iter()
        .filter(|v| in_track(v.y))
        .map(|&v| shift_y(v))
        .collect();
    println!(
        "车体 {} 体素，履带（两侧合计）{} 体素",
        body.len(),
        tracks.len()
    );

    // ---- 车体文件：单 model 单节点 ----
    fs::create_dir_all(body_path.parent().unwrap()).unwrap();
    let mut body_out: Vec<u8> = Vec::new();
    body_out.extend_from_slice(b"VOX ");
    body_out.extend_from_slice(&150u32.to_le_bytes());
    let mut children: Vec<u8> = Vec::new();
    push_model(&mut children, &body);
    // 车体文件同样需要自带调色板与材质，体素颜色才与原模型一致
    children.extend_from_slice(&palette_chunk);
    for m in &material_chunks {
        children.extend_from_slice(m);
    }
    // 场景图：nTRN(root@0) → nGRP(@1) → nTRN "body"(@2) → nSHP(@3)。
    // node_id / child 引用一律用 chunk 出现顺序（graph 位置索引），root 必须最先写出。
    push_root_trn(&mut children, 1);
    push_grp(&mut children, 1, &[2]);
    push_trn_shp(&mut children, 2, "body", 0);
    push_main(&mut body_out, &children);
    fs::write(&body_path, &body_out).expect("写出 body.vox 失败");
    println!("已写出 {}（{} 字节）", body_path.display(), body_out.len());

    // ---- 履带包文件：每侧每相位一个 model，nTRN 只带名字 ----
    // side: (该侧履带新 y 区间, 侧面凸齿 y)
    let sides = [("l", (1, 4), 0), ("r", (21, 24), 25)];
    let rim = track_loop();
    let mut track_models: Vec<(String, Vec<Voxel>)> = Vec::new();
    for &(side, (ty0, ty1), tooth_y) in &sides {
        for p in 0..PITCH {
            // 轮组保留原色且各相位不变；有序映射同时消除角点与顶齿的重复体素。
            let mut cells: BTreeMap<(i32, i32, i32), u8> = tracks
                .iter()
                .filter(|v| (ty0..=ty1).contains(&v.y))
                .filter(|v| v.z != 0 || (v.x != 0 && v.x != 29))
                .map(|v| ((v.x, v.y, v.z), v.c))
                .collect();
            for (s, &(x, z)) in rim.iter().enumerate() {
                let tooth = (s as i32 - p).rem_euclid(PITCH) == 0;
                for y in ty0..=ty1 {
                    cells.insert((x, y, z), if tooth { C_LIGHT } else { C_DARK });
                    // 顶齿沿用原高度 z=6，底部与前后端不外扩，保持碰撞盒尺寸。
                    if tooth && z == 5 {
                        cells.insert((x, y, 6), C_LIGHT);
                    }
                }
                if tooth {
                    cells.insert((x, tooth_y, z), C_LIGHT);
                }
            }
            let variant = cells
                .into_iter()
                .map(|((x, y, z), c)| Voxel { x, y, z, c })
                .collect();
            track_models.push((format!("track_{side}_{p}"), variant));
        }
    }
    for (name, m) in &track_models {
        println!("履带 model {name}: {} 体素", m.len());
    }

    fs::create_dir_all(tracks_path.parent().unwrap()).unwrap();
    let mut tracks_out: Vec<u8> = Vec::new();
    tracks_out.extend_from_slice(b"VOX ");
    tracks_out.extend_from_slice(&150u32.to_le_bytes());
    let mut children: Vec<u8> = Vec::new();
    for (_, m) in &track_models {
        push_model(&mut children, m);
    }
    children.extend_from_slice(&palette_chunk);
    for m in &material_chunks {
        children.extend_from_slice(m);
    }
    // 场景图：nTRN(root@0) → nGRP(@1) → 6 × (nTRN@2i+2 → nSHP@2i+3)。
    // node_id = chunk 出现顺序（graph 位置索引），root 最先写出。
    push_root_trn(&mut children, 1);
    let grp_children: Vec<u32> = (0..track_models.len() as u32).map(|i| 2 + i * 2).collect();
    push_grp(&mut children, 1, &grp_children);
    for (i, (name, _)) in track_models.iter().enumerate() {
        push_trn_shp(&mut children, 2 + (i as u32) * 2, name, i as u32);
    }
    push_main(&mut tracks_out, &children);
    fs::write(&tracks_path, &tracks_out).expect("写出履带包失败");
    println!(
        "已写出 {}（{} 字节，6 变体）",
        tracks_path.display(),
        tracks_out.len()
    );
}

/// 写一个 model：SIZE + XYZI（统一 SIZE，体素全局坐标）
fn push_model(out: &mut Vec<u8>, m: &[Voxel]) {
    push_chunk(out, b"SIZE", &{
        let mut c = Vec::new();
        c.extend_from_slice(&SIZE.0.to_le_bytes());
        c.extend_from_slice(&SIZE.1.to_le_bytes());
        c.extend_from_slice(&SIZE.2.to_le_bytes());
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

    /// 闭环没有重复角点或断裂，周长是齿距整数倍，保证末相位回到首相位不跳齿。
    #[test]
    fn tread_loop_closes_without_duplicate_points_or_phase_seam() {
        let rim = track_loop();
        let points: std::collections::BTreeSet<_> = rim.iter().copied().collect();
        assert_eq!(points.len(), rim.len());
        assert_eq!(rim.len() as i32 % PITCH, 0);
        for i in 0..rim.len() {
            let (x, z) = rim[i];
            let (next_x, next_z) = rim[(i + 1) % rim.len()];
            assert_eq!((next_x - x).abs().max((next_z - z).abs()), 1);
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

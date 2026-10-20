//! 直接生成现代主战坦克的车身、炮管与六相位履带，纯标准库、确定性输出。
//! rustc --edition 2024 -O scripts/generate_player.rs -o target/generate_player.exe
//! 车体与履带共享画布；局部轮模型以轴心居中，运行时按共享契约安装。

use std::{collections::BTreeMap, env, fs, path::PathBuf};
#[path = "../src/player_model.rs"]
mod player_model;
use player_model::{BARREL_X, BORE_HEIGHT, CANVAS as SIZE, HULL_X};

/// 三相位每次推进一格，闭环周长必须被齿距整除。
const PITCH: i32 = 3;
/// 固定调色板索引：履带深灰和轮组浅灰。
const C_DARK: u8 = 4;
const C_LIGHT: u8 = 5;
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

/// 沿车头 +X 闭环，削去底部两个角，周长 168 格。
fn track_loop() -> Vec<(i32, i32)> {
    (30..105)
        .map(|x| (x, 11))
        .chain((1..11).rev().map(|z| (104, z)))
        .chain((31..104).rev().map(|x| (x, 0)))
        .chain((1..11).map(|z| (30, z)))
        .collect()
}

/// 填充半开区间盒体；bevel 削去平面四角，保持体素装甲的棱角感。
fn box_cells(
    cells: &mut Cells,
    x: (i32, i32),
    y: (i32, i32),
    z: (i32, i32),
    color: u8,
    bevel: i32,
) {
    for a in x.0..x.1 {
        for b in y.0..y.1 {
            if (a - x.0).min(x.1 - 1 - a) + (b - y.0).min(y.1 - 1 - b) < bevel {
                continue;
            }
            for c in z.0..z.1 {
                cells.insert((a, b, c), color);
            }
        }
    }
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

/// 生成车身和炮塔；裙板高于履带顶齿，外侧保留负重轮视窗。
fn body() -> Vec<Voxel> {
    let mut cells = Cells::new();
    box_cells(&mut cells, HULL_X, (7, 29), (4, 11), 1, 3);
    // 首上逐层后收，形成朝向 +X 的倾斜装甲。
    for z in 11..16 {
        box_cells(
            &mut cells,
            (31, 105 - (z - 10) * 2),
            (6, 30),
            (z, z + 1),
            if z == 15 { 2 } else { 1 },
            3,
        );
    }
    // 车尾最后四格向后下降两格，裙板用内侧窄梁连接。
    cells.retain(|&(x, _, z), _| x >= 34 || z < 14 + (x - 30) / 2);
    for y in [(1, 7), (29, 35)] {
        let beam = if y.0 == 1 { (6, 7) } else { (29, 30) };
        box_cells(&mut cells, (30, 105), beam, (14, 15), 3, 0);
        for x in (33..94).step_by(10) {
            box_cells(&mut cells, (x, x + 9), y, (14, 16), 3, 1);
            box_cells(&mut cells, (x, x + 9), y, (15, 16), 2, 1);
        }
        box_cells(&mut cells, (30, 33), y, (14, 15), 3, 1);
        box_cells(&mut cells, (102, 105), y, (14, 15), 3, 1);
    }
    // 尾舱 13 格、中部 15 格、前楔 13 格；高度和前壁接缝保持不变。
    for (z0, z1, half, color) in [(16, 20, 13, 3), (20, 23, 12, 1), (23, 25, 10, 2)] {
        box_cells(
            &mut cells,
            (49, 62),
            (18 - half, 18 + half),
            (z0, z1.min(24)),
            color,
            2,
        );
        box_cells(
            &mut cells,
            (62, 77),
            (18 - half, 18 + half),
            (z0, z1),
            color,
            0,
        );
        for x in 77..90 {
            // 两侧逐格收向炮盾；中层从 24 格收至 12 格宽。
            let width = half - (half - 6) * (x - 77) / 12;
            box_cells(
                &mut cells,
                (x, x + 1),
                (18 - width, 18 + width),
                (z0, z1),
                color,
                0,
            );
        }
    }
    box_cells(&mut cells, (61, 70), (11, 19), (25, 26), 3, 2);
    box_cells(&mut cells, (75, 82), (19, 25), (25, 26), 3, 2);
    // 炮盾属于车身，前面紧贴炮管根部，轴线在地面以上 2 米。
    box_cells(&mut cells, (86, 90), (14, 22), (17, 23), 3, 1);
    for x in (34..48).step_by(3) {
        box_cells(&mut cells, (x, x + 1), (10, 26), (15, 16), C_DARK, 0);
    }
    voxels(cells)
}

/// 炮管外径主体两格；根部加粗、中央抽烟器和暗色炮口表达火炮结构。
fn barrel() -> Vec<Voxel> {
    let mut cells = Cells::new();
    let h = BORE_HEIGHT as i32;
    box_cells(&mut cells, BARREL_X, (17, 19), (h - 1, h + 1), 2, 0);
    box_cells(&mut cells, (90, 96), (16, 20), (h - 2, h + 2), 3, 1);
    box_cells(&mut cells, (110, 116), (16, 20), (h - 2, h + 2), 1, 1);
    box_cells(&mut cells, (133, 135), (17, 19), (h - 1, h + 1), 3, 0);
    box_cells(&mut cells, (134, 135), (17, 19), (h - 1, h + 1), C_DARK, 0);
    voxels(cells)
}

/// 履带只保留闭环；内侧连接条位于轮宽之外，不填实轮间空隙。
fn tracks() -> Vec<(String, Vec<Voxel>)> {
    let rim = track_loop();
    let mut models = Vec::new();
    for (side, y0, y1, outer, inner) in [("l", 1, 7, 0, 6), ("r", 29, 35, 35, 29)] {
        for phase in 0..PITCH {
            let mut cells = Cells::new();
            box_cells(&mut cells, (31, 104), (inner, inner + 1), (5, 7), C_DARK, 0);
            for (i, &(x, z)) in rim.iter().enumerate() {
                let tooth = (i as i32 - phase).rem_euclid(PITCH) == 0;
                for y in y0..y1 {
                    // 上沿内侧一格让开车体斜装甲，外侧闭环保持完整。
                    if z == 11 && y == inner {
                        continue;
                    }
                    cells.insert((x, y, z), if tooth { C_LIGHT } else { C_DARK });
                    if tooth && z == 11 {
                        cells.insert((x, y, 12), C_LIGHT);
                    }
                }
                if tooth {
                    cells.insert((x, outer, z), C_LIGHT);
                }
            }
            models.push((format!("track_{side}_{phase}"), voxels(cells)));
        }
    }
    for spec in player_model::WHEELS.iter().flatten() {
        models.push((spec.name.into(), wheel_model(spec)));
    }
    models
}

/// 仅保留四角都在旋转包络内的体素；轴心等于局部 SIZE 中心。
fn wheel_model(spec: &player_model::WheelSpec) -> Vec<Voxel> {
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
        let size = player_model::WHEELS
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

/// 输出三个默认资源包，可传工程根目录；不读取旧模型或敌人资源。
fn main() {
    let root = PathBuf::from(env::args().nth(1).unwrap_or_else(|| ".".into()));
    for (path, models) in [
        (
            "assets/tank/player/default/body.vox",
            vec![("body".into(), body())],
        ),
        (
            "assets/tank/barrels/player/default.vox",
            vec![("barrel".into(), barrel())],
        ),
        ("assets/tank/tracks/player/lugged/default.vox", tracks()),
    ] {
        let path = root.join(path);
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

    /// 检查提交的二进制资源确实来自当前生成器，也锁定稳定的字节顺序。
    #[test]
    fn generated_packages_match_delivered_assets() {
        for (path, models) in [
            (
                "assets/tank/player/default/body.vox",
                vec![("body".into(), body())],
            ),
            (
                "assets/tank/barrels/player/default.vox",
                vec![("barrel".into(), barrel())],
            ),
            ("assets/tank/tracks/player/lugged/default.vox", tracks()),
        ] {
            assert_eq!(package(&models), fs::read(path).unwrap(), "{path}");
        }
    }

    /// 按全部履带相位与车身的真实格边界核对碰撞尺寸和零中心偏移。
    #[test]
    fn non_barrel_bounds_match_centered_collision_contract() {
        let mut geometry = body();
        for (_, model) in tracks().into_iter().take(6) {
            geometry.extend(model);
        }
        let mut low = [i32::MAX; 3];
        let mut high = [i32::MIN; 3];
        for v in geometry {
            for (axis, value) in [v.x, v.y, v.z].into_iter().enumerate() {
                low[axis] = low[axis].min(value);
                high[axis] = high[axis].max(value + 1);
            }
        }
        assert_eq!(low, player_model::COLLISION_MIN);
        assert_eq!(high, player_model::COLLISION_MAX);
        assert_eq!(
            [low[0] + high[0], low[1] + high[1], low[2] + high[2]],
            [SIZE.0 as i32, SIZE.1 as i32, SIZE.2 as i32],
            "碰撞中心必须与三包加载中心重合"
        );
        assert!(
            barrel().iter().any(|v| v.x >= high[0]),
            "炮管超出框但不参与边界并集"
        );
    }

    /// 轮子独立画布不能直接并入全局边界；按安装轴心核对整个旋转包络。
    #[test]
    fn wheel_envelopes_fit_collision_and_track_clearance() {
        for spec in player_model::WHEELS.iter().flatten() {
            for axis in 0..3 {
                let extent = if axis == 1 {
                    spec.size.1 as f32 / 2.0
                } else {
                    spec.envelope
                };
                assert!(spec.center[axis] - extent >= player_model::COLLISION_MIN[axis] as f32);
                assert!(spec.center[axis] + extent <= player_model::COLLISION_MAX[axis] as f32);
            }
            assert!(spec.center[2] - spec.envelope > 1.0);
            assert!(spec.center[2] + spec.envelope < 11.0);
        }
        assert_eq!(track_loop().len(), 168);
    }

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

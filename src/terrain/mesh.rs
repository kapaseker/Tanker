//! 地形视觉网格：把量化档位与土色索引编译成体素风格的分块网格。
//!
//! 顶面按同档位同色号贪心合并成矩形，边界处按 0.1 米生成台阶竖直面；
//! 顶点位置直接是世界坐标，实体无需平移。

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

use super::{AUTHOR_RES, TERRAIN_STEP};

/// 竖直面相对顶面的明度系数，按格子奇偶交替，避免大片台阶同色。
const SIDE_SHADE: [f32; 2] = [0.72, 0.84];

/// 一块地形网格的编译输入。
pub(super) struct ChunkSource<'a> {
    /// 整图的量化档位（高度字节 − `FLAT`），单位 0.1 米，0 为平地。
    pub(super) levels: &'a [i32],
    /// 整图的调色板下标，与 `levels` 同尺寸；已由 `parse_albedo` 校验越界。
    pub(super) colors: &'a [u8],
    /// 文件自带的土色调色板（sRGB）。
    pub(super) palette: &'a [[u8; 3]],
    /// 整图单边采样数。
    pub(super) side: usize,
    /// 本块在整图中的采样起点（行 i0、列 j0）。
    pub(super) start: (usize, usize),
    /// 本块单边采样数。
    pub(super) cells: usize,
    /// 本块第 0 行 0 列格子的西/北世界边缘 (x, z)。
    pub(super) origin: (f32, f32),
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
pub(super) fn build_chunk(source: &ChunkSource) -> Mesh {
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

/// 调色板下标 → 线性 RGBA；下标已由 `parse_albedo` 校验。
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
}

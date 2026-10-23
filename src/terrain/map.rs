//! 地形图数据：`.ht` / `.abd` 字节加载、土色图解析与贴图构造。
//!
//! 本模块只搬运与解释资产字节，不做几何编译；格式校验由使用方在主线程 panic。

use bevy::{
    asset::{AssetLoader, LoadContext, RenderAssetUsages, io::Reader},
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

use super::MAP_SIDE;

/// 地形图的原始字节：高度图（`.ht`）每格一个高度档位（`FLAT` 为 0 米，每档 0.1 米）；
/// 土色图（`.abd`）为调色板 + 索引矩阵（格式见 [`parse_albedo`]）。
/// 本类型只承载原始字节，格式校验由使用方在主线程 panic。
#[derive(Asset, TypePath, Debug)]
pub(super) struct GroundMap(pub(super) Vec<u8>);

/// 读取地形图的 `.ht` / `.abd` 字节流；只负责搬运字节，格式校验由 `spawn_terrain` panic。
#[derive(Default, TypePath)]
pub(super) struct GroundMapLoader;

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

/// 解析土色图 `.abd`：`[调色板数量 u8][数量×3 字节 sRGB][1200×1200 索引字节]`。
/// 返回 (调色板, 索引矩阵)；文件不规范时 panic，panic 消息即错误日志。
pub(super) fn parse_albedo(bytes: &[u8]) -> (Vec<[u8; 3]>, &[u8]) {
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
pub(super) fn terrain_texture(indices: &[u8], palette: &[[u8; 3]]) -> Image {
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

/// 读取仓库内的真实高度图；缺失或尺寸不符即 panic。
/// 供本模块与 `collider` 的真实地形测试共用。
#[cfg(test)]
pub(super) fn real_heights() -> Vec<u8> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/terrain/ground/ground.ht");
    let bytes = std::fs::read(path).expect("真实高度图 ground.ht 必须随仓库提供");
    assert_eq!(bytes.len(), MAP_SIDE * MAP_SIDE, "真实高度图尺寸不合规");
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::{AUTHOR_RES, FLAT};

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
}

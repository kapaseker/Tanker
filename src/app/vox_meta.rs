//! VOX 资产单位配置（`.vox.meta`）完整性校验。
//!
//! bevy_vox_scene 0.23 对缺失 meta 的 `.vox` 会静默回退插件默认 `voxel_size=1.0`
//! （见 `tests/vox_meta_settings.rs` 的"故意缺失"行为记录），对非正/非有限单位
//! 也不校验；任何受管理资产缺配置或非法配置都会以十倍尺寸静默错误呈现。
//! 本模块在启动时遍历 `assets/` 下全部 `.vox`，要求每个都有配对且合法的
//! `.vox.meta`，违规时返回带完整路径的中文原因，由 `app::run` panic 拦截
//! （与地形资产"损坏即 panic"同策略）。

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use bevy::asset::meta::{AssetAction, AssetMeta};
use bevy::asset::{AssetLoader, LoadContext};
use bevy::reflect::TypePath;
use bevy_vox_scene::VoxLoaderSettings;

/// 真实 VOX 加载器的类型路径；meta 的 `loader` 字段必须与此一致，
/// 否则 AssetServer 会按错误加载器解析资产（01 工单已由真实加载器冻结）。
const VOX_LOADER_TYPE_PATH: &str = "bevy_vox_scene::load::VoxSceneLoader";

/// 仅用于按 serde 结构反序列化 meta 的探针加载器：真实 `VoxSceneLoader` 类型
/// 未被插件公开导出，但其 `Settings` 同为 [`VoxLoaderSettings`]，meta 反序列化
/// 格式等价；从不注册进 AssetServer，也永不加载资产。
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
        unreachable!("MetaProbe 只用于反序列化 meta，永不加载资产")
    }
}

/// 校验 `root`（assets 目录）下每个 `.vox` 都有配对且合法的 `.vox.meta`。
/// 合法 = loader 指向真实 VoxSceneLoader，且 voxel_size 为有限正数。
pub(crate) fn validate(root: &Path) -> Result<(), String> {
    for vox in managed_vox_paths(root)? {
        let settings = meta_settings(&vox)?;
        // 插件本身不校验单位值（0/-1/NaN 均静默产生错误几何），此处兜底。
        if !settings.voxel_size.is_finite() || settings.voxel_size <= 0.0 {
            return Err(format!(
                "VOX 单位配置非法：{} 的 voxel_size 必须为有限正数，实际 {}",
                vox.display(),
                settings.voxel_size
            ));
        }
    }
    Ok(())
}

/// 递归收集 `root` 下全部 `.vox` 路径；排序保证失败信息顺序确定。
fn managed_vox_paths(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries =
            std::fs::read_dir(&dir).map_err(|err| format!("无法读取资产目录 {}：{err}", dir.display()))?;
        for entry in entries {
            let path = entry.map_err(|err| format!("遍历资产目录 {} 失败：{err}", dir.display()))?.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension() == Some(OsStr::new("vox")) {
                // `.vox.meta` 的扩展名是 "meta"，天然不会被误收。
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// 读取单个 `.vox` 的配对 meta 并反序列化出加载器设置；
/// 缺文件、RON 损坏或 loader 字段不符都返回带路径的错误。
fn meta_settings(vox: &Path) -> Result<VoxLoaderSettings, String> {
    let meta_path = meta_path_of(vox);
    let bytes = std::fs::read(&meta_path).map_err(|err| {
        format!(
            "VOX 缺少可读的单位配置：{}（每个 .vox 必须有配对的 .vox.meta，读取失败：{err}）",
            meta_path.display()
        )
    })?;
    let meta: AssetMeta<MetaProbe, ()> = AssetMeta::deserialize(&bytes).map_err(|err| {
        format!("VOX 单位配置损坏：{} 反序列化失败：{err}", meta_path.display())
    })?;
    let AssetAction::Load { loader, settings } = meta.asset else {
        return Err(format!(
            "VOX 单位配置无效：{} 不是 Load 配置，无法确定加载单位",
            meta_path.display()
        ));
    };
    if loader != VOX_LOADER_TYPE_PATH {
        return Err(format!(
            "VOX 单位配置无效：{} 的 loader 应为 {VOX_LOADER_TYPE_PATH}，实际 {loader}",
            meta_path.display()
        ));
    }
    Ok(settings)
}

/// `.vox` 的配对 meta 路径：Bevy 约定为原路径直接追加 `.meta` 后缀。
fn meta_path_of(vox: &Path) -> PathBuf {
    let mut os = vox.as_os_str().to_os_string();
    os.push(".meta");
    PathBuf::from(os)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::meta::AssetMetaDyn;

    /// 真实资产完整性：assets 下全部 .vox 的配置可解析且单位为有限正数。
    #[test]
    fn real_assets_all_have_valid_meta() {
        validate(Path::new("assets")).expect("受管理 VOX 的单位配置应全部合法");
    }

    /// 阶段 A 配置清单：现有九份资产逐一列名且全部为 0.1 米/格。
    /// 阶段 B 细分后七个坦克文件改为 0.02，清单随该工单同步更新。
    #[test]
    fn stage_a_manifest_lists_all_nine_assets_at_point_one() {
        const MANAGED: [&str; 9] = [
            "effects/explosion.vox",
            "obstacles/concrete/obstacle.vox",
            "tank/barrels/enemy/spit/barrel.vox",
            "tank/barrels/player/lance/barrel.vox",
            "tank/barrels/player/lance/bullet.vox",
            "tank/bodies/enemy/carapace/body.vox",
            "tank/bodies/player/anvil/body.vox",
            "tank/tracks/enemy/crawl/track.vox",
            "tank/tracks/player/cleat/track.vox",
        ];
        let found = managed_vox_paths(Path::new("assets")).expect("应能遍历 assets 目录");
        let found: Vec<String> = found
            .iter()
            // 去掉首段 assets/ 后统一为 / 分隔，跨平台与清单字面量比较。
            .map(|path| {
                path.iter().skip(1).collect::<PathBuf>().display().to_string().replace('\\', "/")
            })
            .collect();
        assert_eq!(found.len(), MANAGED.len(), "受管理 .vox 数量应与配置清单一致：{found:?}");
        for expected in MANAGED {
            assert!(found.contains(&expected.to_string()), "清单缺少 {expected}，实际 {found:?}");
            let size = meta_settings(Path::new("assets").join(expected).as_path())
                .unwrap_or_else(|err| panic!("{expected} 配置非法：{err}"))
                .voxel_size;
            assert_eq!(size, 0.1, "{expected} 应配置为 0.1 米/格");
        }
    }

    /// 生成一份合法（或按参数调整）的 meta RON 字节，与 01 工单小样的
    /// `meta_bytes` 等价：除 voxel_size 外全部取插件默认值。
    fn meta_bytes(voxel_size: f32) -> Vec<u8> {
        let meta = AssetMeta::<MetaProbe, ()>::new(AssetAction::Load {
            loader: VOX_LOADER_TYPE_PATH.to_string(),
            settings: VoxLoaderSettings {
                voxel_size,
                ..Default::default()
            },
        });
        meta.serialize()
    }

    /// 在系统临时目录搭一个只含一个 `.vox`（空文件，校验不解析体素）和
    /// 可选 meta 的最小资产树，供失败样例复用。
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tanker_vox_meta_{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("应能创建临时资产目录");
        std::fs::write(dir.join("a.vox"), b"").expect("应能写入空 .vox");
        dir
    }

    #[test]
    fn valid_meta_passes_validation() {
        let dir = scratch("valid");
        std::fs::write(dir.join("a.vox.meta"), meta_bytes(0.02)).unwrap();
        validate(&dir).expect("合法配置应通过校验");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_meta_fails_with_path() {
        let dir = scratch("missing");
        let err = validate(&dir).expect_err("缺配置不能静默通过");
        assert!(err.contains("a.vox.meta"), "错误应包含 meta 路径：{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_meta_fails_with_path() {
        let dir = scratch("corrupt");
        std::fs::write(dir.join("a.vox.meta"), b"this is not ron").unwrap();
        let err = validate(&dir).expect_err("损坏配置必须失败");
        assert!(err.contains("a.vox.meta"), "错误应包含 meta 路径：{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn non_positive_or_non_finite_voxel_size_fails() {
        for (name, size) in [("zero", 0.0f32), ("negative", -1.0), ("nan", f32::NAN)] {
            let dir = scratch(name);
            std::fs::write(dir.join("a.vox.meta"), meta_bytes(size)).unwrap();
            let err = validate(&dir).expect_err("非正/非有限单位必须失败");
            assert!(err.contains("a.vox"), "错误应包含资产路径：{err}");
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn wrong_loader_fails() {
        let dir = scratch("loader");
        let meta = AssetMeta::<MetaProbe, ()>::new(AssetAction::Load {
            loader: "some::other::Loader".to_string(),
            settings: VoxLoaderSettings {
                voxel_size: 0.1,
                ..Default::default()
            },
        });
        std::fs::write(dir.join("a.vox.meta"), meta.serialize()).unwrap();
        let err = validate(&dir).expect_err("loader 不符必须失败");
        assert!(err.contains("a.vox.meta"), "错误应包含 meta 路径：{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

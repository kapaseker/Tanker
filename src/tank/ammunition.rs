//! 模型同目录的弹药性能；启动读取，进程内保持不变。
use bevy::prelude::*;
use serde::Deserialize;
use std::path::Path;

/// 弹药性能；剩余装填时间属于 Gun，不写回配置。
#[derive(Resource, Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub(crate) struct AmmunitionSpec {
    /// 炮口速度，米/秒；当前弹道不模拟减速。
    pub(crate) muzzle_velocity: f32,
    /// 发射频率，发/分钟。
    pub(crate) rate_of_fire: f32,
    /// 弹体主体的扫掠盒。
    pub(crate) collision: BulletCollision,
}
/// 弹丸局部空间的主体包围盒，尺寸与偏移单位为米。
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub(crate) struct BulletCollision {
    /// 沿飞行方向的长度、高度和宽度。
    pub(crate) size: [f32; 3],
    /// 盒中心从模型根沿前方的偏移。
    pub(crate) center_forward_offset: f32,
}
impl AmmunitionSpec {
    /// 每发后的冷却秒数；加载时确保计算结果有效。
    pub(crate) fn cooldown(&self) -> f32 {
        60.0 / self.rate_of_fire
    }

    /// 从弹丸模型同名的 JSONC 文件读取配置，root 为实际 assets 根目录。
    pub(crate) fn load(root: &Path, model: &str) -> Result<Self, String> {
        let path = root.join(model).with_extension("jsonc");
        let bytes = std::fs::read(&path)
            .map_err(|e| format!("弹药配置读取失败 {}：{e}", path.display()))?;
        Self::parse(&bytes).map_err(|e| format!("弹药配置非法 {}：{e}", path.display()))
    }
    /// 注释过滤后仍由严格 JSON 解析器校验字段及数值。
    fn parse(bytes: &[u8]) -> Result<Self, String> {
        let settings = json_comments::CommentSettings::c_style();
        let spec: Self =
            serde_json::from_reader(json_comments::StripComments::with_settings(settings, bytes))
                .map_err(|e| e.to_string())?;
        if [spec.muzzle_velocity, spec.rate_of_fire, spec.cooldown()]
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.0)
            || spec.collision.size.iter().any(|v| !v.is_finite() || *v <= 0.0)
            || !spec.collision.center_forward_offset.is_finite()
        {
            return Err("初速、射速、冷却和碰撞尺寸必须为有限正数，碰撞偏移必须有限".into());
        }
        Ok(spec)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_configuration_and_comments() {
        let spec =
            AmmunitionSpec::load(Path::new("assets"), super::super::visual::PLAYER_BULLET).unwrap();
        assert_eq!(spec.muzzle_velocity, 1640.0);
        assert_eq!(spec.cooldown(), 7.5);
        let spec = AmmunitionSpec::parse(
            b"{/*block*/\"muzzle_velocity\":600, //line\n\"rate_of_fire\":30,\"collision\":{\"size\":[0.72,0.04,0.04],\"center_forward_offset\":0.16}}",
        )
        .unwrap();
        assert_eq!(spec.muzzle_velocity, 600.0);
        assert_eq!(spec.cooldown(), 2.0);
    }
    /// 目录可随模型移动；临时资源根不会回退到真实资产默认参数。
    #[test]
    fn reads_configuration_next_to_model_under_alternate_root() {
        let root = std::env::temp_dir().join(format!("tanker-ammunition-{}", std::process::id()));
        let dir = root.join("custom/ammo");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("shell.jsonc"), br#"{"muzzle_velocity":600,"rate_of_fire":30,"collision":{"size":[0.72,0.04,0.04],"center_forward_offset":0.16}}"#).unwrap();
        let spec = AmmunitionSpec::load(&root, "custom/ammo/shell.vox").unwrap();
        assert_eq!(spec.muzzle_velocity, 600.0);
        assert_eq!(spec.cooldown(), 2.0);
        std::fs::remove_file(dir.join("shell.jsonc")).unwrap();
        std::fs::remove_dir(&dir).unwrap();
        std::fs::remove_dir(root.join("custom")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
    #[test]
    fn invalid_configurations_are_rejected() {
        for collision in [
            r#"{"size":[0,0.04,0.04],"center_forward_offset":0.16}"#,
            r#"{"size":[0.72,0.04,0.04],"center_forward_offset":1e99}"#,
            r#"{"size":[0.72,0.04,0.04],"center_forward_offset":0.16,"extra":1}"#,
        ] {
            let input = format!(r#"{{"muzzle_velocity":1,"rate_of_fire":12,"collision":{collision}}}"#);
            assert!(AmmunitionSpec::parse(input.as_bytes()).is_err(), "{input}");
        }
        for input in [
            "{}",
            "{",
            r#"{"muzzle_velocity":1,"rate_of_fire":0}"#,
            r#"{"muzzle_velocity":-1,"rate_of_fire":12}"#,
            r#"{"muzzle_velocity":1e99,"rate_of_fire":12}"#,
            r#"{"muzzle_velocity":1,"rate_of_fire":1e-44}"#,
            r#"{"muzzle_velocity":1,"rate_of_fire":12,"extra":1}"#,
            r#"{"muzzle_velocity":1,"rate_of_fire":12,"rate_of_fire":20}"#,
            r#"{"muzzle_velocity":"1","rate_of_fire":12}"#,
            r#"{"muzzle_velocity":1,"rate_of_fire":12,}"#,
        ] {
            assert!(AmmunitionSpec::parse(input.as_bytes()).is_err(), "{input}");
        }
        assert!(
            AmmunitionSpec::load(Path::new("missing-assets"), "sample/bullet.vox")
                .unwrap_err()
                .contains("bullet.jsonc")
        );
    }
}

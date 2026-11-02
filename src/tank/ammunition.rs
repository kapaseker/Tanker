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
}
impl AmmunitionSpec {
    /// 每发后的冷却秒数；加载时确保计算结果有效。
    pub(crate) fn cooldown(&self) -> f32 {
        60.0 / self.rate_of_fire
    }

    /// 从弹丸模型所在目录读取固定命名配置，root 为实际 assets 根目录。
    pub(crate) fn load(root: &Path, model: &str) -> Result<Self, String> {
        let path = root.join(model).with_file_name("ammunition.jsonc");
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
        {
            return Err("初速、射速与推导冷却必须为有限正数".into());
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
        assert_eq!(spec.cooldown(), 5.0);
        let spec = AmmunitionSpec::parse(
            b"{/*block*/\"muzzle_velocity\":600, //line\n\"rate_of_fire\":30}",
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
        std::fs::write(dir.join("ammunition.jsonc"), br#"{"muzzle_velocity":600,"rate_of_fire":30}"#).unwrap();
        let spec = AmmunitionSpec::load(&root, "custom/ammo/bullet.vox").unwrap();
        assert_eq!(spec.muzzle_velocity, 600.0);
        assert_eq!(spec.cooldown(), 2.0);
        std::fs::remove_file(dir.join("ammunition.jsonc")).unwrap();
        std::fs::remove_dir(&dir).unwrap();
        std::fs::remove_dir(root.join("custom")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
    #[test]
    fn invalid_configurations_are_rejected() {
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
                .contains("ammunition.jsonc")
        );
    }
}

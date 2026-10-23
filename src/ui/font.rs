//! 全局 UI 字体：启动时加载 MiSans 可变字体并保留句柄。
//!
//! 首页菜单、键盘提示和版本信息共用字体；标题由相同字体的 HTML 设计稿导出。
//! 句柄保存在 [`UiFont`] 中，避免资产被释放。
//! 许可：MiSans 由小米提供，免费商用但要求在软件中注明使用了 MiSans 字体；
//! 发布前需在界面中加入该声明，并随包附上官方许可协议。

use bevy::{asset::LoadState, prelude::*};

/// MiSans 可变字体路径（相对 `assets/`）；`wght` 轴范围 150–700。
const UI_FONT_PATH: &str = "fonts/misans/MiSansVF.ttf";

/// 全局 UI 字体句柄；首页通过 `Res<UiFont>` 取用。
#[derive(Resource)]
pub(crate) struct UiFont(pub(crate) Handle<Font>);

/// 注册字体加载与状态汇报系统。
pub(crate) struct UiFontPlugin;

impl Plugin for UiFontPlugin {
    fn build(&self, app: &mut App) {
        // 构建期直接发起加载并插入资源：初始 OnEnter(Index) 发生在启动期
        // StateTransition（早于 Startup），spawn_index 依赖本资源必须提前就位。
        let handle = app.world().resource::<AssetServer>().load(UI_FONT_PATH);
        app.insert_resource(UiFont(handle))
            .add_systems(Update, report_ui_font);
    }
}

/// 成功与失败各汇报一次；失败必须显式报错，避免等到首个界面才发现缺字体。
fn report_ui_font(font: Res<UiFont>, assets: Res<AssetServer>, mut reported: Local<bool>) {
    if *reported {
        return;
    }
    match assets.get_load_state(&font.0) {
        Some(LoadState::Loaded) => {
            info!("UI 字体加载完成：assets/{UI_FONT_PATH}");
            *reported = true;
        }
        Some(LoadState::Failed(error)) => {
            error!("UI 字体加载失败：{error}");
            *reported = true;
        }
        _ => {}
    }
}

/// 字体是否已加载完成；离屏验证据此等待，确保日志与资源状态真实可见。
pub(crate) fn assets_ready(world: &World) -> bool {
    let Some(font) = world.get_resource::<UiFont>() else {
        return false;
    };
    world
        .get_resource::<AssetServer>()
        .is_some_and(|assets| assets.is_loaded_with_dependencies(&font.0))
}

//! Tanker —— 基于 Bevy 的 3D voxel 游戏工程。
//!
//! VOX 地板与坦克场景，方向键驾驶，空格开炮，相机从车尾上方跟随坦克位置与朝向。
//! `assets/pisa_diffuse.ktx2` 与 `pisa_specular.ktx2` 仍取自 bevy_vox_scene 官方仓库示例资源（环境贴图）。

mod app;
mod gameplay;
mod pages;
mod tank;
mod terrain;
mod ui;
mod validation;

/// 先校验命令行，再创建渲染器；参数错误不会意外打开窗口。
fn main() -> std::process::ExitCode {
    let result =
        validation::Options::parse(std::env::args_os().skip(1).collect()).and_then(app::run);
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("验证失败：{error}");
            std::process::ExitCode::FAILURE
        }
    }
}

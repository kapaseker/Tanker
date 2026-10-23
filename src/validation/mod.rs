//! 无窗口验证工具：共享状态、运行入口与断言，业务系统不依赖本模块。

mod capture;
mod cli;
mod offscreen;
mod scenarios;

pub(crate) use cli::Options;
pub(crate) use offscreen::{configure, run};

use bevy::prelude::*;
use scenarios::Scenario;
use std::time::Duration;

/// 所有离屏场景共用的随机种子，仅改变验证进程的敌人布局。
pub(super) const SEED: u64 = 42;
/// 图片尺寸固定，避免窗口大小和桌面缩放参与画面对比。
const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;
/// 精确的同一 Duration 同时用于可变步长时钟与固定物理时钟。
const STEP: Duration = Duration::from_nanos(16_666_667);
/// 使用墙钟限制加载、GPU 等待与模拟总耗时，防止无人值守进程残留。
const TIMEOUT: Duration = Duration::from_secs(120);

/// 最终合成相机的离屏目标；内容纹理与此输出纹理始终分离。
#[derive(Resource)]
struct Target(Handle<Image>);

/// 下一次更新要注入的模拟帧；None 用于加载、预热和截图等待。
#[derive(Resource)]
struct InputFrame(Scenario, Option<u32>);

/// 下一帧菜单鼠标按下状态，与窗口输入清理时序一致。
#[derive(Resource, Default)]
struct MenuMouse(bool);

/// wheels 测试显式驱动的敌人；正常游戏不创建，不改变玩家标记。
#[derive(Resource)]
struct WheelTarget(Entity);

/// 每帧记录真实游戏事件与子弹轨迹，防止只看最终状态造成假通过。
#[derive(Resource, Default)]
struct Evidence {
    errors: Vec<String>,
    destroyed: usize,
    bullet_seen: bool,
    bullet_moved: bool,
    last_bullet: Option<Vec3>,
    explosion_seen: bool,
}

/// 截图 observer 的完成结果；必须等编码与落盘完成后才能推进场景。
#[derive(Resource, Default)]
struct CaptureResult(Option<Result<(), String>>);

/// 将断言失败转换为可报告的运行错误，不依赖 panic 作为正常失败路径。
fn ensure(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

/// 行为断言失败仍收集后续关键帧，最终统一失败退出，保留完整诊断证据。
fn record_check(report: &mut String, failures: &mut Vec<String>, condition: bool, message: &str) {
    let status = if condition {
        "通过"
    } else {
        failures.push(message.into());
        "失败"
    };
    report.push_str(&format!("断言{status}：{message}\n"));
}

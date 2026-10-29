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
/// 180 秒：detail 预设（9 目录项 × 进入/返回共 23 次转场）在独占运行时需要
/// 约 112 秒（9177 模拟帧 ≈ 96.4s + 截图等待），120 秒预算在并发抖动下反复
/// 误杀已通过全部断言的运行；保留 60 秒余量用于隔离真实挂起与偶发资源竞争。
const TIMEOUT: Duration = Duration::from_secs(180);
/// 候选光照实验（仅 wheels 接受）的固定光源参数；报告与实际光源共用，
/// 避免两处手写漂移。方向以世界空间向量记录，归一化后作为照射方向。
const CANDIDATE_LIGHT_ILLUMINANCE: f32 = 3000.0;
const CANDIDATE_LIGHT_DIRECTION: Vec3 = Vec3::new(0.55, -0.35, -0.76);

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

/// 故障场景只允许这一条明确指定的缺失资源；其它加载错误仍立即失败。
#[derive(Resource)]
struct ExpectedAssetFailure {
    path: String,
    seen: bool,
}

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

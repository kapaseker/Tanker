//! 经真实首页菜单进入田园页，验证视觉内容、飞行和反复往返隔离。
use super::*;
use crate::{
    app::{
        state::GameState,
        transition::{Fade, Phase},
    },
    pages::pastoral::{PreviewCamera, PreviewEntity},
};

/// 限时等候实际状态与揭幕，不绕过 CPU/GPU 就绪检查。
fn settled(app: &mut App, target: GameState) -> Result<(), String> {
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(65) {
        update(app, STEP, None)?;
        if *app.world().resource::<State<GameState>>().get() == target
            && app.world().resource::<Fade>().accepts_input()
        {
            return Ok(());
        }
    }
    Err(format!(
        "田园页切换超时：{}",
        crate::app::transition::diagnostics(app.world_mut())
    ))
}
/// 同一输入注入路径产生按下与释放。
fn key(app: &mut App, frame: u32) -> Result<(), String> {
    update(app, STEP, Some(frame))?;
    update(app, STEP, None)
}
pub(super) fn run(app: &mut App, options: &Options, report: &mut String) -> Result<(), String> {
    wait_render(app, false)?;
    settled(app, GameState::Index)?;
    capture(app, &options.output.join("index.png"))?;
    let ambient = app.world().resource::<GlobalAmbientLight>().brightness;
    for _ in 0..3 {
        key(app, 7)?;
    }
    ensure(
        app.world()
            .resource::<crate::pages::index::MenuInput>()
            .focus
            == Some(2),
        "第三项键盘导航失败",
    )?;
    for cycle in 0..2 {
        key(app, 3)?;
        settled(app, GameState::TestScene)?;
        ensure(
            crate::pages::pastoral::content_ready(app.world_mut()),
            "田园几何或相机未就绪",
        )?;
        ensure(
            app.world_mut()
                .query_filtered::<(), Or<(
                    With<crate::gameplay::player::Tank>,
                    With<crate::gameplay::enemy::EnemyTank>,
                )>>()
                .iter(app.world())
                .count()
                == 0,
            "测试场景出现坦克",
        )?;
        ensure(
            app.world_mut()
                .query_filtered::<(), With<crate::gameplay::hud::HudRoot>>()
                .iter(app.world())
                .count()
                == 0,
            "测试场景出现战斗 HUD",
        )?;
        ensure(
            app.world_mut()
                .query_filtered::<(), With<crate::pages::index::IndexRoot>>()
                .iter(app.world())
                .count()
                == 0,
            "首页未清理",
        )?;
        if cycle == 0 {
            capture(app, &options.image_path(0))?;
            let camera = app
                .world_mut()
                .query_filtered::<Entity, With<PreviewCamera>>()
                .single(app.world())
                .map_err(|e| e.to_string())?;
            let before = app.world().get::<Transform>(camera).unwrap().translation;
            for frame in 20..=80 {
                update(app, STEP, Some(frame))?;
            }
            update(app, STEP, None)?;
            ensure(
                app.world()
                    .get::<Transform>(camera)
                    .unwrap()
                    .translation
                    .distance(before)
                    > 10.0,
                "自由相机未按输入前进",
            )?;
            capture(app, &options.output.join("flight.png"))?;
            *app.world_mut().get_mut::<Transform>(camera).unwrap() =
                Transform::from_xyz(15.0, 13.0, 24.0)
                    .looking_at(Vec3::new(-12.0, 3.0, -3.0), Vec3::Y);
            capture(app, &options.output.join("bridge.png"))?;
        }
        key(app, 5)?;
        settled(app, GameState::Index)?;
        ensure(
            app.world_mut()
                .query_filtered::<(), With<PreviewEntity>>()
                .iter(app.world())
                .count()
                == 0,
            "预览实体残留",
        )?;
        ensure(
            app.world_mut()
                .query_filtered::<(), With<DirectionalLight>>()
                .iter(app.world())
                .count()
                == 0,
            "太阳光残留",
        )?;
        ensure(
            app.world().resource::<GlobalAmbientLight>().brightness == ambient,
            "共享环境光被污染",
        )?;
        ensure(
            app.world()
                .resource::<crate::pages::index::MenuInput>()
                .focus
                == Some(2),
            "返回焦点未恢复",
        )?;
        ensure(
            app.world().resource::<Fade>().phase == Phase::Idle,
            "返回未完成揭幕",
        )?;
    }
    report.push_str("断言通过：首页第三项进入、无坦克、15种材质批次、W前进、两次往返清理、太阳清理、共享环境光不变、返回焦点；全景/飞行/石桥截图已保存。\n");
    Ok(())
}

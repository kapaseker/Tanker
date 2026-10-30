//! 首页交互、双向过渡与对局清理验证。

use super::*;
use bevy::world_serialization::WorldInstance;

/// 菜单回归共用真实输入和渲染链路；全黑交接帧单独验收。
pub(super) fn run_index_scenario(
    app: &mut App,
    options: &Options,
    report: &mut String,
) -> Result<(), String> {
    use crate::app::transition::{Fade, Phase};
    wait_render(app, false)?;
    while !crate::app::transition::content_ready(app.world_mut()) {
        update(app, Duration::ZERO, None)?;
    }
    ensure(
        app.world()
            .resource::<crate::pages::index::MenuInput>()
            .focus
            .is_none(),
        "初始首页不应选中菜单项",
    )?;
    let orders: Vec<_> = app
        .world_mut()
        .query::<&crate::pages::index::MenuItem>()
        .iter(app.world())
        .map(|item| item.order)
        .collect();
    ensure(
        orders.len() == 3 && orders.contains(&0) && orders.contains(&1) && orders.contains(&2),
        "首页必须含三项可交互菜单",
    )?;
    capture(app, &options.image_path(0))?;
    report.push_str("初始首页：无焦点；三项菜单；已保存 frame-000000.png\n");
    // 保留两个相隔四分之一周期的漂浮关键帧，便于检查实际运动幅度。
    for frame in 1..=210 {
        update(app, STEP, None)?;
        if frame == 105 || frame == 210 {
            capture(app, &options.output.join(format!("drift-{frame:03}.png")))?;
        }
    }
    // UI 遮罩受控对照：移除后必须恢复，不能据历史失败认定引擎永久黑屏。
    let camera = app
        .world_mut()
        .query_filtered::<Entity, With<crate::pages::index::IndexCamera>>()
        .single(app.world())
        .map_err(|e| e.to_string())?;
    let overlay = app
        .world_mut()
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                position_type: PositionType::Absolute,
                ..default()
            },
            UiTargetCamera(camera),
            GlobalZIndex(100),
            BackgroundColor(Color::BLACK.with_alpha(0.5)),
        ))
        .id();
    capture(app, &options.output.join("control-ui-overlay.png"))?;
    app.world_mut().despawn(overlay);
    capture(app, &options.output.join("control-ui-removed.png"))?;
    report.push_str("受控对照：全屏 UI 遮罩添加与移除均正常渲染；无永久黑屏\n");
    for i in 0..3 {
        let l = *app.world().resource::<crate::pages::index::Layout>();
        app.world_mut()
            .resource_mut::<crate::pages::index::MenuInput>()
            .pointer = Some(Vec2::new(
            l.hero.x + 50.0,
            l.menu_y + 20.0 + l.row_height * i as f32,
        ));
        for _ in 0..20 {
            update(app, STEP, None)?;
        }
        ensure(
            app.world()
                .resource::<crate::pages::index::MenuInput>()
                .hover
                == Some(i),
            "鼠标未命中菜单项",
        )?;
        capture(app, &options.output.join(format!("hover-{}.png", i + 1)))?;
    }
    app.world_mut()
        .resource_mut::<crate::pages::index::MenuInput>()
        .pointer = None;
    for _ in 0..20 {
        update(app, STEP, None)?;
    }
    // Tab 到第一项、再到 Gallery；业务切换由 gallery 离屏场景覆盖。
    for key_frame in [7, 7] {
        update(app, STEP, Some(key_frame))?;
        update(app, STEP, None)?;
    }
    ensure(
        app.world().resource::<Fade>().phase == Phase::Idle,
        "仅导航不得发起切换",
    )?;
    update(app, STEP, Some(8))?;
    update(app, STEP, None)?;
    ensure(
        app.world()
            .resource::<crate::pages::index::MenuInput>()
            .focus
            == Some(0),
        "Shift+Tab 应返回第一项",
    )?;
    for _ in 0..20 {
        update(app, STEP, None)?;
    }
    capture(app, &options.output.join("keyboard-focus.png"))?;
    // 退出确认弹框：Esc 打开（初始选中确认）→ ← 切取消 → Esc 取消关闭。
    let dialog_count = |app: &mut App| {
        app.world_mut()
            .query::<&crate::pages::index::ExitDialog>()
            .iter(app.world())
            .count()
    };
    update(app, STEP, Some(5))?;
    update(app, STEP, None)?;
    ensure(dialog_count(app) == 1, "Esc 未打开退出确认弹框")?;
    ensure(
        app.world_mut()
            .query::<&crate::pages::index::ExitDialog>()
            .single(app.world())
            .unwrap()
            .selection
            == crate::pages::index::ExitDialogSelection::Confirm,
        "弹框初始应选中确认",
    )?;
    for _ in 0..12 {
        update(app, STEP, None)?;
    }
    capture(app, &options.output.join("exit-dialog.png"))?;
    update(app, STEP, Some(15))?;
    for _ in 0..14 {
        update(app, STEP, None)?;
    }
    ensure(
        app.world_mut()
            .query::<&crate::pages::index::ExitDialog>()
            .single(app.world())
            .unwrap()
            .selection
            == crate::pages::index::ExitDialogSelection::Cancel,
        "← 未切换选中到取消",
    )?;
    capture(app, &options.output.join("exit-dialog-cancel.png"))?;
    update(app, STEP, Some(5))?;
    update(app, STEP, None)?;
    for _ in 0..12 {
        update(app, STEP, None)?;
    }
    ensure(dialog_count(app) == 0, "Esc 未关闭弹框")?;
    ensure(
        app.world().resource::<Fade>().phase == Phase::Idle,
        "取消弹框不应触发页面过渡",
    )?;
    report.push_str("退出确认弹框：打开/切换选中/取消关闭断言通过；已保存 exit-dialog*.png\n");
    // 取消弹框后焦点归还电源框（设计行为，见 close_exit_dialog）：此时直接 Enter
    // 会重新打开弹框而非确认菜单。模拟真实用户连按三次 ↓（项 0→1→2→0）把焦点导航
    // 回菜单第一项并清除电源框焦点，后续 Enter 才走菜单确认链路。
    for _ in 0..3 {
        update(app, STEP, Some(1))?;
        update(app, STEP, None)?;
        update(app, STEP, None)?;
    }
    ensure(
        app.world().resource::<crate::pages::index::MenuInput>().focus == Some(0),
        "方向键未把焦点导航回菜单第一项",
    )?;
    for (width, height) in [
        (1920, 1080),
        (901, 720),
        (900, 720),
        (641, 720),
        (640, 720),
        (480, 800),
        (1280, 720),
    ] {
        let size = bevy::render::render_resource::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let handles = [
            app.world().resource::<Target>().0.clone(),
            app.world()
                .resource::<crate::app::transition::SceneTarget>()
                .0
                .clone(),
        ];
        for handle in handles {
            app.world_mut()
                .resource_mut::<Assets<Image>>()
                .get_mut(&handle)
                .unwrap()
                .resize(size);
        }
        capture(
            app,
            &options.output.join(format!("layout-{width}x{height}.png")),
        )?;
        report.push_str(&format!("整体缩放布局：{width}×{height}\n"));
    }
    let mut frame = 0u32;
    let mut session_instances = Vec::new();
    for cycle in 1..=3 {
        if cycle > 1 {
            update(app, STEP, Some(1))?;
            update(app, STEP, None)?;
        }
        for (direction, key_frame, target) in [
            ("enter", 3, crate::app::state::GameState::InGame),
            ("return", 5, crate::app::state::GameState::Index),
        ] {
            update(app, STEP, Some(key_frame))?;
            let mut saved = [false; 3];
            let mut iterations = 0;
            loop {
                let phase = app.world().resource::<Fade>().phase;
                let alpha = app.world().resource::<Fade>().alpha;
                // 加载期间也推进正常墙钟步长，验证游戏自身冻结物理，而非靠测试停钟。
                update(app, STEP, None)?;
                frame += 1;
                let new_phase = app.world().resource::<Fade>().phase;
                let new_alpha = app.world().resource::<Fade>().alpha;
                let slot = if phase == Phase::Out && (0.45..=0.55).contains(&alpha) {
                    Some(0)
                } else if alpha == 1.0 && phase == Phase::Waiting {
                    Some(1)
                } else if phase == Phase::In && (0.45..=0.55).contains(&alpha) {
                    Some(2)
                } else {
                    None
                };
                if cycle == 1 {
                    if let Some(slot) = slot.filter(|&slot| !saved[slot]) {
                        saved[slot] = true;
                        let path = options.image_path(frame);
                        // 首页↔对局仍为整体黑场（FullBlack）：黑场帧全画面必须为黑。
                        capture_checked(
                            app,
                            &path,
                            if slot == 1 {
                                PixelCheck::FullBlack
                            } else {
                                PixelCheck::Content
                            },
                        )?;
                        report.push_str(&format!(
                            "{}\n",
                            crate::app::transition::diagnostics(app.world_mut())
                        ));
                        report.push_str(&format!("第{cycle}轮 {direction}：阶段={phase:?} alpha={new_alpha:.4} frame={frame} 图片={}\n",path.display()));
                    }
                }
                iterations += 1;
                ensure(iterations < 20000, "页面过渡未能完成")?;
                if new_phase == Phase::Idle && new_alpha == 0.0 {
                    break;
                }
            }
            ensure(
                *app.world()
                    .resource::<State<crate::app::state::GameState>>()
                    .get()
                    == target,
                "过渡未到达目标页面",
            )?;
            if cycle == 1 {
                ensure(saved.iter().all(|saved| *saved), "缺少双向过渡关键阶段截图")?;
            }
            for _ in 0..120 {
                update(app, STEP, None)?;
                frame += 1;
            }
            ensure(
                app.world().resource::<Fade>().phase == Phase::Idle
                    && app.world().resource::<Fade>().alpha == 0.0,
                "过渡结束后黑幕重新出现",
            )?;
            if target == crate::app::state::GameState::InGame {
                validate_scene(app.world_mut())?;
                session_instances = app
                    .world_mut()
                    .query::<&WorldInstance>()
                    .iter(app.world())
                    .map(|instance| **instance)
                    .collect();
                ensure(
                    app.world_mut()
                        .query_filtered::<(), With<crate::pages::index::IndexRoot>>()
                        .iter(app.world())
                        .count()
                        == 0,
                    "游戏中残留首页",
                )?;
                let follows = app
                    .world_mut()
                    .query_filtered::<(), With<crate::gameplay::camera::FollowCamera>>()
                    .iter(app.world())
                    .count();
                ensure(follows == 1, "游戏相机重复生成")?;
                report.push_str(&format!(
                    "第{cycle}轮进入：资源、实例与 GPU 就绪；玩家、敌人、地形与相机断言通过\n"
                ));
            } else {
                let world = app.world_mut();
                ensure(
                    world
                        .query_filtered::<(), With<crate::gameplay::session::SessionEntity>>()
                        .iter(world)
                        .count()
                        == 0,
                    "返回首页后残留对局实体",
                )?;
                ensure(
                    world.query::<&WorldAssetRoot>().iter(world).count() == 0,
                    "返回首页后残留场景实例根",
                )?;
                ensure(
                    world
                        .query_filtered::<(), With<crate::pages::index::IndexCamera>>()
                        .iter(world)
                        .count()
                        == 1,
                    "首页相机重复生成",
                )?;
                ensure(
                    session_instances.iter().all(|&instance| {
                        !world
                            .resource::<WorldInstanceSpawner>()
                            .instance_is_ready(instance)
                    }),
                    "返回首页后场景实例未注销",
                )?;
                ensure(
                    !world.contains_resource::<crate::terrain::TerrainReady>(),
                    "地形生成标志未清除",
                )?;
                report.push_str(&format!(
                    "第{cycle}轮返回：实体与场景根清零；首页相机唯一；生成标志已重置\n"
                ));
            }
            capture(app, &options.image_path(frame))?;
            report.push_str(&format!(
                "{}\n",
                crate::app::transition::diagnostics(app.world_mut())
            ));
        }
    }
    report.push_str("三轮双向过渡全部完成，持续120帧未重建黑幕\n");
    Ok(())
}

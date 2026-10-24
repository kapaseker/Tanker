//! 画廊预览、滚动、自转与反复往返验证。

use super::*;
use bevy::world_serialization::WorldInstance;

/// 画廊通过真实首页入口进入，注入输入并检查全部模型与反复往返的清理。
pub(super) fn run_gallery_scenario(
    app: &mut App,
    options: &Options,
    report: &mut String,
) -> Result<(), String> {
    use crate::app::{
        state::GameState,
        transition::{Fade, Phase},
    };
    wait_render(app, false)?;
    while !crate::app::transition::content_ready(app.world_mut()) {
        update(app, Duration::ZERO, None)?;
    }
    capture(app, &options.image_path(0))?;
    for cycle in 1..=3 {
        if cycle == 1 {
            for key in [7, 7] {
                update(app, STEP, Some(key))?;
                update(app, STEP, None)?;
            }
        }
        if cycle == 2 {
            let layout = *app.world().resource::<crate::pages::index::Layout>();
            app.world_mut()
                .resource_mut::<crate::pages::index::MenuInput>()
                .pointer = Some(Vec2::new(
                layout.hero.x + 40.0,
                layout.menu_y + layout.row_height + 20.0,
            ));
            app.world_mut().resource_mut::<MenuMouse>().0 = true;
            update(app, STEP, None)?;
            app.world_mut().resource_mut::<MenuMouse>().0 = false;
            update(app, STEP, None)?;
            app.world_mut()
                .resource_mut::<crate::pages::index::MenuInput>()
                .pointer = None;
        } else {
            update(app, STEP, Some(3))?;
        }
        finish_gallery_transition(app, options, cycle, "enter", GameState::Gallery)?;
        report.push_str(&format!("第{cycle}次进入画廊：过渡完成，alpha=0\n"));
        report.push_str(&crate::pages::gallery::validate(app.world())?);
        ensure(
            app.world_mut()
                .query_filtered::<(), With<crate::pages::index::IndexRoot>>()
                .iter(app.world())
                .count()
                == 0,
            "进入画廊未清理首页",
        )?;
        ensure(
            crate::pages::gallery::view_mut(app.world_mut()).scroll == 0.0,
            "进入画廊必须从顶部开始",
        )?;
        let images: Vec<_> = app
            .world()
            .resource::<crate::pages::gallery::Previews>()
            .items
            .iter()
            .map(|p| p.image.id())
            .collect();
        let instances: Vec<_> = app
            .world_mut()
            .query::<&WorldInstance>()
            .iter(app.world())
            .map(|i| **i)
            .collect();
        if cycle == 1 {
            capture(app, &options.output.join("gallery-top.png"))?;
            crate::pages::gallery::view_mut(app.world_mut()).pointer =
                Some(Vec2::new(120.0, 250.0));
            for _ in 0..20 {
                update(app, STEP, None)?;
            }
            capture(app, &options.output.join("gallery-hover.png"))?;
            crate::pages::gallery::view_mut(app.world_mut()).pointer = None;
            // 真实轮滚消息与键盘共用滚动状态，滚动不得改变标题栏。
            app.world_mut()
                .write_message(bevy::input::mouse::MouseWheel {
                    unit: bevy::input::mouse::MouseScrollUnit::Line,
                    x: 0.0,
                    y: -2.0,
                    window: Entity::PLACEHOLDER,
                    phase: bevy::input::touch::TouchPhase::Moved,
                });
            update(app, STEP, None)?;
            ensure(
                crate::pages::gallery::view_mut(app.world_mut()).scroll == 128.0,
                "滚轮步长不符",
            )?;
            for section in 1..=3 {
                update(app, STEP, Some(13))?;
                update(app, STEP, None)?;
                capture(
                    app,
                    &options.output.join(format!("gallery-scroll-{section}.png")),
                )?;
            }
            update(app, STEP, Some(11))?;
            update(app, STEP, None)?;
            capture(app, &options.output.join("gallery-bottom.png"))?;
            let rotations: Vec<_> = app
                .world()
                .resource::<crate::pages::gallery::Previews>()
                .items
                .iter()
                .map(|p| app.world().get::<Transform>(p.root).unwrap().rotation)
                .collect();
            for _ in 0..120 {
                update(app, STEP, None)?;
            }
            ensure(
                app.world()
                    .resource::<crate::pages::gallery::Previews>()
                    .items
                    .iter()
                    .zip(rotations)
                    .all(|(p, before)| {
                        app.world()
                            .get::<Transform>(p.root)
                            .unwrap()
                            .rotation
                            .angle_between(before)
                            > 0.5
                    }),
                "模型未按时间自转",
            )?;
            capture(app, &options.output.join("gallery-rotation.png"))?;
            // 非交互卡片点击和 Enter 不打开页面，也不停止自转。
            crate::pages::gallery::view_mut(app.world_mut()).pointer =
                Some(Vec2::new(120.0, 430.0));
            app.world_mut().resource_mut::<MenuMouse>().0 = true;
            update(app, STEP, None)?;
            app.world_mut().resource_mut::<MenuMouse>().0 = false;
            update(app, STEP, Some(3))?;
            update(app, STEP, None)?;
            ensure(
                app.world().resource::<Fade>().phase == Phase::Idle,
                "卡片确认不应发起页面跳转",
            )?;
            crate::pages::gallery::view_mut(app.world_mut()).pointer = None;
            for (width, height) in [(640, 720), (1920, 720), (1280, 720)] {
                let size = bevy::render::render_resource::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                };
                for handle in [
                    app.world().resource::<Target>().0.clone(),
                    app.world()
                        .resource::<crate::app::transition::SceneTarget>()
                        .0
                        .clone(),
                ] {
                    app.world_mut()
                        .resource_mut::<Assets<Image>>()
                        .get_mut(&handle)
                        .unwrap()
                        .resize(size);
                }
                update(app, STEP, Some(12))?;
                update(app, STEP, None)?;
                capture(
                    app,
                    &options
                        .output
                        .join(format!("gallery-layout-{width}x{height}.png")),
                )?;
            }
            report.push_str(
                "滚轮、PageDown、Home/End、悬停、自转与三种窗口尺寸已验证；卡片确认无动作\n",
            );
        }
        if cycle == 2 {
            crate::pages::gallery::view_mut(app.world_mut()).pointer = Some(Vec2::new(90.0, 40.0));
            // 悬停 220ms 过渡完成后截图，验证返回框薄荷高亮与键帽深色描边。
            for _ in 0..16 {
                update(app, STEP, None)?;
            }
            capture(app, &options.output.join("gallery-back-hover.png"))?;
            report.push_str("返回框悬停高亮与键帽描边已截图\n");
            app.world_mut().resource_mut::<MenuMouse>().0 = true;
            update(app, STEP, None)?;
            app.world_mut().resource_mut::<MenuMouse>().0 = false;
            update(app, STEP, None)?;
        } else if cycle == 3 {
            update(app, STEP, Some(7))?;
            update(app, STEP, None)?;
            update(app, STEP, Some(3))?;
        } else {
            update(app, STEP, Some(5))?;
        }
        finish_gallery_transition(app, options, cycle, "return", GameState::Index)?;
        for _ in 0..3 {
            update(app, Duration::ZERO, None)?;
        }
        ensure(
            !app.world()
                .contains_resource::<crate::pages::gallery::Previews>(),
            "退出后残留预览资源",
        )?;
        ensure(
            app.world_mut()
                .query_filtered::<(), With<crate::pages::gallery::GalleryEntity>>()
                .iter(app.world())
                .count()
                == 0,
            "退出后残留画廊实体",
        )?;
        ensure(
            images
                .iter()
                .all(|id| app.world().resource::<Assets<Image>>().get(*id).is_none()),
            "退出后残留专用纹理",
        )?;
        ensure(
            instances.iter().all(|id| {
                !app.world()
                    .resource::<WorldInstanceSpawner>()
                    .instance_is_ready(*id)
            }),
            "退出后残留场景实例",
        )?;
        ensure(
            app.world()
                .resource::<crate::pages::index::MenuInput>()
                .focus
                == Some(1),
            "返回未恢复画廊入口焦点",
        )?;
        report.push_str(&format!(
            "第{cycle}次返回：首页焦点恢复；预览实体、场景实例和纹理全部清理\n"
        ));
    }
    capture(app, &options.output.join("gallery-return-index.png"))?;
    Ok(())
}

/// 等待真实过渡，首次往返保留渐出、黑场、渐入关键帧。
pub(super) fn finish_gallery_transition(
    app: &mut App,
    options: &Options,
    cycle: u32,
    direction: &str,
    target: crate::app::state::GameState,
) -> Result<(), String> {
    use crate::app::transition::{Fade, Phase};
    let mut saved = [false; 3];
    for _ in 0..2000 {
        update(app, STEP, None)?;
        let fade = app.world().resource::<Fade>();
        let (phase, alpha) = (fade.phase, fade.alpha);
        if cycle == 1 {
            let slot = if phase == Phase::Out && (0.45..=0.55).contains(&alpha) {
                Some(0)
            } else if phase == Phase::Waiting && alpha == 1.0 {
                Some(1)
            } else if phase == Phase::In && (0.45..=0.55).contains(&alpha) {
                Some(2)
            } else {
                None
            };
            if let Some(slot) = slot.filter(|&i| !saved[i]) {
                saved[slot] = true;
                capture_checked(
                    app,
                    &options
                        .output
                        .join(format!("gallery-{direction}-phase-{slot}.png")),
                    slot == 1,
                )?;
            }
        }
        if phase == Phase::Idle
            && *app
                .world()
                .resource::<State<crate::app::state::GameState>>()
                .get()
                == target
        {
            ensure(alpha == 0.0, "画廊过渡完成后仍有黑幕")?;
            ensure(
                cycle != 1 || saved.iter().all(|s| *s),
                "缺少画廊过渡关键阶段",
            )?;
            return Ok(());
        }
    }
    Err("画廊过渡未完成".into())
}

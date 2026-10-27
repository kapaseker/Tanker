//! 详情的真实键鼠输入、全部 VOX 模型、故障返回与资源生命周期验收。
use super::*;
use crate::{
    app::{
        state::GameState,
        transition::{Fade, SceneTarget},
    },
    pages::{
        detail::{SelectedModel, View},
        gallery,
        model_preview::{self as preview, Previews},
    },
    ui::widgets::{TitleBarState, TitleBarTitle},
};
use bevy::{
    asset::AssetId,
    input::mouse::{MouseScrollUnit, MouseWheel},
    world_serialization::{InstanceId, WorldInstance},
};

/// 使用与正常游戏相同的输入系统，按键帧之间释放，避免重复按住被当作一次。
fn key(app: &mut App, frame: u32) -> Result<(), String> {
    update(app, STEP, Some(frame))?;
    update(app, STEP, None)
}

/// 收集当前预览实体、纹理与实例，页面交接后逐个检查释放。
fn owned(app: &mut App) -> (Vec<Entity>, Vec<AssetId<Image>>, Vec<InstanceId>) {
    let previews = app.world().resource::<Previews>();
    let roots = previews
        .items
        .iter()
        .flat_map(|p| [p.root, p.camera])
        .collect();
    let images = previews.items.iter().map(|p| p.image.id()).collect();
    let instances = app
        .world_mut()
        .query::<&WorldInstance>()
        .iter(app.world())
        .map(|i| **i)
        .collect();
    (roots, images, instances)
}

/// 新页已就绪时，旧页面拥有的三类资源必须全部销毁。
fn released(
    app: &App,
    old: (Vec<Entity>, Vec<AssetId<Image>>, Vec<InstanceId>),
) -> Result<(), String> {
    ensure(
        old.0.iter().all(|&e| app.world().get_entity(e).is_err()),
        "旧预览实体残留",
    )?;
    ensure(
        old.1
            .iter()
            .all(|&id| app.world().resource::<Assets<Image>>().get(id).is_none()),
        "旧预览纹理残留",
    )?;
    ensure(
        old.2.iter().all(|id| {
            !app.world()
                .resource::<WorldInstanceSpawner>()
                .instance_is_ready(*id)
        }),
        "旧预览场景实例残留",
    )
}

/// 模型查看区注入同一逻辑像素指针，标题栏注入独立区域以验证命中隔离。
fn pointer(app: &mut App, point: Option<Vec2>) {
    app.world_mut().resource_mut::<View>().pointer = point;
    app.world_mut().resource_mut::<TitleBarState>().pointer = point;
}

/// 正负缩放都发送真实滚轮消息，绝不直接改生产缩放状态。
fn wheel(app: &mut App, lines: f32) -> Result<(), String> {
    app.world_mut().write_message(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: lines,
        window: Entity::PLACEHOLDER,
        phase: bevy::input::touch::TouchPhase::Moved,
    });
    update(app, STEP, None)
}

/// 尺寸轴与屏幕层标签存在性断言：3 个标签文本为数值、轴线组唯一、
/// 伴随子弹只在 lance（玩家炮管）出现。
fn assert_dims(app: &mut App, index: usize) -> Result<(), String> {
    let world = app.world_mut();
    let mut values = world.query::<(&crate::pages::detail::DimValue, &Text)>();
    let mut found: [Option<String>; 3] = [None, None, None];
    for (dim, text) in values.iter(world) {
        ensure(dim.0 < 3, "尺寸标签序号越界")?;
        found[dim.0] = Some(text.0.clone());
    }
    ensure(
        found.iter().all(|v| v.is_some()),
        "详情缺少尺寸标签（长/宽/高）",
    )?;
    if world.resource::<Previews>().items[0].size.is_some() {
        for value in found.into_iter().flatten() {
            ensure(
                value.parse::<f32>().is_ok(),
                format!("尺寸标签不是数值：{value}").as_str(),
            )?;
        }
    }
    let axes = world
        .query_filtered::<(), With<crate::pages::detail::DimAxes>>()
        .iter(world)
        .count();
    ensure(axes == 1, "详情缺少唯一的尺寸轴组")?;
    let content = world.resource::<Previews>().items[0].content;
    let mut stack = vec![content];
    let mut bullet = false;
    while let Some(entity) = stack.pop() {
        if world
            .get::<crate::pages::model_preview::BulletCompanion>(entity)
            .is_some()
        {
            bullet = true;
        }
        if let Some(children) = world.get::<Children>(entity) {
            stack.extend(children.iter());
        }
    }
    ensure(
        bullet == (preview::ENTRIES[index].path == crate::tank::visual::PLAYER_BARREL),
        "伴随子弹只应出现在 lance",
    )?;
    Ok(())
}

/// 整个链路从首页发起；重复往返每个目录项，同时验证返回保持原位置。
pub(super) fn run_detail_scenario(
    app: &mut App,
    options: &Options,
    report: &mut String,
) -> Result<(), String> {
    wait_render(app, false)?;
    while !crate::app::transition::content_ready(app.world_mut()) {
        update(app, Duration::ZERO, None)?;
    }
    capture(app, &options.image_path(0))?;
    key(app, 7)?;
    key(app, 7)?;
    key(app, 3)?;
    super::gallery::finish_gallery_transition(
        app,
        options,
        2,
        "detail-gallery",
        GameState::Gallery,
    )?;
    for index in 0..preview::ENTRIES.len() {
        // 初次方向键落首卡；其后左右/上下沿真实网格走到下一项。
        if index == 0 {
            key(app, 14)?;
        } else if index % 2 == 1 {
            key(app, 16)?;
        } else {
            // 先回左列再下移，才能进入末行只有左卡的障碍物分类。
            key(app, 18)?;
            key(app, 14)?;
        }
        ensure(
            gallery::view_mut(app.world_mut()).focus == Some(index),
            "方向键未选中目标卡片",
        )?;
        let scroll = gallery::view_mut(app.world_mut()).scroll;
        let old = owned(app);
        if index % 2 == 1 {
            let rect = gallery::ui::card_rect(index, 64.0, scroll);
            gallery::view_mut(app.world_mut()).pointer = Some(rect.center());
            app.world_mut().resource_mut::<MenuMouse>().0 = true;
            update(app, STEP, None)?;
            app.world_mut().resource_mut::<MenuMouse>().0 = false;
            update(app, STEP, None)?;
            gallery::view_mut(app.world_mut()).pointer = None;
        } else {
            key(app, 3)?;
        }
        super::gallery::finish_gallery_transition(
            app,
            options,
            if index == 0 { 1 } else { 2 },
            "detail-enter",
            GameState::Detail,
        )?;
        released(app, old)?;
        ensure(
            app.world().resource::<SelectedModel>().0 == index,
            "详情选择与卡片不一致",
        )?;
        ensure(
            app.world().resource::<Previews>().items.len() == 1,
            "详情必须只创建一个预览",
        )?;
        report.push_str(&preview::validate(app.world())?);
        let title = app
            .world_mut()
            .query_filtered::<&Text, With<TitleBarTitle>>()
            .single(app.world())
            .map_err(|e| e.to_string())?;
        ensure(
            title.0 == preview::ENTRIES[index].name(),
            "详情标题不是实际型号",
        )?;
        let view = app.world().resource::<View>();
        ensure(
            view.yaw == -28.0 && view.pitch == -14.0 && view.zoom == 1.0 && !view.dragging,
            "进入详情未重置视角",
        )?;
        let root = app.world().resource::<Previews>().items[0].root;
        let rotation = app.world().get::<Transform>(root).unwrap().rotation;
        for _ in 0..4 {
            update(app, STEP, None)?;
        }
        ensure(
            app.world().get::<Transform>(root).unwrap().rotation == rotation,
            "详情不应自动旋转",
        )?;
        assert_dims(app, index)?;
        capture(
            app,
            &options.output.join(format!("detail-{index}-initial.png")),
        )?;
        if index == 6 {
            pointer(app, Some(Vec2::new(650.0, 400.0)));
            app.world_mut().resource_mut::<MenuMouse>().0 = true;
            update(app, STEP, None)?;
            // 垂直拖拽独立截图，避免混合水平输入掩盖方向反转。
            pointer(app, Some(Vec2::new(650.0, 460.0)));
            update(app, STEP, None)?;
            ensure(
                app.world().resource::<View>().pitch > -14.0,
                "向下拖拽未增加俯仰角",
            )?;
            capture(app, &options.output.join("detail-drag-down.png"))?;
            pointer(app, Some(Vec2::new(650.0, 340.0)));
            update(app, STEP, None)?;
            ensure(
                app.world().resource::<View>().pitch < -14.0,
                "向上拖拽未减少俯仰角",
            )?;
            capture(app, &options.output.join("detail-drag-up.png"))?;
            pointer(app, Some(Vec2::new(810.0, 460.0)));
            update(app, STEP, None)?;
            ensure(
                app.world().resource::<View>().yaw != -28.0
                    && app.world().resource::<View>().dragging,
                "拖拽未旋转模型",
            )?;
            app.world_mut().resource_mut::<MenuMouse>().0 = false;
            update(app, STEP, None)?;
            ensure(!app.world().resource::<View>().dragging, "松开后仍在拖拽")?;
            capture(app, &options.output.join("detail-rotated.png"))?;
            wheel(app, 100.0)?;
            ensure(app.world().resource::<View>().zoom == 1.5, "缩放上限不正确")?;
            capture(app, &options.output.join("detail-zoom-in.png"))?;
            wheel(app, -100.0)?;
            ensure(app.world().resource::<View>().zoom == 0.5, "缩放下限不正确")?;
            capture(app, &options.output.join("detail-zoom-out.png"))?;
            pointer(app, Some(Vec2::new(500.0, 40.0)));
            wheel(app, 100.0)?;
            ensure(
                app.world().resource::<View>().zoom == 0.5,
                "标题栏滚轮不应缩放模型",
            )?;
            pointer(app, None);
            for (width, height) in [(640, 720), (1920, 720), (1280, 720)] {
                let extent = bevy::render::render_resource::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                };
                for handle in [
                    app.world().resource::<Target>().0.clone(),
                    app.world().resource::<SceneTarget>().0.clone(),
                ] {
                    app.world_mut()
                        .resource_mut::<Assets<Image>>()
                        .get_mut(&handle)
                        .unwrap()
                        .resize(extent);
                }
                update(app, STEP, None)?;
                capture(
                    app,
                    &options
                        .output
                        .join(format!("detail-layout-{width}x{height}.png")),
                )?;
                let preview = &app.world().resource::<Previews>().items[0];
                ensure(
                    app.world()
                        .resource::<Assets<Image>>()
                        .get(&preview.image)
                        .unwrap()
                        .size()
                        .x
                        == width,
                    "详情模型纹理未随窗口更新",
                )?;
            }
            report.push_str("拖拽、松开、手动静止、滚轮上下限与标题栏输入隔离通过\n");
        }
        let old = owned(app);
        if index % 3 == 0 {
            key(app, 5)?;
        } else if index % 3 == 1 {
            key(app, 7)?;
            key(app, 3)?;
        } else {
            pointer(app, Some(Vec2::new(90.0, 40.0)));
            app.world_mut().resource_mut::<MenuMouse>().0 = true;
            update(app, STEP, None)?;
            app.world_mut().resource_mut::<MenuMouse>().0 = false;
            update(app, STEP, None)?;
        }
        super::gallery::finish_gallery_transition(
            app,
            options,
            if index == 0 { 1 } else { 2 },
            "detail-return",
            GameState::Gallery,
        )?;
        pointer(app, None);
        released(app, old)?;
        let view = gallery::view_mut(app.world_mut());
        ensure(
            view.scroll == scroll && view.focus == Some(index),
            "详情返回未恢复原滚动与卡片焦点",
        )?;
        report.push_str(&format!(
            "目录项{index}：进入/返回、视角重置、原焦点与滚动恢复、实体/实例/纹理清理通过\n"
        ));
    }
    capture(app, &options.output.join("detail-return-gallery.png"))?;
    // 真实缺失文件验证错误揭幕与返回；只豁免指定文件的预期事件，其它错误照常失败。
    key(app, 3)?;
    // 在黑场刚生成详情时替换一项加载请求，覆盖首次加载失败的揭幕链路。
    for _ in 0..100 {
        update(app, STEP, None)?;
        if *app.world().resource::<State<GameState>>().get() == GameState::Detail {
            break;
        }
    }
    ensure(
        *app.world().resource::<State<GameState>>().get() == GameState::Detail,
        "故障场景未进入详情",
    )?;
    const MISSING: &str = "tank/bodies/__missing_detail_test__/body.vox";
    app.world_mut().insert_resource(ExpectedAssetFailure {
        path: MISSING.into(),
        seen: false,
    });
    let handle = app.world().resource::<AssetServer>().load(MISSING);
    let content = app.world().resource::<Previews>().items[0].content;
    app.world_mut()
        .entity_mut(content)
        .insert(WorldAssetRoot(handle));
    app.world_mut().resource_mut::<Previews>().items[0].size = None;
    super::gallery::finish_gallery_transition(app, options, 2, "failure-enter", GameState::Detail)?;
    ensure(
        app.world().resource::<ExpectedAssetFailure>().seen
            && app.world().resource::<Previews>().items[0].error.is_some(),
        "真实加载失败未被识别",
    )?;
    ensure(
        preview::assets_ready(app.world()),
        "错误页面不应永久等待加载",
    )?;
    capture(app, &options.output.join("detail-load-failed.png"))?;
    key(app, 5)?;
    super::gallery::finish_gallery_transition(
        app,
        options,
        2,
        "failure-return",
        GameState::Gallery,
    )?;
    app.world_mut().remove_resource::<ExpectedAssetFailure>();
    report.push_str("真实缺失资源：错误提示可见，错误项完成等待，Esc 可返回画廊\n");
    key(app, 5)?;
    super::gallery::finish_gallery_transition(app, options, 2, "detail-index", GameState::Index)?;
    key(app, 3)?;
    super::gallery::finish_gallery_transition(
        app,
        options,
        2,
        "detail-reenter",
        GameState::Gallery,
    )?;
    ensure(
        gallery::view_mut(app.world_mut()).scroll == 0.0
            && gallery::view_mut(app.world_mut()).focus.is_none(),
        "首页重新进入画廊未重置位置",
    )?;
    report.push_str("首页重新进入画廊：滚动和卡片焦点重置\n");
    ensure(
        app.world().resource::<Fade>().accepts_input(),
        "最终输入锁未释放",
    )?;
    // 保留真实内容纹理引用，用于尺寸断言，避免仅检查输出图片规格。
    let target = &app.world().resource::<SceneTarget>().0;
    ensure(
        app.world()
            .resource::<Assets<Image>>()
            .get(target)
            .unwrap()
            .size()
            == UVec2::new(1280, 720),
        "详情场景内容纹理尺寸异常",
    )?;
    Ok(())
}

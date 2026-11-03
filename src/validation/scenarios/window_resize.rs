//! 无窗口模拟高 DPI 进入对局与窗口连续变更尺寸。
use super::*;
use crate::app::{
    state::GameState,
    transition::{Fade, SceneTarget},
};
use bevy::{render::render_resource::Extent3d, window::WindowResolution};

/// 检查每帧主副相机取到的目标尺寸一致。
fn check_camera_sizes(app: &mut App) -> Result<(), String> {
    let world = app.world_mut();
    let follow = world
        .query_filtered::<&Camera, With<crate::gameplay::camera::FollowCamera>>()
        .single(world)
        .map_err(|e| e.to_string())?
        .physical_target_size();
    let gunner = world
        .query_filtered::<&Camera, With<crate::gameplay::gunner::GunnerCamera>>()
        .single(world)
        .map_err(|e| e.to_string())?
        .physical_target_size();
    ensure(
        follow.is_some() && follow == gunner,
        &format!("主副相机目标尺寸不同：{follow:?} / {gunner:?}"),
    )
}

/// 修改模拟窗口和离屏输出纹理，模拟系统窗口的物理尺寸变化。
fn resize(app: &mut App, window: Entity, width: u32, height: u32) {
    let mut resolution = WindowResolution::new(width, height);
    resolution.set_scale_factor_override(Some(2.0));
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .resolution = resolution;
    let target = app.world().resource::<Target>().0.clone();
    app.world_mut()
        .resource_mut::<Assets<Image>>()
        .get_mut(&target)
        .unwrap()
        .resize(Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        });
}

/// 在真实游戏渲染和状态切换链路中调整模拟窗口，不创建 OS 窗口。
pub(super) fn run(app: &mut App, options: &Options, report: &mut String) -> Result<(), String> {
    let mut resolution = WindowResolution::new(1280, 720);
    resolution.set_scale_factor_override(Some(2.0));
    let window = app
        .world_mut()
        .spawn((
            Window {
                resolution,
                focused: true,
                ..default()
            },
            super::articulation::SimulatedFocus,
        ))
        .id();
    let start = std::time::Instant::now();
    loop {
        update(app, STEP, None)?;
        if *app.world().resource::<State<GameState>>().get() == GameState::Index
            && app.world().resource::<Fade>().accepts_input()
        {
            break;
        }
        ensure(
            start.elapsed() < Duration::from_secs(60),
            &format!(
                "首页加载超时：{}",
                crate::app::transition::diagnostics(app.world_mut())
            ),
        )?;
    }
    update(app, STEP, Some(1))?;
    update(app, STEP, None)?;
    update(app, STEP, Some(3))?;
    let start = std::time::Instant::now();
    let mut resized_loading = false;
    loop {
        update(app, STEP, None)?;
        if !resized_loading
            && *app.world().resource::<State<GameState>>().get() == GameState::InGame
        {
            resize(app, window, 1282, 720);
            resized_loading = true;
        }
        if *app.world().resource::<State<GameState>>().get() == GameState::InGame
            && app.world().resource::<Fade>().accepts_input()
        {
            break;
        }
        ensure(
            start.elapsed() < Duration::from_secs(90),
            &format!(
                "对局加载超时：{}",
                crate::app::transition::diagnostics(app.world_mut())
            ),
        )?;
    }
    check_camera_sizes(app)?;
    ensure(resized_loading, "未在加载期间调整窗口")?;
    report.push_str("模拟 2× DPI 与异步管线：加载期间调整窗口，从首页进入对局并解除黑场\n");
    for (width, height) in [(1282, 720), (1284, 722), (1600, 900), (1280, 720)] {
        resize(app, window, width, height);
        update(app, STEP, None)?;
        check_camera_sizes(app)?;
        for _ in 0..4 {
            update(app, STEP, None)?;
        }
        report.push_str(&format!("窗口变更为 {width}×{height} 后主副相机尺寸一致\n"));
    }
    let scene_size = app
        .world()
        .resource::<Assets<Image>>()
        .get(&app.world().resource::<SceneTarget>().0)
        .unwrap()
        .size();
    ensure(scene_size == UVec2::new(1280, 720), "场景目标纹理未恢复")?;
    capture(app, &options.output.join("window-resize.png"))?;
    Ok(())
}

//! 截图读回、像素验证和排他文件输出；等待截图时不推进模拟。

use super::*;
use super::{
    offscreen::{update, wait_render},
    scenarios::scene::subject_state,
};
use bevy::{
    image::ImageFormat,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
};

/// 临时取消跟随以拍摄固定的车体局部视点；截图等待仍不推进模拟。
fn capture_view(app: &mut App, path: &Path, offset: Vec3) -> Result<(), String> {
    let (player, _) = subject_state(app.world_mut())?;
    let world = app.world_mut();
    let (camera, saved) = world
        .query_filtered::<(Entity, &Transform), With<crate::gameplay::camera::FollowCamera>>()
        .single(world)
        .map(|(entity, transform)| (entity, *transform))
        .map_err(|e| e.to_string())?;
    let target = player.translation + Vec3::Y * -0.15;
    world
        .entity_mut(camera)
        .remove::<crate::gameplay::camera::FollowCamera>()
        .insert(
            Transform::from_translation(target + player.rotation * offset)
                .looking_at(target, Vec3::Y),
        );
    let result = capture(app, path);
    app.world_mut()
        .entity_mut(camera)
        .insert((crate::gameplay::camera::FollowCamera, saved));
    result
}

/// 侧面与车体一起移动旋转，使逐帧比较只显示轮组自身动画。
pub(super) fn capture_side(app: &mut App, path: &Path) -> Result<(), String> {
    capture_view(app, path, Vec3::new(0.0, 0.5, 10.0))
}

/// 相同侧面、前侧和后侧视点，并用细红线标示碰撞框外包围边界（不含炮管）。
pub(super) fn capture_wheel_views(app: &mut App, output: &Path) -> Result<(), String> {
    capture_side(app, &output.join("side.png"))?;
    capture_view(app, &output.join("front.png"), Vec3::new(-9.0, 3.0, 9.0))?;
    capture_view(app, &output.join("rear.png"), Vec3::new(9.0, 3.0, 9.0))?;
    let (player, _) = subject_state(app.world_mut())?;
    let material = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.05, 0.02),
            unlit: true,
            ..default()
        });
    capture_view(
        app,
        &output.join("comparison.png"),
        Vec3::new(13.0, 10.0, 18.0),
    )?;
    let half = if app.world().contains_resource::<WheelTarget>() {
        crate::gameplay::enemy::ENEMY_SIZE
    } else {
        crate::tank::driving::TANK_SIZE
    } / 2.0;
    let mut edges = Vec::new();
    for axis in 0..3 {
        for a in [-1.0, 1.0] {
            for b in [-1.0, 1.0] {
                let mut size = Vec3::splat(0.025);
                size[axis] = half[axis] * 2.0;
                let mut center = Vec3::ZERO;
                center[(axis + 1) % 3] = half[(axis + 1) % 3] * a;
                center[(axis + 2) % 3] = half[(axis + 2) % 3] * b;
                let mesh = app
                    .world_mut()
                    .resource_mut::<Assets<Mesh>>()
                    .add(Cuboid::from_size(size));
                edges.push(
                    app.world_mut()
                        .spawn((
                            Mesh3d(mesh),
                            MeshMaterial3d(material.clone()),
                            Transform::from_translation(
                                player.translation + player.rotation * center,
                            )
                            .with_rotation(player.rotation),
                        ))
                        .id(),
                );
            }
        }
    }
    let result = capture_view(app, &output.join("bounds.png"), Vec3::new(-9.0, 3.0, 9.0));
    for edge in edges {
        app.world_mut().despawn(edge);
    }
    result
}

/// 请求一次截图并等待 observer 完成保存，绝不以请求已提交作为成功判据。
pub(super) fn capture(app: &mut App, path: &Path) -> Result<(), String> {
    capture_checked(app, path, false)
}

/// 全黑交接专用像素断言，其余帧仍拒收空画面。
pub(super) fn capture_checked(app: &mut App, path: &Path, black: bool) -> Result<(), String> {
    wait_render(app, false)?;
    app.world_mut().resource_mut::<CaptureResult>().0 = None;
    let target = app.world().resource::<Target>().0.clone();
    let size = app
        .world()
        .resource::<Assets<Image>>()
        .get(&target)
        .unwrap()
        .size();
    let path = path.to_owned();
    app.world_mut().spawn(Screenshot::image(target)).observe(
        move |event: On<ScreenshotCaptured>, mut result: ResMut<CaptureResult>| {
            result.0 = Some(save_image(&event.image, &path, size, black));
        },
    );
    loop {
        update(app, Duration::ZERO, None)?;
        if let Some(result) = app.world_mut().resource_mut::<CaptureResult>().0.take() {
            return result;
        }
    }
}

/// RGB 编码会丢弃 HDR 输出中用于亮度的 alpha，与 Bevy 官方截图行为一致。
fn save_image(image: &Image, path: &Path, size: UVec2, black: bool) -> Result<(), String> {
    let rgb = image
        .clone()
        .try_into_dynamic()
        .map_err(|error| format!("截图转换失败：{error}"))?
        .to_rgb8();
    ensure(
        rgb.width() == size.x && rgb.height() == size.y,
        "截图尺寸不符",
    )?;
    // 临时诊断：像素校验失败也落盘 reject 文件，便于人工检查（验证后删除）。
    if let Err(message) = validate_capture_pixels(rgb.as_raw(), size, black) {
        let reject = path.with_extension("reject.png");
        let mut file = create_new(&reject)?;
        rgb.write_to(
            &mut file,
            ImageFormat::Png
                .as_image_crate_format()
                .ok_or("未启用 PNG 编码")?,
        )
        .map_err(|error| format!("PNG 编码或写入失败：{error}"))?;
        drop(file);
        return Err(format!("{message}；已保存诊断图：{}", reject.display()));
    }
    let mut file = create_new(path)?;
    rgb.write_to(
        &mut file,
        ImageFormat::Png
            .as_image_crate_format()
            .ok_or("未启用 PNG 编码")?,
    )
    .map_err(|error| format!("PNG 编码或写入失败：{error}"))?;
    file.sync_all()
        .map_err(|error| format!("截图落盘失败：{error}"))?;
    Ok(())
}

/// 排除纯色/黑色等明显空图；具体模型和材质效果仍需读取关键帧人工检查。
#[cfg(test)]
fn validate_pixels(rgb: &[u8]) -> Result<(), String> {
    validate_capture_pixels(rgb, UVec2::new(WIDTH, HEIGHT), false)
}

/// 验证目标尺寸与阶段预期，避免把正常黑场与异常黑屏混为一谈。
fn validate_capture_pixels(rgb: &[u8], size: UVec2, black: bool) -> Result<(), String> {
    ensure(
        rgb.len() == (size.x * size.y * 3) as usize && !rgb.is_empty(),
        "截图像素长度不符",
    )?;
    if black {
        return ensure(
            rgb.iter().all(|&value| value <= 3),
            "全黑交接帧仍有可见内容",
        );
    }
    let first = &rgb[..3];
    ensure(
        rgb.chunks_exact(3).filter(|pixel| *pixel != first).count()
            > (size.x * size.y / 100) as usize,
        "截图缺少实际场景内容（接近纯色）",
    )
}

/// 排他创建产物；即使预检查后有其它进程写入，也不会覆盖已有文件。
pub(super) fn create_new(path: &Path) -> Result<File, String> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("无法创建产物 {}：{error}", path.display()))
}

/// 报告也按排他创建、完整写入与刷新处理，不吞掉磁盘错误。
pub(super) fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = create_new(path)?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("写入报告失败：{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_or_solid_images_and_failed_assertions_cannot_pass() {
        assert!(validate_pixels(&[]).is_err());
        assert!(validate_pixels(&vec![0; (WIDTH * HEIGHT * 3) as usize]).is_err());
        let mut pixels = vec![0; (WIDTH * HEIGHT * 3) as usize];
        pixels[..(WIDTH * HEIGHT * 3 / 2) as usize].fill(128);
        assert!(validate_pixels(&pixels).is_ok());
        assert!(ensure(false, "断言失败").is_err());
        let mut report = String::new();
        let mut failures = Vec::new();
        record_check(&mut report, &mut failures, false, "驾驶位移");
        record_check(&mut report, &mut failures, true, "相机跟随");
        assert_eq!(failures, ["驾驶位移"]);
        assert!(report.contains("断言失败：驾驶位移"));
        assert!(ensure(failures.is_empty(), &failures.join("；")).is_err());
    }
}

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
    let target = player.translation + Vec3::Y * -0.15;
    capture_at(app, path, target, player.rotation * offset)
}

/// 以固定目标拍摄近景；临时机位仅用于离屏取证，不改变游戏相机参数。
fn capture_at(app: &mut App, path: &Path, target: Vec3, offset: Vec3) -> Result<(), String> {
    let world = app.world_mut();
    let (camera, saved) = world
        .query_filtered::<(Entity, &Transform), With<crate::gameplay::camera::FollowCamera>>()
        .single(world)
        .map(|(entity, transform)| (entity, *transform))
        .map_err(|e| e.to_string())?;
    world
        .entity_mut(camera)
        .remove::<crate::gameplay::camera::FollowCamera>()
        .insert(
            Transform::from_translation(target + offset)
                .looking_at(target, Vec3::Y),
        );
    let result = capture(app, path);
    app.world_mut()
        .entity_mut(camera)
        .insert((crate::gameplay::camera::FollowCamera, saved));
    result
}

/// 在首发出生帧对准真实弹丸，截图等待不推进弹道。
pub(super) fn capture_bullet(app: &mut App, path: &Path) -> Result<(), String> {
    let world = app.world_mut();
    let transform = *world
        .query_filtered::<&Transform, With<crate::gameplay::combat::firing::Bullet>>()
        .single(world)
        .map_err(|e| e.to_string())?;
    capture_at(
        app,
        path,
        transform.translation + transform.rotation * Vec3::NEG_X * 0.16,
        transform.rotation * Vec3::new(0.5, 0.5, 1.5),
    )
}

/// 混凝土墩前侧近景，能同时看到长边、斜面和贴地关系。
pub(super) fn capture_concrete(app: &mut App, path: &Path) -> Result<(), String> {
    capture_at(app, path, crate::gameplay::obstacle::POSITION, Vec3::new(7.0, 3.0, 6.0))
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
    capture_view(
        app,
        &output.join("front-on.png"),
        Vec3::new(-12.0, 1.0, 0.0),
    )?;
    capture_view(app, &output.join("rear-on.png"), Vec3::new(10.0, 1.0, 0.0))?;
    capture_view(
        app,
        &output.join("front-above.png"),
        Vec3::new(-9.0, 7.0, 9.0),
    )?;
    // 尾舱固定近景用于检查资产格凹槽，目标相对共同车体原点，单位为米。
    let (player, _) = subject_state(app.world_mut())?;
    capture_at(
        app,
        &output.join("grille.png"),
        player.translation + player.rotation * Vec3::new(2.7, 0.28, 0.0),
        player.rotation * Vec3::new(1.0, 2.0, 1.5),
    )?;
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

/// 截图像素断言模式：区分正常内容帧、整体黑场与"仅内容黑"的常驻 chrome 帧。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PixelCheck {
    /// 正常内容帧：拒收纯色/黑屏。
    Content,
    /// 整体黑场交接帧（FullBlack）：全画面必须为黑。
    FullBlack,
    /// 仅内容黑帧（ContentOnly 黑场）：标题栏带与右下直角框属于常驻 chrome，
    /// 其余画面必须为黑，且标题栏带内必须有可见内容。
    ContentBlack,
}

/// 请求一次截图并等待 observer 完成保存，绝不以请求已提交作为成功判据。
pub(super) fn capture(app: &mut App, path: &Path) -> Result<(), String> {
    capture_checked(app, path, PixelCheck::Content)
}

/// 按模式做像素断言后落盘。
pub(super) fn capture_checked(app: &mut App, path: &Path, check: PixelCheck) -> Result<(), String> {
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
            result.0 = Some(save_image(&event.image, &path, size, check));
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
fn save_image(image: &Image, path: &Path, size: UVec2, check: PixelCheck) -> Result<(), String> {
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
    if let Err(message) = validate_capture_pixels(rgb.as_raw(), size, check) {
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
    validate_capture_pixels(rgb, UVec2::new(WIDTH, HEIGHT), PixelCheck::Content)
}

/// 验证目标尺寸与阶段预期，避免把正常黑场与异常黑屏混为一谈。
fn validate_capture_pixels(rgb: &[u8], size: UVec2, check: PixelCheck) -> Result<(), String> {
    ensure(
        rgb.len() == (size.x * size.y * 3) as usize && !rgb.is_empty(),
        "截图像素长度不符",
    )?;
    match check {
        PixelCheck::FullBlack => ensure(
            rgb.iter().all(|&value| value <= 3),
            "全黑交接帧仍有可见内容",
        ),
        PixelCheck::ContentBlack => validate_content_black(rgb, size),
        PixelCheck::Content => {
            let first = &rgb[..3];
            ensure(
                rgb.chunks_exact(3).filter(|pixel| *pixel != first).count()
                    > (size.x * size.y / 100) as usize,
                "截图缺少实际场景内容（接近纯色）",
            )
        }
    }
}

/// 仅内容黑帧：常驻 chrome 区域（顶部标题栏带 + 右下直角框）豁免黑场断言，
/// 其余像素必须全黑，且标题栏带内必须有可见内容（证明 chrome 确实亮着）。
fn validate_content_black(rgb: &[u8], size: UVec2) -> Result<(), String> {
    use crate::ui::resources::dimens;
    // 离屏纹理尺寸即物理尺寸（缩放系数 1），逻辑像素 × zoom 即物理像素。
    let zoom = crate::ui::zoom(size.x as f32);
    let band = (dimens::title_bar::HEIGHT * zoom) as usize;
    // 右下直角框：内缩 CORNER_INSET + 边长 CORNER_SIZE 的方形区域。
    let corner_box = ((dimens::index::CORNER_INSET + dimens::index::CORNER_SIZE) * zoom) as usize;
    let (w, h) = (size.x as usize, size.y as usize);
    let mut band_bright = false;
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 3;
            let bright = rgb[i] > 3 || rgb[i + 1] > 3 || rgb[i + 2] > 3;
            if y < band {
                band_bright |= bright;
            } else if x < w - corner_box && y < h - corner_box && bright {
                return Err(format!("内容黑场帧 chrome 外仍有亮像素 ({x},{y})"));
            }
        }
    }
    ensure(band_bright, "内容黑场帧标题栏带内没有可见 chrome")
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
        // 内容黑场：chrome 区域（标题栏带、右下直角框）豁免，其余亮像素或标题栏全黑都失败。
        let size = UVec2::new(WIDTH, HEIGHT);
        let set = |px: &mut Vec<u8>, x: u32, y: u32| {
            let i = ((y * WIDTH + x) * 3) as usize;
            px[i] = 200;
        };
        let mut chrome_only = vec![0u8; (WIDTH * HEIGHT * 3) as usize];
        set(&mut chrome_only, 10, 10);
        set(&mut chrome_only, WIDTH - 10, HEIGHT - 10);
        assert!(validate_capture_pixels(&chrome_only, size, PixelCheck::ContentBlack).is_ok());
        let mut content_lit = chrome_only.clone();
        set(&mut content_lit, WIDTH / 2, HEIGHT / 2);
        assert!(validate_capture_pixels(&content_lit, size, PixelCheck::ContentBlack).is_err());
        let all_black = vec![0u8; (WIDTH * HEIGHT * 3) as usize];
        assert!(validate_capture_pixels(&all_black, size, PixelCheck::ContentBlack).is_err());
        assert!(validate_capture_pixels(&all_black, size, PixelCheck::FullBlack).is_ok());
        let mut report = String::new();
        let mut failures = Vec::new();
        record_check(&mut report, &mut failures, false, "驾驶位移");
        record_check(&mut report, &mut failures, true, "相机跟随");
        assert_eq!(failures, ["驾驶位移"]);
        assert!(report.contains("断言失败：驾驶位移"));
        assert!(ensure(failures.is_empty(), &failures.join("；")).is_err());
    }
}

//! Tanker —— 基于 Bevy 的 3D 游戏工程（起步阶段：仅打开一个基础窗口）。

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Tanker".into(),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup_camera)
        .run();
}

/// 基础 3D 相机，为后续 3D 游戏内容铺路。
fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

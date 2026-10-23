//! 菜单与对局共用画面合成：内容相机写入纹理，最终相机统一混合到黑色。

use super::state::GameState;
use avian3d::prelude::{Physics, PhysicsTime};
use bevy::{
    asset::{AssetId, UntypedAssetLoadFailedEvent},
    camera::{RenderTarget, ScalingMode, visibility::RenderLayers},
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        mesh::RenderMesh,
        render_asset::RenderAssets,
        render_resource::{CachedPipelineState, PipelineCache, TextureFormat},
        texture::GpuImage,
    },
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

/// 每个方向的单段时长；加载等待不占用渐变时间。
const FADE_SECONDS: f32 = 0.6;

/// 最终输出相机；离屏入口只重定向这台相机。
#[derive(Component)]
pub(crate) struct OutputCamera;

/// 内容相机共享的内部纹理，尺寸随真实视口变化。
#[derive(Resource)]
pub(crate) struct SceneTarget(pub Handle<Image>);

/// 一次过渡的明确阶段；Idle 永远不会自动重新开始淡入。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    Idle,
    Out,
    Waiting,
    In,
}

/// 页面过渡进度；alpha=1 为全黑，alpha=0 为内容完全可见。
#[derive(Resource)]
pub(crate) struct Fade {
    pub alpha: f32,
    pub phase: Phase,
    target: GameState,
    stable_frames: u8,
    /// 阻止跨页面继续按住的键立即触发驾驶、开火或确认。
    pub input_armed: bool,
}

impl Default for Fade {
    fn default() -> Self {
        Self {
            alpha: 0.0,
            phase: Phase::Idle,
            target: GameState::Menu,
            stable_frames: 0,
            input_armed: true,
        }
    }
}

impl Fade {
    /// 重复请求被忽略；返回值用于决定是否进入过渡状态。
    pub fn begin(&mut self, target: GameState) -> bool {
        if self.phase != Phase::Idle || !self.input_armed {
            return false;
        }
        self.target = target;
        self.phase = Phase::Out;
        self.stable_frames = 0;
        self.input_armed = false;
        true
    }

    /// 只积分当前段；黑场必须由就绪检查显式释放。
    fn tick(&mut self, delta: f32) -> bool {
        match self.phase {
            Phase::Out => {
                self.alpha = (self.alpha + delta / FADE_SECONDS).min(1.0);
                if self.alpha == 1.0 {
                    self.phase = Phase::Waiting;
                    return true;
                }
            }
            Phase::In => {
                self.alpha = (self.alpha - delta / FADE_SECONDS).max(0.0);
                if self.alpha == 0.0 {
                    self.phase = Phase::Idle;
                }
            }
            _ => {}
        }
        false
    }

    /// 游戏输入只有在揭幕完成且上一页按键全部释放后才可使用。
    pub fn accepts_input(&self) -> bool {
        self.phase == Phase::Idle && self.input_armed
    }
}

/// 渲染线程检查当前所需资产与管线，通过原子值返回真实 GPU 就绪状态。
#[derive(Resource, Clone, ExtractResource, Default)]
#[extract_app(RenderApp)]
struct RenderReadiness {
    meshes: Vec<AssetId<Mesh>>,
    images: Vec<AssetId<Image>>,
    ready: Arc<AtomicBool>,
    failure: Arc<Mutex<Option<String>>>,
}

/// 最终合成四边形使用的材质；只更改 RGB，保持不透明输出。
#[derive(Resource)]
struct CompositeMaterial(Handle<StandardMaterial>);

/// 安装最终输出、黑场状态机和渲染就绪桥接。
pub(crate) struct TransitionPlugin;

impl Plugin for TransitionPlugin {
    fn build(&self, app: &mut App) {
        let image = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::new_target_texture(
                1280,
                720,
                TextureFormat::Rgba8UnormSrgb,
                None,
            ));
        app.insert_resource(SceneTarget(image))
            .init_resource::<Fade>()
            .init_resource::<RenderReadiness>()
            .add_plugins(ExtractResourcePlugin::<RenderReadiness>::default())
            .add_systems(Startup, setup_output)
            .add_systems(PreUpdate, sync_physics)
            // 相机在 StateTransition 生成，必须在其后写入纹理缩放：Bevy 只在图像资产
            // 变更或相机新增的那一帧重算 target_info，错过这一帧缩放就会一直缺失。
            .add_systems(Update, resize_target)
            .add_systems(
                Update,
                (report_asset_failure, return_to_menu, advance).chain(),
            )
            .add_systems(OnEnter(GameState::Menu), crate::gameplay::session::cleanup);
        app.sub_app_mut(RenderApp)
            .add_systems(Render, check_render_ready.in_set(RenderSystems::Cleanup));
    }
}

/// 层31只包含合成四边形，内容相机默认层0，不会递归采样自己。
fn setup_output(
    mut commands: Commands,
    target: Res<SceneTarget>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let material = materials.add(StandardMaterial {
        base_color_texture: Some(target.0.clone()),
        unlit: true,
        cull_mode: None,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(2.0, 2.0))),
        MeshMaterial3d(material.clone()),
        Transform::from_xyz(0.0, 0.0, -1.0),
        RenderLayers::layer(31),
    ));
    commands.insert_resource(CompositeMaterial(material));
    commands.spawn((
        Camera3d::default(),
        OutputCamera,
        RenderLayers::layer(31),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::Fixed {
                width: 2.0,
                height: 2.0,
            },
            ..OrthographicProjection::default_3d()
        }),
        Camera {
            order: 1,
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        Tonemapping::None,
        DebandDither::Disabled,
        Msaa::Off,
    ));
}

/// 内容纹理随真实视口变化；菜单 UI 在此叠加 scale.js 的 zoom 等比缩放，
/// 对局相机保持 1:1，HUD 不参与菜单的整体缩放。
fn resize_target(
    windows: Query<&Window>,
    target: Res<SceneTarget>,
    mut images: ResMut<Assets<Image>>,
    mut menu_cameras: Query<
        &mut RenderTarget,
        Or<(
            With<crate::pages::menu::MenuCamera>,
            With<crate::pages::gallery::GalleryCamera>,
        )>,
    >,
    mut scene_cameras: Query<
        &mut RenderTarget,
        (
            Without<crate::pages::menu::MenuCamera>,
            Without<crate::pages::gallery::GalleryCamera>,
            With<crate::gameplay::camera::FollowCamera>,
        ),
    >,
) {
    // 有窗口时以窗口为准；无窗口（离屏）时纹理尺寸即物理尺寸，缩放系数固定为 1。
    let (physical, scale_factor) = match windows.single() {
        Ok(window) => (
            window.resolution.physical_size().max(UVec2::ONE),
            window.resolution.scale_factor(),
        ),
        Err(_) => (images.get(&target.0).unwrap().size(), 1.0),
    };
    let mut image = images.get_mut(&target.0).unwrap();
    if image.size() != physical {
        image.resize(bevy::render::render_resource::Extent3d {
            width: physical.x,
            height: physical.y,
            depth_or_array_layers: 1,
        });
    }
    drop(image);
    // 菜单相机缩放系数 = 窗口缩放 × zoom；只缩不放大，高度不参与。
    let menu_scale = scale_factor * crate::pages::menu::zoom(physical.x as f32 / scale_factor);
    for mut camera in &mut menu_cameras {
        if let RenderTarget::Image(image) = &mut *camera {
            if (image.scale_factor - menu_scale).abs() > f32::EPSILON {
                image.scale_factor = menu_scale;
            }
        }
    }
    for mut camera in &mut scene_cameras {
        if let RenderTarget::Image(image) = &mut *camera {
            if (image.scale_factor - scale_factor).abs() > f32::EPSILON {
                image.scale_factor = scale_factor;
            }
        }
    }
}

/// 黑场等资产时冻结物理，防止正常帧率下玩家在地形加载前落穿地面。
fn sync_physics(fade: Res<Fade>, mut physics: ResMut<Time<Physics>>) {
    if fade.phase == Phase::Waiting {
        physics.pause();
        physics.advance_by(std::time::Duration::ZERO);
    } else {
        physics.unpause();
    }
}

/// Esc 只在对局可操作时生效；首次请求立即锁输入。
fn return_to_menu(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
    mut fade: ResMut<Fade>,
) {
    if *state.get() == GameState::InGame
        && keys.just_pressed(KeyCode::Escape)
        && fade.begin(GameState::Menu)
    {
        next.set(GameState::ToMenu);
    }
}

/// 资产错误必须可诊断，黑场等待不能把加载失败当成尚未完成。
fn report_asset_failure(
    mut failures: MessageReader<UntypedAssetLoadFailedEvent>,
    readiness: Res<RenderReadiness>,
    state: Res<State<GameState>>,
) {
    for event in failures.read() {
        let message = format!("页面资源加载失败：{}：{}", event.path, event.error);
        error!("{message}");
        // 画廊以错误卡片结束加载，保留返回入口；战斗资源错误仍阻止揭幕。
        if !matches!(state.get(), GameState::Gallery | GameState::GalleryToMenu) {
            *readiness.failure.lock().unwrap() = Some(message);
        }
    }
}

/// 就绪检查统一用于正常游戏与离屏，不以“加载成功日志”代替实体实例化。
/// 对局业务就绪汇总委托 `gameplay::session`，本模块只按页面分流并保留字体门槛。
pub(crate) fn content_ready(world: &mut World) -> bool {
    if !crate::ui::font::assets_ready(world) {
        return false;
    }
    match world.resource::<State<GameState>>().get() {
        GameState::Menu => crate::pages::menu::assets_ready(world),
        GameState::Gallery => crate::pages::gallery::assets_ready(world),
        _ => crate::gameplay::session::content_ready(world),
    }
}

/// 全黑交接后至少等待八个稳定渲染帧，避免把上一页面的 GPU 就绪值当成新页面。
fn advance(world: &mut World) {
    let delta = world.resource::<Time>().delta_secs();
    let change = world.resource_mut::<Fade>().tick(delta);
    if change {
        let target = world.resource::<Fade>().target;
        world.resource_mut::<NextState<GameState>>().set(target);
        world
            .resource::<RenderReadiness>()
            .ready
            .store(false, Ordering::Release);
    }
    if world.resource::<Fade>().phase == Phase::Waiting && !change {
        let meshes = world
            .query::<&Mesh3d>()
            .iter(world)
            .map(|mesh| mesh.0.id())
            .collect();
        let images = world.resource::<Assets<Image>>().ids().collect();
        let cpu = content_ready(world);
        let gpu = {
            let mut readiness = world.resource_mut::<RenderReadiness>();
            readiness.meshes = meshes;
            readiness.images = images;
            readiness.ready.load(Ordering::Acquire) && readiness.failure.lock().unwrap().is_none()
        };
        let mut fade = world.resource_mut::<Fade>();
        fade.stable_frames = if cpu && gpu {
            fade.stable_frames + 1
        } else {
            0
        };
        if fade.stable_frames >= 8 {
            fade.phase = Phase::In;
        }
    }
    if world.resource::<Fade>().phase == Phase::Idle
        && !world.resource::<Fade>().input_armed
        && world
            .resource::<ButtonInput<KeyCode>>()
            .get_pressed()
            .next()
            .is_none()
    {
        world.resource_mut::<Fade>().input_armed = true;
    }
    let alpha = world.resource::<Fade>().alpha;
    if let Some(material) = world
        .get_resource::<CompositeMaterial>()
        .map(|m| m.0.clone())
    {
        if let Some(mut material) = world
            .resource_mut::<Assets<StandardMaterial>>()
            .get_mut(&material)
        {
            material.base_color = Color::linear_rgb(1.0 - alpha, 1.0 - alpha, 1.0 - alpha);
        }
    }
}

/// 错误保留在共享槽内，正常运行也打印具体原因，禁止无条件揭幕。
fn check_render_ready(
    readiness: Res<RenderReadiness>,
    cache: Res<PipelineCache>,
    meshes: Res<RenderAssets<RenderMesh>>,
    images: Res<RenderAssets<GpuImage>>,
) {
    let mut pending = cache.waiting_pipelines().next().is_some();
    for pipeline in cache.pipelines() {
        if let CachedPipelineState::Err(error) = &pipeline.state {
            if matches!(
                error,
                bevy::shader::ShaderCacheError::ShaderNotLoaded(_)
                    | bevy::shader::ShaderCacheError::ShaderImportNotYetAvailable
            ) {
                pending = true;
            } else {
                let mut failure = readiness.failure.lock().unwrap();
                if failure.is_none() {
                    error!("页面渲染管线失败：{error}");
                    *failure = Some(error.to_string());
                }
            }
        }
    }
    readiness.ready.store(
        !pending
            && readiness.meshes.iter().all(|id| meshes.get(*id).is_some())
            && readiness.images.iter().all(|id| images.get(*id).is_some()),
        Ordering::Release,
    );
}

/// 报告记录实际阶段、GPU 就绪与相机目标，避免依赖终端的截断探针。
pub(crate) fn diagnostics(world: &mut World) -> String {
    let fade = world.resource::<Fade>();
    let state = world.resource::<State<GameState>>().get();
    let readiness = world.resource::<RenderReadiness>();
    let progress = format!(
        "state={state:?} phase={:?} alpha={:.4} GPU_ready={} meshes={} images={}",
        fade.phase,
        fade.alpha,
        readiness.ready.load(Ordering::Acquire),
        readiness.meshes.len(),
        readiness.images.len()
    );
    let cameras: Vec<_> = world
        .query::<(Entity, &Camera, &RenderTarget)>()
        .iter(world)
        .map(|(entity, camera, target)| {
            format!(
                "{entity:?}:active={},order={},target={target:?}",
                camera.is_active, camera.order
            )
        })
        .collect();
    format!("{progress} cameras={cameras:?}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 无地形的加载等待必须冻结重力；揭幕后继续真实物理积分。
    #[test]
    fn loading_freezes_physics_until_reveal() {
        use avian3d::prelude::*;
        use bevy::time::TimeUpdateStrategy;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugins::default()))
            .init_resource::<Fade>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_secs_f32(1.0 / 60.0),
            ))
            .add_systems(PreUpdate, sync_physics);
        app.world_mut().resource_mut::<Fade>().phase = Phase::Waiting;
        let body = app
            .world_mut()
            .spawn((
                RigidBody::Dynamic,
                Collider::cuboid(1.0, 1.0, 1.0),
                Transform::from_xyz(0.0, 4.0, 0.0),
            ))
            .id();
        app.finish();
        app.cleanup();
        for _ in 0..120 {
            app.update();
        }
        assert!((app.world().get::<Position>(body).unwrap().y - 4.0).abs() < 1e-4);
        app.world_mut().resource_mut::<Fade>().phase = Phase::In;
        for _ in 0..60 {
            app.update();
        }
        assert!(app.world().get::<Position>(body).unwrap().y < 3.0);
    }

    /// 已完成的渐入在后续任意多帧保持完成，而不是重启黑幕。
    #[test]
    fn completed_fade_does_not_restart() {
        let mut fade = Fade::default();
        assert!(fade.begin(GameState::InGame));
        assert!(fade.tick(0.6));
        assert_eq!(fade.phase, Phase::Waiting);
        fade.tick(5.0);
        assert_eq!(fade.alpha, 1.0);
        fade.phase = Phase::In;
        fade.tick(0.6);
        for _ in 0..200 {
            assert!(!fade.tick(0.1));
        }
        assert_eq!((fade.alpha, fade.phase), (0.0, Phase::Idle));
    }

    /// 两个方向均拒绝重入，直到所有跨页按键释放后才接受下一次请求。
    #[test]
    fn transition_blocks_reentry_and_held_input() {
        let mut fade = Fade::default();
        assert!(fade.begin(GameState::Menu));
        assert!(!fade.begin(GameState::InGame));
        assert!(!fade.accepts_input());
        fade.phase = Phase::In;
        fade.tick(1.0);
        assert!(!fade.accepts_input());
        fade.input_armed = true;
        assert!(fade.accepts_input());
    }
}

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
    /// 过渡终点页面；chrome 变体同步按它即时换标题/图标。
    pub(crate) target: GameState,
    /// true = ContentOnly：合成材质保持白色，由 chrome 遮罩节点只压暗页面内容，
    /// 标题栏与角标常驻；false = FullBlack：整屏（含 chrome）随材质向黑相乘。
    pub(crate) content_only: bool,
    stable_frames: u8,
    /// 阻止跨页面继续按住的键立即触发驾驶、开火或确认。
    pub input_armed: bool,
    /// 退出消息只写一次；黑场保持到进程结束，不再重复请求退出。
    exit_written: bool,
}

impl Default for Fade {
    fn default() -> Self {
        Self {
            alpha: 0.0,
            phase: Phase::Idle,
            target: GameState::Index,
            content_only: false,
            stable_frames: 0,
            input_armed: true,
            exit_written: false,
        }
    }
}

impl Fade {
    /// 重复请求被忽略；返回值用于决定是否进入过渡状态。
    /// `content_only` 由调用方按链路决定（target=Index 既可能是画廊返回，也可能是局内 Esc）。
    pub fn begin(&mut self, target: GameState, content_only: bool) -> bool {
        if self.phase != Phase::Idle || !self.input_armed {
            return false;
        }
        self.target = target;
        self.content_only = content_only;
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
            .add_message::<AppExit>()
            .add_plugins(ExtractResourcePlugin::<RenderReadiness>::default())
            .add_systems(Startup, setup_output)
            .add_systems(PreUpdate, sync_physics)
            // 相机在 StateTransition 生成，必须在其后写入纹理缩放：Bevy 只在图像资产
            // 变更或相机新增的那一帧重算 target_info，错过这一帧缩放就会一直缺失。
            .add_systems(Update, resize_target)
            .add_systems(
                Update,
                (report_asset_failure, return_to_index, advance, rearm_input).chain(),
            )
            .add_systems(OnEnter(GameState::Index), crate::gameplay::session::cleanup);
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
    mut index_cameras: Query<
        &mut RenderTarget,
        Or<(
            With<crate::pages::index::IndexCamera>,
            With<crate::pages::gallery::GalleryCamera>,
            With<crate::pages::detail::DetailCamera>,
            With<crate::app::chrome::ChromeCamera>,
        )>,
    >,
    mut scene_cameras: Query<
        &mut RenderTarget,
        (
            Without<crate::pages::index::IndexCamera>,
            Without<crate::pages::gallery::GalleryCamera>,
            Without<crate::pages::detail::DetailCamera>,
            Without<crate::app::chrome::ChromeCamera>,
            Or<(With<crate::gameplay::camera::FollowCamera>, With<crate::pages::pastoral::PreviewCamera>)>,
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
    let index_scale = scale_factor * crate::ui::zoom(physical.x as f32 / scale_factor);
    for mut camera in &mut index_cameras {
        if let RenderTarget::Image(image) = &mut *camera {
            if (image.scale_factor - index_scale).abs() > f32::EPSILON {
                image.scale_factor = index_scale;
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
fn return_to_index(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
    mut fade: ResMut<Fade>,
) {
    if matches!(state.get(), GameState::InGame | GameState::TestScene)
        && keys.just_pressed(KeyCode::Escape)
        && fade.begin(GameState::Index, false)
    {
        next.set(GameState::ToIndex);
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
        if !matches!(
            state.get(),
            GameState::Gallery
                | GameState::GalleryToIndex
                | GameState::GalleryToDetail
                | GameState::Detail
                | GameState::DetailToGallery
        ) {
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
        GameState::Index => crate::pages::index::assets_ready(world),
        GameState::TestScene => crate::pages::pastoral::content_ready(world),
        GameState::Gallery | GameState::Detail => crate::pages::model_preview::assets_ready(world),
        _ => crate::gameplay::session::content_ready(world),
    }
}

/// 全黑交接后至少等待八个稳定渲染帧，避免把上一页面的 GPU 就绪值当成新页面。
fn advance(world: &mut World) {
    let delta = world.resource::<Time>().delta_secs();
    let change = world.resource_mut::<Fade>().tick(delta);
    if change {
        let target = world.resource::<Fade>().target;
        // 退出目标不发起页面切换，黑场即终点；其余目标照常切页并等待新页面就绪。
        if target != GameState::ToExit {
            world.resource_mut::<NextState<GameState>>().set(target);
        }
        world
            .resource::<RenderReadiness>()
            .ready
            .store(false, Ordering::Release);
    }
    if world.resource::<Fade>().phase == Phase::Waiting && !change {
        if world.resource::<Fade>().target == GameState::ToExit {
            // 退出：黑场已达成，直接写退出消息（默认 runner 消费后结束进程），
            // 不再做就绪检查，也不等待揭幕；输入保持锁定到进程结束。
            if !world.resource::<Fade>().exit_written {
                world.write_message(AppExit::Success);
                world.resource_mut::<Fade>().exit_written = true;
            }
            return;
        }
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
    let fade = world.resource::<Fade>();
    let (alpha, content_only) = (fade.alpha, fade.content_only);
    if let Some(material) = world
        .get_resource::<CompositeMaterial>()
        .map(|m| m.0.clone())
    {
        if let Some(mut material) = world
            .resource_mut::<Assets<StandardMaterial>>()
            .get_mut(&material)
        {
            // ContentOnly 时材质恒白：压暗由 chrome 遮罩节点完成，标题栏/角标保持亮度。
            let shade = if content_only { 1.0 } else { 1.0 - alpha };
            material.base_color = Color::linear_rgb(shade, shade, shade);
        }
    }
}

/// 揭幕完成后释放输入锁：上一页按住的键鼠全部松开才恢复输入。
fn rearm_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut fade: ResMut<Fade>,
) {
    if fade.phase == Phase::Idle
        && !fade.input_armed
        && keys.get_pressed().next().is_none()
        && mouse.get_pressed().next().is_none()
    {
        fade.input_armed = true;
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

    /// 淡入完成仍必须等待键鼠全部释放，防止按住鼠标跨页继续拖拽。
    #[test]
    fn rearm_waits_for_mouse_and_keyboard_release() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let mut fade = Fade::default();
        fade.input_armed = false;
        world.insert_resource(fade);
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<ButtonInput<MouseButton>>();
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        world.run_system_once(rearm_input).unwrap();
        assert!(!world.resource::<Fade>().accepts_input());
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        world.run_system_once(rearm_input).unwrap();
        assert!(!world.resource::<Fade>().accepts_input());
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::Enter);
        world.run_system_once(rearm_input).unwrap();
        assert!(world.resource::<Fade>().accepts_input());
    }

    /// 已完成的渐入在后续任意多帧保持完成，而不是重启黑幕。
    #[test]
    fn completed_fade_does_not_restart() {
        let mut fade = Fade::default();
        assert!(fade.begin(GameState::InGame, false));
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
        assert!(fade.begin(GameState::Index, true));
        assert!(!fade.begin(GameState::InGame, false));
        assert!(!fade.accepts_input());
        fade.phase = Phase::In;
        fade.tick(1.0);
        assert!(!fade.accepts_input());
        fade.input_armed = true;
        assert!(fade.accepts_input());
    }

    /// 退出目标在黑场达成时直接写退出消息并终止过渡；不切页、不等揭幕。
    #[test]
    fn exit_target_exits_at_black_without_page_switch() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        world.init_resource::<Fade>();
        world.init_resource::<NextState<GameState>>();
        world.init_resource::<Time>();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<RenderReadiness>();
        world.init_resource::<Messages<AppExit>>();
        assert!(world.resource_mut::<Fade>().begin(GameState::ToExit, true));
        // 600ms 淡出完成，Out → Waiting（全黑）。
        assert!(world.resource_mut::<Fade>().tick(FADE_SECONDS));
        assert_eq!(world.resource::<Fade>().phase, Phase::Waiting);
        world.run_system_once(advance).unwrap();
        assert_eq!(world.resource::<Messages<AppExit>>().len(), 1);
        // 退出不发起页面切换，黑场保持到进程结束。
        assert!(matches!(
            world.resource::<NextState<GameState>>(),
            NextState::Unchanged
        ));
        assert_eq!(world.resource::<Fade>().phase, Phase::Waiting);
    }
}

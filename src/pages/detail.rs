//! 单模型详情：真实视觉预览、手动旋转缩放与返回原画廊位置。
use super::model_preview::{self as preview, ENTRIES, Previews};
use crate::{
    app::{
        state::GameState,
        transition::{Fade, SceneTarget},
    },
    pages::index::{IndexBackground, IndexPalette},
    ui::{
        font::UiFont,
        resources::{
            colors,
            dimens::{detail as dims, index as index_dims, title_bar as bar},
            strings, strings_cn,
        },
        widgets::{self, text_font},
        zoom,
    },
};
use bevy::{
    asset::RenderAssetUsages,
    camera::{Hdr, RenderTarget, ScalingMode, ShadowLodOrigin, visibility::RenderLayers},
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    image::{CompressedImageFormats, ImageSampler, ImageType},
    input::mouse::{MouseScrollUnit, MouseWheel},
    light::{EnvironmentMapLight, NotShadowCaster, ShadowFilteringMethod},
    material::AlphaMode,
    prelude::*,
    text::LetterSpacing,
    ui_render::prelude::MaterialNode,
};

/// 由画廊确认操作设置的目录索引；详情不接受外部路径。
#[derive(Resource, Default)]
pub(crate) struct SelectedModel(pub(crate) usize);

/// 详情页面相机，随菜单整体缩放。
#[derive(Component)]
pub(crate) struct DetailCamera;
/// 详情拥有的 UI 根和相机；预览本身由共用模块清理。
#[derive(Component)]
pub(crate) struct DetailEntity;
/// 详情 UI 根节点；屏幕层尺寸标签挂在其下，绘制顺序在模型纹理之上。
#[derive(Component)]
struct DetailUiRoot;
/// 模型子树内的 3D 尺寸轴组标记；轴线随预览实体一起清理。
#[derive(Component)]
pub(crate) struct DimAxes;
/// 内置地面纹理在首次进入详情前解码，随后由通用 GPU 揭幕检查等待上传。
#[derive(Resource)]
struct ShowroomGround {
    color: Handle<Image>,
    normal: Handle<Image>,
}

impl FromWorld for ShowroomGround {
    fn from_world(world: &mut World) -> Self {
        let mut images = world.resource_mut::<Assets<Image>>();
        let mut load = |bytes: &[u8], srgb| {
            images.add(
                Image::from_buffer(
                    bytes,
                    ImageType::Extension("png"),
                    CompressedImageFormats::NONE,
                    srgb,
                    ImageSampler::Default,
                    RenderAssetUsages::default(),
                )
                .expect("内置展示地面应为有效 PNG"),
            )
        };
        Self {
            color: load(include_bytes!("../../assets/image/detail/concrete.png"), true),
            normal: load(include_bytes!("../../assets/image/detail/concrete-normal.png"), false),
        }
    }
}

/// 展示预览按 1.5 倍分辨率渲染，缩回 UI 时减轻体素阶梯和履带细线的锯齿。
pub(crate) const SHOWROOM_RENDER_SCALE: f32 = 1.5;
/// 将展示车移到参数区右侧，按查看区宽度计。
const SHOWROOM_VIEW_ORIGIN_X: f32 = 0.57;

/// 展示场景尺寸轴抬升量（米），让长度和宽度线浮在地面上。
const SHOWROOM_AXIS_LIFT: f32 = dims::AXIS_GAP + 0.02;

/// 屏幕层尺寸标签节点标记；序号与 tag_anchors 的锚点顺序一致。
#[derive(Component)]
struct DimTag(usize);
/// 详情展示地面，供离屏验证与页面生命周期管理。
#[derive(Component)]
pub(crate) struct StudioFloor;
/// 尺寸标签数值文本（0 长 / 1 宽 / 2 高），供逐帧更新与离屏验收断言。
#[derive(Component)]
pub(crate) struct DimValue(pub(crate) usize);
/// 有装备参数时才生成的左上角详情面板。
#[derive(Component)]
pub(crate) struct SpecsPanel;
/// 玩家坦克的底部按键提示。
#[derive(Component)]
pub(crate) struct AimHelp;
/// 左上角规格区中的一条装备参数行；没有定义的型号不生成。
#[derive(Component)]
pub(crate) struct SpecRow;
/// 模型纹理填满标题栏以下区域。
#[derive(Component)]
struct Viewer;
/// 加载失败时覆盖模型查看区的说明文字。
#[derive(Component)]
struct Status;

/// 角度使用度，指针使用逻辑像素；无窗口验收注入同一资源。
#[derive(Resource)]
pub(crate) struct View {
    pub(crate) pointer: Option<Vec2>,
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
    pub(crate) zoom: f32,
    pub(crate) dragging: bool,
    last: Vec2,
}
impl Default for View {
    fn default() -> Self {
        Self {
            pointer: None,
            yaw: -28.0,
            pitch: -14.0,
            zoom: 1.0,
            dragging: false,
            last: Vec2::ZERO,
        }
    }
}
impl View {
    /// 水平方向连续旋转，俯仰限位防止翻面；相对移动不受 UI 缩放影响。
    fn rotate(&mut self, delta: Vec2) {
        self.yaw = (self.yaw + delta.x * 0.45).rem_euclid(360.0);
        // 屏幕 Y 向下；相机在 +Z，Bevy 的正 X 旋转使前侧表面下移，不能照搬 CSS 的减号。
        self.pitch = (self.pitch + delta.y * 0.45).clamp(-80.0, 80.0);
    }
    /// 测试原有模型旋转方向约定。
    #[cfg(test)]
    fn orientation(&self) -> Quat {
        // 先绕模型 Y 轴水平转动，再绕屏幕水平轴俯仰，避免偏航后上下拖拽变成侧滚。
        Quat::from_rotation_x(self.pitch.to_radians())
            * Quat::from_rotation_y(self.yaw.to_radians())
    }
    /// 滚轮正方向放大；指数映射兼容细粒度触控板和多格滚轮。
    fn scale(&mut self, lines: f32) {
        self.zoom = (self.zoom * (lines * 0.08).exp()).clamp(0.5, 1.5);
    }
}

/// 单条装备规格：标签与单位是静态文案，数值取自游戏装备定义，避免网页与玩法参数分叉。
#[derive(Clone)]
struct Spec {
    label: &'static str,
    value: String,
    unit: &'static str,
}

/// 详情只读取已校验配置，敌车不生成空行。
fn specs_for(
    entry: preview::Entry,
    ammo: &crate::tank::ammunition::AmmunitionSpec,
    tanks: &crate::tank::driving::TankConfigs,
) -> Vec<Spec> {
    match (entry.category, entry.enemy) {
        (0, _) => vec![
            Spec {
                label: strings_cn::detail::MUZZLE_VELOCITY,
                value: ammo.muzzle_velocity.to_string(),
                unit: strings::UNIT_METERS_PER_SECOND,
            },
            Spec {
                label: strings_cn::detail::RATE_OF_FIRE,
                value: ammo.rate_of_fire.to_string(),
                unit: strings::UNIT_ROUNDS_PER_MINUTE,
            },
        ],
        (1, false) => {
            let aim = tanks
                .player
                .articulation
                .expect("启动已校验豹 2A5 炮塔配置");
            vec![
                Spec {
                    label: strings_cn::detail::FORWARD_SPEED,
                    value: tanks.player.driving.forward_speed.to_string(),
                    unit: strings::UNIT_METERS_PER_SECOND,
                },
                Spec {
                    label: strings_cn::detail::REVERSE_SPEED,
                    value: format!("{:.2}", tanks.player.driving.reverse_speed)
                        .trim_end_matches('0').trim_end_matches('.').to_owned(),
                    unit: strings::UNIT_METERS_PER_SECOND,
                },
                Spec {
                    label: strings_cn::detail::TRAVERSE_RANGE,
                    // 配置校验只接受 360° 连续旋转，界面用“全圈”表达该模式。
                    value: strings_cn::detail::FULL_CIRCLE.into(),
                    unit: "",
                },
                Spec {
                    label: strings_cn::detail::ELEVATION_RANGE,
                    value: format!(
                        "{:+}{}～{:+}{}",
                        aim.min_elevation_deg, strings::UNIT_DEGREES,
                        aim.max_elevation_deg, strings::UNIT_DEGREES
                    ),
                    unit: "",
                },
                Spec {
                    label: strings_cn::detail::TRAVERSE_SPEED,
                    value: format!("{}{}", aim.traverse_speed_deg_s, strings::UNIT_DEGREES),
                    unit: strings::UNIT_PER_SECOND,
                },
                Spec {
                    label: strings_cn::detail::ELEVATION_SPEED,
                    value: format!("{}{}", aim.elevation_speed_deg_s, strings::UNIT_DEGREES),
                    unit: strings::UNIT_PER_SECOND,
                },
                Spec {
                    label: strings_cn::detail::SHOT_DAMAGE,
                    value: tanks.player.shot_damage.expect("启动已校验单发伤害").to_string(),
                    unit: "HP",
                },
                Spec {
                    label: strings_cn::detail::MAX_HEALTH,
                    value: tanks.player.max_health.expect("启动已校验生命上限").to_string(),
                    unit: "HP",
                },
            ]
        }
        _ => vec![],
    }
}

/// 安装独立详情生命周期，共用真实资源与预览就绪检查。
pub(crate) struct DetailPlugin;
impl Plugin for DetailPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SelectedModel>()
            .init_resource::<ShowroomGround>()
            .init_resource::<View>()
            .add_systems(
                OnEnter(GameState::Detail),
                (spawn_preview, spawn_ui).chain(),
            )
            // 场景实例在 Update 与 PostUpdate 之间的 SpawnScene 才生成，放在 Last 才能
            // 给同帧新出现的网格补渲染层，避免测量/验收时遇到未分层实体。
            .add_systems(
                Last,
                (preview::prepare_previews, spawn_axes)
                    .chain()
                    .run_if(in_detail),
            )
            .add_systems(
                Update,
                (
                    input.before(widgets::title_bar::interaction),
                    control_aim,
                    update_view,
                )
                    .chain()
                    .run_if(in_detail),
            )
            .add_systems(
                OnExit(GameState::DetailToGallery),
                (cleanup, preview::cleanup),
            );
    }
}

/// 渐出期间保留最后一幅详情，输入仍由 Fade 门控。
fn in_detail(state: Res<State<GameState>>) -> bool {
    matches!(state.get(), GameState::Detail | GameState::DetailToGallery)
}

/// 只组装选中的一项；背景透明，使下方平面网格可见。
fn spawn_preview(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    selected: Res<SelectedModel>,
    tanks: Res<crate::tank::driving::TankConfigs>,
) {
    preview::spawn(&mut commands, &assets, &mut images, &[ENTRIES[selected.0]], &tanks);
}

/// 内容节点按网页原型布局；常驻标题栏和角标不在页面中重复生成。
fn spawn_ui(
    mut commands: Commands,
    ammo: Res<crate::tank::ammunition::AmmunitionSpec>,
    tanks: Res<crate::tank::driving::TankConfigs>,
    target: Res<SceneTarget>,
    font: Res<UiFont>,
    previews: Res<Previews>,
    mut view: ResMut<View>,
    mut title: ResMut<widgets::TitleBarState>,
    mut materials: ResMut<Assets<IndexBackground>>,
) {
    *view = View::default();
    // 炮管朝向画面右侧，给左侧参数区留出空间。
    view.yaw = 135.0;
    view.pitch = -22.0;
    title.external_navigation = false;
    title.set_focus(false);
    commands.entity(previews.items[0].camera).insert((
        Camera {
            order: -3,
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 30.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    let camera = commands
        .spawn((
            DetailEntity,
            DetailCamera,
            Camera3d::default(),
            Camera {
                order: -1,
                ..default()
            },
            RenderTarget::Image(target.0.clone().into()),
            RenderLayers::none(),
            Tonemapping::None,
            DebandDither::Disabled,
            Msaa::Off,
        ))
        .id();
    let plate = Color::srgba(0.06, 0.055, 0.05, 0.86);
    let mut palette = IndexPalette::default();
    // 透明网格先在 sRGB 空间混合，保持原型 rgba(200,219,177,0.05) 的低对比。
    palette.grid = Vec4::ZERO;
    let background = materials.add(IndexBackground {
        viewport: Vec4::new(1280.0, 720.0, 0.0, 0.0),
        // W > 0 表示平面网格间距，Y 是查看区顶部，替换透视地面。
        cubes: Vec4::new(
            0.0,
            bar::HEIGHT,
            0.0,
            0.0,
        ),
        palette,
    });
    commands
        .spawn((
            DetailEntity,
            DetailUiRoot,
            UiTargetCamera(camera),
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                MaterialNode(background),
            ));
            root.spawn((
                Viewer,
                Node {
                    position_type: PositionType::Absolute,
                    top: px(bar::HEIGHT),
                    width: percent(100),
                    ..default()
                },
                ImageNode::new(previews.items[0].image.clone()),
            ));
            // 没有装备参数时隐藏整块面板，避免只显示分类标题。
            let spec_rows = specs_for(previews.items[0].entry, &ammo, &tanks);
            if !spec_rows.is_empty() {
            root.spawn((SpecsPanel, Node {
                position_type: PositionType::Absolute,
                left: px(crate::ui::resources::dimens::hud::INSET),
                top: px(bar::HEIGHT + crate::ui::resources::dimens::hud::INSET),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexStart,
                row_gap: px(dims::SPECS_GAP),
                padding: UiRect::all(px(12.0)),
                ..default()
            }, BackgroundColor(plate)))
                .with_children(|specs| {
                    specs
                        .spawn((Node {
                            align_items: AlignItems::Center,
                            column_gap: px(dims::EYEBROW_GAP),
                            ..default()
                        },))
                        .with_children(|eyebrow| {
                            eyebrow.spawn((
                                Node {
                                    width: px(dims::EYEBROW_LINE.0),
                                    height: px(dims::EYEBROW_LINE.1),
                                    ..default()
                                },
                                BackgroundColor(colors::PRIMARY),
                            ));
                            eyebrow.spawn((
                                Text::new(
                                    strings_cn::gallery::CATEGORIES
                                        [previews.items[0].entry.category],
                                ),
                                text_font(&font, dims::EYEBROW_SIZE, 400.0),
                                LetterSpacing::Px(dims::EYEBROW_SPACING),
                                TextColor(colors::PRIMARY),
                            ));
                        });
                    specs
                        .spawn(Node {
                            display: Display::Grid,
                            grid_template_columns: RepeatedGridTrack::px(
                                if previews.items[0].entry.category == 1
                                    && !previews.items[0].entry.enemy
                                {
                                    2
                                } else {
                                    1
                                },
                                dims::SPEC_COLUMN_WIDTH,
                            ),
                            column_gap: px(dims::SPEC_COLUMN_GAP),
                            row_gap: px(dims::SPECS_GAP),
                            ..default()
                        })
                        .with_children(|rows| {
                            for spec in spec_rows {
                                rows.spawn((
                                    SpecRow,
                                    Node {
                                        flex_direction: FlexDirection::Column,
                                        row_gap: px(dims::SPEC_LABEL_GAP),
                                        ..default()
                                    },
                                ))
                                .with_children(|row| {
                                    row.spawn((
                                        Text::new(spec.label),
                                        text_font(&font, dims::SPEC_LABEL_SIZE, 400.0),
                                        LetterSpacing::Px(dims::SPEC_LABEL_SPACING),
                                        TextColor(colors::ON_SURFACE_MUTED),
                                    ));
                                    row.spawn((Node {
                                        flex_direction: FlexDirection::Row,
                                        align_items: AlignItems::Baseline,
                                        column_gap: px(dims::SPEC_VALUE_GAP),
                                        ..default()
                                    },))
                                        .with_children(|value| {
                                            // 角度符号与数字同字形排版，保留 ° 自身的右上位置。
                                            value.spawn((
                                                Text::new(spec.value),
                                                text_font(&font, dims::SPEC_VALUE_SIZE, 700.0),
                                                TextColor(colors::gallery::NAME_ACTIVE),
                                            ));
                                            if !spec.unit.is_empty() {
                                                value.spawn((
                                                    Text::new(spec.unit),
                                                    text_font(&font, dims::SPEC_UNIT_SIZE, 400.0),
                                                    LetterSpacing::Px(dims::SPEC_UNIT_SPACING),
                                                    TextColor(colors::ON_SURFACE_MUTED),
                                                ));
                                            }
                                        });
                                });
                            }
                        });
                });
            }
            root.spawn((
                Status,
                Node {
                    position_type: PositionType::Absolute,
                    top: percent(50),
                    width: percent(100),
                    ..default()
                },
                Text::new(""),
                TextLayout::justify(Justify::Center),
                text_font(&font, 18.0, 400.0),
                TextColor(colors::ON_SURFACE),
            ));
            if previews.items[0].entry.category == 1 && !previews.items[0].entry.enemy {
                root.spawn((AimHelp, Node {
                    position_type: PositionType::Absolute,
                    bottom: px(crate::ui::resources::dimens::hud::INSET),
                    width: percent(100),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    column_gap: px(dims::AIM_HELP_GROUP_GAP),
                    ..default()
                }))
                .with_children(|hint| {
                    for (keycaps, label) in [
                        (
                            [strings::KEY_W, strings::KEY_S].as_slice(),
                            strings_cn::detail::GUN_ELEVATION,
                        ),
                        (
                            [strings::KEY_A, strings::KEY_D].as_slice(),
                            strings_cn::detail::TURRET_TRAVERSE,
                        ),
                        ([strings::KEY_R].as_slice(), strings_cn::detail::TURRET_RESET),
                        ([strings::KEY_F].as_slice(), strings_cn::detail::GUN_RESET),
                    ] {
                        hint.spawn((Node {
                            align_items: AlignItems::Center,
                            column_gap: px(index_dims::HINT_ITEM_GAP),
                            padding: UiRect::axes(px(8.0), px(4.0)),
                            ..default()
                        }, BackgroundColor(plate)))
                        .with_children(|group| {
                            for key in keycaps {
                                widgets::spawn_keycap(group, &font, key, ());
                            }
                            group.spawn((
                                Text::new(label),
                                text_font(&font, dims::AIM_HELP_SIZE, 400.0),
                                TextColor(colors::ON_SURFACE_MUTED),
                            ));
                        });
                    }
                });
            }
            widgets::spawn_version_text(root, &font);
        });
}

/// 包围盒测得后生成尺寸轴与屏幕层标签；只生成一次，随详情与预览清理。
#[allow(clippy::too_many_arguments)]
fn spawn_axes(
    mut commands: Commands,
    previews: Res<Previews>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut pbr_materials: ResMut<Assets<StandardMaterial>>,
    font: Res<UiFont>,
    ground: Res<ShowroomGround>,
    mut environment_maps: Query<&mut EnvironmentMapLight>,
    spawned: Query<(), With<DimAxes>>,
    roots: Query<Entity, With<DetailUiRoot>>,
) {
    if !spawned.is_empty() {
        return;
    }
    let p = &previews.items[0];
    let Some(bounds) = p.bounds else {
        return;
    };
    let Ok(ui_root) = roots.single() else {
        return;
    };
    {
        let scale = (p.size.unwrap().max_element() / 11.0).max(0.03);
        // 展示场景与画廊使用独立渲染层，地面取静止模型的履带最低点。
        commands.spawn((
            DetailEntity,
            StudioFloor,
            RenderLayers::layer(1),
            Mesh3d(meshes.add(
                Plane3d::default()
                    .mesh()
                    .size(140.0 * scale, 140.0 * scale)
                    .build()
                    .with_generated_tangents()
                    .expect("平面网格应有有效切线"),
            )),
            MeshMaterial3d(pbr_materials.add(StandardMaterial {
                base_color: Color::linear_rgb(0.45, 0.48, 0.51),
                base_color_texture: Some(ground.color.clone()),
                normal_map_texture: Some(ground.normal.clone()),
                perceptual_roughness: 0.8,
                reflectance: 0.32,
                ..default()
            })),
            Transform::from_xyz(0.0, bounds.0.y - p.pivot.y - if p.entry.category == 0 { p.size.unwrap().max_element() * 0.25 } else { 0.002 * scale }, 0.0),
        ));
        preview::spawn_studio_lights(&mut commands, 1, scale, true);
        if let Ok(mut environment) = environment_maps.get_mut(p.camera) {
            environment.intensity = 85.0;
        }
        let backdrop = Color::srgb(0.33, 0.31, 0.29);
        commands.entity(p.camera).insert((
            Camera {
                order: -3,
                clear_color: ClearColorConfig::Custom(backdrop),
                ..default()
            },
            Hdr,
            ShadowLodOrigin,
            ShadowFilteringMethod::Gaussian,
            bevy::pbr::ScreenSpaceAmbientOcclusion::default(),
            // 覆盖对局的强全局环境光，给展示台保留接触阴影和明暗差。
            AmbientLight {
                color: Color::srgb(0.82, 0.87, 1.0),
                brightness: 260.0,
                ..default()
            },
            // 淡化远处地面边缘，低角度下也看不到台面断边。
            DistanceFog {
                color: backdrop,
                falloff: FogFalloff::Linear { start: 43.0, end: 65.0 },
                ..default()
            },
        ));
    }
    // 三种线材质：主轴、刻度、延长线；无光照混色，跟随模型旋转缩放。
    let materials = [
        StandardMaterial {
            base_color: colors::detail::AXIS,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        },
        StandardMaterial {
            base_color: colors::detail::TICK,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        },
        StandardMaterial {
            base_color: colors::detail::EXT,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        },
    ]
    .map(|material| pbr_materials.add(material));
    let segments = axis_segments(bounds);
    // 旧尺寸线位于履带下方；有地面时把整组标注抬到地面以上。
    let lift = SHOWROOM_AXIS_LIFT * dimension_scale(bounds);
    commands.entity(p.content).with_children(|axes| {
        axes.spawn((DimAxes, Transform::from_xyz(0.0, lift, 0.0), Visibility::Inherited))
            .with_children(|group| {
                for (center, size, kind) in segments {
                    group.spawn((
                        Mesh3d(meshes.add(Cuboid::from_size(size))),
                        MeshMaterial3d(materials[kind].clone()),
                        Transform::from_translation(center),
                        NotShadowCaster,
                    ));
                }
            });
    });
    // 屏幕层尺寸标签：底板+细边框，行内基线排列；锚点位置由 update_view 逐帧投影。
    for (i, name) in strings_cn::detail::DIMENSIONS.iter().enumerate() {
        commands.entity(ui_root).with_children(|tags| {
            tags.spawn((
                DimTag(i),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Baseline,
                    column_gap: px(6.0),
                    padding: UiRect::px(8.0, 8.0, 4.0, 4.0),
                    border: UiRect::all(px(1.0)),
                    ..default()
                },
                BackgroundColor(colors::detail::TAG_PLATE),
                BorderColor::all(colors::detail::TAG_BORDER),
            ))
            .with_children(|tag| {
                tag.spawn((
                    Text::new(*name),
                    text_font(&font, dims::TAG_NAME_SIZE, 400.0),
                    TextColor(colors::ON_SURFACE_MUTED),
                ));
                tag.spawn((
                    DimValue(i),
                    Text::new("—"),
                    text_font(&font, dims::TAG_VALUE_SIZE, 700.0),
                    TextColor(colors::gallery::NAME_ACTIVE),
                ));
                tag.spawn((
                    Text::new(strings_cn::gallery::UNIT),
                    text_font(&font, dims::TAG_UNIT_SIZE, 400.0),
                    TextColor(colors::ON_SURFACE_MUTED),
                ));
            });
        });
    }
}

/// 以 10 米级整车为标尺缩小小模型的标注；避免弹丸的线宽和间距盖住本体。
fn dimension_scale(bounds: (Vec3, Vec3)) -> f32 {
    ((bounds.1 - bounds.0).max_element() / 10.0).min(1.0)
}

/// 尺寸轴几何（content 局部空间，米）。布局复刻网页 buildDimAxes：
/// 长度轴沿前缘正下方沿 X、宽度轴沿右缘正下方沿 Z，在右前下角相接成 L；
/// 高度轴在右前角外侧沿 Y；端点带刻度，角点引出延长线。
/// 返回 (中心, 尺寸, 材质序号 0 轴线 / 1 刻度 / 2 延长线)。
fn axis_segments(bounds: (Vec3, Vec3)) -> Vec<(Vec3, Vec3, usize)> {
    let scale = dimension_scale(bounds);
    let (low, high) = (bounds.0 / scale, bounds.1 / scale);
    let ground = low.y - dims::AXIS_GAP; // 轴线地面：模型最低点再向下偏移
    let t = dims::AXIS_THICKNESS;
    let tick = dims::HALF_TICK * 2.0;
    let mut segments = Vec::new();
    let mut push = |center: Vec3, size: Vec3, kind: usize| segments.push((center, size, kind));
    // 长度轴：前缘正下方沿 X；宽度轴：右缘正下方沿 Z
    push(
        Vec3::new((low.x + high.x) / 2.0, ground, high.z),
        Vec3::new(high.x - low.x, t, t),
        0,
    );
    push(
        Vec3::new(high.x, ground, (low.z + high.z) / 2.0),
        Vec3::new(t, t, high.z - low.z),
        0,
    );
    // 长度轴端点刻度 + 角点向下引出的延长线
    for xe in [low.x, high.x] {
        push(Vec3::new(xe, ground, high.z), Vec3::new(t, t, tick), 1);
        push(
            Vec3::new(xe, (low.y + ground) / 2.0, high.z),
            Vec3::new(t, dims::AXIS_GAP, t),
            2,
        );
    }
    // 宽度轴端点刻度 + 延长线（右前角延长线已由长度轴画出，跳过）
    for ze in [low.z, high.z] {
        push(Vec3::new(high.x, ground, ze), Vec3::new(tick, t, t), 1);
        if ze != high.z {
            push(
                Vec3::new(high.x, (low.y + ground) / 2.0, ze),
                Vec3::new(t, dims::AXIS_GAP, t),
                2,
            );
        }
    }
    // 高度轴：右前角外侧沿 Y
    push(
        Vec3::new(high.x + dims::AXIS_GAP, (low.y + high.y) / 2.0, high.z),
        Vec3::new(t, high.y - low.y, t),
        0,
    );
    for ye in [low.y, high.y] {
        // 上下端水平延长线引自模型角点；刻度取十字两向，任意偏航不侧边消失
        push(
            Vec3::new(high.x + dims::AXIS_GAP / 2.0, ye, high.z),
            Vec3::new(dims::AXIS_GAP, t, t),
            2,
        );
        push(
            Vec3::new(high.x + dims::AXIS_GAP, ye, high.z),
            Vec3::new(tick, t, t),
            1,
        );
        push(
            Vec3::new(high.x + dims::AXIS_GAP, ye, high.z),
            Vec3::new(t, t, tick),
            1,
        );
    }
    segments
        .into_iter()
        .map(|(center, size, kind)| (center * scale, size * scale, kind))
        .collect()
}

/// 尺寸标签锚点（content 局部空间，米）：长/宽钉在各自轴线中点，高度钉在轴外侧中点。
fn tag_anchors(bounds: (Vec3, Vec3)) -> [Vec3; 3] {
    let scale = dimension_scale(bounds);
    let (low, high) = (bounds.0 / scale, bounds.1 / scale);
    let ground = low.y - dims::AXIS_GAP;
    [
        Vec3::new((low.x + high.x) / 2.0, ground, high.z),
        Vec3::new(high.x, ground, (low.z + high.z) / 2.0),
        Vec3::new(
            high.x + dims::AXIS_GAP + dims::HEIGHT_TAG_OFFSET,
            (low.y + high.y) / 2.0,
            high.z,
        ),
    ]
    .map(|anchor| anchor * scale)
}

/// 模型空间锚点 → 查看区逻辑坐标。复现预览链路的正交投影：
/// 将锚点旋入相机坐标系，相机距原点 30 米，再按取景高换算 NDC 与图像像素，
/// 最后按查看区尺寸映射为逻辑坐标；与网页 projectToScreen 同构。
#[allow(clippy::too_many_arguments)]
fn project_to_viewer(
    anchor: Vec3,
    orientation: Quat,
    pivot_shift: Vec3,
    viewport_height: f32,
    aspect: f32,
    image_size: Vec2,
    viewer: Vec2,
) -> Vec2 {
    let world = orientation * (anchor + pivot_shift);
    let view = world - Vec3::new(0.0, 0.0, 30.0);
    let half = viewport_height / 2.0;
    let ndc = Vec2::new(view.x / (half * aspect), view.y / half);
    let pixel = Vec2::new(
        (ndc.x + 1.0) / 2.0 * image_size.x,
        (1.0 - ndc.y) / 2.0 * image_size.y,
    );
    // 图像恰好铺满查看区，物理像素线性映射为逻辑坐标
    Vec2::new(
        pixel.x / image_size.x * viewer.x,
        pixel.y / image_size.y * viewer.y,
    )
}

/// 控件只发返回消息，详情决定返回画廊；失焦/过渡立即取消拖拽。
#[allow(clippy::too_many_arguments)]
fn input(
    windows: Query<&Window>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut wheel: MessageReader<MouseWheel>,
    mut activated: MessageReader<widgets::TitleBarBackActivated>,
    mut view: ResMut<View>,
    images: Res<Assets<Image>>,
    target: Res<SceneTarget>,
    mut fade: ResMut<Fade>,
    mut next: ResMut<NextState<GameState>>,
) {
    let size = crate::ui::logical_viewport(&windows, &images, &target.0);
    let focused = windows.single().map_or(true, |w| w.focused);
    if let Ok(window) = windows.single() {
        view.pointer = window.cursor_position().map(|p| p / zoom(window.width()));
    }
    let scale = windows.single().map_or_else(
        |_| zoom(images.get(&target.0).unwrap().size().x as f32),
        |w| zoom(w.width()),
    );
    let delta: f32 = wheel
        .read()
        .map(|event| {
            event.y
                * if event.unit == MouseScrollUnit::Line {
                    1.0
                } else {
                    1.0 / (64.0 * scale)
                }
        })
        .sum();
    let back = activated.read().next().is_some();
    if !fade.accepts_input() || !focused {
        view.dragging = false;
        return;
    }
    if back && fade.begin(GameState::Gallery, true) {
        view.dragging = false;
        next.set(GameState::DetailToGallery);
        return;
    }
    let inside = view
        .pointer
        .is_some_and(|p| p.x >= 0.0 && p.x < size.x && p.y >= bar::HEIGHT && p.y < size.y);
    if mouse.just_pressed(MouseButton::Left) && inside {
        view.dragging = true;
        view.last = view.pointer.unwrap();
    }
    if view.dragging {
        if let Some(pointer) = view.pointer {
            let mut delta = pointer - view.last;
            // 相机俯仰与模型俯仰互为逆变换，拖拽仍沿用抓住车体的方向。
            delta.y = -delta.y;
            view.rotate(delta);
            view.pitch = view.pitch.min(-8.0);
            view.last = pointer;
        } else {
            view.dragging = false;
        }
    }
    if !mouse.pressed(MouseButton::Left) {
        view.dragging = false;
    }
    if inside {
        view.scale(delta);
    }
}

/// 仅就绪的豹 2A5 详情接收按键，预览角度保存在独立实体，离页即销毁。
fn control_aim(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    windows: Query<&Window>,
    fade: Res<Fade>,
    previews: Res<Previews>,
    mut aims: Query<(
        &mut crate::tank::articulation::TankAim,
        &crate::tank::articulation::AimSpec,
        &mut crate::tank::articulation::AimReset,
    )>,
) {
    let preview = &previews.items[0];
    if !fade.accepts_input()
        || windows.iter().any(|window| !window.focused)
        || preview.size.is_none()
        || preview.error.is_some()
    {
        return;
    }
    if let Ok((mut aim, spec, mut reset)) = aims.get_mut(preview.content) {
        let yaw =
            (i32::from(keys.pressed(KeyCode::KeyA)) - i32::from(keys.pressed(KeyCode::KeyD))) as f32;
        let pitch =
            (i32::from(keys.pressed(KeyCode::KeyW)) - i32::from(keys.pressed(KeyCode::KeyS))) as f32;
        // 单按 R/F 触发对应轴回正；对应轴有主动输入时输入优先，取消回正。
        if keys.just_pressed(KeyCode::KeyR) {
            reset.yaw = true;
        }
        if keys.just_pressed(KeyCode::KeyF) {
            reset.pitch = true;
        }
        if yaw != 0.0 {
            reset.yaw = false;
        }
        if pitch != 0.0 {
            reset.pitch = false;
        }
        spec.advance(&mut aim, yaw, pitch, time.delta_secs());
        spec.reset_step(&mut aim, &mut reset, time.delta_secs());
    }
}

/// 尺寸、纹理和相机随窗口变化；旋转包络按纵横比适配，主动放大允许裁切。
/// 尺寸标签逐帧投影轴上锚点，跟随旋转、缩放与窗口变化，字号恒定。
#[allow(clippy::too_many_arguments)]
fn update_view(
    windows: Query<&Window>,
    target: Res<SceneTarget>,
    view: Res<View>,
    previews: Res<Previews>,
    mut images: ResMut<Assets<Image>>,
    mut transforms: Query<&mut Transform>,
    mut projections: Query<&mut Projection>,
    mut viewers: Query<&mut Node, With<Viewer>>,
    mut values: Query<(&DimValue, &mut Text), Without<Status>>,
    mut tags: Query<(&DimTag, &mut Node, Option<&ComputedNode>), (With<DimTag>, Without<Viewer>)>,
    mut status: Query<&mut Text, With<Status>>,
    mut materials: ResMut<Assets<IndexBackground>>,
) {
    let size = crate::ui::logical_viewport(&windows, &images, &target.0);
    let height = (size.y - bar::HEIGHT).max(1.0);
    let p = &previews.items[0];
    let physical = images.get(&target.0).unwrap().size().as_vec2();
    let density = physical.x / size.x;
    // 超采样只用于常见窗口尺寸，避免高分屏进一步放大 GPU 纹理与带宽。
    let render_scale = if physical.max_element() <= 2560.0 {
        SHOWROOM_RENDER_SCALE
    } else {
        1.0
    };
    let desired = UVec2::new(
        (physical.x * render_scale).round() as u32,
        (height * density * render_scale).round().max(1.0) as u32,
    );
    let mut image = images.get_mut(&p.image).unwrap();
    if image.size() != desired {
        image.resize(bevy::render::render_resource::Extent3d {
            width: desired.x,
            height: desired.y,
            depth_or_array_layers: 1,
        });
    }
    for mut node in &mut viewers {
        node.height = px(height);
        node.display = if p.error.is_some() {
            Display::None
        } else {
            Display::Flex
        };
    }
    // 相机环绕固定模型，弹丸与坦克都保持各自的展示姿态。
    let orbit = Quat::from_rotation_y(-view.yaw.to_radians())
        * Quat::from_rotation_x(view.pitch.to_radians());
    if let Ok(mut camera) = transforms.get_mut(p.camera) {
        *camera = Transform::from_translation(orbit * Vec3::Z * 30.0).with_rotation(orbit);
    }
    let orientation = orbit.inverse();
    let mut viewport_height = 1.0;
    if let Ok(mut projection) = projections.get_mut(p.camera) {
        if let Projection::Orthographic(ref mut ortho) = *projection {
            let framing = 0.60;
            ortho.viewport_origin.x = SHOWROOM_VIEW_ORIGIN_X;
            // 低仰角缩小时，近侧地面也要覆盖正交视口底部。
            ortho.near = -35.0;
            let vh = (p.radius * 2.0 * framing / (size.x / height).min(1.0) / view.zoom).max(0.1);
            ortho.scaling_mode = ScalingMode::FixedVertical {
                viewport_height: vh,
            };
            viewport_height = vh;
        }
    }
    for (dimension, mut text) in &mut values {
        let value = p.size.map_or_else(
            || "—".into(),
            |s| format!("{:.1}", [s.x, s.z, s.y][dimension.0]),
        );
        if text.0 != value {
            text.0 = value;
        }
    }
    // 尺寸标签定位：与 detail.rs 取景同一套正交投影（pivot 平移 → 旋转 → 30 米相机），
    // 图像像素按查看区尺寸映射为逻辑坐标；底板以自身中心对准投影点，并沿
    // “锚点 → 屏幕外侧”方向推开 LABEL_PUSH，避免压住模型。
    if let Some(bounds) = p.bounds {
        let viewer = Vec2::new(size.x, height);
        // content 平移为 -pivot（见 model_preview 的测量），投影前还原该平移
        let lift = SHOWROOM_AXIS_LIFT * dimension_scale(bounds);
        let pivot_shift = -p.pivot + Vec3::Y * lift;
        let image_size = Vec2::new(desired.x as f32, desired.y as f32);
        for (tag, mut node, computed) in &mut tags {
            let mut pos = project_to_viewer(
                tag_anchors(bounds)[tag.0],
                orientation,
                pivot_shift,
                viewport_height,
                image_size.x / image_size.y,
                image_size,
                viewer,
            );
            pos.x += (SHOWROOM_VIEW_ORIGIN_X - 0.5) * viewer.x;
            // 与网页 projectToScreen 的 x*(1+k) 一致：沿“锚点 → 远离屏幕中心”
            // 的离心方向推开 LABEL_PUSH，避免标签压在模型上。
            let to_edge = pos - viewer / 2.0;
            let len = to_edge.length();
            if len > 1.0 {
                pos += to_edge / len * dims::LABEL_PUSH;
            }
            if let Some(computed) = computed {
                // ComputedNode.size 是物理像素，换算回逻辑像素再取负，让底板中心对准投影点
                node.margin = UiRect::px(
                    -computed.size.x / density / 2.0,
                    0.0,
                    -computed.size.y / density / 2.0,
                    0.0,
                );
            }
            // 标签挂在全屏根节点下，pos 相对查看区顶部，需叠加标题栏高度
            node.left = px(pos.x);
            node.top = px(bar::HEIGHT + pos.y);
        }
    }
    for mut text in &mut status {
        let value = if p.error.is_some() {
            strings_cn::gallery::FAILED
        } else if p.size.is_none() {
            strings_cn::gallery::LOADING
        } else {
            ""
        };
        if text.0 != value {
            text.0 = value.into();
        }
    }
    for (_, material) in materials.iter_mut() {
        material.viewport = Vec4::new(size.x, size.y, 0.0, 0.0);
    }
}

/// 黑场时删除详情 UI，预览的场景实例与动态纹理由共用模块释放。
fn cleanup(mut commands: Commands, roots: Query<Entity, With<DetailEntity>>) {
    for root in &roots {
        commands.entity(root).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// 只安装真实输入系统所需资源，避免用独立模拟替代拖拽状态机。
    fn input_app() -> App {
        let mut app = App::new();
        let mut images = Assets::<Image>::default();
        let image = images.add(Image::new_target_texture(
            1280,
            720,
            bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
            None,
        ));
        app.add_plugins(MinimalPlugins)
            .insert_resource(images)
            .insert_resource(SceneTarget(image))
            .init_resource::<View>()
            .init_resource::<SelectedModel>()
            .init_resource::<Fade>()
            .init_resource::<NextState<GameState>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<MouseWheel>()
            .add_message::<widgets::TitleBarBackActivated>()
            .add_systems(Update, input);
        app
    }

    /// 独立弹丸初始取景也必须容纳尺寸轴，不能沿用坦克米制线宽和偏移。
    #[test]
    fn ammunition_axes_fit_the_model_scale() {
        let half = Vec3::new(0.36, 0.05, 0.05);
        let bounds = (-half, half);
        for (_, size, _) in axis_segments(bounds) {
            // 弹丸长度外，轴线的各向厚度/刻度均不应超过 5 厘米。
            assert!(size.min_element() < 0.002);
            assert!(size.y < 0.11 && size.z < 0.11);
        }
        assert!(
            tag_anchors(bounds)
                .iter()
                .all(|anchor| anchor.length() < 0.45)
        );
    }

    /// 标题栏不开始拖拽，查看区开始后进入标题栏仍持续，松开/过渡/失焦均取消。
    #[test]
    fn drag_lifecycle_uses_viewer_hit_and_input_lock() {
        let mut app = input_app();
        app.world_mut().resource_mut::<View>().pointer = Some(Vec2::new(400.0, 40.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(!app.world().resource::<View>().dragging);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
        app.world_mut().resource_mut::<View>().pointer = Some(Vec2::new(600.0, 400.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(app.world().resource::<View>().dragging);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        app.world_mut().resource_mut::<View>().pointer = Some(Vec2::new(650.0, 40.0));
        app.update();
        assert_ne!(app.world().resource::<View>().yaw, -28.0);
        assert!(app.world().resource::<View>().dragging);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        assert!(!app.world().resource::<View>().dragging);
        app.world_mut().resource_mut::<View>().dragging = true;
        app.world_mut()
            .resource_mut::<Fade>()
            .begin(GameState::Gallery, true);
        let before = app.world().resource::<View>().yaw;
        app.world_mut().resource_mut::<View>().pointer = Some(Vec2::new(800.0, 500.0));
        app.update();
        assert!(!app.world().resource::<View>().dragging);
        assert_eq!(app.world().resource::<View>().yaw, before);
        app.world_mut().insert_resource(Fade::default());
        app.world_mut().resource_mut::<View>().dragging = true;
        app.world_mut().spawn(Window {
            focused: false,
            ..default()
        });
        app.update();
        assert!(!app.world().resource::<View>().dragging);
    }

    /// 查看区外滚轮不改变缩放；返回消息交给实际 Fade 而非直接切状态。
    #[test]
    fn wheel_is_viewer_only_and_back_uses_transition() {
        let mut app = input_app();
        for (point, expected) in [
            (Vec2::new(500.0, 40.0), 1.0),
            (Vec2::new(500.0, 400.0), 1.5),
        ] {
            app.world_mut().resource_mut::<View>().pointer = Some(point);
            app.world_mut().write_message(MouseWheel {
                unit: MouseScrollUnit::Line,
                x: 0.0,
                y: 100.0,
                window: Entity::PLACEHOLDER,
                phase: bevy::input::touch::TouchPhase::Moved,
            });
            app.update();
            assert_eq!(app.world().resource::<View>().zoom, expected);
        }
        app.world_mut()
            .write_message(widgets::TitleBarBackActivated);
        app.update();
        let fade = app.world().resource::<Fade>();
        assert_eq!(fade.target, GameState::Gallery);
        assert!(fade.content_only && !fade.accepts_input());
        assert!(matches!(
            *app.world().resource::<NextState<GameState>>(),
            NextState::Pending(GameState::DetailToGallery)
        ));
    }

    /// 相机位于 +Z、屏幕向下为 +Y：前侧表面必须随拖拽下移，且不随水平转角反向。
    #[test]
    fn vertical_drag_follows_pointer_at_every_yaw() {
        for yaw in [0.0_f32, 45.0, 90.0, 180.0, 270.0] {
            let mut view = View {
                yaw,
                pitch: 0.0,
                ..default()
            };
            let front = Quat::from_rotation_y(-yaw.to_radians()) * Vec3::Z;
            let before = view.orientation() * front;
            view.rotate(Vec2::new(0.0, 20.0));
            let after = view.orientation() * front;
            assert!(
                after.y < before.y - 0.1,
                "向下拖拽时，前侧表面应下移：yaw={yaw}, before={before:?}, after={after:?}"
            );
            view.rotate(Vec2::new(0.0, -40.0));
            let up = view.orientation() * front;
            assert!(
                up.y > before.y + 0.1,
                "向上拖拽时，前侧表面应上移：yaw={yaw}"
            );
        }
    }

    /// 交互边界不依赖帧率；大幅拖拽或滚轮仍保持有限姿态和缩放。
    #[test]
    fn rotation_and_zoom_clamp_and_reset() {
        let mut view = View::default();
        view.rotate(Vec2::new(10000.0, -10000.0));
        assert_eq!(view.pitch, -80.0);
        assert!((0.0..360.0).contains(&view.yaw));
        view.rotate(Vec2::new(0.0, 10000.0));
        assert_eq!(view.pitch, 80.0);
        view.scale(1000.0);
        assert_eq!(view.zoom, 1.5);
        view.scale(-1000.0);
        assert_eq!(view.zoom, 0.5);
        view = View::default();
        assert_eq!(
            (view.yaw, view.pitch, view.zoom, view.dragging),
            (-28.0, -14.0, 1.0, false)
        );
    }
    /// 详情显示传入配置，调参不会只改变玩法而留下旧的界面数字。
    #[test]
    fn specs_follow_equipment_configuration() {
        let mut ammo = crate::tank::ammunition::AmmunitionSpec::load(
            std::path::Path::new("assets"),
            crate::tank::visual::PLAYER_BULLET,
        )
        .unwrap();
        ammo.muzzle_velocity = 600.0;
        ammo.rate_of_fire = 30.0;
        let mut tanks =
            crate::tank::driving::TankConfigs::load(std::path::Path::new("assets")).unwrap();
        tanks.player.driving.forward_speed = 21.0;
        tanks.player.driving.reverse_speed = 5.0;
        tanks.player.shot_damage = std::num::NonZeroU32::new(470);
        tanks.player.max_health = std::num::NonZeroU32::new(3900);
        let aim = tanks.player.articulation.as_mut().unwrap();
        aim.traverse_speed_deg_s = 25.0;
        aim.elevation_speed_deg_s = 8.0;
        aim.min_elevation_deg = -5.0;
        aim.max_elevation_deg = 15.0;
        let rows = specs_for(ENTRIES[0], &ammo, &tanks);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].value, "600");
        assert_eq!(rows[1].value, "30");
        let rows = specs_for(ENTRIES[1], &ammo, &tanks);
        assert_eq!(rows.len(), 8);
        assert_eq!(rows[0].value, "21");
        assert_eq!(rows[1].value, "5");
        assert_eq!(rows[2].value, "全圈");
        assert!(rows[2].unit.is_empty());
        assert_eq!(rows[3].value, "-5°～+15°");
        assert_eq!(rows[4].value, "25°");
        assert_eq!(rows[5].value, "8°");
        assert_eq!(rows[6].value, "470");
        assert_eq!(rows[7].value, "3900");
        assert_eq!(rows[4].unit, strings::UNIT_PER_SECOND);
        assert!(specs_for(ENTRIES[2], &ammo, &tanks).is_empty());
    }
}

//! 田园视觉样板：独立布景、光照与自由相机，不挂载对局玩法。

use crate::{
    app::{
        state::GameState,
        transition::{Fade, SceneTarget},
    },
    ui::font::UiFont,
};
use bevy::{
    camera::RenderTarget, core_pipeline::tonemapping::Tonemapping,
    input::mouse::AccumulatedMouseMotion, prelude::*,
};

/// 相机及场景所有权分别标记，退出时统一释放实体与资产句柄。
#[derive(Component)]
pub(crate) struct PreviewCamera;
#[derive(Component)]
pub(crate) struct PreviewEntity;

/// 安装独立页面；返回首页后才清理，保留完整的退出淡出画面。
pub(crate) struct PastoralPlugin;
impl Plugin for PastoralPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::TestScene), setup)
            .add_systems(OnEnter(GameState::Index), cleanup)
            .add_systems(Update, fly.run_if(in_state(GameState::TestScene)));
    }
}

/// 固定色盘：草、叶、土、石、灰泥、瓦片、木、水与白色装饰。
const PALETTE: [[f32; 3]; 15] = [
    [0.36, 0.43, 0.19],
    [0.39, 0.46, 0.21],
    [0.23, 0.33, 0.10],
    [0.35, 0.43, 0.13],
    [0.47, 0.53, 0.18],
    [0.63, 0.52, 0.35],
    [0.70, 0.61, 0.45],
    [0.57, 0.57, 0.49],
    [0.70, 0.68, 0.58],
    [0.80, 0.76, 0.62],
    [0.56, 0.27, 0.14],
    [0.65, 0.35, 0.19],
    [0.27, 0.22, 0.13],
    [0.24, 0.40, 0.45],
    [0.87, 0.87, 0.73],
];

/// 颜色分批的立方体网格；每批单个实体，避免逐草叶/石块创建实体。
struct Blocks(Vec<Option<Mesh>>);
impl Blocks {
    fn new() -> Self {
        Self((0..PALETTE.len()).map(|_| None).collect())
    }
    fn block(&mut self, color: usize, center: Vec3, size: Vec3) {
        let mut mesh =
            Mesh::from(Cuboid::from_size(size)).transformed_by(Transform::from_translation(center));
        // 同色材质内保留小幅块面差异，避免石墙和瓦片呈机械棋盘格。
        let shade = 0.86
            + noise(
                (center.x * 31.0 + center.y * 13.0) as i32,
                (center.z * 29.0) as i32,
            ) * 0.2;
        // 水面保持连续，大片地面与灰泥只允许很小的明度变化。
        let shade = match color {
            13 => 1.0,
            0 | 1 | 9 => 0.98 + (shade - 0.86) * 0.2,
            _ => shade,
        };
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_COLOR,
            vec![[shade, shade, shade, 1.0]; mesh.count_vertices()],
        );
        if let Some(batch) = &mut self.0[color] {
            batch.merge(&mesh).expect("同属性立方体应可合并");
        } else {
            self.0[color] = Some(mesh);
        }
    }
}

/// 确定性布局噪声，只用于色块与植被分布，不读取随机进程种子。
fn noise(x: i32, z: i32) -> f32 {
    let mut n = (x as u32).wrapping_mul(374761393) ^ (z as u32).wrapping_mul(668265263) ^ 42;
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    (n ^ (n >> 16)) as f32 / u32::MAX as f32
}
/// 河心沿 X 方向蜿蜒，单位米。
fn river(x: f32) -> f32 {
    2.0 + (x * 0.034).sin() * 9.0
}
/// 近景地面以 12 厘米台阶量化，缓坡与河岸保持连续。
fn ground(x: f32, z: f32) -> f32 {
    let bank = ((z - river(x)).abs() - 3.7).max(0.0);
    ((0.35
        + (bank * 0.24).min(2.0)
        + ((-z - 9.0) * 0.09).max(0.0)
        + (x * 0.065).sin() * (z * 0.045).cos() * 0.35)
        / 0.12)
        .floor()
        .max(1.0)
        * 0.12
}

/// 多个偏移椭球形成分枝树冠，细叶块只填外壳，保持体积而避免实心堆砌。
fn foliage(b: &mut Blocks, center: Vec3, radius: Vec3, seed: i32, cell: f32) {
    let extent = (radius / cell).ceil().as_ivec3();
    for x in -extent.x..=extent.x {
        for y in -extent.y..=extent.y {
            for z in -extent.z..=extent.z {
                let p = Vec3::new(x as f32, y as f32, z as f32) * cell;
                let r = (p / radius).length_squared();
                let n = noise(x + seed + y * 19, z + seed * 7);
                if r < 1.0 + n * 0.22 && r > 0.48 && n > 0.08 {
                    b.block(
                        2 + (n * 2.99) as usize,
                        center + p,
                        Vec3::splat(cell * (0.9 + n * 0.35)),
                    );
                }
            }
        }
    }
}

/// 有分叉树干的阔叶树，大小叶簇错落形成轮廓，精细度随树高变化。
fn tree(b: &mut Blocks, x: f32, z: f32, height: f32) {
    let y = ground(x, z);
    let seed = (x * 17.0 + z * 29.0) as i32;
    for j in 0..18 {
        let f = j as f32 / 18.0;
        b.block(
            12,
            Vec3::new(x + f * f * 0.7, y + f * height * 0.72, z),
            Vec3::new(0.65 - f * 0.4, height / 24.0, 0.65 - f * 0.4),
        );
    }
    for branch in 0..7 {
        let angle = branch as f32 * 2.4;
        let offset = Vec3::new(
            angle.cos() * height * 0.23,
            height * (0.58 + noise(seed, branch) * 0.2),
            angle.sin() * height * 0.21,
        );
        for j in 0..9 {
            let t = j as f32 / 9.0;
            b.block(
                12,
                Vec3::new(x, y + height * 0.4, z)
                    + Vec3::new(offset.x, offset.y - height * 0.4, offset.z) * t,
                Vec3::splat(0.32),
            );
        }
        foliage(
            b,
            Vec3::new(x, y, z) + offset,
            Vec3::new(height * 0.24, height * 0.22, height * 0.23),
            seed + branch,
            // 远处林带使用较粗叶块，近景保留细树冠。
            height / if z < -44.0 { 18.0 } else { 28.0 },
        );
    }
}

/// 灰泥农舍配阶梯双坡瓦顶、门窗与石基；朝 +Z 为正面。
fn house(b: &mut Blocks, x: f32, z: f32, width: f32, depth: f32, height: f32) {
    let y = ground(x, z);
    b.block(
        8,
        Vec3::new(x, y + 0.4, z),
        Vec3::new(width + 0.4, 0.8, depth + 0.4),
    );
    b.block(
        9,
        Vec3::new(x, y + height / 2.0, z),
        Vec3::new(width, height, depth),
    );
    for row in 0..17 {
        let half = width / 2.0 + 0.5 - row as f32 * width / 32.0;
        // 山墙封住坡屋顶下方，顶部瓦片相交形成闭合屋脊。
        b.block(
            9,
            Vec3::new(x, y + height + row as f32 * 0.21 - 0.2, z),
            Vec3::new((half * 2.0).min(width), 0.21, depth),
        );
        for side in [-1.0, 1.0] {
            for tile in 0..(depth / 0.3) as i32 + 3 {
                b.block(
                    10 + ((row + tile) % 2) as usize,
                    Vec3::new(
                        x + side * half,
                        y + height + row as f32 * 0.21,
                        z - depth / 2.0 - 0.3 + tile as f32 * 0.3,
                    ),
                    Vec3::new(width / 16.0 + 0.15, 0.25, 0.29),
                );
            }
        }
    }
    // 石砌墙脚、墙角与零散灰泥剥落，细节依附建筑结构。
    for row in 0..(height / 0.35) as i32 {
        for side in [-1.0, 1.0] {
            b.block(
                8,
                Vec3::new(
                    x + side * (width / 2.0 - 0.18),
                    y + row as f32 * 0.35 + 0.18,
                    z + depth / 2.0 + 0.06,
                ),
                Vec3::new(if row % 2 == 0 { 0.7 } else { 0.4 }, 0.32, 0.18),
            );
        }
        if row < 3 {
            for i in 0..(width / 0.6) as i32 {
                b.block(
                    7 + (i % 2) as usize,
                    Vec3::new(
                        x - width / 2.0 + 0.3 + i as f32 * 0.6,
                        y + row as f32 * 0.35 + 0.18,
                        z + depth / 2.0 + 0.04,
                    ),
                    Vec3::new(0.56, 0.32, 0.13),
                );
            }
        }
    }
    for dx in [-width * 0.28, width * 0.28] {
        b.block(
            8,
            Vec3::new(x + dx, y + height * 0.6, z + depth / 2.0 + 0.04),
            Vec3::new(1.05, 1.55, 0.2),
        );
        b.block(
            12,
            Vec3::new(x + dx, y + height * 0.6, z + depth / 2.0 + 0.16),
            Vec3::new(0.7, 1.2, 0.08),
        );
    }
    // 两侧窗洞与凸出的窗台，让斜向机位也能读出墙体尺度。
    for side in [-1.0, 1.0] {
        for dz in [-depth * 0.27, depth * 0.27] {
            b.block(
                8,
                Vec3::new(x + side * (width / 2.0 + 0.04), y + height * 0.56, z + dz),
                Vec3::new(0.15, 1.6, 1.2),
            );
            b.block(
                12,
                Vec3::new(x + side * (width / 2.0 + 0.13), y + height * 0.56, z + dz),
                Vec3::new(0.08, 1.3, 0.85),
            );
            b.block(
                8,
                Vec3::new(
                    x + side * (width / 2.0 + 0.15),
                    y + height * 0.56 - 0.85,
                    z + dz,
                ),
                Vec3::new(0.4, 0.18, 1.4),
            );
        }
    }
    for i in 0..70 {
        let dx = (noise(i, (x * 7.0) as i32) - 0.5) * width;
        let py = noise(i, (z * 9.0) as i32) * height;
        // 避开门窗，在墙脚和檐下集中露石，不把整墙铺满噪点。
        if py < 0.9 || py > height - 0.6 {
            b.block(
                8,
                Vec3::new(x + dx, y + py, z + depth / 2.0 + 0.03),
                Vec3::new(0.24 + noise(i, 37) * 0.4, 0.16, 0.08),
            );
        }
    }
    // 木质百叶窗、窗台与细窗框，正面开口不再是平贴的色片。
    for dx in [-width * 0.28, width * 0.28] {
        b.block(
            8,
            Vec3::new(x + dx, y + height * 0.6 - 0.78, z + depth / 2.0 + 0.23),
            Vec3::new(1.3, 0.16, 0.45),
        );
        for side in [-1.0, 1.0] {
            for j in 0..6 {
                b.block(
                    12,
                    Vec3::new(
                        x + dx + side * 0.62,
                        y + height * 0.6 - 0.55 + j as f32 * 0.22,
                        z + depth / 2.0 + 0.2,
                    ),
                    Vec3::new(0.35, 0.17, 0.14),
                );
            }
        }
    }
    b.block(
        12,
        Vec3::new(x, y + 1.1, z + depth / 2.0 + 0.08),
        Vec3::new(1.15, 2.2, 0.2),
    );
    b.block(
        8,
        Vec3::new(x + width * 0.25, y + height + 2.5, z - depth * 0.2),
        Vec3::new(0.8, 2.0, 0.8),
    );
}

/// 土路连向桥头，路肩带自然摆动；单位米。
fn on_road(x: f32, z: f32) -> bool {
    (z - (23.0 + (x * 0.055).sin() * 6.0)).abs() < 2.4
        || ((x + 12.0).abs() < 2.5 && z < 26.0 && z > -30.0)
}

/// 石墙由错缝石块构成，局部缺口与不齐的压顶打破规则轮廓。
fn wall(b: &mut Blocks, x: f32, z: f32, length: i32) {
    for i in 0..length {
        for row in 0..3 {
            let px = x + i as f32 * 0.65 + (row % 2) as f32 * 0.28;
            let n = noise(i + (x * 4.0) as i32, row + (z * 7.0) as i32);
            if row == 2 && n < 0.18 {
                continue;
            }
            b.block(
                7 + (n > 0.45) as usize,
                Vec3::new(px, ground(px, z) + 0.18 + row as f32 * 0.34, z),
                Vec3::new(0.60, 0.30 + n * 0.07, 0.65),
            );
        }
    }
}

/// 近景以桥、两栋农舍和植被为主体；外围简化，细节集中在约 90×100 米内。
/// shortcut: 水面仍为静态色块，先验收构图与材质再增加水流模拟。
fn landscape() -> Blocks {
    let mut b = Blocks::new();
    b.block(0, Vec3::new(0.0, -2.0, 0.0), Vec3::new(800.0, 1.0, 800.0));
    for ix in -95..95 {
        for iz in -95..75 {
            let (x, z) = (ix as f32 * 0.75, iz as f32 * 0.75);
            let d = (z - river(x)).abs();
            let edge = 3.8 + 0.35 * (x * 0.8).sin() + 0.2 * (x * 1.7).cos();
            if d < edge {
                continue;
            }
            let y = ground(x, z);
            let color = if on_road(x, z) {
                5
            } else if d < edge + 0.8 {
                6
            } else {
                ((x * 0.13).sin() + (z * 0.17).cos() > 0.4) as usize
            };
            b.block(color, Vec3::new(x, y - 0.7, z), Vec3::new(0.75, 1.4, 0.75));
        }
    }
    // 随机位置形成草簇而非栅格散点，沿道路、河岸留出裸土与碎石。
    for i in 0..18000 {
        let x = noise(i, 51) * 130.0 - 65.0;
        let z = noise(i, 83) * 115.0 - 65.0;
        let d = (z - river(x)).abs();
        if d < 4.1 {
            continue;
        }
        let y = ground(x, z);
        let n = noise(i, 11);
        let building = ((x + 24.0).abs() < 5.2 && (z + 23.0).abs() < 4.2)
            || ((x - 2.0).abs() < 6.2 && (z + 33.0).abs() < 4.7);
        if building {
            continue;
        }
        if on_road(x, z) {
            if n > 0.82 {
                b.block(
                    6,
                    Vec3::new(x, y + 0.04, z),
                    Vec3::new(0.15 + n * 0.12, 0.08, 0.20),
                );
            }
        } else {
            let cluster = (x * 0.33).sin() * (z * 0.27).cos();
            if cluster > -0.25 || d < 7.5 {
                for j in 0..3 {
                    let h = 0.12 + noise(i, j + 21) * 0.38;
                    b.block(
                        2 + (j % 3) as usize,
                        Vec3::new(x + j as f32 * 0.12, y + h / 2.0, z + (j % 2) as f32 * 0.14),
                        Vec3::new(0.13, h, 0.13),
                    );
                }
                if n > 0.94 {
                    b.block(14, Vec3::new(x, y + 0.42, z), Vec3::new(0.13, 0.09, 0.13));
                }
            }
            if d < 6.5 && n > 0.68 {
                b.block(
                    7 + (i % 2) as usize,
                    Vec3::new(x, y + 0.12, z),
                    Vec3::new(0.3 + n * 0.5, 0.3, 0.4),
                );
            }
        }
    }
    for i in -145..145 {
        let x = i as f32 * 0.5;
        b.block(13, Vec3::new(x, 0.01, river(x)), Vec3::new(0.51, 0.12, 8.7));
        for j in 0..3 {
            let n = noise(i, j + 38);
            if n > 0.45 {
                b.block(
                    if n > 0.87 { 14 } else { 8 },
                    Vec3::new(x, 0.08, river(x) + (n - 0.5) * 7.0),
                    Vec3::new(0.15 + n * 0.8, 0.012, 0.035),
                );
            }
        }
    }
    // 拱桥中心留洞，侧立面细分成错缝石块，桥头用台阶缓坡接回道路。
    let rz = river(-12.0);
    for j in -21i32..=21 {
        let z = rz + j as f32 * 0.36;
        let arch = if j.abs() < 13 {
            0.3 + 2.2 * (1.0 - (j as f32 / 13.0).powi(2)).sqrt()
        } else {
            0.0
        };
        let top = 3.3;
        b.block(
            7,
            Vec3::new(-12.0, (top + arch) * 0.5, z),
            Vec3::new(5.4, top - arch, 0.36),
        );
        b.block(
            6,
            Vec3::new(-12.0, top + 0.06, z),
            Vec3::new(4.6, 0.12, 0.36),
        );
        for side in [-1.0, 1.0] {
            for row in 0..12 {
                let y = 0.15 + row as f32 * 0.30;
                if y < arch {
                    continue;
                }
                b.block(
                    7 + (noise(j, row) > 0.63) as usize,
                    Vec3::new(-12.0 + side * 2.72, y, z),
                    Vec3::new(0.18, 0.27, 0.33),
                );
            }
            for row in 0..2 {
                b.block(
                    8,
                    Vec3::new(-12.0 + side * 2.55, top + 0.22 + row as f32 * 0.32, z),
                    Vec3::new(0.48, 0.29, 0.34),
                );
            }
        }
    }
    for side in [-1.0, 1.0] {
        for j in 0..14 {
            let z = rz + side * (7.75 + j as f32 * 0.5);
            let top = (3.3 - j as f32 * 0.14).max(ground(-12.0, z));
            b.block(6, Vec3::new(-12.0, top * 0.5, z), Vec3::new(5.0, top, 0.52));
        }
    }
    house(&mut b, -24.0, -23.0, 9.0, 7.0, 5.0);
    house(&mut b, 2.0, -33.0, 11.0, 8.0, 6.2);
    wall(&mut b, -36.0, -15.0, 26);
    wall(&mut b, -7.0, -15.0, 38);
    wall(&mut b, 14.0, 25.0, 23);
    wall(&mut b, -40.0, 18.0, 22);
    // 前景大树框景，中景叶簇包围农舍，保留桥与房屋正面的视线。
    for (x, z, h) in [
        (-27., 23., 13.),
        (39., 18., 11.),
        (-35., -20., 11.),
        (13., -32., 11.),
        (-12., -40., 12.),
        (-42., -1., 10.),
        (28., -13., 9.),
    ] {
        tree(&mut b, x, z, h);
    }
    for i in 0..52 {
        let x = noise(i, 57) * 125.0 - 63.0;
        let z = -46.0 - noise(i, 95) * 25.0;
        tree(&mut b, x, z, 5.0 + noise(i, 33) * 5.0);
    }
    for i in 0..110 {
        let x = noise(i, 101) * 100.0 - 50.0;
        let z = noise(i, 102) * 88.0 - 43.0;
        if (z - river(x)).abs() < 5.0 || on_road(x, z) {
            continue;
        }
        if ((x + 24.0).abs() < 6.0 && (z + 23.0).abs() < 5.0)
            || ((x - 2.0).abs() < 7.0 && (z + 33.0).abs() < 6.0)
        {
            continue;
        }
        foliage(
            &mut b,
            Vec3::new(x, ground(x, z) + 0.65, z),
            Vec3::new(1.1, 0.8, 0.9),
            i,
            0.28,
        );
    }
    // 远景坡面采用连续高度场，近端接地；岩层横向成带，而非棋盘噪点。
    for ix in -65..65 {
        for iz in -90..-30 {
            let (x, z) = (ix as f32 * 3.0, iz as f32 * 3.0);
            let h = (22.0 + 12.0 * (x * 0.022).sin() + 7.0 * (x * 0.051).cos())
                * (1.0 - ((z + 160.0 + (x * 0.015).sin() * 20.0) / 76.0).powi(2)).max(0.0);
            let h = (h / 0.6).floor().max(1.0) * 0.6;
            b.block(
                if (9.0..13.0).contains(&h) { 7 } else { 0 },
                Vec3::new(x, h / 2.0 - 1.0, z),
                Vec3::new(3.0, h, 3.0),
            );
        }
    }
    for i in 0..7 {
        for j in 0..9 {
            b.block(
                14,
                Vec3::new(
                    -150.0 + i as f32 * 45.0 + j as f32 * 3.0,
                    51.0 + noise(i, 56) * 14.0 + (j as f32 * 0.9).sin() * 2.0,
                    -190.0,
                ),
                Vec3::new(6.0, 2.0 + noise(i, j) * 2.0, 6.0),
            );
        }
    }
    b
}

/// 正常入口与离屏验证共用的全景机位。
pub(crate) fn initial_view() -> Transform {
    Transform::from_xyz(22.0, 12.0, 37.0).looking_at(Vec3::new(-10.0, 3.0, -9.0), Vec3::Y)
}

/// 相机环境光覆盖全局默认值；离开时无需恢复或改写共享光照资源。
fn setup(
    mut commands: Commands,
    target: Res<SceneTarget>,
    font: Res<UiFont>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let camera = commands
        .spawn((
            PreviewCamera,
            PreviewEntity,
            Camera3d::default(),
            Camera {
                order: -1,
                clear_color: ClearColorConfig::Custom(Color::srgb(0.56, 0.72, 0.84)),
                ..default()
            },
            RenderTarget::Image(target.0.clone().into()),
            Msaa::Off,
            initial_view(),
            Tonemapping::BlenderFilmic,
            AmbientLight {
                color: Color::srgb(0.72, 0.81, 1.0),
                brightness: 650.0,
                ..default()
            },
            DistanceFog {
                color: Color::srgb(0.65, 0.75, 0.80),
                falloff: FogFalloff::Linear {
                    start: 75.0,
                    end: 290.0,
                },
                ..default()
            },
            bevy::pbr::ScreenSpaceAmbientOcclusion::default(),
            bevy::light::ShadowFilteringMethod::Gaussian,
        ))
        .id();
    commands.spawn((
        PreviewEntity,
        DirectionalLight {
            color: Color::srgb(1.0, 0.93, 0.82),
            illuminance: 14000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        bevy::light::CascadeShadowConfigBuilder {
            maximum_distance: 120.0,
            ..default()
        }
        .build(),
        Transform::from_xyz(-60.0, 90.0, 45.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    for (i, mesh) in landscape().0.into_iter().enumerate() {
        if let Some(mesh) = mesh {
            commands.spawn((
                PreviewEntity,
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb_from_array(PALETTE[i]),
                    perceptual_roughness: if i == 13 { 0.3 } else { 0.95 },
                    ..default()
                })),
                Transform::default(),
            ));
        }
    }
    commands
        .spawn((
            PreviewEntity,
            Node {
                position_type: PositionType::Absolute,
                left: px(24),
                bottom: px(24),
                padding: UiRect::all(px(12)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.055, 0.045, 0.82)),
            UiTargetCamera(camera),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(crate::ui::resources::strings_cn::pastoral::HELP),
                TextFont {
                    font: font.0.clone().into(),
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::srgb(0.92, 0.91, 0.87)),
            ));
        });
    info!("田园测试场景已生成：近景精修样板，无坦克，自由相机");
}

/// 飞行输入只在揭幕后且窗口聚焦时消费；俯仰限制防止翻转。
fn fly(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    time: Res<Time>,
    fade: Res<Fade>,
    windows: Query<&Window>,
    mut cameras: Query<&mut Transform, With<PreviewCamera>>,
    mut return_focus: ResMut<super::index::ReturnFocus>,
) {
    return_focus.0 = Some(2);
    if !fade.accepts_input() || windows.iter().any(|w| !w.focused) {
        return;
    }
    for mut t in &mut cameras {
        if mouse.pressed(MouseButton::Right) {
            let (yaw, pitch, _) = t.rotation.to_euler(EulerRot::YXZ);
            t.rotation = Quat::from_euler(
                EulerRot::YXZ,
                yaw - motion.delta.x * 0.003,
                (pitch - motion.delta.y * 0.003).clamp(-1.5, 1.5),
                0.0,
            );
        }
        let axis = |a, b| i32::from(keys.pressed(a)) as f32 - i32::from(keys.pressed(b)) as f32;
        let movement = *t.forward() * axis(KeyCode::KeyW, KeyCode::KeyS)
            + *t.right() * axis(KeyCode::KeyD, KeyCode::KeyA)
            + Vec3::Y * axis(KeyCode::KeyE, KeyCode::KeyQ);
        t.translation += movement.normalize_or_zero()
            * time.delta_secs()
            * if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
                45.0
            } else {
                15.0
            };
    }
}

/// CPU 就绪只检查自身内容，GPU 上传与管线由公共过渡流程验证。
pub(crate) fn content_ready(world: &mut World) -> bool {
    world
        .query_filtered::<(), With<PreviewCamera>>()
        .iter(world)
        .count()
        == 1
        && world
            .query_filtered::<(), (With<PreviewEntity>, With<Mesh3d>)>()
            .iter(world)
            .count()
            == PALETTE.len()
}
/// 首页恢复时释放所有预览实体，句柄随之释放；可重复调用。
fn cleanup(mut commands: Commands, entities: Query<Entity, With<PreviewEntity>>) {
    for e in &entities {
        commands.entity(e).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// 输入锁、真实窗口失焦与右键门控共同防止跨页相机漂移。
    #[test]
    fn camera_requires_focus_reveal_and_right_button_to_turn() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<ButtonInput<MouseButton>>();
        world.init_resource::<AccumulatedMouseMotion>();
        world.init_resource::<Fade>();
        world.init_resource::<super::super::index::ReturnFocus>();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs(1));
        world.insert_resource(time);
        let camera = world.spawn((PreviewCamera, initial_view())).id();
        let window = world.spawn(Window::default()).id();
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        world.resource_mut::<Fade>().input_armed = false;
        world.run_system_once(fly).unwrap();
        assert_eq!(*world.get::<Transform>(camera).unwrap(), initial_view());
        world.resource_mut::<Fade>().input_armed = true;
        world.get_mut::<Window>(window).unwrap().focused = false;
        world.run_system_once(fly).unwrap();
        assert_eq!(*world.get::<Transform>(camera).unwrap(), initial_view());
        world.get_mut::<Window>(window).unwrap().focused = true;
        world.resource_mut::<AccumulatedMouseMotion>().delta = Vec2::new(20.0, 10000.0);
        world.run_system_once(fly).unwrap();
        let moved = *world.get::<Transform>(camera).unwrap();
        assert!((moved.translation.distance(initial_view().translation) - 15.0).abs() < 0.001);
        assert_eq!(moved.rotation, initial_view().rotation);
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        world.run_system_once(fly).unwrap();
        let (yaw, pitch, roll) = world
            .get::<Transform>(camera)
            .unwrap()
            .rotation
            .to_euler(EulerRot::YXZ);
        assert!(yaw.is_finite() && pitch.abs() <= 1.5001 && roll.abs() < 0.001);
    }

    #[test]
    fn layout_is_finite_batched_and_river_is_below_banks() {
        let b = landscape();
        assert!(b.0.iter().all(Option::is_some));
        for mesh in b.0.iter().flatten() {
            assert!(mesh.count_vertices() > 0);
        }
        for x in [-80.0, 0.0, 80.0] {
            assert!(ground(x, river(x) + 12.0) > 0.1);
        }
        assert!(initial_view().translation.is_finite());
    }
}

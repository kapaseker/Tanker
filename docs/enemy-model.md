# 敌方坦克模型契约

模块导航：`scripts/generate_enemy.py` 读取 `src/tank/model/enemy.rs`，共用轮型契约在 `src/tank/model/mod.rs`；生成与销毁消息归 `src/gameplay/enemy.rs`，共用驾驶与表现归 `src/tank/`，战斗归 `src/gameplay/combat/`。目录整理验收见 [任务 10](testing/architecture-step10-20261023.md)。

默认敌人为原创现代坦克：暗红倾斜首上、紧凑削角炮塔、短尾舱、分段侧裙和深灰透空履带。敌人仍是开局生成的10辆静止靶车，不新增AI、开火或补充生成。

## 尺寸与坐标

权威定义在 `src/tank/model/enemy.rs`；每格0.1米，VOX +X 为车头、Y 为宽度、Z 为高度。生成器直接读取其中的画布、车体和炮管范围、炮轴、轮位及轮型参数，不读取旧模型。画布为114×34×26格，车体 X=[24,90)，画布中心X=57与车体中心一致。

- 非炮管圆角碰撞体：长6.6米、高2.6米、宽3.4米，圆角0.2米。运行时尺寸由 `src/tank/model/enemy.rs` 的 `COLLISION_SIZE` 统一推导，高度沿用玩家底盘契约（同一权威值），资源测试同时核对两种模型高度契约。
- 非炮管几何边界为[24,0,0]至[90,34,26]；地面至顶盖2.6米，零偏移挂载，底部与玩家一致。
- 炮管 X=[76,114)，轴线离地2米，炮口在车体中心前方5.7米；车尾至炮口全长9米。炮管不拥有碰撞体，也不扩大整车碰撞框。
- 现实比例参考加拿大坦克博物馆的 [T-72M1](https://tankmuseum.ca/blog-post/t-72m1-at-the-museum/)（车体长6.95米、宽3.6米、高2.2米）；本模型为适应游戏高度与4+2轮组的原创设计，不是实车复刻。

| 资源路径（相对 assets） | 节点 | 内容 |
|---|---|---|
| tank/bodies/enemy/carapace/body.vox | body | 车身、炮塔、炮盾、顶盖、侧裙与支架 |
| tank/barrels/enemy/spit/barrel.vox | barrel | 炮管、抽烟器与炮口 |
| tank/tracks/enemy/crawl/track.vox | track_l_0..2 / track_r_0..2、wheel_l_0..5 / wheel_r_0..5 | 六相位履带与十二个独立轮 |

三包保留原敌人五色调色板：RGB(100,42,38)、(121,55,46)、(75,31,29)、(36,39,37)、(66,60,53)。车身、炮管与履带相位共用画布；独立轮使用局部居中画布，按契约轴心安装。每辆敌人共20个场景实例，根实体销毁时全部注销。

## 履带与轮组

每侧四个直径1米的负重轮，两个直径0.6米的端轮，轮宽均0.4米。负重轮距车尾1.35、2.65、3.95、5.25米，轴高0.65米；端轮距车尾0.45、6.15米，轴高0.7米。横向轴心距中线1.3米。

负重轮/端轮局部画布为10×4×10 / 6×4×6格；体素顶点的完整旋转包络分别不超过0.53 / 0.33米。轮子没有独立碰撞体。轮胎深灰、轮毂暗红、辐条灰褐，绕自身轴心连续旋转，角度取对应侧累计里程除以名义半径；停车保留角度。

履带闭环150格，齿距3格，每行驶0.1米切换一相位；主体高1.3米、顶齿最高1.4米，底部削角。轮组的旋转包络位于离地0.1～1.2米的内腔内；连接条位于轮宽内侧，轮间保持透空。侧裙离地1.5～1.7米，通过凸齿上方的横梁连接甲板。

出生净空采用双方水平碰撞框的外接半径之和再加1米，敌人与玩家中心距约8.87米、敌人之间约8.42米，覆盖任意随机朝向。数量及±120米的出生采样范围不变。

## 生成与验收

```text
python3 scripts/generate_enemy.py
python3 -m unittest discover -s scripts -p test_generate_enemy.py
python3 -m unittest discover -s scripts -p test_tracks.py
cargo check
cargo test
cargo run -- --offscreen-test wheels --output .scratch/offscreen/enemy-wheels --model enemy
```

`wheels` 默认仍验收玩家；只有该预设接受 `--model enemy`。敌人模式将一辆真实敌人放在出生平地旁，脚本意图只写入该车，玩家与其它敌人保持原有输入逻辑。输出20张动作关键帧、side/front/rear/bounds/comparison图和报告；bounds仅画圆角碰撞体的外包围框。其余scene、drive、combat命令不变，combat已移除旧敌人的抬高台座，在平地验证玩家水平射击命中及爆炸清理。

几何测试检查三包对齐、炮盾接缝、全相位边界、配色、完整轮包络与车身/履带及其它轮之间的净空；真实资源测试检查节点就绪、网格轴心、运动方向、停车和实例注销。离屏结果必须检查报告和关键帧，编译或资源加载成功不能替代画面验收。

## 击毁爆炸

复用现有12帧爆炸资源，运行时整体放大2倍，最大直径由4米增至约8米，覆盖6.6米长的车体；起爆位置仍为被毁车体中心，播放时长仍为1秒，不增加伤害范围。combat额外输出第48、60模拟帧，用于检查扩张与消散阶段。

## 本机验收记录（2026-10-20）

- `cargo check --offline`通过，Rust测试47项全部通过；玩家生成器5项、玩家资源4项、敌人资源2项、共享履带1项测试通过。
- Python全套13项中12项通过，1项既有错误：`test_generate_ground.py::test_spawn_pad_is_flat`仍引用已删除的`SPAWN_PAD_M`，生成器现使用`SPAWN_CLEAR_M`。已核对HEAD中即存在此不一致，本轮未修改地形源码或测试，不宣称Python全套通过。
- Linux llvmpipe（LLVM 20.1.2）/ Vulkan软件渲染：scene、drive、combat、玩家wheels与敌人wheels全部退出0，报告通过。已读取场景、转向、平地开炮及清理关键帧，以及敌人前后视图、碰撞框、同场比例和轮组动画关键帧；未见明显轴心漂移或部件穿插。
- drive直行约6米，转向正常；combat靶车平地中心高1.3米，无台座，一发击毁，爆炸及子弹清理完成。敌人wheels前进约3米、倒车约2米，左右转向里程符号相反，停车末10帧的左右里程变化小于0.000001米。
- 本机证据（忽略目录，不提交）：`.scratch/offscreen/enemy-modern-{scene,drive,combat}-01/`、`.scratch/offscreen/player-regression-wheels-01/`、`.scratch/offscreen/enemy-modern-wheels-02/`。敌人wheels-01是加侧裙连接横梁前的中间版本，最终画面以wheels-02为准。
- 未启动可见窗口；未验证硬件GPU、其它驱动或窗口输入行为。

### 爆炸放大验收

`cargo check --offline`及47项Rust测试通过。`.scratch/offscreen/enemy-explosion-large-01/`的combat退出0，已读取第6、24、48、60帧，确认放大的起爆、外扩和消散效果；报告确认命中后爆炸生成，并在第120帧前清理完成。本次仍使用llvmpipe/Vulkan软件渲染，未启动可见窗口。

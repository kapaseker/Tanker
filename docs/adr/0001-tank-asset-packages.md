# ADR-0001：坦克部件资产按变体目录分包

- 状态：已采纳
- 日期：2026-10-22

## 背景

坦克由三个可独立替换的部件组成：车体（body）、履带（track）、炮管（barrel）。
原资产按阵营组织（`tank/player/default/body.vox`、`tank/barrels/player/default.vox`、`tank/tracks/player/lugged/default.vox`），
三种部件三种形式，`default` 不携带任何变体信息，子弹与发射它的炮管分离，爆炸特效挂在敌方车体目录下。
后续要为每个部件增加多种型号（不同履带样式、不同炮管），需要一套各部件一致、可按目录扩展的规则。

## 决策

### 路径约定

```
tank/<部件>/<适配车型>/<变体>/<部件名>.vox
```

- `<部件>`：`bodies` / `tracks` / `barrels`；新增部件种类时在此层加目录。
- `<适配车型>`：当前与阵营一一对应（`player` / `enemy`）；同阵营出第二底盘时加子目录。
- `<变体>`：型号 ID，目录名即标识。
- `<部件名>`：包内主模型固定命名 `body.vox` / `track.vox` / `barrel.vox`，不用变体名做文件名。
- 附属资源与主模型平铺在同一变体目录（子弹是炮管的附属）。
- 与坦克型号无关的表现资源放 `assets/effects/`。

### 命名规则

- 变体 ID 为单个小写英文单词，避免空格与连字符。
- 名字描述部件的性能取向，而不是主题氛围：车体=防护/机动，炮管=射程/射速/口径，履带=抓地/接地压力/速度。
- 玩家与敌人使用互不重叠的两套词库：玩家取自工匠器物，敌人取自掠食动作。后续新增变体沿用该风格作为命名参考。
- 中文显示名只进文案表，不进路径。

### 备选词库

所有已议定的变体名。加粗为当前已落地并有模型的变体，其余是预留名，做出对应模型后再建目录。

玩家（工匠器物）：

| 部件 | ID | 中文 | 特色 |
|---|---|---|---|
| 车体 | **anvil** | 铁砧 | 重甲、低速、正面硬 |
| 车体 | keel | 龙骨 | 均衡中坚、脊梁结构 |
| 车体 | skiff | 轻舟 | 轻甲、高速、脆 |
| 炮管 | **lance** | 长矛 | 长管直射、高初速、单发精准 |
| 炮管 | repeater | 连发机 | 速射、低单发、压制 |
| 炮管 | mortar | 臼炮 | 曲射、高弧线、范围 |
| 履带 | **cleat** | 抓地齿 | 标准齿纹、抓地均衡 |
| 履带 | ribbon | 宽幅带 | 宽接地、低压力、稳但慢 |
| 履带 | spur | 窄刺带 | 窄、轻、快、抓地差 |

敌人（掠食动作）：

| 部件 | ID | 中文 | 特色 |
|---|---|---|---|
| 车体 | **carapace** | 甲壳 | 弧形装甲、防御均衡 |
| 车体 | hulk | 巨骸 | 超重、缓慢、血厚 |
| 车体 | stalker | 潜猎 | 低矮伏击、轻甲、转向快 |
| 炮管 | **spit** | 喷吐 | 短管、近距、散射 |
| 炮管 | roar | 咆哮 | 大口径、高伤、后坐大 |
| 炮管 | gnaw | 啃噬 | 小弹连发、持续消耗 |
| 履带 | **crawl** | 蠕行带 | 宽、慢、稳 |
| 履带 | claw | 爪带 | 带刺、强抓地、颠簸 |
| 履带 | scuttle | 疾爬带 | 窄、快、抖动 |

变体之间按部件自由组合，不存在锁定的“套装”。

### 参考组合

以下组合是已议定的默认搭配，用于说明三件如何配合；新变体只需遵循上面的词库风格，不必凑成套。

| 组合 | 车体 | 炮管 | 履带 | 打法 |
|---|---|---|---|---|
| `anvil-lance-cleat` | anvil | lance | cleat | 重甲直射，正面碾压 |
| `skiff-repeater-spur` | skiff | repeater | spur | 高速骚扰，打完就跑 |
| `keel-mortar-ribbon` | keel | mortar | ribbon | 均衡曲射，中距离站桩 |
| `hulk-roar-crawl` | hulk | roar | crawl | 攻城重炮，慢但致命 |
| `stalker-spit-claw` | stalker | spit | claw | 伏击近战，贴脸 |
| `carapace-gnaw-scuttle` | carapace | gnaw | scuttle | 消耗压制，缠斗 |

## 当前清单

| 部件 | 车型 | 变体 | 路径 |
|---|---|---|---|
| 车体 | player | `anvil` | `tank/bodies/player/anvil/body.vox` |
| 车体 | enemy | `carapace` | `tank/bodies/enemy/carapace/body.vox` |
| 炮管 | player | `lance` | `tank/barrels/player/lance/barrel.vox` + `bullet.vox` |
| 炮管 | enemy | `spit` | `tank/barrels/enemy/spit/barrel.vox` |
| 履带 | player | `cleat` | `tank/tracks/player/cleat/track.vox` |
| 履带 | enemy | `crawl` | `tank/tracks/enemy/crawl/track.vox` |
| 爆炸特效 | — | — | `assets/effects/explosion.vox` |

选名依据现有模型特征：玩家车体 7.5 m 重装 → `anvil`；敌人车体 6.6 m 紧凑削角 → `carapace`；
玩家炮管 4.5 m 长管 → `lance`；敌人炮管 3.8 m 短管 → `spit`；玩家带齿履带 → `cleat`；敌人 4+2 轮宽接地 → `crawl`。

## 扩展方式

新增一个变体：

1. 复制同部件同车型下的现有变体目录，改目录名为新 ID；
2. 替换包内模型，保持包内文件名与几何契约（画布、碰撞边界、节点名）不变；
3. 在代码中把该部件的路径常量或 `TrackSpec.path` 指向新目录；
4. 同步 `docs/player-model.md` / `docs/enemy-model.md` 的路径表与 `scripts/` 中的生成器输出路径。

不创建空的占位变体目录；变体只在有真实模型且被引用时落地。

## 后果

- 三部件形式统一，换装只需替换变体目录，新增型号不改变目录层级。
- 换装/选型逻辑尚不存在：路径由各模块常量硬编码，变体 ID 目前只是资源标识。
- 变体名承诺了性能取向。若模型或数值与该取向不符，必须同步改名，否则名不副实。
- 子弹目前只被玩家 `lance` 炮管使用，因此随该变体打包；将来多炮管共用子弹时，把子弹提为独立路径并在各炮管变体中引用。

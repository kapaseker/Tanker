# 03: 建立保色保形的五倍细分与分块转换器

Status: done
Type: task
Blocked by: 02

## What to build

用一个可复用的标准库转换器，把粗源体素精确细分并写成可由现有插件加载的分块 VOX，先用小样贯穿到真实加载器。

上下文：先读 [规格](../spec.md) 和 [执行方案](../plan.md)，以及阻塞任务的结果与证据。状态仅表示计划可执行，本轮没有开始实施。

## Acceptance criteria

- [x] 每个输入格恰生成 125 个同色子格，调色板和材质保持；小样涵盖块边界、空白画布、负平移、命名节点与 675/570 格画布，输出坐标和 chunk 长度合法。
- [x] 保留语义父节点，分块变换还原后彩色体积、米制原点/顶点与粗源一致；奇偶尺寸不产生半格舍入。
- [x] 转换必须知道源/目标单位并拒绝重复细分；写入新 staging、验证后发布 VOX+meta，失败不损坏输入；避免用数百万 Python 元组常驻内存。

## Verification

- [x] python -X utf8 -m unittest discover -s scripts -p "test_*.py"，包含新增转换器测试。（scripts.test_subdivide_vox 25 项 OK，含发布配对回滚与内部空块跳过）
- [x] 以真实 VOX 加载器验证转换小样，不仅自写解析器往返；覆盖 #节点 返回完整分块子树。（tests/vox_subdivide_samples.rs 4 项通过：300/675/570 画布分块子树 AABB 与粗样一致、命名节点负平移保形、无缩放）
- [x] 如有 Rust 变更则 cargo check；记录转换体素数、字节数、峰值内存和耗时。（cargo check 通过；cargo test 全量 113 项通过；统计见证据文档）

## 代码定位（实施前重新核对）

建议 scripts/subdivide_vox.py 及专用测试；复用 generate_ground 的 chunk/dictionary；现有 test_tracks 的场景解析接缝；真实加载小样。

## 边界

不支持无关格式全集，不转换爆炸，不删内部格子。保留所有输入几何信息，不做美术修改。

## Comments

2026-10-28：用户明确本次仅细分，不做美术精修。完成后在此记录命令、结果、证据和遗留项；通过前不把任务记为完成。

2026-10-28（完成）：交付 `scripts/subdivide_vox.py`（解析/单位门/分块/场景重建/写出/独立验证/CLI 统计）+ `scripts/test_subdivide_vox.py`（25 项）+ `tests/vox_subdivide_samples.rs`（4 项真实加载器集成）。分块规则：单轴 ≤256 不分块，奇画布 K=255、偶画布 K=256；平移 `t = origin + (C−F)/2`，无半格舍入。非根帧 `_t`×5、`_r` 原样保留，根帧原样复制（加载器忽略）。staging 拒绝覆盖，`--publish` 先备份源 VOX 再配对替换 VOX+meta、失败回滚，失败保留证据。真实七文件全部转换+验证通过（车体 357 万体素、14.3MB、堆峰值 54.7MiB、64s），统计与结论见 [证据](../evidence/03-converter.md)。遗留项：无（游戏内集成属工单 05）。

2026-10-28（code-review 采纳记录）：双轴评审后修复——① chunk/dictionary 改为从 generate_ground 导入，消除三重复制；② 按方案 §4.3 过滤无体素的空块（真实文件中敌方车体/炮管/爬行履带因此少 1-2 个空模型，统计已用 run-213301 重跑刷新）；③ `--publish` 改为备份+配对回滚，新增注入 meta 替换失败的回滚测试；④ 675/570 画布补入真实加载器集成测试（工单 AC1 点名画布）；⑤ 杂项：check_frames 去掉未用默认参数、publish 统计不再打印 None、证据文档明示 AABB 为必要非充分代理。未采纳：Rust 测试与 vox_meta_settings.rs 的公共函数暂不抽 tests/common（两份拷贝可接受，第三处再抽）；分块节点 layer=-1/0 帧与现有 generate_* 脚本先例一致，保留。

# Leopard 2A6 炮塔与主炮运动参数来源

核查日期：2026-11-02。适用对象为当前玩家 Leopard 2A6 / L55；实车公开资料、产品系列参考与游戏采用值必须分开看待。本次未取得 2A6 专属原厂操作/验收手册，因此不声称已还原其完整火控或驱动性能。

## 可核实的范围与结构

| 项目 | 公开资料 | 适用与限制 |
| --- | --- | --- |
| 主炮俯仰 | −9° 至 +20° | 斯洛伐克国防部《Slovak Armed Forces》2009 年 10 月刊，印刷页 26 / PDF 第 25 页的 Leopard 2 条目先说明 2A6 / L55，再给出该范围；美军 WEG 2011 / 2015 的 2A4、2A5、2A6 联合条目亦一致。它是公开总范围，不证明全方位均能压到 −9°。 |
| 炮塔水平运动 | 360° 全周 | 德国联邦国防军协会的 Leopard 2 40 周年报道明确说明炮塔可旋转 360°。作为车系范围依据；未取得 A6 专属限位图。 |
| 驱动类别 | A5 起改为电动武器随动/稳定系统 | 稳定系统供应商 VINCORION 的 Leopard 2 升级说明明确区分早期液压与 A5 起电动系统。因此不能把早期 WNA-H22 液压型参数直接视作 A6 核实值。 |

来源：

- [斯洛伐克国防部刊物，PDF 第 25 页](https://www.mod.gov.sk/data/files/1420.pdf#page=25)：本次成功读取全文并定位上述范围。
- [美军 WEG 2011，CGSC 馆藏](https://cgsc.contentdm.oclc.org/digital/api/collection/p4013coll11/id/2089/download)、[WEG 2015 副本](https://upload.wikimedia.org/wikipedia/commons/1/1f/WorldwideEquipmentGuide_2015_Ground_Systems.pdf)：搜索索引可读到对应条目；本次前者直接读取为 JS 页面，后者超过网页工具文件大小限制，故以可完整读取的斯洛伐克国防部刊物为范围主依据。
- [德国联邦国防军协会：40 Jahre Leopard 2 – Stärke auf Ketten](https://www.dbwv.de/aktuelle-themen/blickpunkt/beitrag/40-jahre-leopard-2-staerke-auf-ketten)。该协会不是车辆制造商，本文为车系报道。
- [VINCORION：Turm- und Waffenstabilisierung](https://www.vincorion.com/turm-und-waffenstabilisierung/)，其中 “Upgrade für den Leopard 2” 说明 A5 起电动化；旧网址现重定向异常，文字仍可从搜索索引读取。[现站 Vehicle Systems](https://vincorion.com/en/vehicle-systems/)确认该供应商的 Leopard 2 稳定系统业务。

## 速度：原厂系列参考，不是 A6 专属认证

VINCORION 的 GTD chaserV 原厂数据表把“最大运动速度”和“最大瞄准速度”分开。其 **large weapons** 栏列 Leopard 2 为平台类别参考：最大运动速度两个轴均为 **≥40°/s**，最大瞄准速度俯仰 **≥9°/s**、水平 **≥30°/s**。数值带不等号，不能写成实车精确上限；相邻 medium weapons 栏的 20°/s、40°/s 不属于该栏目。

- [GTD chaserV 原厂数据表（2022 年文件名）](https://www.vincorion.com/wp-content/uploads/2022/04/220627_VINC_DB_GTD_chaserV_EN_DINA4_WEB.pdf)：本次搜索引擎返回完整技术表，但原链接直接访问为 404，未能重新取得 PDF 以复核排版。只能作为可追溯、带访问限制的系列参考。
- [VINCORION 2024-06-19 原厂发布](https://vincorion.com/en/large-scale-weapon-stabilization-gtd-chaser/)说明新 GTD chaser 系统针对 A8 等后续车辆进入量产。它与旧系统有技术传承，但这不足以证明 2022 系列表格正好等于德国 2A6 某批次的设定。

本次按键速度建议取水平 **30°/s**、俯仰 **9°/s**，作为供应商大口径系列瞄准速度参考的保守游戏映射。此选择属于项目采用值；**A6 专属最大速度仍未核实**，不应在 UI 或文档中标成精确实车性能。最终运行值以玩家 `tank.jsonc` 为准，后续取得可公开使用的 A6 原厂/军方手册后再修订。

## 排除项与还原边界

- 不使用 War Thunder、Armored Warfare 等游戏的乘员等级/模式参数证明实车性能。
- 常见的水平 40°/s、俯仰 45°/s 二手文章[Leopard 2 / Kampfpanzer.de](https://kpz.axlbox.de/leo2en.html)明确在液压系统段落讨论这些值；不能据此声称已核实 A6 电动型。
- 目前没有可确认的 A6 方位相关压炮限位图；−9°～+20°只是全车公开总范围。全周使用同一限位是游戏简化，不等于还原实车后部防碰撞联锁。
- 本次用户确认的行为是炮塔保持相对车体角度、随车体转向，WASD 直接调节角度；不模拟稳定器保持世界瞄准方向、观瞄装置跟踪、驱动加减速、应急手摇或火控模式。

连续旋转补充：供应商 [SCHLEIFRING 的豹 2 产品说明](https://www.schleifring.com/applications/safety/)明确说明炮塔与车体之间采用滑环传输电源、数据与信号，即使双方反向转动仍可传输。这支持连续旋转的结构解释，不能由“360°范围”单独推导所有工况的安全许可。运行配置以 `traverse_range_deg: 360` 表示当前全周连续模式。

// 模型数据与 CSS 3D 占位模型构建器，供画廊页与详情页共用。
// 使用经典脚本；弹药规格由详情页通过 HTTP 读取模型旁配置。
// dims 为三维尺寸 [长, 宽, 高]（米）；parts 为占位模型盒子组合（px），以模型中心为原点偏移 (x, y, z)。

// 三面色阶：顶面亮、正面中、侧面暗，模拟体素三面明暗
const TONES = {
  mint: ['#e2edcf', '#b7cb9f', '#91a97a'],
  gray: ['#d8dbd8', '#b0b5b2', '#868c89'],
  sand: ['#8b7660', '#6d5b49', '#524437'],
  dark: ['#3a3830', '#2c2a24', '#211f1a'],
};

// 单个盒子面：w/h 为面尺寸（px），transform 负责摆到盒子对应方位
function face(w, h, transform, color) {
  const el = document.createElement('div');
  el.className = 'model__face';
  el.style.cssText =
    `width:${w}px;height:${h}px;left:${-w / 2}px;top:${-h / 2}px;` +
    `transform:${transform};background:${color};`;
  return el;
}

// 单个部件盒子：按 tone 色阶拼六面，原点为盒子中心，translate3d 摆位
function buildBox(part, tone) {
  const box = document.createElement('div');
  box.className = 'model__box';
  const x = part.x || 0;
  const y = part.y || 0;
  const z = part.z || 0;
  box.style.transform = `translate3d(${x}px, ${y}px, ${z}px)`;
  const [light, mid, dark] = TONES[part.tone || tone];
  const hw = part.w / 2;
  const hh = part.h / 2;
  const hd = part.d / 2;
  box.append(
    face(part.w, part.h, `translateZ(${hd}px)`, mid),
    face(part.w, part.h, `rotateY(180deg) translateZ(${hd}px)`, dark),
    face(part.d, part.h, `rotateY(-90deg) translateZ(${hw}px)`, dark),
    face(part.d, part.h, `rotateY(90deg) translateZ(${hw}px)`, dark),
    face(part.w, part.d, `rotateX(90deg) translateZ(${hh}px)`, light),
    face(part.w, part.d, `rotateX(-90deg) translateZ(${hh}px)`, dark),
  );
  return box;
}

// 组合模型：整车示意以 body 中心为原点，与游戏内的旋转轴约定一致。
// 部件带 turret:true 时归入炮塔组（绕模型中心竖轴 rotateY）；
// 带 barrel:true 的炮管再套一层耳轴枢轴（绕 Z 轴 rotateZ 做俯仰），
// 枢轴取炮管后端 (x - w/2, y, z)，与游戏内炮管俯仰轴一致。
// 返回的 el 上挂 turretGroup / barrelPivot / barrelBase，供详情页 WASD 驱动。
function buildModel(model) {
  const el = document.createElement('div');
  el.className = 'model';
  el.setAttribute('aria-hidden', 'true');
  let turretGroup = null;
  for (const part of model.parts) {
    const box = buildBox(part, model.tone);
    if (part.turret) {
      if (!turretGroup) {
        turretGroup = document.createElement('div');
        turretGroup.className = 'model__turret';
        el.append(turretGroup);
      }
      if (part.barrel) {
        const pivot = document.createElement('div');
        pivot.className = 'model__barrel';
        // 枢轴基点：炮管后端；俯仰时在此基础上追加 rotateZ
        const base = `translate3d(${(part.x || 0) - part.w / 2}px, ${part.y || 0}px, ${part.z || 0}px)`;
        pivot.style.transform = base;
        // 炮管盒改为相对耳轴向前伸出
        box.style.transform = `translate3d(${part.w / 2}px, 0px, 0px)`;
        pivot.append(box);
        turretGroup.append(pivot);
        el.barrelPivot = pivot;
        el.barrelBase = base;
      } else {
        turretGroup.append(box);
      }
    } else {
      el.append(box);
    }
  }
  el.turretGroup = turretGroup;
  return el;
}

// 与游戏同步：独立弹丸一项，完整坦克两项；网页盒子只表达构图。
const CATEGORIES = [
  { name: '子弹', code: 'AM', models: [
    { name: 'APFSDS', tone: 'dark', dims: [0.72, 0.10, 0.10], parts: [{ w: 180, h: 10, d: 10 }, { w: 30, h: 25, d: 4, x: -75 }, { w: 30, h: 4, d: 25, x: -75 }] },
  ] },
  { name: '坦克', code: 'TK', models: [
    // 网页静态规格与运行时配置对齐： assets/tank/player/leopard2_a5/tank.jsonc
    { name: 'Leopard 2A5', tone: 'mint', dims: [10.98, 3.76, 4.32],
      specs: [
        { label: '前进速度', value: '19', unit: 'm/s' },
        { label: '倒车速度', value: '8.94', unit: 'm/s' },
        { label: '水平旋转', value: '全圈', unit: '' },
        { label: '炮管俯仰范围', value: '-9°～+20°', unit: '' },
        { label: '水平转速', value: '40°', unit: '/s' },
        { label: '俯仰速度', value: '9°', unit: '/s' },
        { label: '单发伤害', value: '510', unit: 'HP' },
        { label: '最大生命值', value: '4100', unit: 'HP' },
      ],
      // WASD 驱动占位模型用：度/秒与俯仰限位，与游戏配置一致
      articulation: { traverse: 40, elevation: 9, minElevation: -9, maxElevation: 20 },
      parts: [{ w: 150, h: 30, d: 72 }, { w: 60, h: 22, d: 48, y: -26, turret: true }, { w: 106, h: 8, d: 8, x: 90, y: -26, turret: true, barrel: true }, { w: 150, h: 20, d: 16, y: 20, z: 28, tone: 'dark' }, { w: 150, h: 20, d: 16, y: 20, z: -28, tone: 'dark' }] },
    { name: 'carapace', tone: 'sand', dims: [9, 3.4, 2.6], parts: [{ w: 132, h: 30, d: 68 }, { w: 54, h: 22, d: 46, y: -26 }, { w: 76, h: 8, d: 8, x: 78, y: -26 }, { w: 132, h: 20, d: 16, y: 20, z: 26, tone: 'dark' }, { w: 132, h: 20, d: 16, y: 20, z: -26, tone: 'dark' }] },
  ] },
];

// 模型卡片/详情页锚点编号，例如 AM-01；画廊与详情页用它互相对应
function modelCardId(category, modelIndex) {
  return `${category.code}-${String(modelIndex + 1).padStart(2, '0')}`;
}

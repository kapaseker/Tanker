// 模型数据与 CSS 3D 占位模型构建器，供画廊页与详情页共用。
// 使用经典脚本（非 ES module），保证 file:// 直接打开也能加载。
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
function buildModel(model) {
  const el = document.createElement('div');
  el.className = 'model';
  el.setAttribute('aria-hidden', 'true');
  for (const part of model.parts) {
    el.append(buildBox(part, model.tone));
  }
  return el;
}

// 与游戏同步：独立弹丸一项，完整坦克两项；网页盒子只表达构图。
const CATEGORIES = [
  { name: '子弹', code: 'AM', models: [
    { name: 'APFSDS', tone: 'dark', dims: [0.72, 0.10, 0.10], parts: [{ w: 180, h: 10, d: 10 }, { w: 30, h: 25, d: 4, x: -75 }, { w: 30, h: 4, d: 25, x: -75 }] },
  ] },
  { name: '坦克', code: 'TK', models: [
    { name: 'LeoPard2 A6', tone: 'mint', dims: [10.98, 3.76, 4.32], specs: [{ label: '初速', value: '1200', unit: 'm/s' }, { label: '射速', value: '12', unit: 'rpm' }, { label: '前进速度', value: '16', unit: 'm/s' }, { label: '倒车速度', value: '8', unit: 'm/s' }], parts: [{ w: 150, h: 30, d: 72 }, { w: 60, h: 22, d: 48, y: -26 }, { w: 106, h: 8, d: 8, x: 90, y: -26 }, { w: 150, h: 20, d: 16, y: 20, z: 28, tone: 'dark' }, { w: 150, h: 20, d: 16, y: 20, z: -28, tone: 'dark' }] },
    { name: 'carapace', tone: 'sand', dims: [9, 3.4, 2.6], parts: [{ w: 132, h: 30, d: 68 }, { w: 54, h: 22, d: 46, y: -26 }, { w: 76, h: 8, d: 8, x: 78, y: -26 }, { w: 132, h: 20, d: 16, y: 20, z: 26, tone: 'dark' }, { w: 132, h: 20, d: 16, y: 20, z: -26, tone: 'dark' }] },
  ] },
];

// 模型卡片/详情页锚点编号，例如 AM-01；画廊与详情页用它互相对应
function modelCardId(category, modelIndex) {
  return `${category.code}-${String(modelIndex + 1).padStart(2, '0')}`;
}

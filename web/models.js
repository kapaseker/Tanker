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

// 组合模型：整车示意以 body 中心为原点，与游戏内的旋转轴约定一致。
function buildModel(model) {
  const el = document.createElement('div');
  el.className = 'model';
  el.setAttribute('aria-hidden', 'true');
  for (const part of model.parts) {
    const box = document.createElement('div');
    box.className = 'model__box';
    const x = part.x || 0;
    const y = part.y || 0;
    const z = part.z || 0;
    box.style.transform = `translate3d(${x}px, ${y}px, ${z}px)`;
    const [light, mid, dark] = TONES[part.tone || model.tone];
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
    el.append(box);
  }
  return el;
}

// 分类与模型数据；ci/mi 为分类与模型下标，用于页面间 URL 传递
// 与游戏当前九项真实清单同步；尺寸为可见几何长宽高（米）。
// 网页盒子仅表达构图，游戏必须通过 VOX 加载器渲染真实资产。
const CATEGORIES = [
  { name: '炮管', code: 'BR', models: [
    { name: 'lance', tone: 'mint', dims: [4.5, 0.4, 0.4], parts: [{ w: 170, h: 16, d: 16 }] },
    { name: 'spit', tone: 'sand', dims: [3.8, 0.4, 0.4], parts: [{ w: 144, h: 16, d: 16 }] },
  ] },
  { name: '履带', code: 'TR', models: [
    { name: 'cleat', tone: 'dark', dims: [7.5, 3.6, 1.3], parts: [{ w: 180, h: 31, d: 20, z: 33 }, { w: 180, h: 31, d: 20, z: -33 }] },
    { name: 'crawl', tone: 'sand', dims: [6.6, 3.4, 1.4], parts: [{ w: 158, h: 34, d: 20, z: 31 }, { w: 158, h: 34, d: 20, z: -31 }] },
  ] },
  { name: '车身', code: 'CH', models: [
    { name: 'anvil', tone: 'mint', dims: [7.5, 3.4, 2.2], parts: [{ w: 180, h: 30, d: 82 }, { w: 76, h: 23, d: 60, y: -26 }] },
    { name: 'carapace', tone: 'sand', dims: [6.5, 3.2, 2.1], parts: [{ w: 156, h: 30, d: 77 }, { w: 68, h: 20, d: 52, y: -25 }] },
  ] },
  { name: '坦克', code: 'TK', models: [
    { name: 'anvil', tone: 'mint', dims: [10.5, 3.6, 2.6], parts: [{ w: 150, h: 30, d: 72 }, { w: 60, h: 22, d: 48, y: -26 }, { w: 90, h: 8, d: 8, x: 90, y: -26 }, { w: 150, h: 20, d: 16, y: 20, z: 28, tone: 'dark' }, { w: 150, h: 20, d: 16, y: 20, z: -28, tone: 'dark' }] },
    { name: 'carapace', tone: 'sand', dims: [9, 3.4, 2.6], parts: [{ w: 132, h: 30, d: 68 }, { w: 54, h: 22, d: 46, y: -26 }, { w: 76, h: 8, d: 8, x: 78, y: -26 }, { w: 132, h: 20, d: 16, y: 20, z: 26, tone: 'dark' }, { w: 132, h: 20, d: 16, y: 20, z: -26, tone: 'dark' }] },
  ] },
  { name: '障碍物', code: 'OB', models: [
    { name: 'concrete', tone: 'gray', dims: [4, 1.5, 3], parts: [
      { w: 120, h: 30, d: 45, y: 30 },
      { w: 120, h: 30, d: 36 },
      { w: 120, h: 30, d: 27, y: -30 },
    ] },
  ] },
];

// 模型卡片/详情页锚点编号，例如 BR-01；画廊与详情页用它互相对应
function modelCardId(category, modelIndex) {
  return `${category.code}-${String(modelIndex + 1).padStart(2, '0')}`;
}

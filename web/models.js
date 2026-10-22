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

// 组合模型：返回以模型中心为原点的 3D 盒子组合节点
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
const CATEGORIES = [
  {
    name: '炮管', code: 'BR',
    models: [
      {
        name: '长管加农', tone: 'gray', dims: [7, 1, 1],
        parts: [
          { w: 170, h: 16, d: 16 },
          { w: 30, h: 28, d: 28, x: -86, tone: 'dark' },
          { w: 12, h: 22, d: 22, x: 88, tone: 'dark' },
        ],
      },
      {
        name: '短管榴弹', tone: 'gray', dims: [4, 1.5, 1.5],
        parts: [
          { w: 96, h: 24, d: 24 },
          { w: 18, h: 32, d: 32, x: 46, tone: 'dark' },
          { w: 34, h: 36, d: 36, x: -52, tone: 'dark' },
        ],
      },
      {
        name: '双联机炮', tone: 'mint', dims: [4, 2, 1.5],
        parts: [
          { w: 116, h: 12, d: 12, z: 15 },
          { w: 116, h: 12, d: 12, z: -15 },
          { w: 30, h: 30, d: 52, x: -60, tone: 'dark' },
        ],
      },
    ],
  },
  {
    name: '履带', code: 'TR',
    models: [
      {
        name: '标准履带', tone: 'sand', dims: [4.5, 3, 1.5],
        parts: [
          { w: 168, h: 30, d: 24, z: 36 },
          { w: 168, h: 30, d: 24, z: -36 },
          { w: 120, h: 10, d: 60, y: -20, tone: 'dark' },
        ],
      },
      {
        name: '宽履带', tone: 'sand', dims: [4.5, 3.5, 1.5],
        parts: [
          { w: 168, h: 26, d: 34, z: 46 },
          { w: 168, h: 26, d: 34, z: -46 },
          { w: 110, h: 10, d: 70, y: -18, tone: 'dark' },
        ],
      },
      {
        name: '轻型履带', tone: 'gray', dims: [3.5, 2.5, 1],
        parts: [
          { w: 128, h: 22, d: 18, z: 28 },
          { w: 128, h: 22, d: 18, z: -28 },
        ],
      },
      // 以下为布局示例占位模型，非实际游戏资源
      {
        name: '窄履带', tone: 'sand', dims: [3.5, 2, 1],
        parts: [
          { w: 120, h: 20, d: 14, z: 22 },
          { w: 120, h: 20, d: 14, z: -22 },
        ],
      },
    ],
  },
  {
    name: '车身', code: 'CH',
    models: [
      {
        name: '标准底盘', tone: 'gray', dims: [4, 2.5, 1.5],
        parts: [
          { w: 150, h: 38, d: 92 },
          { w: 112, h: 12, d: 66, y: -25, tone: 'dark' },
          { w: 16, h: 24, d: 88, x: 80, tone: 'dark' },
        ],
      },
      {
        name: '装甲底盘', tone: 'sand', dims: [4.5, 3, 2],
        parts: [
          { w: 150, h: 48, d: 98 },
          { w: 116, h: 16, d: 72, y: -30, tone: 'dark' },
          { w: 18, h: 30, d: 92, x: 80, tone: 'dark' },
        ],
      },
      // 以下为布局示例占位模型，非实际游戏资源
      {
        name: '轻型底盘', tone: 'mint', dims: [3.5, 2, 1],
        parts: [
          { w: 120, h: 26, d: 72 },
          { w: 90, h: 10, d: 50, y: -18, tone: 'dark' },
        ],
      },
      {
        name: '加长底盘', tone: 'gray', dims: [5.5, 2.5, 1.5],
        parts: [
          { w: 190, h: 38, d: 92 },
          { w: 150, h: 12, d: 66, y: -25, tone: 'dark' },
        ],
      },
      {
        name: '低矮底盘', tone: 'dark', dims: [4, 2.5, 1],
        parts: [
          { w: 150, h: 24, d: 92 },
          { w: 112, h: 10, d: 66, y: -17, tone: 'gray' },
        ],
      },
    ],
  },
  {
    name: '坦克', code: 'TK',
    models: [
      {
        name: '侦察型', tone: 'mint', dims: [4.5, 2.5, 2],
        parts: [
          { w: 132, h: 32, d: 84 },
          { w: 58, h: 24, d: 52, y: -28, tone: 'gray' },
          { w: 84, h: 10, d: 10, x: 70, y: -28, tone: 'dark' },
        ],
      },
      {
        name: '主战型', tone: 'gray', dims: [7, 3.5, 2.5],
        parts: [
          { w: 156, h: 40, d: 100 },
          { w: 76, h: 30, d: 66, y: -35, tone: 'mint' },
          { w: 104, h: 13, d: 13, x: 92, y: -35, tone: 'dark' },
          { w: 14, h: 18, d: 18, x: 142, y: -35, tone: 'dark' },
        ],
      },
      {
        name: '重装型', tone: 'sand', dims: [7.5, 4, 3],
        parts: [
          { w: 164, h: 48, d: 112 },
          { w: 88, h: 36, d: 78, y: -42, tone: 'dark' },
          { w: 108, h: 12, d: 12, x: 96, y: -46, z: 12, tone: 'dark' },
          { w: 108, h: 12, d: 12, x: 96, y: -46, z: -12, tone: 'dark' },
        ],
      },
      // 以下为布局示例占位模型，非实际游戏资源
      {
        name: '轻型坦克', tone: 'mint', dims: [4, 2.5, 1.5],
        parts: [
          { w: 110, h: 28, d: 72 },
          { w: 48, h: 20, d: 44, y: -24, tone: 'gray' },
          { w: 66, h: 9, d: 9, x: 60, y: -24, tone: 'dark' },
        ],
      },
      {
        name: '自行火炮', tone: 'sand', dims: [6.5, 3, 2],
        parts: [
          { w: 150, h: 34, d: 92 },
          { w: 70, h: 28, d: 60, y: -31, tone: 'gray' },
          { w: 120, h: 10, d: 10, x: 100, y: -31, tone: 'dark' },
        ],
      },
      {
        name: '坦克歼击车', tone: 'dark', dims: [6, 3, 2],
        parts: [
          { w: 140, h: 30, d: 88 },
          { w: 60, h: 24, d: 52, y: -27, tone: 'sand' },
          { w: 96, h: 11, d: 11, x: 82, y: -27, tone: 'gray' },
        ],
      },
    ],
  },
];

// 模型卡片/详情页锚点编号，例如 BR-01；画廊与详情页用它互相对应
function modelCardId(category, modelIndex) {
  return `${category.code}-${String(modelIndex + 1).padStart(2, '0')}`;
}

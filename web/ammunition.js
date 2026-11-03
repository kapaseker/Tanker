// 网页原型与游戏读取同一份模型旁配置；HTTP 下 fetch 真实文件，
// file://（设计稿直接双击预览）下浏览器禁止 fetch，退回内置快照。
const Ammunition = (() => {
  // 配置与弹丸模型同名（ADR-0007）：bullet.vox 对应 bullet.jsonc
  const path = '../assets/tank/ammunition/apfsds/bullet.jsonc';
  // file:// 回退快照：数值必须与 bullet.jsonc 保持一致，
  // ammunition.test.cjs 断言两者同步，防止调参后静默漂移。
  const fileSnapshot =
    '{"muzzle_velocity":1200,"rate_of_fire":12,' +
    '"collision":{"size":[0.72,0.04,0.04],"center_forward_offset":0.16}}';

  // 保留字符串与换行，只去掉 JSONC 注释；JSON.parse 继续拒绝损坏结构和尾逗号。
  function parse(text) {
    const json = text.replace(/"(?:\\.|[^"\\])*"|\/\/[^\r\n]*|\/\*[\s\S]*?\*\//g,
      token => token.startsWith('"') ? token : token.replace(/[^\r\n]/g, ' '));
    const config = JSON.parse(json);
    const keys = Array.from(json.matchAll(/"(?:\\.|[^"\\])*"\s*:/g),
      match => JSON.parse(match[0].slice(0, -1)));
    if (new Set(keys).size !== keys.length) throw new Error('弹药配置不允许重复字段');
    // 顶层字段与游戏配置契约（ADR-0007）一致：初速、射速与弹体扫掠盒。
    // collision 页面不使用，只校验存在且为对象，数值页面不消费。
    const fields = ['muzzle_velocity', 'rate_of_fire'];
    if (!config || typeof config !== 'object' || Array.isArray(config)
        || Object.keys(config).length !== fields.length + 1
        || !fields.every(field => Object.hasOwn(config, field)
          && typeof config[field] === 'number' && Number.isFinite(Math.fround(config[field]))
          && Math.fround(config[field]) > 0)
        || !config.collision || typeof config.collision !== 'object'
        || Array.isArray(config.collision)) {
      throw new Error('只允许 muzzle_velocity、rate_of_fire 和 collision，且前两者必须是有限正数');
    }
    // 运行时 Rust 使用 f32；网页也按相同精度拒绝溢出、下溢和不可用冷却。
    for (const field of fields) config[field] = Math.fround(config[field]);
    const cooldown = Math.fround(60 / config.rate_of_fire);
    if (!Number.isFinite(cooldown) || cooldown <= 0) throw new Error('射速换算的冷却必须是有限正数');
    return { ...config, cooldown };
  }

  // 与游戏 HUD 一致：冷却秒数向上取整后等分，极长冷却只显示最多 60 格。
  function reloadTicks(cooldown) {
    const count = Math.max(1, Math.min(60, Math.ceil(cooldown)));
    return Array.from({ length: count - 1 }, (_, index) => (index + 1) / count * 100);
  }

  async function load() {
    if (location.protocol === 'file:') return parse(fileSnapshot);
    try {
      const response = await fetch(path, { cache: 'no-store' });
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      return parse(await response.text());
    } catch (error) {
      throw new Error(`弹药配置加载失败（${path}）：${error.message}`);
    }
  }

  return { parse, reloadTicks, load, path, fileSnapshot };
})();

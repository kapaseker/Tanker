// 网页原型与游戏读取同一份模型旁配置；通过 HTTP 服务打开，避免 file:// 的读取限制。
const Ammunition = (() => {
  const path = '../assets/tank/ammunition/apfsds/ammunition.jsonc';

  // 保留字符串与换行，只去掉 JSONC 注释；JSON.parse 继续拒绝损坏结构和尾逗号。
  function parse(text) {
    const json = text.replace(/"(?:\\.|[^"\\])*"|\/\/[^\r\n]*|\/\*[\s\S]*?\*\//g,
      token => token.startsWith('"') ? token : token.replace(/[^\r\n]/g, ' '));
    const config = JSON.parse(json);
    const keys = Array.from(json.matchAll(/"(?:\\.|[^"\\])*"\s*:/g),
      match => JSON.parse(match[0].slice(0, -1)));
    if (new Set(keys).size !== keys.length) throw new Error('弹药配置不允许重复字段');
    const fields = ['muzzle_velocity', 'rate_of_fire'];
    if (!config || typeof config !== 'object' || Array.isArray(config)
        || Object.keys(config).length !== fields.length
        || !fields.every(field => Object.hasOwn(config, field)
          && typeof config[field] === 'number' && Number.isFinite(Math.fround(config[field]))
          && Math.fround(config[field]) > 0)) {
      throw new Error('只允许 muzzle_velocity 和 rate_of_fire，且两者都必须是有限正数');
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
    try {
      const response = await fetch(path, { cache: 'no-store' });
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      return parse(await response.text());
    } catch (error) {
      throw new Error(`弹药配置加载失败（${path}）：${error.message}`);
    }
  }

  return { parse, reloadTicks, load };
})();

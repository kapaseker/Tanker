// 无依赖自检：node web/ammunition.test.cjs。
const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { join } = require('node:path');
const vm = require('node:vm');

(async () => {
  let fetchResult;
  const context = vm.createContext({ fetch: async (path, options) => {
    assert.equal(path, '../assets/tank/ammunition/apfsds/ammunition.jsonc');
    assert.equal(options.cache, 'no-store');
    return fetchResult;
  } });
  const ammunition = vm.runInContext(readFileSync(join(__dirname, 'ammunition.js'), 'utf8')
    + '\nAmmunition;', context);
  const actual = readFileSync(join(__dirname, '../assets/tank/ammunition/apfsds/ammunition.jsonc'), 'utf8');
  const config = ammunition.parse(actual);
  assert.equal(config.muzzle_velocity, 1200);
  assert.equal(config.rate_of_fire, 12);
  assert.equal(config.cooldown, 5);
  assert.equal(ammunition.parse('{/*单位 m/s*/"muzzle_velocity":800,// rpm\n"rate_of_fire":30}').cooldown, 2);
  for (const invalid of [
    '{}', 'null', '[]', '{"muzzle_velocity":0,"rate_of_fire":12}',
    '{"muzzle_velocity":1200,"rate_of_fire":-1}',
    '{"muzzle_velocity":1200,"rate_of_fire":"12"}',
    '{"muzzle_velocity":1e999,"rate_of_fire":12}',
    '{"muzzle_velocity":1e99,"rate_of_fire":12}',
    '{"muzzle_velocity":1200,"rate_of_fire":1e-44}',
    '{"muzzle_velocity":1200,"rate_of_fire":12,"cooldown":5}',
    '{"muzzle_velocity":1,"muzzle_velocity":1200,"rate_of_fire":12}',
    '{"muzzle_velocity":1200,"rate_of_fire":12,}',
    '{"muzzle_velocity":1200,"rate_of_fire":12} /* unfinished',
  ]) assert.throws(() => ammunition.parse(invalid), undefined, invalid);
  assert.equal(ammunition.reloadTicks(0.5).length, 0);
  assert.equal(ammunition.reloadTicks(5).join(','), '20,40,60,80');
  assert.equal(ammunition.reloadTicks(4.5).length, 4);
  assert.equal(ammunition.reloadTicks(4.5)[3], 80);
  assert.equal(ammunition.reloadTicks(1e9).length, 59);
  assert.ok(ammunition.reloadTicks(1e9).at(-1) < 100);
  fetchResult = { ok: true, text: async () => actual };
  assert.equal((await ammunition.load()).cooldown, 5);
  fetchResult = { ok: false, status: 404 };
  await assert.rejects(ammunition.load(), /弹药配置加载失败.*HTTP 404/);
  fetchResult = { ok: true, text: async () => '{}' };
  await assert.rejects(ammunition.load(), /弹药配置加载失败/);
  const catalog = vm.runInNewContext(readFileSync(join(__dirname, 'models.js'), 'utf8') + '\nCATEGORIES;');
  assert.equal(catalog[0].models[0].specs, undefined, '弹药数值应从配置读取');
  assert.equal(catalog[1].models[0].specs.map(spec => spec.label).join(','), '前进速度,倒车速度');
  console.log('弹药配置解析、同源加载、规格归属和冷却分格自检通过');
})().catch(error => { console.error(error); process.exitCode = 1; });

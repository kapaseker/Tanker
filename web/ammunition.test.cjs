// 无依赖自检：node web/ammunition.test.cjs。
const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { join } = require('node:path');
const vm = require('node:vm');

(async () => {
  let fetchResult;
  const context = vm.createContext({ location: { protocol: 'https:' }, fetch: async (path, options) => {
    assert.equal(path, '../assets/tank/ammunition/apfsds/bullet.jsonc');
    assert.equal(options.cache, 'no-store');
    return fetchResult;
  } });
  const ammunition = vm.runInContext(readFileSync(join(__dirname, 'ammunition.js'), 'utf8')
    + '\nAmmunition;', context);
  const actual = readFileSync(join(__dirname, '../assets/tank/ammunition/apfsds/bullet.jsonc'), 'utf8');
  const config = ammunition.parse(actual);
  assert.equal(config.muzzle_velocity, 1640);
  assert.equal(config.rate_of_fire, 8);
  assert.equal(config.cooldown, 7.5);
  const COLLISION = '"collision":{"size":[0.72,0.04,0.04],"center_forward_offset":0.16}';
  assert.equal(ammunition.parse(`{/*单位 m/s*/"muzzle_velocity":800,// rpm\n"rate_of_fire":30,${COLLISION}}`).cooldown, 2);
  for (const invalid of [
    '{}', 'null', '[]', '{"muzzle_velocity":0,"rate_of_fire":12}',
    '{"muzzle_velocity":1200,"rate_of_fire":-1}',
    '{"muzzle_velocity":1200,"rate_of_fire":"12"}',
    '{"muzzle_velocity":1200,"rate_of_fire":12}',
    '{"muzzle_velocity":1200,"rate_of_fire":12,"collision":null}',
    '{"muzzle_velocity":1200,"rate_of_fire":12,"collision":[]}',
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
  assert.equal((await ammunition.load()).cooldown, 7.5);
  fetchResult = { ok: false, status: 404 };
  await assert.rejects(ammunition.load(), /弹药配置加载失败.*HTTP 404/);
  fetchResult = { ok: true, text: async () => '{}' };
  await assert.rejects(ammunition.load(), /弹药配置加载失败/);
  // file:// 下不发 fetch，直接解析内置快照，且快照必须与真实配置一致
  const fileAmmunition = vm.runInContext(
    readFileSync(join(__dirname, 'ammunition.js'), 'utf8') + '\nAmmunition;',
    vm.createContext({ location: { protocol: 'file:' },
      fetch: () => assert.fail('file:// 下不应发起 fetch') }));
  const snapshot = await fileAmmunition.load();
  assert.deepEqual([snapshot.muzzle_velocity, snapshot.rate_of_fire, snapshot.cooldown],
    [config.muzzle_velocity, config.rate_of_fire, config.cooldown]);
  const catalog = vm.runInNewContext(readFileSync(join(__dirname, 'models.js'), 'utf8') + '\nCATEGORIES;');
  assert.equal(catalog[0].models[0].specs, undefined, '弹药数值应从配置读取');
  const player = catalog[1].models[0];
  const tank = JSON.parse(readFileSync(join(__dirname, '../assets/tank/player/leopard2_a5/tank.jsonc'), 'utf8')
    .replace(/^\s*\/\/.*$/gm, ''));
  assert.equal(player.name, 'Leopard 2A5');
  assert.equal(player.specs.map(spec => spec.label).join(','),
    '前进速度,倒车速度,水平旋转,炮管俯仰范围,水平转速,俯仰速度,单发伤害,最大生命值');
  assert.deepEqual(Array.from(player.specs, spec => spec.value), [
    String(tank.driving.forward_speed), String(Number(tank.driving.reverse_speed.toFixed(2))),
    '全圈', `${tank.articulation.min_elevation_deg}°～+${tank.articulation.max_elevation_deg}°`,
    `${tank.articulation.traverse_speed_deg_s}°`, `${tank.articulation.elevation_speed_deg_s}°`,
    String(tank.shot_damage), String(tank.max_health),
  ]);
  console.log('弹药配置解析、同源加载、规格归属和冷却分格自检通过');
})().catch(error => { console.error(error); process.exitCode = 1; });

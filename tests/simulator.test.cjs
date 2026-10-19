'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const vm = require('node:vm');
const html = readFileSync(require('node:path').join(__dirname, '../index.html'), 'utf8');

// Run the real inline application in a minimal DOM/canvas shell. This checks
// simulation state, not browser rendering; no production test API is exported.
function simulator({ scheme = '2808', auto = true, clock = '4' } = {}) {
  const elements = new Map();
  const noop = () => {};
  const ctx = new Proxy({}, { get: (_, name) => name === 'createRadialGradient' ? () => ({ addColorStop: noop }) : noop, set: () => true });
  const element = id => {
    if (!elements.has(id)) elements.set(id, {
      value: '', checked: false, textContent: '', innerHTML: '', dataset: {}, children: [],
      parentElement: { getBoundingClientRect: () => ({ width: 600, height: 600 }) },
      getContext: () => ctx, listeners: {},
      addEventListener(type, listener) { this.listeners[type] = listener; },
      setAttribute(name, value) { this[name] = value; },
    });
    return elements.get(id);
  };
  for (const match of html.matchAll(/\bid="([^"]+)"/g)) element(match[1]);
  element('scheme').value = scheme;
  element('autoCycle').checked = auto;
  element('clockSpeed').value = clock;
  element('bunchSpeed').value = '0.6';
  element('betaTarget').value = '0.3';
  const sandbox = {
    document: { getElementById: id => { assert(elements.has(id), `missing DOM id ${id}`); return element(id); }, createElement: () => element('generated-canvas'), documentElement: {} },
    matchMedia: () => ({ matches: false }),
    ResizeObserver: class { observe() {} },
    getComputedStyle: () => ({ getPropertyValue: () => '#ffffff' }),
    devicePixelRatio: 1, performance: { now: () => 0 }, requestAnimationFrame: noop,
  };
  vm.createContext(sandbox);
  const script = html.match(/<script>([\s\S]*?)<\/script>/)[1];
  vm.runInContext(script.replace(/\}\)\(\);\s*$/, 'globalThis.api = { S, update, reset, buildScheme, lumiAt, startInject, startRamp, startSqueeze, startCollide, startDump, readouts, makeEvent, frame }; })();'), sandbox);
  return { ...sandbox.api, elements, until(predicate, max = 40000) {
    for (let i = 0; i < max && !predicate(); i++) sandbox.api.update(0.05);
    assert(predicate(), `timed out in ${sandbox.api.S.mode}`);
  } };
}

test('filling schemes have exact unique in-range slots and abort clearance', () => {
  const s = simulator();
  for (const count of [1, 12, 1224, 2808]) {
    const slots = s.buildScheme(count).flat();
    assert.equal(slots.length, count);
    assert.equal(new Set(slots).size, count);
    assert(slots.every(n => Number.isInteger(n) && n >= 0 && n < 3564));
    assert(3564 - Math.max(...slots) - 1 >= 120);
  }
});

test('all schemes reach stable beams and accumulate finite luminosity', () => {
  for (const scheme of ['1', '12', '1224', '2808']) {
    const s = simulator({ scheme });
    s.startInject();
    s.until(() => s.S.mode === 'STABLE BEAMS');
    assert.equal(s.S.E, 6800);
    assert.equal(s.S.betaStar, 0.3);
    for (const beam of s.S.beams) assert.equal(beam.filter(b => b.state === 1).length, Number(scheme));
    const before = s.S.beams[0][0].N;
    s.update(0.05);
    for (const x of [s.S.lumi, s.S.lint, s.S.coll]) assert(Number.isFinite(x) && x > 0);
    assert(s.S.beams[0][0].N < before);
  }
});

test('manual cycle gates stages and dump returns to no beam', () => {
  const s = simulator({ scheme: '12', auto: false });
  s.startRamp(); assert.equal(s.S.mode, 'NO BEAM');
  s.startInject(); s.startInject();
  s.until(() => s.S.fillingDone);
  assert.equal(s.S.mode, 'INJECTION PHYSICS BEAM');
  s.startRamp(); s.until(() => s.S.mode === 'FLAT TOP');
  for (let i = 0; i < 50; i++) s.update(0.05);
  assert.equal(s.S.mode, 'FLAT TOP');
  s.startSqueeze(); s.until(() => s.S.modeT >= 600);
  s.startCollide(); s.until(() => s.S.mode === 'STABLE BEAMS');
  s.startDump(); s.until(() => s.S.mode === 'NO BEAM');
  assert.equal(s.S.E, 0);
  assert.equal(s.S.fill, 10422);
  assert(s.S.beams.every(b => b.length === 0));
});

test('interrupted injection dumps pending beams safely; reset permits a new fill', () => {
  const s = simulator({ auto: false });
  s.startInject(); s.update(0.05); s.startDump();
  s.until(() => s.S.mode === 'NO BEAM');
  s.reset(true); s.startInject();
  s.until(() => s.S.fillingDone);
  assert(s.S.beams.every(b => b.length === 2808));
});

test('luminosity zero/no-beam and intensity-squared scaling', () => {
  const s = simulator();
  assert.equal(s.lumiAt(0, 6800, 0.3, 1.15e11), 0);
  const l = s.lumiAt(2808, 6800, 0.3, 1.15e11);
  assert(l > 1e34 && l < 3e34);
  assert.equal(s.lumiAt(2808, 6800, 0.3, 2.3e11), 4 * l);
});

test('single-file site has no local asset dependencies or executable external scripts', () => {
  assert.equal((html.match(/<script\b/g) || []).length, 1);
  assert(!/<script[^>]+src=/.test(html));
  for (const m of html.matchAll(/(?:src|href)="([^"]+)"/g)) assert(m[1].startsWith('https://fonts.googleapis.com/'), m[1]);
  assert(!/\b(?:fetch|eval)\s*\(/.test(html));
});

test('enabling sequencer after manual filling resumes the cycle', () => {
  const s = simulator({ auto: false });
  s.startInject();
  s.until(() => s.S.fillingDone);
  assert.equal(s.S.mode, 'INJECTION PHYSICS BEAM');
  s.elements.get('autoCycle').checked = true;
  s.update(0.05);
  assert.equal(s.S.mode, 'PREPARE RAMP');
});


test('pause freezes time; resume and reset controls remain usable', () => {
  const s = simulator({ scheme: '1' });
  s.startInject(); s.frame(50);
  const pause = s.elements.get('btnPause');
  pause.listeners.click();
  const clock = s.S.tM, phase = s.S.phi;
  s.frame(100); s.frame(150);
  assert.equal(s.S.tM, clock); assert.equal(s.S.phi, phase);
  assert.equal(pause['aria-pressed'], 'true');
  s.elements.get('btnReset').listeners.click();
  assert.equal(s.S.mode, 'NO BEAM');
  s.elements.get('btnInject').listeners.click();
  pause.listeners.click(); s.frame(200);
  assert(s.S.tM > 0);
  assert.equal(pause['aria-pressed'], 'false');
});

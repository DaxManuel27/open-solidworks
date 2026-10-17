// Phase 5 desktop smoke: sheet metal through the real ribbon, panels, tree,
// store and core.
import assert from 'node:assert/strict'
import { existsSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

export async function sheetMetalSmoke(context) {
  const { js } = context
  await js(`window.__smConfirm = window.confirm; window.confirm = () => true; void 0`)
  try {
    await scenario(context)
  } finally {
    await js(`window.confirm = window.__smConfirm; void 0`)
  }
}

async function scenario({ js, call, wait, temp, win }) {
  console.log('Sheet metal smoke: base and edge flanges, flatten, gauge, flat DXF')
  const frame = () =>
    js('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))')
  const store = (expr) =>
    js(`(async () => { const s = window.__openforgeStore.getState(); return ${expr} })()`)
  const run = (method, params = {}) =>
    store(`s.run(${JSON.stringify(method)}, ${JSON.stringify(params)}).then(() => true)`)
  const field = (label, n = 0) =>
    `[...document.querySelectorAll('.panel label')].filter(l => l.querySelector('span')?.textContent === ${JSON.stringify(label)})[${n}]`
  const choose = async (label, value) => {
    await js(`(() => {
      const select = ${field(label)}?.querySelector('select'); if (!select) throw new Error('Missing select ${label}');
      select.value = ${JSON.stringify(String(value))}; select.dispatchEvent(new Event('change', { bubbles: true }));
    })()`)
    await frame()
  }
  const check = async (label, on) => {
    await js(`(() => {
      const box = ${field(label)}?.querySelector('input[type=checkbox]'); if (!box) throw new Error('Missing check ${label}');
      if (box.checked !== ${on}) box.click();
    })()`)
    await frame()
  }
  const type = async (label, value) => {
    await js(`(() => {
      const input = ${field(label)}?.querySelector('input'); if (!input) throw new Error('Missing field ${label}');
      input.focus();
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(input, ${JSON.stringify(String(value))});
      input.dispatchEvent(new Event('input', { bubbles: true }));
    })()`)
    await frame()
    await js(
      `document.activeElement.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))`,
    )
    await js('document.activeElement.blur()')
    await frame()
  }
  const panel = (title) =>
    wait(
      () => js(`document.querySelector('.panel-title')?.textContent === ${JSON.stringify(title)}`),
      `${title} panel`,
    )
  const accept = async (what) => {
    await wait(
      () =>
        js(
          `!!document.querySelector('.panel .ok:not(:disabled)') && !document.querySelector('.panel-error')`,
        ),
      `valid ${what} preview`,
    )
    await js(`document.querySelector('.panel .ok').click()`)
    await wait(() => js(`!document.querySelector('.panel')`), `${what} accepted`)
  }
  const tab = (label) =>
    js(
      `[...document.querySelectorAll('.of-tabs button')].find(b => b.textContent === ${JSON.stringify(label)}).click()`,
    )
  const command = async (id) => {
    await wait(
      () => js(`!!document.querySelector('[data-command="${id}"]:not(:disabled)')`),
      `${id} enabled`,
    )
    await js(`document.querySelector('[data-command="${id}"]').click()`)
  }
  const pick = (kind, ref) =>
    store(
      `(s.pickTarget.onPick({ kind: ${JSON.stringify(kind)}, index: 0 }, ${JSON.stringify(ref)}), true)`,
    )
  const shot = async (name) => {
    const dir = process.env.OPENFORGE_SMOKE_SHOTS
    if (!dir) return
    await frame()
    writeFileSync(join(dir, `sheet-metal-${name}.png`), (await win.capturePage()).toPNG())
  }
  const close = (a, b, what, tol = 1e-4) =>
    assert.ok(Math.abs(a - b) < tol * Math.max(1, Math.abs(b)), `${what}: ${a} != ${b}`)

  // An 80 × 50 sketch, through the core; the store follows.
  await run('doc.new')
  const sketch = (await call('sketch.new', { plane: { type: 'front_plane' } })).id
  await call('sketch.edit', { id: sketch, op: { op: 'rectangle', a: [0, 0], b: [80, 50] } })
  await call('sketch.close')
  await run('doc.info')
  await wait(() => store(`s.snapshot.features.some(f => f.type === 'sketch')`), 'sketch in tree')

  // Sheet metal commands wait for a sheet metal body.
  await tab('Sheet Metal')
  assert.match(
    await js(`document.querySelector('[data-command="edgeFlange"]').title`),
    /Requires a sheet metal body/,
  )

  // Base flange from the sketch, 1.5 mm with R1.5 bends.
  await command('baseFlange')
  await panel('Base Flange/Tab')
  await choose('Sketch', sketch)
  await type('Thickness', '1.5')
  await type('Bend radius', '1.5')
  await accept('base flange')
  await wait(() => store(`s.snapshot.sheet_metal?.length === 1`), 'sheet metal body')
  assert.equal(await store(`s.snapshot.sheet_metal[0].parameters.thickness`), 1.5)

  // Edge flanges on the four top edges.
  const t = 1.5
  const topEdges = (await call('doc.topology')).edges.filter(
    (e) => e.curve === 'line' && Math.abs(e.start[2] - t) < 1e-6 && Math.abs(e.end[2] - t) < 1e-6,
  )
  assert.equal(topEdges.length, 4)
  await command('edgeFlange')
  await panel('Edge-Flange')
  // One pick per frame, as clicks arrive.
  for (const e of topEdges) {
    await pick('edge', e.ref)
    await frame()
  }
  await wait(() => js(`document.querySelectorAll('.panel .chip').length === 4`), 'four edges')
  await type('Flange length', '20')
  await shot('edge-flange-panel')
  await accept('edge flange')
  await wait(() => store(`s.snapshot.sheet_metal[0].bends === 4`), 'four bends')
  const formed = (await call('mass_properties')).volume

  // Flatten: the body lies flat with its bend lines shown.
  await command('flatten')
  await wait(() => store(`s.snapshot.sheet_metal[0].flattened`), 'flattened')
  await wait(() => store(`(s.scene?.bend_lines ?? []).length === 4`), 'bend lines in scene')
  await shot('flattened')
  const flat = await call('doc.sheet_metal')
  const props = flat.bodies[0].properties
  assert.equal(props.bends, 4)
  const leg = 20 - (1.5 + 1.5) + (Math.PI / 2) * (1.5 + 0.44 * 1.5)
  // Material inside: each bend takes r + t from the plate.
  close(props.flat_length, 80 - 2 * (1.5 + 1.5) + 2 * leg, 'flat length', 1e-6)
  await command('flatten')
  await wait(() => store(`!s.snapshot.sheet_metal[0].flattened`), 'formed again')
  close((await call('mass_properties')).volume, formed, 'formed volume')

  // Gauge 16 steel from the tree's Sheet Metal folder: the flat follows.
  await js(
    `[...document.querySelectorAll('.tree-item')].find(e => e.textContent.includes(' bends'))
      .dispatchEvent(new MouseEvent('dblclick', { bubbles: true }))`,
  )
  await panel('Base Flange/Tab')
  await check('Use gauge table', true)
  await wait(() => js(`!!${field('Gauge')}?.querySelector('option[value="16"]')`), 'gauges listed')
  await choose('Gauge', '16')
  await accept('gauge')
  await wait(() => store(`s.snapshot.sheet_metal[0].parameters.gauge?.gauge === '16'`), 'gauge set')
  const gauged = (await call('doc.sheet_metal')).bodies[0].properties
  close(gauged.thickness, 1.519, 'gauge 16 thickness', 1e-3)

  // Flat DXF through the core.
  const dxf = join(temp, 'smoke-flat.dxf')
  await call('export', { path: dxf })
  assert.ok(existsSync(dxf))
  const text = readFileSync(dxf, 'utf8')
  assert.ok(text.includes('BEND_UP') || text.includes('BEND_DOWN'))
  assert.ok(text.includes('CUT'))
  assert.deepEqual(await js('window.__smokeErrors'), [], 'renderer errors in sheet metal')
}

// Phase 5 desktop smoke: weldments through the real ribbon, panels, tree,
// store and core.
import assert from 'node:assert/strict'
import { join } from 'node:path'

export async function weldmentSmoke(context) {
  const { js } = context
  await js(`window.__wdConfirm = window.confirm; window.confirm = () => true; void 0`)
  try {
    await scenario(context)
  } finally {
    await js(`window.confirm = window.__wdConfirm; void 0`)
  }
}

async function scenario({ js, call, wait, temp }) {
  console.log('Weldment smoke: members from a sketch, trim, cut list, cut list table')
  const frame = () =>
    js('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))')
  const store = (expr) =>
    js(`(async () => { const s = window.__openforgeStore.getState(); return ${expr} })()`)
  const run = (method, params = {}) =>
    store(`s.run(${JSON.stringify(method)}, ${JSON.stringify(params)}).then(() => true)`)
  const field = (label, n = 0) =>
    `[...document.querySelectorAll('.panel label')].filter(l => l.querySelector('span')?.textContent === ${JSON.stringify(label)})[${n}]`
  const choose = async (label, value, n = 0) => {
    await js(`(() => {
      const select = ${field(label, n)}?.querySelector('select'); if (!select) throw new Error('Missing select ${label}');
      select.value = ${JSON.stringify(String(value))}; select.dispatchEvent(new Event('change', { bubbles: true }));
    })()`)
    await frame()
  }
  const check = async (label, on, n = 0) => {
    await js(`(() => {
      const box = ${field(label, n)}?.querySelector('input[type=checkbox]'); if (!box) throw new Error('Missing check ${label}');
      if (box.checked !== ${on}) box.click();
    })()`)
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
  const labelOf = async (id) => {
    const curve = (await call('sketch.get', { id: sketch })).curves.find((c) => c.id === id)
    const [a, b] = [curve.samples[0], curve.samples[curve.samples.length - 1]]
    return `Line ${id} (${Math.hypot(b[0] - a[0], b[1] - a[1]).toFixed(1)} mm)`
  }

  // A 1200 × 600 rectangle and two cross lines, through the core.
  await run('doc.new')
  const { id: sketch } = await call('sketch.new', { plane: { type: 'front_plane' } })
  const rect = await call('sketch.edit', {
    id: sketch,
    op: { op: 'rectangle', a: [0, 0], b: [1200, 600] },
  })
  const cross = []
  for (const y of [200, 400]) {
    const r = await call('sketch.edit', {
      id: sketch,
      op: { op: 'add_line', a: { at: [0, y] }, b: { at: [1200, y] } },
    })
    cross.push(r.created[0])
  }
  await call('sketch.close')
  await run('doc.info')

  // The frame: SHS 40x40x3 on the rectangle, mitered.
  await tab('Weldments')
  await command('structuralMember')
  await panel('Structural Member')
  await wait(() => js(`!!${field('Size')}?.querySelector('option[value="SHS 40x40x3"]')`), 'sizes')
  await wait(() => js(`!!document.querySelector('.profile-preview svg')`), 'section preview')
  await choose('Sketch', sketch)
  for (const id of rect.created.slice(4, 8)) {
    const label = await labelOf(id)
    await wait(() => js(`!!${field(label)}`), `${label} listed`)
    await check(label, true)
  }
  await accept('frame members')
  await wait(() => store(`s.snapshot.features.some(f => f.type === 'weldment')`), 'weldment added')
  // Cross members: RHS 60x40x3 on the two inner lines.
  await command('structuralMember')
  await panel('Structural Member')
  await wait(() => js(`!!${field('Size')}?.querySelector('option[value="RHS 60x40x3"]')`), 'sizes')
  await choose('Size', 'RHS 60x40x3')
  await wait(
    () => js(`${field('Size')}?.querySelector('select').value === 'RHS 60x40x3'`),
    'RHS chosen',
  )
  await choose('Sketch', sketch)
  for (const id of cross) {
    const label = await labelOf(id)
    await wait(() => js(`!!${field(label)}`), `${label} listed`)
    await check(label, true)
  }
  await accept('cross members')
  const features = await store(
    `s.snapshot.features.filter(f => f.type === 'structural_member').map(f => f.id)`,
  )
  const [f1, f2] = features
  const bodyName = (id) => store(`s.scene.bodies.find(b => b.id === ${JSON.stringify(id)}).name`)

  // Trim the cross members to the frame's sides.
  await command('trimExtend')
  await panel('Trim/Extend')
  for (const id of [`b${f2}:g0.0`, `b${f2}:g0.1`]) await check(await bodyName(id), true, 0)
  for (const id of [`b${f1}:g0.3`, `b${f1}:g0.1`]) {
    // The boundary list leaves out the members being trimmed.
    await check(await bodyName(id), true, 1)
  }
  await accept('trim')

  // The cut list: three member items.
  const list = await call('doc.cut_list')
  const rows = list.items.map((i) => [i.properties.DESCRIPTION, i.properties.LENGTH, i.quantity])
  assert.deepEqual(rows, [
    ['SHS 40x40x3', '1240', 2],
    ['SHS 40x40x3', '640', 2],
    ['RHS 60x40x3', '1160', 2],
  ])
  await wait(
    () =>
      js(
        `[...document.querySelectorAll('.tree-item')].some(e => e.textContent.startsWith('Cut List (3)'))`,
      ),
    'cut list in the tree',
  )

  // A drawing of the part with a cut list table.
  const part = join(temp, 'smoke-frame.ofp')
  await call('doc.save', { path: part })
  await store(`s.run('doc.info')`)
  await store(`s.run('drawing.new', { model: ${JSON.stringify(part)} })`)
  await wait(() => store(`!!s.drawing && s.tab === 'drawing'`), 'drawing adopted')
  await tab('Annotation')
  await store(`(s.openPanel({ type: 'table', variant: 'cut_list' }), true)`)
  await panel('Weldment Cut List')
  await js(
    `(() => { const t = window.__openforgeDrawing.getState().target; if (!t) throw new Error('no pick target'); t.onPick({ type: 'point', at: [30, 280], view: null }); })()`,
  )
  await wait(() => js(`!document.querySelector('.panel')`), 'table placed')
  await wait(
    () =>
      js(
        `(() => { const d = window.__openforgeDrawing.getState(); return !!d.render && !d.rendering && d.render.graphics.items.some(i => i.prim.type === 'text' && i.prim.text === '1160') })()`,
      ),
    'cut list table drawn',
  )
  assert.deepEqual(await js('window.__smokeErrors'), [], 'renderer errors in weldments')
}

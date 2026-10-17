// Phase 6 desktop smoke: surfaces, direct editing, mould analysis and mesh
// bodies through the real ribbon, panels, tree, store and core.
import assert from 'node:assert/strict'
import { join } from 'node:path'

export async function surfacingSmoke(context) {
  const { js } = context
  await js(`window.__p6Confirm = window.confirm; window.confirm = () => true; void 0`)
  try {
    await scenario(context)
  } finally {
    await js(`window.confirm = window.__p6Confirm; void 0`)
  }
}

async function scenario({ js, call, wait, temp, dialogs }) {
  console.log('Phase 6 smoke: surfaces, knit, move face, draft analysis, parting line, mesh import')
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
      () =>
        js(
          `document.querySelector('.panel-title')?.textContent === ${JSON.stringify(title)} && !document.querySelector('.panel-body')?.textContent.includes('Loading…') && !document.querySelector('.panel-error')`,
        ),
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
  const close = (a, b, what, tol = 1e-6) =>
    assert.ok(Math.abs(a - b) < tol * Math.max(1, Math.abs(b)), `${what}: ${a} != ${b}`)

  // A 20 × 10 rectangle; the surface commands wait for it.
  await run('doc.new')
  await tab('Surfaces')
  assert.match(
    await js(`document.querySelector('[data-command="p6Knit"]').title`),
    /Requires a surface body/,
  )
  const sketch = (await call('sketch.new', { plane: { type: 'front_plane' } })).id
  await call('sketch.edit', { id: sketch, op: { op: 'rectangle', a: [0, 0], b: [20, 10] } })
  await call('sketch.close')
  await run('doc.info')

  // A capped extruded surface: a closed surface body in its own folder.
  await command('p6ExtrudedSurface')
  await panel('Extruded Surface')
  await choose('Sketch', sketch)
  await type('Depth', '5')
  await check('Cap ends', true)
  await accept('extruded surface')
  await wait(
    () =>
      js(
        `[...document.querySelectorAll('.tree-item.meta')].some(e => e.textContent === 'Surface Bodies (1)')`,
      ),
    'surface bodies folder',
  )

  // Knit it into a solid.
  await command('p6Knit')
  await panel('Knit Surface')
  const surface = await store(`s.scene.bodies.find(b => b.kind === 'surface').id`)
  await js(`(() => {
    const box = [...document.querySelectorAll('.panel label')].find(l => l.textContent.includes('Surface-Body 1'))?.querySelector('input');
    if (!box) throw new Error('no surface body choice'); if (!box.checked) box.click();
  })()`)
  await frame()
  await check('Create solid', true)
  await accept('knit')
  close((await call('mass_properties')).volume, 1000, 'knitted solid')
  assert.equal(await store(`s.scene.bodies.filter(b => b.kind === 'surface').length`), 0)
  void surface

  // Move the top face up 3 mm from the Direct Editing tab.
  const top = (await call('doc.topology')).faces.find((f) => Math.abs(f.centroid[2] - 5) < 1e-6)
  await tab('Direct Editing')
  await command('p6MoveFace')
  await panel('Move Face')
  await pick('face', top.ref)
  await frame()
  await type('Distance', '3')
  await accept('move face')
  close((await call('mass_properties')).volume, 20 * 10 * 8, 'moved face')

  // Draft analysis colours every face; nothing here has draft.
  await tab('Mold Tools')
  await command('draftAnalysis')
  await panel('Draft Analysis')
  await wait(() => store(`(s.analysis?.legend ?? []).length > 0`), 'draft legend')
  const legend = await store(`s.analysis.legend.map(l => [l.label, l.count])`)
  assert.deepEqual(Object.fromEntries(legend)['Requires draft'], 4)
  await js(`document.querySelector('.panel .cancel').click()`)
  await wait(() => store(`s.analysis === null`), 'overlay cleared')

  // A parting line along the front plane's normal.
  await command('p6PartingLine')
  await panel('Parting Line')
  await pick('std_plane', { type: 'front_plane' })
  await frame()
  await type('Draft angle', '0')
  await accept('parting line')
  await wait(
    () => store(`(s.snapshot.mold?.parting_lines ?? []).length === 1`),
    'parting line recorded',
  )

  // An STL of the part imports as a mesh body, which matches it.
  const stl = join(temp, 'p6-smoke.stl')
  await call('export', { path: stl })
  await tab('Mesh')
  dialogs.open(stl)
  await command('importMesh')
  await wait(() => store(`(s.scene?.meshes ?? []).length === 1`), 'mesh body')
  await wait(
    () =>
      js(
        `[...document.querySelectorAll('.tree-item.meta')].some(e => e.textContent === 'Mesh Bodies (1)')`,
      ),
    'mesh bodies folder',
  )
  await command('deviationAnalysis')
  await panel('Deviation Analysis')
  await wait(() => store(`!!s.analysis?.mesh`), 'deviation colours')
  const within = await store(`s.analysis.legend[0].count`)
  const triangles = await store(`s.analysis.mesh.colors.length`)
  assert.equal(within, triangles, 'every vertex lies on the part')
  await js(`document.querySelector('.panel .cancel').click()`)
  await store(`s.closeDocument?.() ?? true`)
  assert.deepEqual(await js('window.__smokeErrors'), [], 'renderer errors in Phase 6 smoke')
  console.log('Phase 6 smoke passed')
}

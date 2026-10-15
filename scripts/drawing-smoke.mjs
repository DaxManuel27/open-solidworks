// Phase 4 desktop smoke: drawings through the real window, panels, canvas,
// store and core.
import assert from 'node:assert/strict'
import { existsSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

export async function drawingSmoke(context) {
  const { js } = context
  // Scenario documents change through core calls the renderer's dirty flag
  // doesn't see; accept discard prompts instead of blocking on confirm().
  await js(`window.__drawConfirm = window.confirm; window.confirm = () => true; void 0`)
  try {
    await scenario(context)
  } finally {
    await js(`window.confirm = window.__drawConfirm; void 0`)
  }
}

async function scenario({ js, call, click, wait, temp, dialogs, win, shots }) {
  console.log('Drawing smoke: make drawing, views, dimensions, annotations, tables, output')
  const store = (expr) =>
    js(`(async () => { const s = window.__openforgeStore.getState(); return ${expr} })()`)
  const drawingState = (expr) =>
    js(`(async () => { const d = window.__openforgeDrawing.getState(); return ${expr} })()`)
  const panelOpen = (title) =>
    wait(
      () => js(`document.querySelector('.panel-title')?.textContent === ${JSON.stringify(title)}`),
      `${title} panel`,
    )
  const ok = async () => {
    await wait(
      () => js(`!!document.querySelector('.panel .ok:not(:disabled)')`),
      'panel OK enabled',
    )
    await js(`document.querySelector('.panel .ok').click()`)
    await wait(() => js(`!document.querySelector('.panel')`), 'panel closed')
  }
  const tab = async (label) => {
    await js(
      `[...document.querySelectorAll('.of-tabs button')].find(b => b.textContent === ${JSON.stringify(label)}).click()`,
    )
  }
  const command = async (id) => {
    await wait(
      () => js(`!!document.querySelector('[data-command="${id}"]:not(:disabled)')`),
      `${id} enabled`,
    )
    await js(`document.querySelector('[data-command="${id}"]').click()`)
  }
  const doc = () => store(`s.drawing.drawing`)
  const rendered = async () => {
    const rev = await store('s.drawing.revision')
    await wait(
      () => drawingState(`!!d.render && d.renderedRevision >= ${rev} && !d.rendering`),
      'sheet rendered',
    )
    return drawingState('d.render')
  }
  const texts = (render, id) =>
    render.graphics.items
      .filter((i) => i.owner.type === 'annotation' && i.owner.id === id && i.prim.type === 'text')
      .map((i) => i.prim.text)
      .join(' ')
      .replace(/\s+/g, ' ')
      .trim()
  /** Hand a pick to the open panel, as the canvas would. */
  const pick = (p) =>
    js(
      `(() => { const t = window.__openforgeDrawing.getState().target; if (!t) throw new Error('no pick target'); t.onPick(${JSON.stringify(p)}); })()`,
    )
  const shot = async (name) => {
    if (!shots) return
    win.showInactive?.()
    const image = await win.capturePage()
    writeFileSync(join(shots, `${name}.png`), image.toPNG())
  }
  /** The first item of a view drawing a straight model edge matching `f`. */
  const edgeItem = (render, view, f) =>
    render.graphics.items.find(
      (i) => i.owner.type === 'view' && i.owner.view === view && i.owner.entity && f(i.prim),
    )

  // Only errors from this scenario count.
  await store(`(window.__openforgeStore.setState({ toasts: [] }), true)`)

  // A dimensioned bracket with two counterbores and a part number.
  const bracket = join(temp, 'draw-bracket.ofp')
  await call('doc.new')
  const sk = (await call('sketch.new', { plane: { type: 'front_plane' } })).id
  await call('sketch.edit', { id: sk, op: { op: 'rectangle', a: [0, 0], b: [100, 60] } })
  await call('sketch.edit', {
    id: sk,
    op: { op: 'add_circle', center: { at: [75, 30] }, radius: 6 },
  })
  const sketch = await call('sketch.get', { id: sk })
  const circle = sketch.curves.find((c) => c.kind === 'circle' || c.type === 'circle')?.id
  if (circle !== undefined)
    await call('sketch.edit', {
      id: sk,
      op: { op: 'dimension', constraint: { type: 'diameter', curve: circle, value: 12 } },
    })
  await call('sketch.close')
  await call('feature.add', {
    feature: { type: 'extrude', sketch: sk, end1: { type: 'blind', depth: 10 } },
  })
  await call('doc.set_material', { name: 'Steel 1020' }).catch(() =>
    call('doc.set_material', { name: 'Steel', density: 7850 }),
  )
  await call('doc.properties.set', {
    properties: { PartNo: { value: 'BRK-100' }, Description: { value: 'Bracket' } },
  })
  await call('doc.save', { path: bracket })
  await store(`s.run('doc.info')`)

  // Make Drawing from the part opens a drawing window of it.
  const { BrowserWindow } = await import('electron')
  const before = BrowserWindow.getAllWindows().length
  await store(`(s.makeDrawing(), true)`)
  await wait(async () => BrowserWindow.getAllWindows().length === before + 1, 'drawing window')
  const other = BrowserWindow.getAllWindows().find((w) => w !== win)
  await wait(async () => other.webContents.getURL() !== '', 'drawing window loading')
  assert.match(other.webContents.getURL(), /kind=drawing/)
  assert.match(decodeURIComponent(other.webContents.getURL()), /draw-bracket\.ofp/)
  other.destroy()

  // The same drawing here: a standard 3 view and an isometric, fitted.
  await store(`s.run('drawing.new', { model: ${JSON.stringify(bracket)} })`)
  await wait(() => store(`!!s.drawing && s.tab === 'drawing'`), 'drawing adopted')
  let render = await rendered()
  assert.equal(render.views.length, 4)
  for (const v of render.views) assert.equal(v.error, undefined, JSON.stringify(v))
  // The canvas painted the sheet and the title block fields.
  await wait(() => js(`!!document.querySelector('.sheet-canvas canvas')`), 'canvas')
  const formatText = render.graphics.items
    .filter((i) => i.owner.type === 'format' && i.prim.type === 'text')
    .map((i) => i.prim.text)
  assert.ok(formatText.includes('BRK-100'), formatText.join('|'))
  assert.ok(
    formatText.some((t) => t.endsWith('kg')),
    formatText.join('|'),
  )
  await shot('drawing-standard-views')

  // Model items: dimensions and the hole's callout in their truest views.
  await tab('Annotation')
  await command('modelItems')
  await panelOpen('Model Items')
  await ok()
  render = await rendered()
  const sheet = (await doc()).sheets[0]
  const dims = sheet.annotations.filter((a) => a.type === 'dimension' && a.dim === 'model')
  assert.ok(dims.length >= 2, JSON.stringify(sheet.annotations))
  const values = dims.map((d) => texts(render, d.id))
  assert.ok(values.includes('10'), values.join('|'))
  await shot('drawing-model-items')

  // Drive the model through the 10 mm depth: the part is edited and saved.
  const depth = dims.find((d) => texts(render, d.id) === '10')
  await js(
    `window.__openforgeStore.getState().openPanel({ type: 'annotation_properties', editing: ${depth.id} })`,
  )
  await panelOpen('Annotation')
  await js(`(() => {
    const input = document.querySelector('[aria-label="New model value"]');
    const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set;
    set.call(input, '12');
    input.dispatchEvent(new Event('input', { bubbles: true }));
  })()`)
  await click('Apply to model')
  await wait(
    () =>
      js(
        `window.__openforgeDrawing.getState().render?.graphics.items.some(i => i.owner.type === 'annotation' && i.owner.id === ${depth.id} && i.prim.type === 'text' && i.prim.text === '12')`,
      ),
    'driven dimension redrawn',
  )
  await js(`document.querySelector('.panel .cancel').click()`)
  assert.match(readFileSync(bracket, 'utf8'), /"depth": 12/)
  render = await rendered()

  // A smart dimension: pick the front view's longest horizontal edge, place.
  const front = sheet.views.find(
    (v) => v.kind.type === 'model' && v.kind.orientation.type === 'front',
  )
  const horizontal = edgeItem(
    render,
    front.id,
    (p) => p.type === 'line' && Math.abs(p.a[1] - p.b[1]) < 1e-6 && Math.abs(p.a[0] - p.b[0]) > 20,
  )
  assert.ok(horizontal, 'a horizontal edge in the front view')
  await command('smartDimension')
  await panelOpen('Smart Dimension')
  const mid = [(horizontal.prim.a[0] + horizontal.prim.b[0]) / 2, horizontal.prim.a[1]]
  await pick({
    type: 'entity',
    view: front.id,
    entity: horizontal.owner.entity,
    at: mid,
    prim: horizontal.prim,
  })
  await pick({ type: 'point', at: [mid[0], mid[1] - 25], view: null })
  await js(`document.querySelector('.panel .ok').click()`)
  await wait(() => js(`!document.querySelector('.panel')`), 'smart dimension done')
  render = await rendered()
  const smart = (await doc()).sheets[0].annotations.find(
    (a) => a.type === 'dimension' && a.dim === 'linear',
  )
  assert.equal(texts(render, smart.id), '100')

  // A note with a property link and a leader to the part.
  await command('note')
  await panelOpen('Note')
  await js(`(() => {
    const t = document.querySelector('.panel textarea');
    const set = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set;
    set.call(t, 'BREAK EDGES ' + '$PRP:"Description"');
    t.dispatchEvent(new Event('input', { bubbles: true }));
  })()`)
  await pick({ type: 'point', at: [60, 60], view: null })
  await js(`document.querySelector('.panel .ok').click()`)
  await wait(() => js(`!document.querySelector('.panel')`), 'note done')
  render = await rendered()
  const note = (await doc()).sheets[0].annotations.find((a) => a.type === 'note')
  assert.equal(texts(render, note.id), 'BREAK EDGES Bracket')

  // A section through the front view, placed to its right.
  await tab('View Layout')
  await command('sectionView')
  await panelOpen('Section View')
  const box = render.views.find((v) => v.id === front.id).bbox
  const cx = (box[0][0] + box[1][0]) / 2
  await pick({ type: 'point', at: [cx, box[1][1] + 5], view: front.id })
  await pick({ type: 'point', at: [cx, box[0][1] - 5], view: front.id })
  await click('Place View')
  await pick({ type: 'point', at: [box[1][0] + 60, (box[0][1] + box[1][1]) / 2], view: null })
  await wait(() => js(`!document.querySelector('.panel')`), 'section placed')
  render = await rendered()
  const section = (await doc()).sheets[0].views.find((v) => v.kind.type === 'section')
  assert.ok(section, 'a section view')
  assert.equal(render.views.find((v) => v.id === section.id).error, undefined)
  assert.ok(
    render.graphics.items.some(
      (i) => i.layer === 'HATCH' && i.owner.type === 'view' && i.owner.view === section.id,
    ),
    'hatching',
  )

  // A 2:1 detail of the bore.
  await command('detailView')
  await panelOpen('Detail View')
  await pick({ type: 'point', at: [box[1][0] - 10, (box[0][1] + box[1][1]) / 2], view: front.id })
  await pick({
    type: 'point',
    at: [box[1][0] - 10 + 12, (box[0][1] + box[1][1]) / 2],
    view: front.id,
  })
  await pick({ type: 'point', at: [box[0][0] + 20, box[0][1] - 35], view: null })
  await wait(() => js(`!document.querySelector('.panel')`), 'detail placed')
  render = await rendered()
  const detail = (await doc()).sheets[0].views.find((v) => v.kind.type === 'detail')
  assert.ok(detail)
  assert.equal(render.views.find((v) => v.id === detail.id).error, undefined)
  await shot('drawing-section-detail')

  // A revision table drives the title block's revision.
  await tab('Annotation')
  // Dropdown commands open their panels as the menu item would.
  const panel = (type, variant) =>
    js(`window.__openforgeStore.getState().openPanel(${JSON.stringify({ type, variant })})`)
  await panel('table', 'revision')
  await panelOpen('Revision Table')
  await pick({ type: 'point', at: [410, 287], view: null })
  await wait(() => js(`!document.querySelector('.panel')`), 'revision table placed')
  render = await rendered()
  const rev = render.graphics.items
    .filter((i) => i.owner.type === 'format' && i.prim.type === 'text')
    .map((i) => i.prim.text)
  assert.ok(rev.includes('A'), rev.join('|'))

  // Undo and redo the revision table.
  const count = (await doc()).sheets[0].annotations.length
  await store(`s.undo()`)
  await wait(async () => (await doc()).sheets[0].annotations.length === count - 1, 'undo')
  await store(`s.redo()`)
  await wait(async () => (await doc()).sheets[0].annotations.length === count, 'redo')

  // Moving a view drags its annotations with it.
  const moved = (await doc()).sheets[0].views.find((v) => v.id === front.id).origin
  await store(
    `s.run('drawing.view.update', { id: ${front.id}, origin: [${moved[0] + 5}, ${moved[1]}] })`,
  )
  render = await rendered()
  assert.equal(render.views.find((v) => v.id === front.id).origin[0], moved[0] + 5)

  // Export PDF and DXF through the panel.
  for (const format of ['pdf', 'dxf']) {
    const out = join(temp, `draw-out.${format}`)
    await tab('Evaluate')
    await command('drawingExport')
    await panelOpen('Export Drawing')
    if (format === 'dxf')
      await js(
        `(() => { const s = [...document.querySelectorAll('.panel select')][0]; s.value = 'dxf'; s.dispatchEvent(new Event('change', { bubbles: true })); })()`,
      )
    dialogs.save(out)
    await ok()
    await wait(() => existsSync(out), `${format} written`)
    const head = readFileSync(out).subarray(0, 64).toString('latin1')
    assert.ok(format === 'pdf' ? head.startsWith('%PDF') : head.includes('SECTION'), head)
  }

  // Save, reopen and keep everything.
  const file = join(temp, 'draw-bracket.ofd')
  dialogs.save(file)
  assert.equal(await store(`s.save()`), true)
  assert.ok(existsSync(file))
  const saved = await doc()
  await store(`s.run('doc.new')`)
  await wait(() => store(`!s.drawing`), 'back to a part')
  await store(`s.run('doc.open', { path: ${JSON.stringify(file)} })`)
  await wait(() => store(`!!s.drawing`), 'drawing reopened')
  const reopened = await doc()
  assert.equal(reopened.sheets[0].views.length, saved.sheets[0].views.length)
  assert.equal(reopened.sheets[0].annotations.length, saved.sheets[0].annotations.length)
  render = await rendered()
  for (const v of render.views) assert.equal(v.error, undefined, JSON.stringify(v))

  // An assembly drawing: isometric view, BOM and auto balloons.
  const pin = join(temp, 'draw-pin.ofp')
  await call('doc.new')
  const ps = (await call('sketch.new', { plane: { type: 'front_plane' } })).id
  await call('sketch.edit', { id: ps, op: { op: 'add_circle', center: { at: [0, 0] }, radius: 6 } })
  await call('sketch.close')
  await call('feature.add', {
    feature: { type: 'extrude', sketch: ps, end1: { type: 'blind', depth: 40 } },
  })
  await call('doc.properties.set', { properties: { PartNo: { value: 'PIN-12' } } })
  await call('doc.save', { path: pin })
  const assembly = join(temp, 'draw-asm.ofa')
  await call('asm.new')
  await call('asm.insert', { path: bracket })
  await call('asm.insert', {
    path: pin,
    transform: { translation: [75, 30, 10], rotation: [0, 0, 0, 1] },
  })
  await call('doc.save', { path: assembly })
  await store(`s.run('drawing.new')`)
  await wait(() => store(`!!s.drawing`), 'new drawing')
  await tab('View Layout')
  await command('modelView')
  await panelOpen('Model View')
  dialogs.open(assembly)
  await click('Browse…')
  await wait(
    () =>
      js(
        `[...document.querySelectorAll('.panel small')].some(s => s.textContent === 'draw-asm.ofa')`,
      ),
    'model chosen',
  )
  await js(
    `(() => { const s = [...document.querySelectorAll('.panel select')][0]; s.value = 'isometric'; s.dispatchEvent(new Event('change', { bubbles: true })); })()`,
  )
  await pick({ type: 'point', at: [200, 160], view: null })
  await wait(() => js(`!document.querySelector('.panel')`), 'assembly view placed')
  render = await rendered()
  const asmView = (await doc()).sheets[0].views[0]
  assert.equal(render.views[0].error, undefined)
  await tab('Annotation')
  await panel('table', 'bom')
  await panelOpen('Bill of Materials')
  await pick({ type: 'point', at: [20, 280], view: null })
  await wait(() => js(`!document.querySelector('.panel')`), 'BOM placed')
  await panel('auto_balloon')
  await panelOpen('Auto Balloon')
  // The panel takes the selected view, or asks for one.
  if (await drawingState('!!d.target'))
    await pick({ type: 'view', view: asmView.id, at: [200, 160] })
  await ok()
  render = await rendered()
  const annotations = (await doc()).sheets[0].annotations
  const bom = annotations.find((a) => a.type === 'table')
  const bomText = texts(render, bom.id)
  assert.ok(bomText.includes('BRK-100') && bomText.includes('PIN-12'), bomText)
  const balloons = annotations
    .filter((a) => a.type === 'balloon')
    .map((a) => texts(render, a.id))
    .sort()
  assert.deepEqual(balloons, ['1', '2'])
  await shot('drawing-assembly-bom')
  // No error reached the user along the way.
  const errors = await store(`s.toasts.filter((t) => t.kind === 'error').map((t) => t.text)`)
  assert.deepEqual(errors, [])
  console.log('Drawing smoke passed')
}

// Phase 3 desktop smoke: assemblies through the real panels, store and core.
import assert from 'node:assert/strict'
import { existsSync, readFileSync } from 'node:fs'
import { join } from 'node:path'

export async function assemblySmoke(context) {
  const { js } = context
  // Scenario documents change through core calls the renderer's dirty flag
  // doesn't see; accept discard prompts instead of blocking on confirm().
  await js(`window.__asmConfirm = window.confirm; window.confirm = () => true; void 0`)
  try {
    await scenario(context)
  } finally {
    await js(`window.confirm = window.__asmConfirm; void 0`)
  }
}

async function scenario({ js, call, click, wait, menu, temp, dialogs }) {
  console.log('Assembly smoke: parts, insert, mates, motion, tools')
  const store = (expr) =>
    js(`(async () => { const s = window.__openforgeStore.getState(); return ${expr} })()`)
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
  const info = () => call('doc.info')

  // Parts: a plate with a hole and a pin, built and saved through the core.
  const part = async (path, ops, depth) => {
    await call('doc.new')
    const sketch = (await call('sketch.new', { plane: { type: 'front_plane' } })).id
    for (const op of ops) await call('sketch.edit', { id: sketch, op })
    await call('sketch.close')
    await call('feature.add', {
      feature: { type: 'extrude', sketch, end1: { type: 'blind', depth } },
    })
    await call('doc.save', { path })
  }
  const plate = join(temp, 'asm-plate.ofp')
  const pin = join(temp, 'asm-pin.ofp')
  const block = join(temp, 'asm-block.ofp')
  await part(
    plate,
    [
      { op: 'rectangle', a: [0, 0], b: [100, 60] },
      { op: 'add_circle', center: { at: [30, 30] }, radius: 5 },
    ],
    10,
  )
  await part(pin, [{ op: 'add_circle', center: { at: [0, 0] }, radius: 5 }], 40)
  await part(block, [{ op: 'rectangle', a: [0, 0], b: [10, 10] }], 10)

  // A new assembly from the File menu shows the assembly tree and ribbon.
  menu('new-assembly')
  try {
    await wait(() => js(`!!document.querySelector('.assembly-tree')`), 'assembly tree')
  } catch (e) {
    console.error(
      await store(
        `JSON.stringify({ assembly: !!s.assembly, panel: s.panel, sketch: !!s.sketch, toasts: s.toasts, tab: s.tab, busy: s.busy })`,
      ),
      await js('JSON.stringify(window.__smokeErrors)'),
      await js(`document.querySelector('.sidebar')?.innerHTML.slice(0, 300)`),
    )
    throw e
  }
  await wait(() => js(`!!document.querySelector('#tab-assembly')`), 'Assembly tab')

  // Insert Components: the panel asks for the file, OK inserts it.
  const insert = async (path, count) => {
    dialogs.open(path)
    await click('Insert Components')
    await panelOpen('Insert Components')
    await ok()
    await wait(async () => (await info()).components.length === count, `${count} components`)
  }
  await insert(plate, 1)
  await insert(pin, 2)
  await insert(block, 3)
  let doc = await info()
  const [p, s, b] = doc.components.map((c) => c.key[0])
  assert.equal(doc.components[0].fixed, true, 'first component fixed')
  assert.ok(
    await js(`document.querySelector('.assembly-tree').textContent.includes('(f) asm-plate<1>')`),
  )

  // Faces by geometry, as the viewport would pick them.
  const face = async (key, test) => {
    const scene = (await info()).scene
    const inst = scene.instances.find((i) => i.key[0] === key)
    const count = scene.parts[inst.part].face_count
    for (let index = 0; index < count; index++) {
      const r = await call('asm.pick', { component: [key], kind: 'face', index })
      if (test(r.geometry)) return { component: [key], kind: 'face', index }
    }
    throw new Error(`no matching face on ${key}`)
  }
  const cylinder = (g) => g?.type === 'cylinder'
  const up = (z) => (g) =>
    g?.type === 'plane' && g.normal[2] > 0.9 && Math.abs(g.origin[2] - z) < 1e-6
  const down = (g) => g?.type === 'plane' && g.normal[2] < -0.9

  // Pick into a selection box once it is the active one.
  const activeBox = () =>
    js(`document.querySelector('.panel .refbox.active .refbox-label')?.textContent ?? ''`)
  const pickInto = async (label, pick) => {
    await wait(async () => (await activeBox()).startsWith(label), `${label} box active`)
    await wait(() => store(`!!s.asmTarget`), 'selection box listening')
    await store(`s.asmPick(${JSON.stringify(pick)}, false)`)
    await wait(
      () =>
        js(
          `[...document.querySelectorAll('.panel .refbox')].some(b => b.textContent.startsWith(${JSON.stringify(label)}) && b.querySelector('.chip'))`,
        ),
      `${label} filled`,
    )
  }

  // Mate through the Mate panel: pick both entities, pick a type, OK.
  const mate = async (type, first, second) => {
    await click('Mate')
    await panelOpen('Mate')
    await pickInto('First entity', first)
    await pickInto('Second entity', second)
    await js(`(() => {
      const button = [...document.querySelectorAll('.panel .button-grid button')].find(b => b.textContent === ${JSON.stringify(type)});
      if (!button || button.disabled) throw new Error('Mate type unavailable: ' + ${JSON.stringify(type)});
      button.click();
    })()`)
    await ok()
  }
  const plateTop = await face(p, up(10))
  await mate('Concentric', await face(p, cylinder), await face(s, cylinder))
  await mate('Coincident', plateTop, await face(s, down))
  await mate('Coincident', plateTop, await face(b, down))
  doc = await info()
  assert.equal(doc.mates.length, 3)
  assert.equal(doc.components.find((c) => c.key[0] === s).dof, 1, 'pin only turns')
  assert.equal(doc.components.find((c) => c.key[0] === b).dof, 3, 'block slides and turns')
  assert.ok(
    await js(`document.querySelector('.assembly-tree').textContent.includes('Concentric1')`),
  )

  // A mate that conflicts is refused with a message.
  await click('Mate')
  await panelOpen('Mate')
  await pickInto('First entity', await face(s, cylinder))
  await pickInto('Second entity', await face(b, down))
  await js(
    `[...document.querySelectorAll('.panel .button-grid button')].find(b => b.textContent === 'Distance').click()`,
  )
  await wait(
    async () => (await js(`document.querySelector('.panel-error')?.textContent ?? ''`)) !== '',
    'mate preview error or ok',
  )
  await js(`document.querySelector('.panel .cancel').click()`)
  await wait(() => js(`!document.querySelector('.panel')`), 'mate panel closed')

  // Move Component with collision detection: drag the block into the pin.
  await click('Move Component')
  await panelOpen('Move Component')
  await js(
    `[...document.querySelectorAll('.panel .check')].find(c => c.textContent.includes('Collision detection')).querySelector('input').click()`,
  )
  const blockAt = () => info().then((d) => d.scene.instances.find((i) => i.key[0] === b).transform)
  const before = await blockAt()
  // Put the block 20 mm left of the pin's axis, then drag it across.
  await store(
    `s.asmMove([${JSON.stringify(b)}], { mode: 'to', local: [0, 0, 0], point: [5, 25, 10] }, 'end', null)`,
  )
  for (const [phase, x] of [
    ['start', 6],
    ['move', 15],
    ['move', 30],
    ['end', 40],
  ]) {
    await store(
      `s.asmMove([${JSON.stringify(b)}], { mode: 'drag', local: [0, 0, 0], point: [${x}, 25, 10] }, ${JSON.stringify(phase)}, { dragged_only: false })`,
    )
  }
  const after = await blockAt()
  assert.notDeepEqual(after, before, 'the block moved')
  // It stopped against the pin: its right face at the pin's left side (x = 25).
  assert.ok(after[12] + 10 <= 25 + 1e-6 && after[12] + 10 > 24, `stopped at the pin: ${after[12]}`)
  assert.equal(
    (await call('asm.interference', {})).interferences.length,
    0,
    'no overlap after the drag',
  )
  await js(`document.querySelector('.panel .cancel').click()`)
  await wait(() => js(`!document.querySelector('.panel')`), 'move panel closed')

  // Linear component pattern of the pin along X.
  await store(`s.selectComponents([[${JSON.stringify(s)}]])`)
  await click('Linear Component Pattern')
  await panelOpen('Component Pattern')
  await js(`[...document.querySelectorAll('.panel .refbox')][1].click()`)
  await js(
    `[...document.querySelectorAll('.panel .refbox.active .link')].find(b => b.textContent === 'Right Plane').click()`,
  )
  await ok()
  doc = await info()
  assert.equal(doc.patterns.length, 1)
  assert.equal(doc.scene.instances.length, 5, 'plate, pin, block and two patterned pins')

  // Interference Detection through the panel.
  await click('Interference Detection')
  await panelOpen('Interference Detection')
  await js(
    `[...document.querySelectorAll('.panel button')].find(b => b.textContent === 'Calculate').click()`,
  )
  await wait(
    () => js(`document.querySelector('.panel').textContent.includes('No interference')`),
    'no interference',
  )
  await js(`document.querySelector('.panel .cancel').click()`)

  // Exploded view: lift the pin; collapse again.
  await store(`s.selectComponents([[${JSON.stringify(s)}]])`)
  await click('Exploded View')
  await panelOpen('Exploded View')
  await js(
    `[...document.querySelectorAll('.panel button')].find(b => b.textContent === 'Add step').click()`,
  )
  await wait(async () => (await info()).explode.steps.length === 1, 'explode step')
  await wait(() => store(`s.asmScene?.explode_lines.length === 1`), 'explode line shown')
  await js(
    `[...document.querySelectorAll('.panel button')].find(b => b.textContent === 'Collapse').click()`,
  )
  await wait(async () => (await info()).explode.shown === null, 'collapsed')
  await js(`document.querySelector('.panel .cancel').click()`)
  await wait(() => js(`!document.querySelector('.panel')`), 'explode panel closed')

  // Bill of materials exported as CSV.
  const csv = join(temp, 'asm-bom.csv')
  await click('Bill of Materials')
  await panelOpen('Bill of Materials')
  await wait(
    () => js(`document.querySelectorAll('.panel table.bom tbody tr').length === 3`),
    'BOM rows',
  )
  dialogs.save(csv)
  await js(
    `[...document.querySelectorAll('.panel button')].find(b => b.textContent === 'Export CSV…').click()`,
  )
  await wait(() => existsSync(csv), 'BOM CSV written')
  assert.ok(readFileSync(csv, 'utf8').includes('asm-pin'))
  await js(`document.querySelector('.panel .cancel').click()`)
  await wait(() => js(`!document.querySelector('.panel')`), 'BOM panel closed')

  // Undo and redo the pattern, save, reopen.
  menu('undo')
  await wait(async () => (await info()).explode.steps.length === 0, 'undo explode step')
  menu('redo')
  await wait(async () => (await info()).explode.steps.length === 1, 'redo explode step')
  const file = join(temp, 'smoke.ofa')
  dialogs.save(file)
  await click('Save')
  await wait(async () => !(await info()).dirty, 'assembly saved')
  assert.ok(existsSync(file))
  menu('new')
  await wait(() => js(`!document.querySelector('.assembly-tree')`), 'back to a part')
  dialogs.open(file)
  menu('open')
  await wait(() => js(`!!document.querySelector('.assembly-tree')`), 'assembly reopened')
  doc = await info()
  assert.equal(doc.mates.length, 3)
  assert.equal(doc.patterns.length, 1)
  assert.deepEqual(await js('window.__smokeErrors'), [], 'renderer errors in assembly smoke')
  menu('new')
  await wait(() => js(`!document.querySelector('.assembly-tree')`), 'part after assemblies')
  console.log('Assembly smoke passed')
}

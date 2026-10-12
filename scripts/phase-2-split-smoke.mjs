import assert from 'node:assert/strict'

/** Native split creation/editing, using geometry only to identify the middle piece. */
export async function generalSplitSmoke({ js, call, click, wait, enabled, part }) {
  const choose = (label, value) =>
    js(`(() => {
      const field = [...document.querySelectorAll('.panel label')].find(l => l.querySelector('span')?.textContent === ${JSON.stringify(label)});
      const select = field?.querySelector('select'); if (!select) throw new Error('Missing select');
      select.value = ${JSON.stringify(value)}; select.dispatchEvent(new Event('change', { bubbles: true }));
    })()`)
  const check = (section, label) =>
    js(`(() => {
      const section = [...document.querySelectorAll('.panel .section')].find(s => s.querySelector('.section-head')?.textContent.includes(${JSON.stringify(section)}));
      const field = [...(section?.querySelectorAll('label') ?? [])].find(l => l.textContent.trim() === ${JSON.stringify(label)});
      if (!field) throw new Error('Missing checkbox: ' + ${JSON.stringify(label)});
      field.querySelector('input[type=checkbox]').click();
    })()`)
  const accept = async () => {
    await wait(
      () =>
        js(
          `!!document.querySelector('.panel .ok:not(:disabled)') && !document.querySelector('.panel-error')`,
        ),
      'split panel valid',
    )
    await js(`document.querySelector('.panel .ok').click()`)
    await wait(() => js(`!document.querySelector('.panel')`), 'split panel closed')
  }
  const edit = async (name) => {
    await js(`(() => {
      const row = [...document.querySelectorAll('.tree-item')].find(r => r.querySelector('.name')?.textContent === ${JSON.stringify(name)});
      if (!row) throw new Error('Missing feature: ' + ${JSON.stringify(name)});
      row.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
    })()`)
    await wait(() => js(`!!document.querySelector('.panel')`), 'feature editor opened')
  }
  const waitPieces = () =>
    wait(
      () =>
        js(`(() => {
      const section = [...document.querySelectorAll('.panel .section')].find(s => s.querySelector('.section-head')?.textContent.includes('Retained pieces'));
      return section?.querySelectorAll('input[type=checkbox]').length === 5 && !document.querySelector('.panel-error');
    })()`),
      'three selectable split pieces',
    )
  const mass = async () => Math.round((await call('mass_properties')).volume)
  await call('doc.new')
  for (const [a, b, start, depth] of [
    [[0, 0], [10, 20], 0, 5],
    [[4, -5], [6, 25], -5, 15],
  ]) {
    const sketch = (await call('sketch.new', { plane: { type: 'front_plane' } })).id
    await call('sketch.edit', { id: sketch, op: { op: 'rectangle', a, b } })
    await call('sketch.close')
    await call('feature.add', {
      feature: {
        type: 'extrude',
        sketch,
        start_offset: start,
        end1: { type: 'blind', depth },
        merge: false,
      },
    })
  }
  const fixture = await call('doc.info')
  assert.equal(fixture.bodies.length, 2)
  const tool = fixture.bodies[1].id
  const movement = await call('feature.add', {
    feature: {
      type: 'move_body',
      bodies: [tool],
      translation: [0, 0, 0],
      copy: false,
    },
  })
  const movementName = movement.features.at(-1).name
  await call('doc.save', { path: part })
  await click('Open')
  await wait(
    () => js(`document.querySelectorAll('.tree-item[draggable=true]').length === 5`),
    'split fixture opened',
  )
  await click('Features')
  await enabled('splitBody')
  await click('Split Bodies')
  await wait(
    () => js(`document.querySelector('.panel-title')?.textContent === 'Split Bodies'`),
    'split panel',
  )
  await check('Bodies', 'Body 1')
  await choose('Split using', 'tools')
  await check('Tool bodies', 'Body 2')
  await waitPieces()
  await accept()
  const all = await call('doc.info')
  const split = all.features.find((f) => f.type === 'split_body')
  assert.equal(split.error, null)
  assert.equal(all.bodies.length, 4)
  assert.equal(await mass(), 1900, 'split conserves target and tool volumes')
  const topology = await call('doc.topology')
  const middle = all.bodies.find(
    (b) =>
      b.id.includes(':piece:') &&
      b.faces.every((i) => {
        const x = topology.faces[i].centroid[0]
        return x >= 4 - 1e-6 && x <= 6 + 1e-6
      }),
  )
  assert.ok(middle, 'middle piece identified from geometry, not piece order')
  await edit(split.name)
  await waitPieces()
  await check('Retained pieces', middle.name)
  await check('Retained pieces', 'Show all pieces for selection')
  await accept()
  const retained = await call('doc.info')
  const retainedIds = retained.features.find((f) => f.id === split.id).params.retained_pieces
  assert.equal(retainedIds.length, 2)
  assert.ok(!retainedIds.includes(middle.id))
  assert.equal(await mass(), 1700, 'only the two outer source pieces are retained')
  await click('Undo')
  await wait(async () => (await mass()) === 1900, 'piece retention undone')
  await click('Redo')
  await wait(async () => (await mass()) === 1700, 'piece retention redone')
  await edit(split.name)
  await waitPieces()
  await check('Retained pieces', 'Keep all pieces')
  await js(`document.querySelector('.panel .cancel').click()`)
  await wait(() => js(`!document.querySelector('.panel')`), 'split edit cancelled')
  assert.equal(await mass(), 1700, 'cancel preserves retained selection')
  await click('Save')
  await wait(async () => !(await call('doc.info')).dirty, 'split saved')
  const revision = (await call('doc.info')).revision
  await click('Open')
  await wait(async () => (await call('doc.info')).revision > revision, 'split reopened')
  assert.deepEqual(
    (await call('doc.info')).features.find((f) => f.id === split.id).params.retained_pieces,
    retainedIds,
  )
  await edit(movementName)
  await js(`(() => {
    const section = [...document.querySelectorAll('.panel .section')].find(s => s.querySelector('.section-head')?.textContent.includes('Translation'));
    const input = section.querySelector('input');
    input.focus();
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(input, '1mm');
    input.dispatchEvent(new Event('input', { bubbles: true }));
  })()`)
  await js('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))')
  await js(
    `document.activeElement.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))`,
  )
  await js('document.activeElement.blur()')
  await accept()
  const edited = await call('doc.info')
  assert.equal(edited.features.find((f) => f.id === split.id).error, null)
  assert.deepEqual(
    edited.features.find((f) => f.id === split.id).params.retained_pieces,
    retainedIds,
  )
  assert.ok(
    retainedIds.every((id) => edited.bodies.some((b) => b.id === id)),
    'piece IDs survive moving the tool',
  )
  assert.equal(await mass(), 1700)
  assert.deepEqual(await js('window.__smokeErrors'), [], 'renderer errors after general splitting')
}

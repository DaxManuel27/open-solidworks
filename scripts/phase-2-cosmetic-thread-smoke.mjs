import assert from 'node:assert/strict'

/** Lightweight annotations are created and repaired through real desktop controls. */
export async function phase2CosmeticThreadSmoke({ js, call, click, wait, enabled, part }) {
  const frame = () =>
    js('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))')
  const choose = async (label, value) => {
    await js(`(() => {
      const label = [...document.querySelectorAll('.panel label')].find(label => label.querySelector('span')?.textContent === ${JSON.stringify(label)});
      const input = label?.querySelector('select'); if (!input) throw new Error('Missing choice');
      input.value = ${JSON.stringify(String(value))}; input.dispatchEvent(new Event('change', { bubbles: true }));
    })()`)
    await frame()
  }
  const input = async (label, value) => {
    await js(`(() => {
      const label = [...document.querySelectorAll('.panel label')].find(label => label.querySelector('span')?.textContent === ${JSON.stringify(label)});
      const input = label?.querySelector('input'); if (!input) throw new Error('Missing field');
      input.focus(); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(input, ${JSON.stringify(String(value))});
      input.dispatchEvent(new Event('input', { bubbles: true }));
    })()`)
    await frame()
    // Enter commits even when the window lacks OS focus, which suppresses blur.
    await js(`(() => {
      const label = [...document.querySelectorAll('.panel label')].find(label => label.querySelector('span')?.textContent === ${JSON.stringify(label)});
      label.querySelector('input').dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    })()`)
    await js('document.activeElement.blur()')
    await frame()
  }
  const accept = async () => {
    await wait(
      () =>
        js(
          `!!document.querySelector('.panel .ok:not(:disabled)') && !document.querySelector('.panel-error')`,
        ),
      'cosmetic thread valid',
    )
    await js(`document.querySelector('.panel .ok').click()`)
    await wait(() => js(`!document.querySelector('.panel')`), 'cosmetic thread accepted')
  }
  await enabled('save')
  await click('Save')
  await wait(async () => !(await call('doc.info')).dirty, 'save before cosmetic thread smoke')
  await call('doc.new')
  const sketch = (await call('sketch.new', { plane: { type: 'front_plane' } })).id
  await call('sketch.edit', {
    id: sketch,
    op: { op: 'add_circle', center: { at: [0, 0] }, radius: 5 },
  })
  await call('sketch.close')
  await call('feature.add', {
    feature: { type: 'extrude', sketch, end1: { type: 'blind', depth: 20 } },
  })
  await call('doc.save', { path: part })
  await click('Open')
  await wait(
    () => js(`document.querySelectorAll('.tree-item[draggable="true"]').length === 2`),
    'cosmetic thread fixture loaded',
  )
  await click('Features')
  await enabled('cosmeticThread')
  await click('Cosmetic Thread')
  await wait(
    () =>
      js(
        `[...document.querySelectorAll('.panel option')].some(option => option.textContent.includes('Cylinder 1'))`,
      ),
    'cylinder choices loaded',
  )
  await choose('Available cylinder', 0)
  await input('Thread length', '8mm')
  await input('Pitch', '2mm')
  await input('Start offset', '1mm')
  await js(
    `(() => { const label = [...document.querySelectorAll('.panel label')].find(label => label.textContent.trim() === 'Left-handed'); label.querySelector('input').click(); })()`,
  )
  await accept()
  let snapshot = await call('doc.info')
  const annotation = snapshot.scene.cosmetic_threads[0]
  assert.equal(annotation.parameters.length, 8)
  assert.equal(annotation.parameters.pitch, 2)
  assert.equal(annotation.parameters.left_handed, true)
  assert.equal(annotation.error, null)
  assert.ok(annotation.polylines[0].length > 100)
  assert.ok(Math.abs((await call('mass_properties')).volume - 500 * Math.PI) < 1e-7)
  const feature = snapshot.features.find((feature) => feature.type === 'cosmetic_thread')
  await js(`(() => {
    const row = [...document.querySelectorAll('.tree-item')].find(row => row.querySelector('.name')?.textContent === ${JSON.stringify(feature.name)});
    row.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
  })()`)
  await wait(
    () => js(`document.querySelector('.panel-title')?.textContent === 'Cosmetic Thread'`),
    'cosmetic thread editor',
  )
  await input('Start offset', '100mm')
  await wait(
    () =>
      js(
        `!!document.querySelector('.panel-error') && !!document.querySelector('.panel .ok:disabled')`,
      ),
    'thread boundary error',
  )
  await js(`document.querySelector('.panel .cancel').click()`)
  await wait(async () => (await call('doc.info')).editing === null, 'thread cancel finished')
  snapshot = await call('doc.info')
  assert.deepEqual(snapshot.scene.cosmetic_threads[0], annotation)
  await click('Undo')
  await wait(
    async () =>
      !(await call('doc.info')).features.some((feature) => feature.type === 'cosmetic_thread'),
    'thread undo',
  )
  await click('Redo')
  await wait(
    async () =>
      (await call('doc.info')).features.some((feature) => feature.type === 'cosmetic_thread'),
    'thread redo',
  )
  await click('Save')
  await wait(async () => !(await call('doc.info')).dirty, 'thread saved')
  await click('Open')
  await wait(
    async () =>
      !(await call('doc.info')).dirty &&
      (await call('doc.info')).scene.cosmetic_threads.length === 1,
    'thread reopened',
  )
  assert.deepEqual((await call('doc.info')).scene.cosmetic_threads[0], annotation)
  assert.deepEqual(
    await js('window.__smokeErrors'),
    [],
    'renderer errors after cosmetic thread smoke',
  )
}

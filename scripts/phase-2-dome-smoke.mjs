import assert from 'node:assert/strict'

/** Real Dome controls, with a cylinder fixture supplied through the shared core. */
export async function domeSmoke({ js, call, click, wait, enabled, part }) {
  const choose = (label, value) =>
    js(`(() => {
    const field = [...document.querySelectorAll('.panel label')].find(l => l.querySelector('span')?.textContent === ${JSON.stringify(label)});
    const select = field?.querySelector('select'); if (!select) throw new Error('Missing select');
    select.value = ${JSON.stringify(String(value))}; select.dispatchEvent(new Event('change', { bubbles: true }));
  })()`)
  const height = async (value) => {
    await js(`(() => {
      const label = [...document.querySelectorAll('.panel label')].find(l => l.querySelector('span')?.textContent === 'Height');
      const input = label.querySelector('input'); input.focus();
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(input, ${JSON.stringify(String(value))});
      input.dispatchEvent(new Event('input', { bubbles: true }));
    })()`)
    await js('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))')
    await js(
      `document.activeElement.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))`,
    )
    await js('document.activeElement.blur()')
  }
  const accept = async () => {
    await wait(
      () =>
        js(
          `!!document.querySelector('.panel .ok:not(:disabled)') && !document.querySelector('.panel-error')`,
        ),
      'valid dome preview',
    )
    await js(`document.querySelector('.panel .ok').click()`)
    await wait(() => js(`!document.querySelector('.panel')`), 'dome panel closed')
  }
  const edit = async (name) => {
    await js(`(() => {
      const row = [...document.querySelectorAll('.tree-item')].find(r => r.querySelector('.name')?.textContent === ${JSON.stringify(name)});
      row.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
    })()`)
    await wait(
      () => js(`document.querySelector('.panel-title')?.textContent === 'Dome'`),
      'dome editing',
    )
  }
  const mass = async () => (await call('mass_properties')).volume
  const expected = (h, inward) =>
    800 * Math.PI + ((inward ? -1 : 1) * Math.PI * h * (300 + h * h)) / 6
  await call('doc.new')
  const sketch = (await call('sketch.new', { plane: { type: 'front_plane' } })).id
  await call('sketch.edit', {
    id: sketch,
    op: { op: 'add_circle', center: { at: [0, 0] }, radius: 10 },
  })
  await call('sketch.close')
  await call('feature.add', {
    feature: { type: 'extrude', sketch, end1: { type: 'blind', depth: 8 } },
  })
  const planar = (await call('doc.topology')).faces.filter((f) => f.surface === 'plane')
  const top = planar.findIndex((f) => f.normal[2] > 0.99)
  assert.ok(top >= 0)
  await call('doc.save', { path: part })
  await click('Open')
  await wait(
    () => js(`document.querySelectorAll('.tree-item[draggable=true]').length === 2`),
    'dome fixture opened',
  )
  await click('Features')
  await enabled('dome')
  await click('Dome')
  await wait(
    () => js(`document.querySelector('.panel-title')?.textContent === 'Dome'`),
    'dome panel',
  )
  await wait(
    () =>
      js(`(() => {
    const label = [...document.querySelectorAll('.panel label')].find(l => l.querySelector('span')?.textContent === 'Available planar face');
    return label?.querySelectorAll('option').length === 3;
  })()`),
    'dome faces loaded',
  )
  await choose('Available planar face', top)
  await height('3mm')
  await accept()
  const dome = (await call('doc.info')).features.find((f) => f.type === 'dome')
  assert.equal(dome.error, null)
  assert.ok(Math.abs((await mass()) - expected(3, false)) < 1e-5)
  await edit(dome.name)
  await choose('Direction', 'inward')
  await height('2mm')
  await accept()
  assert.ok(Math.abs((await mass()) - expected(2, true)) < 1e-5)
  await click('Undo')
  await wait(async () => Math.abs((await mass()) - expected(3, false)) < 1e-5, 'inward dome undone')
  await click('Redo')
  await wait(async () => Math.abs((await mass()) - expected(2, true)) < 1e-5, 'inward dome redone')
  await edit(dome.name)
  await height('20mm')
  await wait(
    () =>
      js(
        `document.querySelector('.panel-error')?.textContent.includes('radius') && !!document.querySelector('.panel .ok:disabled')`,
      ),
    'dome radius limit shown',
  )
  await js(`document.querySelector('.panel .cancel').click()`)
  await wait(() => js(`!document.querySelector('.panel')`), 'invalid dome edit cancelled')
  assert.ok(Math.abs((await mass()) - expected(2, true)) < 1e-5)
  await click('Save')
  await wait(async () => !(await call('doc.info')).dirty, 'dome saved')
  const revision = (await call('doc.info')).revision
  await click('Open')
  await wait(async () => (await call('doc.info')).revision > revision, 'dome reopened')
  const reopened = (await call('doc.info')).features.find((f) => f.id === dome.id)
  assert.equal(reopened.params.height, 2)
  assert.equal(reopened.params.inward, true)
  assert.equal(reopened.error, null)
  assert.ok(Math.abs((await mass()) - expected(2, true)) < 1e-5)
  assert.deepEqual(await js('window.__smokeErrors'), [], 'renderer errors after dome')
}

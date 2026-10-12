import assert from 'node:assert/strict'

/**
 * Real Rib, Thread, Boundary and Loft panels on fixtures built through the shared core,
 * including `=formula` entry in a creation field.
 */
export async function advancedFeatureSmoke({ js, call, click, wait, enabled, part }) {
  const frame = () =>
    js('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))')
  const field = (label, n = 0) =>
    `[...document.querySelectorAll('.panel label')].filter(l => l.querySelector('span')?.textContent === ${JSON.stringify(label)})[${n}]`
  const choose = async (label, value, n = 0) => {
    await js(`(() => {
      const select = ${field(label, n)}?.querySelector('select'); if (!select) throw new Error('Missing select ${label}');
      select.value = ${JSON.stringify(String(value))}; select.dispatchEvent(new Event('change', { bubbles: true }));
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
  const open = async (count, what) => {
    await call('doc.save', { path: part })
    await click('Open')
    await wait(
      () => js(`document.querySelectorAll('.tree-item[draggable=true]').length === ${count}`),
      `${what} fixture opened`,
    )
  }
  const mass = async () => (await call('mass_properties')).volume
  const sketch = async (plane, ops) => {
    const { id } = await call('sketch.new', { plane })
    for (const op of ops) await call('sketch.edit', { id, op })
    await call('sketch.close')
    return id
  }
  const front = { type: 'front_plane' }
  const rect = (a, b) => ({ op: 'rectangle', a, b })
  const line = (a, b) => ({ op: 'add_line', a: { at: a }, b: { at: b } })
  const close = (a, b, what) =>
    assert.ok(Math.abs(a - b) < 1e-5 * Math.max(1, b), `${what}: ${a} != ${b}`)

  // Rib: a shelled 40 x 20 x 10 housing with a line across the opening.
  await call('doc.new')
  const base = await sketch(front, [rect([0, 0], [40, 20])])
  await call('feature.add', {
    feature: { type: 'extrude', sketch: base, end1: { type: 'blind', depth: 10 } },
  })
  const top = (await call('doc.topology')).faces.find((f) => f.normal?.[2] > 0.99).ref
  await call('feature.add', { feature: { type: 'shell', faces: [top], thickness: 2 } })
  const rim = (
    await call('feature.add', {
      feature: { type: 'plane', mode: 'offset', reference: front, distance: 10 },
    })
  ).id
  const ribSketch = await sketch({ type: 'feature', id: rim }, [line([20, 6], [20, 14])])
  await call('equations.set', { variables: { web: '3mm' }, equations: {} })
  await open(5, 'rib')
  const housing = 8000 - 36 * 16 * 8
  await click('Features')
  await enabled('rib')
  await click('Rib')
  await panel('Rib')
  await choose('Sketch', ribSketch)
  await choose('Extrusion direction', 'normal_to_sketch')
  await type('Thickness', '=web')
  await wait(
    () => js(`${field('Thickness')}?.querySelector('small')?.textContent.startsWith('= ')`),
    'formula evaluated',
  )
  await accept('rib')
  const rib = (await call('doc.info')).features.find((f) => f.type === 'rib')
  assert.equal(rib.error, null)
  close(await mass(), housing + 3 * 16 * 8, 'formula-driven rib')
  assert.equal((await call('doc.info')).equations[`f${rib.id}.thickness`], 'web')
  await call('equations.set', {
    variables: { web: '2mm' },
    equations: { [`f${rib.id}.thickness`]: 'web' },
  })
  close(await mass(), housing + 2 * 16 * 8, 'rib follows the variable')

  // Modeled thread on a cylinder picked from the available list.
  await call('doc.new')
  const disk = await sketch(front, [{ op: 'add_circle', center: { at: [0, 0] }, radius: 5 }])
  await call('feature.add', {
    feature: { type: 'extrude', sketch: disk, end1: { type: 'blind', depth: 20 } },
  })
  await open(2, 'thread')
  const full = await mass()
  await click('Features')
  await enabled('thread')
  await click('Thread')
  await panel('Thread')
  await wait(
    () => js(`${field('Available cylinder')}?.querySelectorAll('option').length === 2`),
    'cylinders loaded',
  )
  await choose('Available cylinder', 0)
  await type('Pitch', '1.5')
  await type('Thread length', '15')
  await accept('thread')
  const thread = (await call('doc.info')).features.find((f) => f.type === 'thread')
  assert.equal(thread.error, null)
  const p = 1.5,
    h = 0.541265877365274 * p,
    a = p / 4,
    b = (7 * p) / 8
  const groove =
    ((a + b) / 2) * h * 2 * Math.PI * (5 - h + (h * (a + 2 * b)) / (3 * (a + b))) * (15 / p)
  assert.ok(Math.abs(full - (await mass()) - groove) < 1e-4 * groove, 'thread groove volume')

  // Boundary: two square sections and two straight curves through their sides.
  await call('doc.new')
  const bottom = await sketch(front, [rect([-5, -5], [5, 5])])
  const lift = (
    await call('feature.add', {
      feature: { type: 'plane', mode: 'offset', reference: front, distance: 10 },
    })
  ).id
  const upper = await sketch({ type: 'feature', id: lift }, [rect([-5, -5], [5, 5])])
  // On the right plane local x runs along world -z and local y along world y.
  const curves = []
  for (const y of [-5, 5])
    curves.push(await sketch({ type: 'right_plane' }, [line([0, y], [-10, y])]))
  await open(5, 'boundary')
  await click('Features')
  await enabled('boundary')
  await click('Boundary Boss/Base')
  await panel('Boundary')
  await choose('Add section', bottom)
  await choose('Add section', upper)
  for (const [n, curve] of curves.entries()) {
    await click('Add curve')
    await frame()
    await choose('Sketch or curve', curve, n)
  }
  await accept('boundary')
  const boundary = (await call('doc.info')).features.find((f) => f.type === 'boundary')
  assert.equal(boundary.error, null)
  close(await mass(), 1000, 'boundary prism')
  await click('Undo')
  await wait(
    async () => (await call('doc.info')).features.every((f) => f.type !== 'boundary'),
    'boundary undone',
  )
  // Loft leaving both sections along their normals: squares of half-width 5 and 3.
  await call('doc.new')
  const wide = await sketch(front, [rect([-5, -5], [5, 5])])
  const raise = (
    await call('feature.add', {
      feature: { type: 'plane', mode: 'offset', reference: front, distance: 10 },
    })
  ).id
  const narrow = await sketch({ type: 'feature', id: raise }, [rect([-3, -3], [3, 3])])
  await open(3, 'tangent loft')
  await click('Features')
  await enabled('loft')
  await click('Lofted Boss/Base')
  await panel('Loft')
  await choose('Add section', wide)
  await choose('Add section', narrow)
  await js(
    `[...document.querySelectorAll('.panel .section-head')].find(b => b.textContent.includes('Start/end constraints')).click()`,
  )
  await frame()
  await choose('Start constraint', 'normal')
  await choose('End constraint', 'normal')
  await accept('loft')
  const loft = (await call('doc.info')).features.find((f) => f.type === 'loft')
  assert.equal(loft.error, null)
  let hermite = 0
  for (let i = 0; i < 4000; i++) {
    const z = (i + 0.5) / 400
    hermite += 4 * (5 - 2 * (3 * (z / 10) ** 2 - 2 * (z / 10) ** 3)) ** 2 * (10 / 4000)
  }
  assert.ok(Math.abs((await mass()) / hermite - 1) < 1e-5, 'tangent loft volume')
  // Leave a saved document, as later scenarios start from a clean one.
  await click('Save')
  await wait(async () => !(await call('doc.info')).dirty, 'advanced features saved')
  assert.deepEqual(await js('window.__smokeErrors'), [], 'renderer errors after advanced features')
}

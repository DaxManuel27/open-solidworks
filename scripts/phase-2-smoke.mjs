import { advancedFeatureSmoke } from './phase-2-advanced-smoke.mjs'
import assert from 'node:assert/strict'
import { domeSmoke } from './phase-2-dome-smoke.mjs'
import { phase2CosmeticThreadSmoke } from './phase-2-cosmetic-thread-smoke.mjs'
import { phase2EquationSmoke } from './phase-2-equation-smoke.mjs'
import { phase2SpatialSmoke } from './phase-2-spatial-smoke.mjs'
import { copyFileSync } from 'node:fs'
import { join } from 'node:path'
import { generalSplitSmoke } from './phase-2-split-smoke.mjs'

/** Exercises actual React controls and the preload/core boundary. */
export async function phase2Smoke(context) {
  const { js } = context
  // These scenarios change documents through core calls, which the renderer's
  // dirty flag doesn't see, then reopen saved files. A real confirm() would block
  // the renderer, so accept discard prompts until they finish.
  await js(`window.__phase2Confirm = window.confirm; window.confirm = () => true; void 0`)
  try {
    await phase2Scenarios(context)
  } finally {
    await js(`window.confirm = window.__phase2Confirm; void 0`)
  }
}

async function phase2Scenarios({ js, call, click, wait, enabled, root, part, sendInput }) {
  console.log('Phase 2 smoke: bodies, curves and contours')
  const input = async (selector, value) => {
    await js(`(() => {
      const element = document.querySelector(${JSON.stringify(selector)});
      if (!element) throw new Error('Missing input: ' + ${JSON.stringify(selector)});
      element.focus();
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(element, ${JSON.stringify(String(value))});
      element.dispatchEvent(new Event('input', { bubbles: true }));
    })()`)
    await js('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))')
    await js(
      `document.querySelector(${JSON.stringify(selector)}).dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))`,
    )
    await js('document.activeElement.blur()')
    await js('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))')
  }
  const choose = (label, value) =>
    js(`(() => {
    const field = [...document.querySelectorAll('.panel label')].find(l => l.querySelector('span')?.textContent === ${JSON.stringify(label)});
    const select = field?.querySelector('select'); if (!select) throw new Error('Missing select');
    select.value = ${JSON.stringify(String(value))}; select.dispatchEvent(new Event('change', { bubbles:true }));
  })()`)
  const accept = async () => {
    await wait(
      () => js(`!!document.querySelector('.panel .ok:not(:disabled)')`),
      'phase 2 panel valid',
    )
    await js(`document.querySelector('.panel .ok').click()`)
    await wait(() => js(`!document.querySelector('.panel')`), 'phase 2 panel closed')
  }
  const mass = () => call('mass_properties')
  await enabled('save')
  await click('Save')
  await wait(async () => !(await call('doc.info')).dirty, 'save before phase 2')
  copyFileSync(join(root, 'crates/of-model/tests/fixtures/v1-multisolid.ofp'), part)
  await enabled('open')
  await click('Open')
  await wait(
    async () => (await call('doc.info')).scene.bodies.length === 2,
    'migrated multibody file',
  )
  await wait(() => js(`!!document.querySelector('input[aria-label="Show Body 1"]')`), 'body tree')
  const originalIds = (await call('doc.info')).scene.bodies.map((b) => b.id)
  await js(`document.querySelector('input[aria-label="Show Body 1"]').click()`)
  await wait(async () => (await call('doc.info')).scene.bodies[0].hidden, 'body hidden')
  assert.equal(Math.round((await mass()).volume), 2000, 'visibility does not alter mass')
  await js(`document.querySelector('input[aria-label="Show Body 1"]').click()`)
  await enabled('moveBody')
  await click('Move/Copy Bodies')
  await wait(
    () => js(`document.querySelector('.panel-title')?.textContent === 'Move/Copy Bodies'`),
    'body panel',
  )
  await js(`(() => {
    const labels=[...document.querySelectorAll('.panel label')];
    labels.find(l=>l.textContent.trim()==='Body 1').querySelector('input').click();
    labels.find(l=>l.textContent.trim()==='Create copy').querySelector('input').click();
  })()`)
  await accept()
  assert.equal((await call('doc.info')).scene.bodies.length, 3)
  await enabled('undo')
  await click('Undo')
  await wait(async () => (await call('doc.info')).scene.bodies.length === 2, 'copy undone')
  assert.deepEqual(
    (await call('doc.info')).scene.bodies.map((b) => b.id),
    originalIds,
  )

  await click('Evaluate')
  await click('Equations')
  await wait(
    () => js(`!!document.querySelector('input[aria-label="New variable name"]')`),
    'equation editor',
  )
  await input('input[aria-label="New variable name"]', 'height')
  await click('Add variable')
  await input('input[aria-label="Expression for height"]', '12mm')
  await choose('Parameter', 'f2.end1.depth')
  await click('Drive with equation')
  await input('input[aria-label="Equation for f2.end1.depth"]', 'height')
  await wait(() => js(`!document.querySelector('.panel-error')`), 'equations evaluated')
  await accept()
  assert.equal(Math.round((await mass()).volume), 3400)
  await enabled('undo')
  await click('Undo')
  await wait(async () => Math.round((await mass()).volume) === 2000, 'equations undone')
  await enabled('redo')
  await click('Redo')
  await wait(async () => Math.round((await mass()).volume) === 3400, 'equations redone')

  await js(`window.__phase2Prompt = window.prompt; window.prompt = () => 'Short'; void 0`)
  await click('New configuration')
  await js(`window.prompt = window.__phase2Prompt; void 0`)
  await wait(
    async () => (await call('doc.info')).configurations.items.length === 2,
    'new configuration',
  )
  await enabled('configurations')
  await click('Configurations')
  await wait(
    () => js(`document.querySelector('.panel-title')?.textContent === 'Configuration Overrides'`),
    'configuration editor',
  )
  await choose('Parameter', 'f2.end1.depth')
  await click('Override parameter')
  await js(
    `(() => { const label=[...document.querySelectorAll('.panel label')].find(l=>l.querySelector('span')?.textContent==='f2.end1.depth'); label.querySelector('input').setAttribute('data-smoke-depth',''); })()`,
  )
  await input('input[data-smoke-depth]', '6mm')
  await accept()
  assert.equal(
    Math.round((await mass()).volume),
    2200,
    JSON.stringify((await call('doc.info')).configurations),
  )
  const activate = async (id, expected) => {
    await js(
      `(() => { const select=document.querySelector('select[aria-label="Active configuration"]'); select.value=${JSON.stringify(String(id))}; select.dispatchEvent(new Event('change',{bubbles:true})); })()`,
    )
    await wait(async () => Math.round((await mass()).volume) === expected, 'configuration switched')
  }
  await activate(1, 3400)
  await activate(2, 2200)
  await enabled('save')
  await click('Save')
  await wait(async () => !(await call('doc.info')).dirty, 'phase 2 saved')
  const savedRevision = (await call('doc.info')).revision
  await enabled('open')
  await click('Open')
  await wait(async () => {
    const snapshot = await call('doc.info')
    return (
      snapshot.revision > savedRevision && snapshot.configurations.active === 2 && !snapshot.dirty
    )
  }, 'phase 2 reopened')
  assert.equal(Math.round((await mass()).volume), 2200)
  assert.deepEqual(
    (await call('doc.info')).scene.bodies.map((b) => b.id),
    originalIds,
  )
  assert.deepEqual(await js('window.__smokeErrors'), [], 'renderer errors after phase 2')

  // Use a saved fixture so the real Open workflow refreshes the renderer before
  // exercising the contour picker. All paths remain inside the smoke temp dir.
  await call('doc.new')
  const profile = (await call('sketch.new', { plane: { type: 'front_plane' } })).id
  await call('sketch.edit', {
    id: profile,
    op: { op: 'add_circle', center: { at: [0, 0] }, radius: 2 },
  })
  await call('sketch.edit', {
    id: profile,
    op: { op: 'add_circle', center: { at: [0, 0] }, radius: 1 },
  })
  await call('sketch.close')
  const path = (await call('sketch.new', { plane: { type: 'top_plane' } })).id
  await call('sketch.edit', {
    id: path,
    op: { op: 'add_line', a: { at: [20, 0] }, b: { at: [20, -5] } },
  })
  const selected = await call('sketch.edit', {
    id: path,
    op: { op: 'add_line', a: { at: [0, 0] }, b: { at: [0, -10] } },
  })
  const selectedContour = selected.contours.find((c) =>
    c.reference.entities.includes(selected.created[0]),
  )
  const reference = {
    ...selectedContour.reference,
    type: 'ordered_sketch_contour',
    start: selectedContour.ordered_entities[0],
    reverse: false,
  }
  const profileContours = (await call('sketch.get', { id: profile })).contours
  const profileContour = profileContours[0]
  const hole = {
    sketch: profile,
    entities: profileContours[1].reference.entities,
    start: profileContours[1].ordered_entities[0],
    reverse: false,
  }
  const selectedProfile = {
    sketch: profile,
    entities: profileContour.reference.entities,
    start: profileContour.ordered_entities[0],
    reverse: false,
  }
  await call('sketch.close')
  await call('doc.save', { path: part })
  await click('Open')
  await wait(
    () => js(`document.querySelectorAll('.tree-item[draggable="true"]').length === 2`),
    'contour fixture opened',
  )
  await click('Features')
  await enabled('sweep')
  await click('Swept Boss/Base')
  await choose('Profile sketch', String(profile))
  await wait(
    () =>
      js(
        `[...document.querySelectorAll('.panel option')].some(o => o.textContent.includes('Closed loop 1'))`,
      ),
    'profile contours loaded',
  )
  await choose('Closed profile contour', selectedProfile.entities.join(','))
  await wait(
    () =>
      js(
        `[...document.querySelectorAll('.panel label')].some(l => l.textContent.trim() === 'Inner contour 1')`,
      ),
    'profile holes available',
  )
  await js(
    `(() => { const label = [...document.querySelectorAll('.panel label')].find(l => l.textContent.trim() === 'Inner contour 1'); label.querySelector('input').click(); })()`,
  )
  await choose('Sketch or curve', String(path))
  await wait(
    () =>
      js(
        `[...document.querySelectorAll('.panel option')].some(o => o.textContent.includes('Open chain 2'))`,
      ),
    'path contours loaded',
  )
  await choose('Path contour', reference.entities.join(','))
  await accept()
  const checkContour = async () => {
    const snapshot = await call('doc.info')
    const sweep = snapshot.features.find((f) => f.type === 'sweep')
    assert.deepEqual(sweep.params.path, reference)
    assert.deepEqual(sweep.params.profile, selectedProfile)
    assert.deepEqual(sweep.params.inner_profiles, [hole])
    assert.equal(sweep.error, null)
    assert.ok(Math.abs((await mass()).volume - 30 * Math.PI) < 1e-5)
  }
  await checkContour()
  await click('Undo')
  await wait(
    async () => !(await call('doc.info')).features.some((f) => f.type === 'sweep'),
    'contour sweep undone',
  )
  await click('Redo')
  await wait(
    async () => (await call('doc.info')).features.some((f) => f.type === 'sweep'),
    'contour sweep redone',
  )
  await checkContour()
  await click('Save')
  await wait(async () => !(await call('doc.info')).dirty, 'contour sweep saved')
  const contourRevision = (await call('doc.info')).revision
  await click('Open')
  await wait(async () => {
    const snapshot = await call('doc.info')
    return !snapshot.dirty && snapshot.revision > contourRevision
  }, 'contour sweep reopened')
  await checkContour()
  assert.deepEqual(await js('window.__smokeErrors'), [], 'renderer errors after contour selection')
  console.log('Phase 2 smoke: generalSplitSmoke')
  await generalSplitSmoke({ js, call, click, wait, enabled, part })
  console.log('Phase 2 smoke: phase2EquationSmoke')
  await phase2EquationSmoke({ js, call, click, wait, enabled, root, part })
  console.log('Phase 2 smoke: phase2SpatialSmoke')
  await phase2SpatialSmoke({ js, call, click, wait, enabled, part, sendInput })
  console.log('Phase 2 smoke: domeSmoke')
  await domeSmoke({ js, call, click, wait, enabled, part })
  console.log('Phase 2 smoke: phase2CosmeticThreadSmoke')
  await phase2CosmeticThreadSmoke({ js, call, click, wait, enabled, root, part })
  console.log('Phase 2 smoke: advancedFeatureSmoke')
  await advancedFeatureSmoke({ js, call, click, wait, enabled, part })
}

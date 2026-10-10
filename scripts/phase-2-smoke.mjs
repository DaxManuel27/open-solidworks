import assert from 'node:assert/strict'
import { copyFileSync } from 'node:fs'
import { join } from 'node:path'

/** Exercises actual React controls and the preload/core boundary. */
export async function phase2Smoke({ js, call, click, wait, enabled, root, part }) {
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
}

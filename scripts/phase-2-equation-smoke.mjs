import assert from 'node:assert/strict'
import { join } from 'node:path'

/** Real feature-panel formula controls, variant scope, cancel, reset and diagnostics. */
export async function phase2EquationSmoke({ js, call, click, wait, enabled, root, part }) {
  const frame = () =>
    js('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))')
  const choose = async (label, value) => {
    await js(`(() => {
      const field = [...document.querySelectorAll('.panel label')].find(l => l.querySelector('span')?.textContent === ${JSON.stringify(label)});
      const select = field?.querySelector('select');
      if (!select) throw new Error('Missing select: ' + ${JSON.stringify(label)});
      select.value = ${JSON.stringify(value)};
      select.dispatchEvent(new Event('change', { bubbles: true }));
    })()`)
    await frame()
  }
  const input = async (selector, value) => {
    await js(`(() => {
      const input = document.querySelector(${JSON.stringify(selector)});
      if (!input) throw new Error('Missing input: ' + ${JSON.stringify(selector)});
      input.focus();
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(input, ${JSON.stringify(value)});
      input.dispatchEvent(new Event('input', { bubbles: true }));
    })()`)
    await frame()
    await js(
      `document.querySelector(${JSON.stringify(selector)}).dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))`,
    )
    await js('document.activeElement.blur()')
    await frame()
  }
  const accept = async () => {
    await wait(
      () => js(`!!document.querySelector('.panel .ok:not(:disabled)')`),
      'parameter preview valid',
    )
    await js(`document.querySelector('.panel .ok').click()`)
    await wait(() => js(`!document.querySelector('.panel')`), 'parameter edit accepted')
  }
  const cancel = async () => {
    await js(`document.querySelector('.panel .cancel').click()`)
    await wait(() => js(`!document.querySelector('.panel')`), 'parameter edit cancelled')
    await wait(async () => (await call('doc.info')).editing === null, 'feature edit ended')
  }
  const depth = async () => (await call('doc.info')).equation_values['f2.end1.depth'].value.value
  const edit = async (scope) => {
    const featureName = (await call('doc.info')).features.find((f) => f.id === 2).name
    await js(`(() => {
      const row = [...document.querySelectorAll('.tree-item[draggable="true"]')].find(row => row.querySelector('.name')?.textContent === ${JSON.stringify(featureName)});
      if (!row) throw new Error('Missing extrusion in feature tree');
      row.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
    })()`)
    await wait(() => js(`!!document.querySelector('.panel')`), 'extrusion editing panel')
    await choose('Edit feature', 'parameters')
    await choose('Edit scope', scope)
    await wait(
      () => js(`!!document.querySelector('input[aria-label="Equation for f2.end1.depth"]')`),
      'feature formula field',
    )
    await wait(
      () => js(`!!document.querySelector('.panel .ok:not(:disabled)')`),
      'initial parameter evaluation',
    )
  }
  const activate = async (id, expected) => {
    await js(`(() => {
      const select = document.querySelector('select[aria-label="Active configuration"]');
      select.value = ${JSON.stringify(String(id))};
      select.dispatchEvent(new Event('change', { bubbles: true }));
    })()`)
    await wait(
      async () =>
        (await call('doc.info')).configurations.active === id && (await depth()) === expected,
      'parameter configuration switch',
    )
  }

  await enabled('save')
  await click('Save')
  await wait(async () => !(await call('doc.info')).dirty, 'save before parameter editor smoke')
  await call('doc.open', { path: join(root, 'crates/of-model/tests/fixtures/v1-block.ofp') })
  await call('equations.set', {
    variables: { height: '8mm' },
    equations: { 'f2.end1.depth': 'height' },
  })
  await call('configuration.create', { name: 'Parameter variant' })
  await call('doc.save', { path: part })
  await click('Open')
  await wait(
    () => js(`document.querySelector('select[aria-label="Active configuration"]')?.value === '2'`),
    'parameter fixture opened',
  )
  const original = await call('doc.info')
  await edit('active')
  await input('input[aria-label="Equation for f2.end1.depth"]', 'height * 2')
  await wait(
    () =>
      js(
        `document.querySelector('[aria-label="Evaluated f2.end1.depth"]')?.textContent?.includes('16')`,
      ),
    'variant formula preview evaluated',
  )
  assert.equal(await depth(), 8, 'preview leaves committed geometry unchanged')
  await cancel()
  let snapshot = await call('doc.info')
  assert.deepEqual(snapshot.configurations, original.configurations)
  assert.deepEqual(snapshot.equations, original.equations)

  await edit('active')
  await input('input[aria-label="Equation for f2.end1.depth"]', 'missing_variable')
  await wait(
    () =>
      js(
        `!!document.querySelector('.panel-error') && !!document.querySelector('.panel .ok:disabled')`,
      ),
    'invalid formula blocks commit',
  )
  await input('input[aria-label="Equation for f2.end1.depth"]', '12mm')
  await accept()
  assert.equal(await depth(), 12)
  snapshot = await call('doc.info')
  assert.equal(snapshot.variables.height, '8mm')
  assert.equal(snapshot.equations['f2.end1.depth'], 'height')
  assert.equal(snapshot.features.find((f) => f.id === 2).params.end1.depth, 5)
  await activate(1, 8)
  await activate(2, 12)
  await activate(1, 8)
  await activate(2, 12)

  await edit('active')
  await js(`(() => {
    const button = [...document.querySelectorAll('.panel button')].find(button => button.textContent.trim() === 'Reset to base' && !button.disabled);
    if (!button) throw new Error('Missing enabled reset-to-base button');
    button.click();
  })()`)
  await accept()
  assert.equal(await depth(), 8)
  assert.deepEqual((await call('doc.info')).configurations.items[1].equations, {})

  await edit('base')
  await input('input[aria-label="Equation for f2.end1.depth"]', 'height + 1mm')
  await accept()
  assert.equal(await depth(), 9)
  assert.equal((await call('doc.info')).equations['f2.end1.depth'], 'height + 1mm')

  await edit('active')
  await js(`(() => {
    const label = [...document.querySelectorAll('.panel label')].find(label => label.textContent.trim() === 'f2.end1.depth — formula');
    label.querySelector('input[type="checkbox"]').click();
  })()`)
  await frame()
  await js(`(() => {
    const label = [...document.querySelectorAll('.panel label')].find(label => label.querySelector('span')?.textContent === 'f2.end1.depth');
    label.querySelector('input').setAttribute('data-equation-depth', '');
  })()`)
  await input('input[data-equation-depth]', '7mm')
  await accept()
  snapshot = await call('doc.info')
  assert.equal(await depth(), 7)
  assert.equal(snapshot.equations['f2.end1.depth'], 'height + 1mm')
  assert.equal(snapshot.configurations.items[1].parameters['f2.end1.depth'], 7)
  assert.deepEqual(await js('window.__smokeErrors'), [], 'renderer errors after scoped parameters')
  await click('Save')
  await wait(async () => !(await call('doc.info')).dirty, 'scoped parameter smoke saved')
}

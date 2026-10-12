import assert from 'node:assert/strict'

export async function phase2SpatialSmoke({ js, call, click, wait, enabled, part, sendInput }) {
  const frame = () =>
    js('new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))')
  const choose = async (label, value) => {
    await js(
      `(() => { const field=[...document.querySelectorAll('.panel label')].find(l=>l.querySelector('span')?.textContent===${JSON.stringify(label)}); const s=field?.querySelector('select'); if(!s) throw new Error('Missing select '+${JSON.stringify(label)}); s.value=${JSON.stringify(value)};s.dispatchEvent(new Event('change',{bubbles:true})); })()`,
    )
    await frame()
  }
  const input = async (label, value) => {
    await js(
      `(() => { const field=[...document.querySelectorAll('.panel label')].find(l=>l.querySelector('span')?.textContent===${JSON.stringify(label)}); const el=field?.querySelector('input'); if(!el) throw new Error('Missing field '+${JSON.stringify(label)}); el.setAttribute('data-spatial-input',''); el.focus(); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set.call(el,${JSON.stringify(String(value))});el.dispatchEvent(new Event('input',{bubbles:true})); })()`,
    )
    await frame()
    await js(
      `document.querySelector('[data-spatial-input]').dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true}));document.activeElement.blur();document.querySelector('[data-spatial-input]').removeAttribute('data-spatial-input')`,
    )
    await frame()
    await wait(() => js(`!document.querySelector('.panel-error')`), 'spatial solve after input')
  }
  const accept = async () => {
    await wait(
      () =>
        js(
          `!!document.querySelector('.panel .ok:not(:disabled)') && !document.querySelector('.panel-error')`,
        ),
      'spatial panel ready',
    )
    await js(`document.querySelector('.panel .ok').click()`)
    await wait(() => js(`!document.querySelector('.panel')`), 'spatial panel accepted')
  }
  const select = async (label, shift = false) => {
    await js(
      `(() => { const b=[...document.querySelectorAll('.panel button')].find(b=>b.textContent.trim()===${JSON.stringify(label)});if(!b)throw new Error('Missing spatial entity');b.dispatchEvent(new MouseEvent('click',{bubbles:true,shiftKey:${shift}})); })()`,
    )
    await frame()
  }
  await call('doc.new')
  await call('doc.save', { path: part })
  await click('Open')
  await wait(
    () => js(`document.querySelectorAll('.tree-item[draggable="true"]').length===0`),
    'empty spatial fixture',
  )
  await click('Sketch')
  await enabled('sketch3d')
  await click('3D Sketch')
  await choose('Active workplane', 'xz')
  await choose('Spatial tool', 'line')
  const box = await js(
    `(() => {const r=document.querySelector('.viewport canvas').getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height}})()`,
  )
  const pointer = async (dx, dy) => {
    const x = Math.round(box.x + box.width / 2 + dx),
      y = Math.round(box.y + box.height / 2 + dy)
    sendInput({ type: 'mouseMove', x, y })
    sendInput({ type: 'mouseDown', x, y, button: 'left', clickCount: 1 })
    sendInput({ type: 'mouseUp', x, y, button: 'left', clickCount: 1 })
    await frame()
  }
  await pointer(0, 0)
  await wait(
    () =>
      js(
        `[...document.querySelectorAll('.panel button')].some(b=>b.textContent.trim()==='point 1')`,
      ),
    'first spatial viewport point',
  )
  await pointer(50, -70)
  await wait(
    () =>
      js(
        `[...document.querySelectorAll('.panel button')].some(b=>b.textContent.trim()==='line 3')`,
      ),
    'spatial viewport line',
  )
  await choose('Spatial tool', 'select')
  await select('point 1')
  for (const axis of ['X', 'Y', 'Z']) await input(`${axis} · point 1`, 0)
  await click('Fix point')
  await select('point 2')
  await input('Z · point 2', 20)
  await input('X · point 2', 0)
  await input('Y · point 2', 0)
  await select('line 3')
  await click('Along Z')
  await select('point 1')
  await select('point 2', true)
  await click('Distance dimension')
  await wait(
    () =>
      js(
        `document.querySelector('.panel [role="status"]')?.textContent.includes('0 degrees of freedom')`,
      ),
    'spatial fully constrained',
  )
  await input('Distance 6', 25)
  await accept()
  const spatial = (await call('doc.info')).features.find((f) => f.type === 'sketch3d')
  assert.equal(spatial.error, null)
  assert.equal(spatial.sketch_status, 'Fully defined')
  assert.ok(Math.abs(spatial.params.entities.find((e) => e.id === 2).at[2] - 25) < 1e-6)
  const profile = (await call('sketch.new', { plane: { type: 'front_plane' } })).id
  await call('sketch.edit', {
    id: profile,
    op: { op: 'add_circle', center: { at: [0, 0] }, radius: 2 },
  })
  await call('sketch.close')
  await call('doc.save', { path: part })
  await click('Open')
  await wait(
    () => js(`document.querySelectorAll('.tree-item[draggable="true"]').length===2`),
    'spatial sweep fixture refreshed',
  )
  await click('Features')
  await enabled('sweep')
  await click('Swept Boss/Base')
  await choose('Profile sketch', String(profile))
  await choose('Sketch or curve', String(spatial.id))
  await accept()
  const volume = async () => (await call('mass_properties')).volume
  assert.ok(Math.abs((await volume()) - 100 * Math.PI) < 1e-4)
  const editSpatial = async () => {
    await js(
      `(() => {const row=[...document.querySelectorAll('.tree-item[draggable="true"]')].find(r=>r.querySelector('.name')?.textContent===${JSON.stringify(spatial.name)});row.dispatchEvent(new MouseEvent('dblclick',{bubbles:true}))})()`,
    )
    await wait(
      () => js(`document.querySelector('.panel-title')?.textContent==='3D Sketch'`),
      'spatial edit panel',
    )
  }
  await editSpatial()
  await input('Distance 6', 30)
  await accept()
  assert.ok(Math.abs((await volume()) - 120 * Math.PI) < 1e-4)
  await click('Undo')
  await wait(
    async () => Math.abs((await volume()) - 100 * Math.PI) < 1e-4,
    'spatial dimension undo',
  )
  await click('Redo')
  await wait(
    async () => Math.abs((await volume()) - 120 * Math.PI) < 1e-4,
    'spatial dimension redo',
  )
  await editSpatial()
  await input('Distance 6', 35)
  await js(`document.querySelector('.panel .cancel').click()`)
  await wait(() => js(`!document.querySelector('.panel')`), 'spatial edit cancelled')
  assert.ok(Math.abs((await volume()) - 120 * Math.PI) < 1e-4)
  await click('Save')
  await wait(async () => !(await call('doc.info')).dirty, 'spatial saved')
  const revision = (await call('doc.info')).revision
  await click('Open')
  await wait(async () => (await call('doc.info')).revision > revision, 'spatial reopened')
  assert.ok(Math.abs((await volume()) - 120 * Math.PI) < 1e-4)
  assert.deepEqual(await js('window.__smokeErrors'), [], 'spatial renderer errors')
}

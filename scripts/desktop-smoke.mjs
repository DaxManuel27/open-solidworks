// Run with Electron after cargo build and pnpm build. Uses an isolated profile.
import { app, dialog, nativeTheme } from 'electron'
import { mkdtempSync, existsSync, statSync, writeFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import assert from 'node:assert/strict'
import { assistantSmoke } from './assistant-smoke.mjs'
import { phase2Smoke } from './phase-2-smoke.mjs'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
process.env.OPENFORGE_CORE = join(
  root,
  'target/debug',
  process.platform === 'win32' ? 'openforge.exe' : 'openforge',
)
const temp = mkdtempSync(join(tmpdir(), 'openforge-smoke-'))
app.setPath('userData', join(temp, 'profile'))
app.setAppPath(join(root, 'apps/desktop'))
app.commandLine.appendSwitch('enable-unsafe-swiftshader')
const part = join(temp, 'smoke.ofp')
let saveDialogs = 0
let openDialogs = 0
let releaseOpenDialog = null
let holdOpenDialog = false
let exportExtension = 'step'
dialog.showSaveDialog = async (_win, options) => {
  saveDialogs++
  return {
    canceled: false,
    filePath: options.filters[0].extensions.includes('ofp')
      ? part
      : join(temp, `smoke.${exportExtension}`),
  }
}
dialog.showOpenDialog = async (_win, options) => {
  openDialogs++
  if (holdOpenDialog)
    await new Promise((resolve) => {
      releaseOpenDialog = resolve
    })
  return {
    canceled: false,
    filePaths: [options.filters[0].extensions.includes('ofp') ? part : join(temp, 'smoke.step')],
  }
}
dialog.showMessageBoxSync = () => 1
dialog.showErrorBox = (title, message) => {
  throw new Error(`${title}: ${message}`)
}
const timeout = setTimeout(() => {
  console.error('Desktop smoke timed out')
  app.exit(1)
}, 360000)

async function run(win) {
  // A renderer script that never settles fails with its source instead of
  // stalling until the whole smoke times out.
  const js = (code) => {
    let timer
    return Promise.race([
      win.webContents.executeJavaScript(code, true).catch((error) => {
        throw new Error(`${error.message}\nRenderer script: ${code}`)
      }),
      new Promise((_, reject) => {
        timer = setTimeout(
          () => reject(new Error(`Renderer script did not settle in 20 s: ${code}`)),
          20000,
        )
      }),
    ]).finally(() => clearTimeout(timer))
  }
  const wait = async (fn, label) => {
    const deadline = Date.now() + 15000
    while (Date.now() < deadline) {
      if (await fn()) return
      await new Promise((r) => setTimeout(r, 50))
    }
    throw new Error(`Timed out: ${label}`)
  }
  const call = (method, params = {}) =>
    js(`(async () => {
    const r = await window.openforge.call(${JSON.stringify(method)}, ${JSON.stringify(params)});
    if (!r.ok) throw new Error(r.error); return r.value;
  })()`)
  const click = (label) =>
    js(`(() => {
    const b = [...document.querySelectorAll('button')].find(b => (b.getAttribute('aria-label') ?? b.querySelector('.command-label')?.textContent ?? b.textContent).trim() === ${JSON.stringify(label)});
    if (!b || b.disabled) throw new Error('Button unavailable: ' + ${JSON.stringify(label)});
    b.click();
  })()`)
  const menu = (command) => win.webContents.send('menu', command)
  const key = (value) =>
    js(
      `document.activeElement.dispatchEvent(new KeyboardEvent('keydown', { key: ${JSON.stringify(value)}, bubbles: true, cancelable: true }))`,
    )
  const enabled = (id) =>
    wait(
      () => js(`!!document.querySelector('[data-command="${id}"]:not(:disabled)')`),
      `${id} enabled`,
    )
  const selectMenu = async (label, item) => {
    await click(label)
    await click(item)
  }
  const captureLayouts = async (name) => {
    for (const theme of ['light', 'dark']) {
      nativeTheme.themeSource = theme
      for (const [width, height] of [
        [1440, 900],
        [1024, 612],
        [900, 600],
      ]) {
        win.setContentSize(width, height)
        await js(`document.querySelector('.of-ribbon-track').scrollLeft = 0`)
        await new Promise((r) => setTimeout(r, 180))
        await wait(
          () =>
            js(
              `(() => { const el=document.querySelector('.of-ribbon-track'); return (el.scrollWidth > el.clientWidth + 1) === !!document.querySelector('[aria-label="Scroll commands right"]'); })()`,
            ),
          'ribbon resize observer settled',
        )
        const layout = await js(`(() => {
          const rect = s => document.querySelector(s).getBoundingClientRect();
          const standard = rect('.of-standard'), ribbon = rect('.of-ribbon'), tabs = rect('.of-tabs');
          const hud = rect('.of-heads-up'), viewport = rect('.viewport');
          return { heights: [standard.height, ribbon.height, tabs.height, hud.height],
            centered: Math.abs(hud.x + hud.width / 2 - viewport.x - viewport.width / 2) < 1,
            below: tabs.top === ribbon.bottom, fits: hud.left >= viewport.left && hud.right <= viewport.right,
            bodyFits: document.body.scrollWidth === window.innerWidth,
            overflow: document.querySelector('.of-ribbon-track').scrollWidth > document.querySelector('.of-ribbon-track').clientWidth + 1,
            arrows: !!document.querySelector('[aria-label="Scroll commands right"]') };
        })()`)
        assert.deepEqual(layout.heights, [32, 76, 24, 28])
        assert.ok(
          layout.centered && layout.below && layout.fits && layout.bodyFits,
          JSON.stringify(layout),
        )
        assert.equal(layout.overflow, layout.arrows)
        if (layout.overflow) {
          await click('Scroll commands right')
          await wait(
            () =>
              js(
                `(() => { const el = document.querySelector('.of-ribbon-track'); return el.scrollLeft >= Math.min(260, el.scrollWidth - el.clientWidth) - 1 })()`,
              ),
            'ribbon scroll',
          )
        }
        await click('Reference Geometry')
        await wait(() => js(`!!document.querySelector('.of-menu')`), 'layout menu opened')
        assert.equal(
          await js(
            `(() => { const r = document.querySelector('.of-menu').getBoundingClientRect(); return r.left >= 0 && r.right <= innerWidth && r.top >= 0 && r.bottom <= innerHeight })()`,
          ),
          true,
          'popup fits viewport',
        )
        if (process.env.OPENFORGE_SMOKE_SCREENSHOT && width === 900)
          writeFileSync(
            process.env.OPENFORGE_SMOKE_SCREENSHOT.replace(/\.png$/, '') +
              `-${name}-${theme}-menu.png`,
            (await win.capturePage()).toPNG(),
          )
        await key('Escape')
        await js(
          `document.activeElement.blur(); document.querySelector('.of-ribbon-track').scrollLeft = 0`,
        )
        await new Promise((r) => setTimeout(r, 60))
        const artifact = process.env.OPENFORGE_SMOKE_SCREENSHOT
        if (artifact)
          writeFileSync(
            artifact.replace(/\.png$/, '') + `-${name}-${theme}-${width}x${height}.png`,
            (await win.capturePage()).toPNG(),
          )
        if (width === 900) {
          for (const tab of ['Sketch', 'Evaluate']) {
            await click(tab)
            await new Promise((r) => setTimeout(r, 60))
            assert.equal(
              await js(`document.querySelector('.of-ribbon').getBoundingClientRect().height`),
              76,
            )
            if (artifact)
              writeFileSync(
                artifact.replace(/\.png$/, '') + `-${name}-${theme}-${tab.toLowerCase()}.png`,
                (await win.capturePage()).toPNG(),
              )
          }
          await click('Features')
        }
      }
    }
    nativeTheme.themeSource = 'light'
    win.setContentSize(1440, 900)
    await new Promise((r) => setTimeout(r, 180))
  }
  await wait(
    () => js(`!!window.openforge && !!document.querySelector('.tree-item')`),
    'renderer ready',
  )
  await js(
    `window.__smokeErrors = []; window.addEventListener('error', e => window.__smokeErrors.push(e.message)); window.addEventListener('unhandledrejection', e => window.__smokeErrors.push(String(e.reason)))`,
  )
  if (process.env.OPENFORGE_SMOKE_ASSISTANT_ONLY) {
    await assistantSmoke({ js, call, click, wait })
    return
  }
  assert.equal(await js(`typeof window.require`), 'undefined', 'renderer must stay sandboxed')
  assert.equal(await js(`document.querySelector('[data-command="print"]').disabled`), true)
  assert.match(
    await js(`document.querySelector('[data-command="print"]').title`),
    /Not implemented/,
  )
  assert.match(
    await js(`document.querySelector('[data-command="cut"]').title`),
    /Requires a solid body/,
  )
  assert.match(
    await js(`document.querySelector('[data-command="extrude"]').title`),
    /Requires a sketch/,
  )
  assert.match(await js(`document.querySelector('[data-command="undo"]').title`), /Nothing to undo/)
  assert.deepEqual(
    await js(
      `[...document.querySelectorAll('[role="tab"]')].map(b => [b.textContent, b.disabled])`,
    ),
    [
      ['Features', false],
      ['Sketch', false],
      ['Surfaces', true],
      ['Sheet Metal', true],
      ['Direct Editing', true],
      ['Evaluate', false],
      ['Annotations', true],
      ['Add-ins', true],
    ],
  )
  await js(`document.querySelector('#tab-features').focus()`)
  await key('ArrowRight')
  assert.equal(await js(`document.activeElement.id`), 'tab-sketch')
  await key('ArrowRight')
  assert.equal(await js(`document.activeElement.id`), 'tab-evaluate')
  await key('Home')
  await js(`document.querySelector('[data-command="new"]').focus()`)
  await key('ArrowRight')
  assert.equal(await js(`document.activeElement.getAttribute('data-command')`), 'open')
  await key('End')
  assert.equal(await js(`document.activeElement.getAttribute('data-command')`), 'select')
  for (const id of [
    'print',
    'rebuild',
    'properties',
    'options',
    'direct',
    'zoomArea',
    'previousView',
    'section',
    'display',
    'visibility',
    'appearance',
    'scene',
    'viewSettings',
  ]) {
    assert.equal(await js(`document.querySelector('[data-command="${id}"]').disabled`), true, id)
    assert.match(
      await js(`document.querySelector('[data-command="${id}"]').title`),
      /Not implemented/,
      id,
    )
  }
  await click('Pattern')
  await key('Escape')
  assert.equal(await js(`!!document.querySelector('.of-menu')`), false, 'disabled menu dismisses')
  await click('Orientation')
  await key('Tab')
  assert.equal(await js(`!!document.querySelector('.of-menu')`), false, 'Tab dismisses')
  await captureLayouts('empty')
  await click('New')
  await enabled('new')
  await click('Sketch')
  await click('Sketch')
  await click('Select')
  // A cancelled pick request must not start a sketch when a plane is clicked.
  await js(
    `document.querySelector('.tree-item[title="Double-click to sketch on this plane"]').click()`,
  )
  assert.equal((await call('doc.info')).features.length, 0)
  await js(
    `document.querySelector('.tree-item[title="Double-click to sketch on this plane"]').dispatchEvent(new MouseEvent('dblclick', { bubbles: true }))`,
  )
  await wait(() => js(`document.body.textContent.includes('Exit Sketch')`), 'sketch mode')
  // Camera orientation animates for 260ms; draw only after it reaches the sketch plane.
  await new Promise((r) => setTimeout(r, 320))
  for (const label of [
    'Line',
    'Centerline',
    'Circle',
    'Slot',
    'Polygon',
    'Spline',
    'Point',
    'Smart Dimension',
    'Trim',
    'Extend',
    'Sketch Fillet',
    'Convert Entities',
  ]) {
    await click(label)
    await wait(
      () =>
        js(`!!document.querySelector('.of-ribbon [aria-pressed="true"][aria-label="${label}"]')`),
      `${label} active`,
    )
  }
  await selectMenu('Corner Rectangle menu', 'Center Rectangle')
  await selectMenu('Centerpoint Arc menu', '3 Point Arc')
  await selectMenu('Centerpoint Arc menu', 'Tangent Arc')
  await click('Line')
  await click('Centerpoint Arc menu')
  await key('End')
  assert.equal(await js(`document.activeElement.textContent.trim()`), 'Tangent Arc')
  await key('Home')
  await key('ArrowDown')
  assert.match(await js(`document.activeElement.textContent`), /3 Point Arc/)
  await key('s')
  await key('Escape')
  assert.equal(await js(`!!document.querySelector('.of-menu')`), false)
  assert.equal(
    await js(`document.activeElement.getAttribute('aria-label')`),
    'Centerpoint Arc menu',
  )
  assert.equal(
    await js(`document.querySelector('[data-command="line"]').getAttribute('aria-pressed')`),
    'true',
    'menu keys never reach sketch shortcuts',
  )
  await click('Orientation')
  await js(
    `document.querySelector('.of-standard').dispatchEvent(new PointerEvent('pointerdown', { bubbles: true }))`,
  )
  assert.equal(await js(`!!document.querySelector('.of-menu')`), false, 'outside dismissal')
  await js(`document.querySelector('.viewport canvas').focus()`)
  await key('r')
  assert.equal(
    await js(`document.querySelector('[data-command="rectangle"]').getAttribute('aria-pressed')`),
    'true',
    'sketch shortcut retained',
  )
  await js(
    `window.__canvasEvents = 0; for (const event of ['pointerdown', 'pointerup', 'wheel', 'dblclick']) document.querySelector('.viewport canvas').addEventListener(event, () => window.__canvasEvents++)`,
  )
  const hud = await js(
    `(() => { const r = document.querySelector('[data-command="normal"]').getBoundingClientRect(); return { x: Math.round(r.x + r.width / 2), y: Math.round(r.y + r.height / 2) } })()`,
  )
  win.webContents.sendInputEvent({ type: 'mouseDown', ...hud, button: 'left', clickCount: 1 })
  win.webContents.sendInputEvent({ type: 'mouseMove', x: hud.x + 2, y: hud.y })
  win.webContents.sendInputEvent({ type: 'mouseUp', ...hud, button: 'left', clickCount: 1 })
  win.webContents.sendInputEvent({ type: 'mouseWheel', ...hud, deltaY: 30, deltaX: 0 })
  await new Promise((r) => setTimeout(r, 100))
  assert.equal(await js('window.__canvasEvents'), 0, 'floating toolbar isolates canvas events')
  const emptySketch = (await call('doc.info')).features.find((f) => f.type === 'sketch').id
  assert.equal(
    (await call('sketch.get', { id: emptySketch })).curves.length,
    0,
    'toolbar cannot draw geometry',
  )
  await click('Select')
  assert.equal(
    await js(
      `document.querySelector('[data-command="sketchSelect"]').getAttribute('aria-pressed')`,
    ),
    'true',
  )
  await click('Corner Rectangle')
  const box = await js(
    `(() => { const r = document.querySelector('.viewport canvas').getBoundingClientRect(); return { x:r.x, y:r.y, width:r.width, height:r.height } })()`,
  )
  const pointer = async (dx, dy) => {
    const x = Math.round(box.x + box.width / 2 + dx)
    const y = Math.round(box.y + box.height / 2 + dy)
    win.webContents.sendInputEvent({ type: 'mouseMove', x, y })
    win.webContents.sendInputEvent({ type: 'mouseDown', x, y, button: 'left', clickCount: 1 })
    win.webContents.sendInputEvent({ type: 'mouseUp', x, y, button: 'left', clickCount: 1 })
    await new Promise((r) => setTimeout(r, 100))
  }
  await pointer(-100, -60)
  await pointer(100, 60)
  const sketch = (await call('doc.info')).features.find((f) => f.type === 'sketch').id
  await wait(
    async () => (await call('sketch.get', { id: sketch })).curves.length === 4,
    'rectangle drawn',
  )
  await click('Features')
  await click('Extruded Boss/Base')
  await wait(
    () =>
      js(
        `!!document.querySelector('.panel .ok:not(:disabled)') && !document.querySelector('.panel-error')`,
      ),
    'extrude preview',
  )
  await js(`document.querySelector('.panel .ok').click()`)
  await wait(
    async () => (await call('doc.info')).features.some((f) => f.type === 'extrude'),
    'extrude committed',
  )
  const mass = await call('mass_properties')
  assert.ok(mass.volume > 1, 'solid volume')
  await js(
    `window.__confirm = window.confirm; window.__confirmCount = 0; window.confirm = () => { window.__confirmCount++; return false }; void 0`,
  )
  await enabled('new')
  await click('New')
  await enabled('open')
  await click('Open')
  assert.equal(await js('window.__confirmCount'), 2, 'unsaved-change prompts retained')
  assert.ok(
    (await call('doc.info')).features.some((f) => f.type === 'extrude'),
    'cancel preserves document',
  )
  await js(`window.confirm = window.__confirm; void 0`)
  await enabled('undo')
  await click('Undo')
  await wait(
    async () => !(await call('doc.info')).features.some((f) => f.type === 'extrude'),
    'undo',
  )
  await enabled('redo')
  await click('Redo')
  await wait(
    async () => (await call('doc.info')).features.some((f) => f.type === 'extrude'),
    'redo',
  )
  await enabled('save')
  await click('Save')
  await wait(() => existsSync(part), 'save')
  await enabled('save')
  const savesBefore = saveDialogs
  await selectMenu('Save menu', 'Save As')
  await wait(() => saveDialogs > savesBefore, 'Save As dialog')
  await enabled('save')
  await enabled('new')
  await click('New')
  await wait(async () => (await call('doc.info')).features.length === 0, 'new document')
  await enabled('open')
  holdOpenDialog = true
  const dialogCount = openDialogs
  await click('Open')
  await wait(() => releaseOpenDialog !== null, 'open dialog pending')
  assert.match(
    await js(`document.querySelector('[data-command="new"]').title`),
    /operation is in progress/,
  )
  assert.equal(await js(`document.querySelector('[data-command="save"]').disabled`), true)
  await js(`document.querySelector('[data-command="open"]').click()`)
  assert.equal(openDialogs, dialogCount + 1, 'duplicate dialog blocked')
  holdOpenDialog = false
  releaseOpenDialog()
  releaseOpenDialog = null
  await wait(
    async () => (await call('doc.info')).features.some((f) => f.type === 'extrude'),
    'reopen',
  )
  assert.ok(Math.abs((await call('mass_properties')).volume - mass.volume) < 1e-6)
  for (const extension of ['step', 'iges', 'stl']) {
    exportExtension = extension
    await selectMenu('Save menu', 'Export')
    await wait(() => js(`!!document.querySelector('.panel select')`), 'export panel')
    await js(
      `(() => { const select = document.querySelector('.panel select'); select.value = '${extension}'; select.dispatchEvent(new Event('change', { bubbles:true })); })()`,
    )
    await new Promise((r) => setTimeout(r, 50))
    await js(`document.querySelector('.panel .ok').click()`)
    const path = join(temp, `smoke.${extension}`)
    await wait(() => existsSync(path) && statSync(path).size > 100, `${extension} export`)
    await wait(() => js(`!document.querySelector('.panel')`), 'export closed')
  }
  await wait(() => js(`document.querySelectorAll('.tree-item').length >= 6`), 'feature tree')
  assert.deepEqual(await js('window.__smokeErrors'), [], 'renderer errors')
  const closePanel = async () => {
    await wait(() => js(`!!document.querySelector('.panel-head button.cancel')`), 'panel open')
    await js(`document.querySelector('.panel-head button.cancel').click()`)
    await wait(() => js(`!document.querySelector('.panel')`), 'panel closed')
  }
  for (const [dropdown, item] of [
    ['Fillet menu', 'Chamfer'],
    ['Pattern', 'Linear Pattern'],
    ['Pattern', 'Circular Pattern'],
    ['Pattern', 'Mirror'],
    ['Reference Geometry', 'Plane'],
    ['Reference Geometry', 'Axis'],
    ['Reference Geometry', 'Point'],
    ['Reference Geometry', 'Coordinate System'],
  ]) {
    await selectMenu(dropdown, item)
    await wait(() => js(`!!document.querySelector('.panel')`), item + ' panel')
    await click('Orientation')
    await key('Escape')
    assert.ok(await js(`!!document.querySelector('.panel')`), 'menu Escape cannot cancel panel')
    assert.match(
      await js(`document.querySelector('[data-command="extrude"]').title`),
      /Finish or cancel/,
    )
    await closePanel()
  }
  await click('Evaluate')
  for (const label of ['Measure', 'Mass Properties', 'Material', 'Units', 'Export']) {
    await click(label)
    await closePanel()
  }
  await click('Features')
  for (const orientation of ['Front', 'Back', 'Left', 'Right', 'Top', 'Bottom', 'Isometric']) {
    await selectMenu('Orientation', orientation)
    await new Promise((r) => setTimeout(r, 300))
  }
  await click('Zoom to Fit')
  await new Promise((r) => setTimeout(r, 300))
  await js(`document.querySelectorAll('.toast').forEach(t => t.click())`)
  await captureLayouts('part')
  await enabled('new')
  await click('New')
  await wait(async () => (await call('doc.info')).features.length === 0, 'new import document')
  const opensBefore = openDialogs
  await selectMenu('Open menu', 'Import')
  await wait(() => openDialogs > opensBefore, 'Import dialog')
  await wait(async () => (await call('doc.info')).features.length > 0, 'import committed')
  assert.ok((await call('mass_properties')).volume > 1)
  await enabled('save')
  await click('Save')
  await enabled('new')
  // Exercise the independent revolve controls through the real renderer.
  await enabled('new')
  await click('New')
  await wait(async () => (await call('doc.info')).features.length === 0, 'new revolve document')
  await wait(
    () =>
      js(`!!document.querySelector('.tree-item[title="Double-click to sketch on this plane"]')`),
    'tree ready',
  )
  await js(
    `document.querySelector('.tree-item[title="Double-click to sketch on this plane"]').dispatchEvent(new MouseEvent('dblclick', { bubbles: true }))`,
  )
  await wait(() => js(`document.body.textContent.includes('Exit Sketch')`), 'second sketch')
  await new Promise((r) => setTimeout(r, 320))
  await click('Corner Rectangle')
  await pointer(60, -10)
  await pointer(120, -60)
  await click('Centerline')
  await pointer(0, 0)
  await pointer(0, -100)
  await click('Features')
  await click('Revolved Boss/Base')
  await wait(
    () => js(`!!document.querySelector('.panel select option[value="up_to_surface"]')`),
    'revolve controls',
  )
  await js(
    `(() => { const select = [...document.querySelectorAll('.panel select')].find(s => s.querySelector('option[value="up_to_surface"]')); select.value='up_to_surface'; select.dispatchEvent(new Event('change', { bubbles:true })); })()`,
  )
  await wait(
    () =>
      js(
        `!![...document.querySelectorAll('.refbox button')].find(b => b.textContent === 'Right Plane')`,
      ),
    'target picking',
  )
  await click('Right Plane')
  await js(
    `(() => { const label = [...document.querySelectorAll('.panel label')].find(l => l.textContent.includes('Enable second direction')); label.querySelector('input').click(); })()`,
  )
  await wait(
    () =>
      js(
        `!!document.querySelector('.panel .ok:not(:disabled)') && !document.querySelector('.panel-error')`,
      ),
    'two-direction revolve preview',
  )
  await js(`document.querySelector('.panel .ok').click()`)
  await wait(
    async () => (await call('doc.info')).features.some((f) => f.type === 'revolve'),
    'revolve committed',
  )
  const rev = (await call('doc.info')).features.find((f) => f.type === 'revolve')
  assert.equal(rev.params.angle.type, 'up_to_surface')
  assert.equal(rev.params.angle.target.type, 'right_plane')
  assert.equal(rev.params.end2.type, 'blind')
  assert.ok((await call('mass_properties')).volume > 1)
  menu('view-iso')
  await new Promise((r) => setTimeout(r, 250))
  menu('zoom-fit')
  await new Promise((r) => setTimeout(r, 250))
  assert.deepEqual(await js('window.__smokeErrors'), [], 'renderer errors after revolve')
  console.log('Core desktop checks passed; testing Phase 2 and assistant…')
  await phase2Smoke({
    js,
    call,
    click,
    wait,
    enabled,
    root,
    part,
    sendInput: (event) => win.webContents.sendInputEvent(event),
  })
  await assistantSmoke({ js, call, click, wait })
  const screenshot = await win.capturePage()
  const artifact = process.env.OPENFORGE_SMOKE_SCREENSHOT
  if (artifact) writeFileSync(artifact, screenshot.toPNG())
  console.log(
    'Desktop smoke passed: toolbar layout/themes, keyboard/menu isolation, sketch tools, dropdown panels, views, draw/extrude/revolve, body visibility/copy/split retention, scoped formulas, configurations, spatial sketches, hollow sweeps, domes, cosmetic threads, ribs with formula entry, modeled threads, boundaries, tangent lofts, undo/redo, save/reopen/import/export',
  )
}

app.once('browser-window-created', (_event, win) => {
  win.webContents.setBackgroundThrottling(false)
  win.hide()
  win.webContents.once('did-finish-load', () => {
    run(win)
      .then(() => {
        clearTimeout(timeout)
        app.quit()
      })
      .catch(async (error) => {
        console.error(error)
        if (process.env.OPENFORGE_SMOKE_SCREENSHOT) {
          const capture = await win.capturePage().catch(() => null)
          if (capture) writeFileSync(process.env.OPENFORGE_SMOKE_SCREENSHOT, capture.toPNG())
        }
        console.error(
          await win.webContents
            .executeJavaScript('document.body.innerText')
            .catch(() => 'renderer unavailable'),
        )
        app.exit(1)
      })
  })
})
app.on('will-quit', () => {
  rmSync(temp, { recursive: true, force: true })
})
await import(pathToFileURL(join(root, 'apps/desktop/out/main/index.js')).href)

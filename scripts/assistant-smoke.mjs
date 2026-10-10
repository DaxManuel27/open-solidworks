import assert from 'node:assert/strict'
import { createServer } from 'node:http'

// Model responses are deterministic; geometry, IPC, React and undo are real.
export async function assistantSmoke({ js, call, click, wait }) {
  let steps = [],
    paused = false,
    arrived = false,
    requestBody
  const server = createServer(async (req, res) => {
    try {
      let body = ''
      for await (const chunk of req) body += chunk
      requestBody = JSON.parse(body)
      arrived = true
      if (paused) return
      const step = steps.shift()
      if (!step) throw new Error('Unexpected model request')
      const calls = step(requestBody)
      res.writeHead(200, { 'content-type': 'application/x-ndjson' })
      res.end(
        JSON.stringify({
          message: {
            role: 'assistant',
            content: calls.length ? '' : 'The part is editable.',
            tool_calls: calls.map(([name, args]) => ({ function: { name, arguments: args } })),
          },
          done: true,
          prompt_eval_count: 20,
          eval_count: 10,
        }) + '\n',
      )
    } catch (e) {
      res.writeHead(500).end(String(e))
    }
  })
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve))
  const endpoint = `http://127.0.0.1:${server.address().port}`
  const last = (body) => JSON.parse(body.messages.findLast((m) => m.role === 'tool').content).result
  const tool = (name, args) => () => [[name, args]]
  const finish = tool('finish_task', { summary: 'Created editable plate.', preview_only: false })
  let sketch
  const plate = () => [
    tool('new_sketch', { plane: { type: 'front_plane' } }),
    (body) => {
      sketch = last(body).id
      return [['edit_sketch', { id: sketch, op: { op: 'rectangle', a: [0, 0], b: [60, 40] } }]]
    },
    tool('close_sketch', {}),
    () => [
      ['add_feature', { feature: { type: 'extrude', sketch, end1: { type: 'blind', depth: 5 } } }],
    ],
    tool('mass_properties', {}),
    (body) => {
      assert.ok(Math.abs(last(body).volume - 12000) < 0.01)
      return finish()
    },
  ]
  const mode = async (value) =>
    js(
      `(() => { const s=document.querySelector('[aria-label="Assistant mode"]'); s.value=${JSON.stringify(value)}; s.dispatchEvent(new Event('change',{bubbles:true})); })()`,
    )
  const send = async (text) => {
    await js(
      `(() => { const t=document.querySelector('[aria-label="Message assistant"]'); t.focus(); Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value').set.call(t,${JSON.stringify(text)}); t.dispatchEvent(new Event('input',{bubbles:true})); })()`,
    )
    await js('new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))')
    await click('Send')
  }
  const done = () =>
    wait(
      () =>
        js(`document.querySelector('.assistant-actions button[type="button"]')?.disabled===true`),
      'assistant finished',
    )
  try {
    await click('New')
    await wait(async () => !(await call('doc.info')).features.length, 'empty part')
    await click('Toggle AI assistant')
    await click('Settings')
    await wait(
      () => js(`!!document.querySelector('[aria-label="AI connection"]')`),
      'assistant settings',
    )
    await js(
      `(() => { const s=document.querySelector('[aria-label="AI connection"]'); s.value='chatgpt'; s.dispatchEvent(new Event('change',{bubbles:true})); })()`,
    )
    await wait(
      () => js(`document.body.innerText.includes('Continue with ChatGPT')`),
      'ChatGPT connection control',
    )
    assert.ok(await js(`!document.querySelector('[aria-label="AI API key"]')`))
    assert.equal((await js('window.openforge.chatGPTState()')).profiles.length, 0)
    await js(
      `window.openforge.saveAssistantSettings({provider:'ollama',model:'fixture',endpoint:${JSON.stringify(endpoint)},rememberKey:false})`,
    )
    await click('Close settings')
    steps = plate()
    await send('Create a 60 × 40 × 5 mm plate.')
    await done()
    assert.equal(steps.length, 0)
    assert.equal((await call('doc.info')).features.length, 2)
    assert.ok(Math.abs((await call('mass_properties')).volume - 12000) < 0.01)
    const width = await js(
      `(() => {const p=document.querySelector('.assistant-pane').getBoundingClientRect(); return {width:p.width,height:p.height};})()`,
    )
    assert.ok(width.width >= 290 && width.height > 300, 'assistant pane occupies the main row')
    await click('Undo')
    await wait(async () => !(await call('doc.info')).features.length, 'one undo removed whole task')
    await mode('preview')
    steps = plate()
    await send('Preview the same plate.')
    await done()
    assert.equal((await call('doc.info')).features.length, 0, 'preview leaves document unchanged')
    await click('Discard preview')
    assert.equal((await call('doc.info')).features.length, 0)
    steps = plate()
    await send('Preview a plate again.')
    await done()
    await click('Apply preview')
    await wait(async () => (await call('doc.info')).features.length === 2, 'preview committed')
    await mode('edit')
    const base = await call('doc.info')
    await js(`document.querySelector('.tree-item[draggable="true"]')?.click()`)
    // Typing Delete in chat must never delete the selected feature.
    await js(
      `(() => { const t=document.querySelector('[aria-label="Message assistant"]'); t.focus(); t.dispatchEvent(new KeyboardEvent('keydown',{key:'Delete',bubbles:true})); })()`,
    )
    assert.equal((await call('doc.info')).revision, base.revision)
    steps = [
      tool('set_parameter', { key: 'f2.end1.depth', value: 8 }),
      tool('finish_task', { summary: 'Plate thickness is 8 mm.', preview_only: false }),
    ]
    await send('Make the plate 8 mm thick.')
    await done()
    assert.ok(Math.abs((await call('mass_properties')).volume - 19200) < 0.01)
    await click('Undo')
    await wait(
      async () => Math.abs((await call('mass_properties')).volume - 12000) < 0.01,
      'dimension edit undone',
    )
    // Stop while a model request is waiting: no changes or history entries.
    paused = true
    arrived = false
    await send('Make another change.')
    await wait(() => arrived, 'model awaiting response')
    const beforeStop = await call('doc.info')
    await click('Stop')
    await done()
    assert.equal((await call('doc.info')).revision, beforeStop.revision)
    // Manual work invalidates a waiting assistant task.
    arrived = false
    await send('Make a change after thinking.')
    await wait(() => arrived, 'second delayed model request')
    await call('parameter.set', { key: 'f2.end1.depth', value: 6 })
    await done()
    assert.ok(Math.abs((await call('mass_properties')).volume - 14400) < 0.01)
    await click('Undo')
    await wait(
      async () => Math.abs((await call('mass_properties')).volume - 12000) < 0.01,
      'manual edit undone',
    )
    paused = false
    await mode('ask')
    steps = [() => []]
    await send('Explain the part.')
    await done()
    assert.ok(
      requestBody.tools.every(
        (t) => !['add_feature', 'set_parameter', 'finish_task'].includes(t.function.name),
      ),
      'ask mode excludes mutations',
    )
    assert.deepEqual(await js('window.__smokeErrors'), [])
    console.log(
      'Assistant desktop smoke passed: ChatGPT UI, plate, parameter edit, preview/discard/apply, grouped undo, read-only tools, cancellation and stale-task protection.',
    )
  } finally {
    server.closeAllConnections()
    await new Promise((resolve) => server.close(resolve))
  }
}

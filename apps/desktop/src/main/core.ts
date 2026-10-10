import { type ChildProcessWithoutNullStreams, spawn } from 'node:child_process'
import { existsSync } from 'node:fs'
import { join } from 'node:path'
import { createInterface } from 'node:readline'

interface Pending {
  resolve: (value: unknown) => void
  reject: (error: Error) => void
}

export class CoreError extends Error {
  constructor(
    message: string,
    readonly code: number,
  ) {
    super(message)
  }
}

/** Finds the `openforge` core binary: OPENFORGE_CORE, the packaged copy, or a cargo build. */
export function findCore(appPath: string, resourcesPath: string): string {
  const exe = process.platform === 'win32' ? 'openforge.exe' : 'openforge'
  const candidates = [
    process.env['OPENFORGE_CORE'],
    join(resourcesPath, exe),
    join(appPath, '../../target/debug', exe),
    join(appPath, '../../target/release', exe),
  ]
  const found = candidates.find((c): c is string => !!c && existsSync(c))
  if (!found) {
    throw new Error(`OpenForge core not found. Build it with "cargo build" first.`)
  }
  return found
}

/** The Rust core process, spoken to with JSON-RPC over stdin/stdout. */
export class CoreProcess {
  private child: ChildProcessWithoutNullStreams | null = null
  private pending = new Map<number, Pending>()
  private nextId = 1
  onExit: (code: number | null) => void = () => {}

  constructor(private readonly path: string) {}

  start(): void {
    const child = spawn(this.path, ['serve'], { stdio: 'pipe' })
    this.child = child
    createInterface({ input: child.stdout }).on('line', (line) => this.receive(line))
    child.stderr.on('data', (d: Buffer) => process.stderr.write(`[core] ${d.toString()}`))
    child.on('exit', (code) => {
      this.child = null
      for (const p of this.pending.values())
        p.reject(new CoreError('The core stopped unexpectedly.', -1))
      this.pending.clear()
      this.onExit(code)
    })
  }

  call(method: string, params: unknown): Promise<unknown> {
    if (!this.child) return Promise.reject(new CoreError('The core is not running.', -1))
    const id = this.nextId++
    const msg = JSON.stringify({ jsonrpc: '2.0', id, method, params: params ?? {} })
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject })
      this.child?.stdin.write(msg + '\n')
    })
  }

  stop(): void {
    this.child?.kill()
  }

  private receive(line: string): void {
    let msg: { id?: number; result?: unknown; error?: { code: number; message: string } }
    try {
      msg = JSON.parse(line)
    } catch {
      process.stderr.write(`[core] unparseable output: ${line}\n`)
      return
    }
    if (typeof msg.id !== 'number') return
    const p = this.pending.get(msg.id)
    if (!p) return
    this.pending.delete(msg.id)
    if (msg.error) p.reject(new CoreError(msg.error.message, msg.error.code))
    else p.resolve(msg.result)
  }
}

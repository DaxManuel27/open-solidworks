import type { Snapshot } from './protocol'

export class CallFailed extends Error {}

/** Call a core method; throws CallFailed with the core's message. */
export async function call<T = unknown>(method: string, params?: unknown): Promise<T> {
  const r = await window.openforge.call(method, params ?? {})
  if (!r.ok) throw new CallFailed(r.error)
  return r.value as T
}

export type { Snapshot }

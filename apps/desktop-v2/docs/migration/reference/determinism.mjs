export const FIXTURE = { clock: '2026-11-11T12:00:00.000Z', timezone: 'UTC', locale: 'en-US', seed: 2352 }
export function installDeterminism(config) {
  const RealDate = globalThis.Date
  const now = RealDate.parse(config.clock)
  globalThis.Date = class extends RealDate {
    constructor(...args) { super(...(args.length ? args : [now])) }
    static now() { return now }
  }
  let seed = config.seed >>> 0
  Math.random = () => { seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0; return seed / 4294967296 }
  let uuidSeed = config.seed >>> 0
  const uuidRandom = () => { uuidSeed = (Math.imul(uuidSeed, 1664525) + 1013904223) >>> 0; return uuidSeed / 4294967296 }
  const uuid = () => 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, c => {
    const r = Math.floor(uuidRandom() * 16); return (c === 'x' ? r : (r & 3) | 8).toString(16)
  })
  for(let i=0;i<(config.uuidSkip ?? 0);i++) uuid()
  Object.defineProperty(globalThis.crypto, 'randomUUID', { value: uuid, configurable: true })
}

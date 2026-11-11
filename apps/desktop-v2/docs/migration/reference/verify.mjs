import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFileSync, writeFileSync, readdirSync, unlinkSync } from 'node:fs'
const root=new URL('./',import.meta.url)
const metrics=JSON.parse(readFileSync(new URL('metrics.json',root)))
const seen=new Map(), paths=new Set(), scenes=new Map()
for(const state of metrics.states) {
 assert.ok(!state.error,`${state.window}/${state.scene}: ${state.error}`)
 const path=new URL(state.file,root)
 const png=readFileSync(path)
 assert.equal(png.subarray(1,4).toString(),'PNG')
 assert.equal(png.readUInt32BE(16),state.viewport.width*2)
 assert.equal(png.readUInt32BE(20),state.viewport.height*2)
 assert.ok(state.landmarks.length>0)
 const hash=createHash('sha256').update(png).digest('hex')
 if(process.argv.includes('--dedupe') && seen.has(hash)) { state.aliasOf=seen.get(hash);state.file=seen.get(hash) }
 else seen.set(hash,state.file)
 paths.add(state.file)
 const key=`${state.window}/${state.scene}`
 const themes=scenes.get(key)??new Set();themes.add(state.theme);scenes.set(key,themes)
}
for(const [scene,themes] of scenes) assert.deepEqual([...themes].sort(),['dark','light'],scene)
const documented=[...readFileSync(new URL('STATES.md',root),'utf8').matchAll(/^\| (main|palette|picker|form|permissions) \| ([^|]+) \|/gm)].map(m=>`${m[1]}/${m[2].trim()}`)
assert.deepEqual([...scenes.keys()].sort(),documented.sort())
if(process.argv.includes('--dedupe')) {
 for(const dir of ['main','palette','picker','form','permissions']) for(const name of readdirSync(new URL(dir+'/',root))) if(name.endsWith('.png')&&!paths.has(dir+'/'+name)) unlinkSync(new URL(dir+'/'+name,root))
 writeFileSync(new URL('metrics.json',root),JSON.stringify(metrics,null,2)+'\n')
}
console.log(`${scenes.size} scenes, ${metrics.states.length} theme captures, ${paths.size} unique PNGs; 2× dimensions and landmarks verified`)

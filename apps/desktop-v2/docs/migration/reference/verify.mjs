import assert from 'node:assert/strict'
import { authoredTokenSources } from './landmarks.mjs'
import { createHash } from 'node:crypto'
import { readFileSync, writeFileSync, readdirSync, unlinkSync } from 'node:fs'
const root=new URL('./',import.meta.url)
const metrics=JSON.parse(readFileSync(new URL('metrics.json',root)))
assert.equal(metrics.schemaVersion,2)
assert.deepEqual(metrics.landmarkColumns,['role','name','occurrence','rect','style'])
assert.ok(readFileSync(new URL('metrics.json',root)).length < 800000,'metrics must stay well under 1 MB')
const {colors,fonts,shadows,styles,colorSources}=metrics.tables
for(const color of colors) {
 assert.equal(color.length,4)
 for(const channel of color.slice(0,3)) assert.ok(Number.isInteger(channel)&&channel>=0&&channel<=255)
 assert.ok(color[3]>=0&&color[3]<=1)
}
for(const [theme,sources] of Object.entries(authoredTokenSources())) for(const [name,token] of Object.entries(metrics.tokens[theme])) { assert.equal(colorSources[token.source],sources[name]);assert.ok(colors[token.rgba]) }
for(const theme of ['light','dark']) for(const name of ['--background','--foreground','--surface','--iris']) assert.ok(metrics.tokens[theme][name],`${theme} ${name}`)
const allowedRoles=new Set(['button','textbox','searchbox','checkbox','radio','slider','switch','combobox','spinbutton','heading','template-chip','region','navigation','banner','complementary','main','contentinfo','dialog','form','search'])
for(const state of metrics.states) {
 const keys=new Set()
 for(const [role,name,occurrence,rect,styleId] of state.landmarks) {
  assert.ok(allowedRoles.has(role),role)
  assert.ok(name.trim() && Number.isInteger(occurrence) && occurrence>0)
  const key=JSON.stringify([role,name,occurrence]);assert.ok(!keys.has(key));keys.add(key)
  assert.ok(rect.length===4&&rect.every(Number.isFinite))
  const [font,size,weight,radius,padding,fg,bg,shadow]=styles[styleId]
  assert.ok(fonts[font] && Number.isFinite(size)&&Number.isFinite(weight))
  assert.ok(radius.length===4&&padding.length===4&&colors[fg]&&colors[bg]&&shadows[shadow]!==undefined)
  if(state.window==='permissions') assert.ok(!['Preview','missing','allowed','paused','secure','restart','revoked'].includes(name),name)
 }
 if(state.window==='form') assert.equal(state.viewport.height,Math.max(160,Math.min(680,state.dialogHeight)))
}
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
for(const state of metrics.states.filter(s=>s.window==='permissions'&&s.scene==='revoked')) if(process.argv.includes('--dedupe') || state.aliasOf) assert.equal(state.aliasOf,`permissions/missing.${state.theme}.png`)
for(const [scene,themes] of scenes) assert.deepEqual([...themes].sort(),['dark','light'],scene)
const documented=[...readFileSync(new URL('STATES.md',root),'utf8').matchAll(/^\| (main|palette|picker|form|permissions) \| ([^|]+) \|/gm)].map(m=>`${m[1]}/${m[2].trim()}`)
assert.deepEqual([...scenes.keys()].sort(),documented.sort())
if(process.argv.includes('--dedupe')) {
 for(const dir of ['main','palette','picker','form','permissions']) for(const name of readdirSync(new URL(dir+'/',root))) if(name.endsWith('.png')&&!paths.has(dir+'/'+name)) unlinkSync(new URL(dir+'/'+name,root))
 writeFileSync(new URL('metrics.json',root),JSON.stringify(metrics)+'\n')
}
console.log(`${scenes.size} scenes, ${metrics.states.length} theme captures, ${paths.size} unique PNGs; 2× dimensions and landmarks verified`)

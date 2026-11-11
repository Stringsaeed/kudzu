// Run from the repository root; no dependency installation or production data access.
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync, writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { FIXTURE, installDeterminism } from './determinism.mjs'
process.env.TZ = FIXTURE.timezone
installDeterminism(FIXTURE)
assert.equal(Intl.DateTimeFormat().resolvedOptions().timeZone,'UTC')
assert.equal(Intl.DateTimeFormat().resolvedOptions().locale,'en-US')
const desktop = new URL('../../../../desktop/', import.meta.url)
const require = createRequire(new URL('package.json', desktop))
const load = name => import(new URL(`lib/${name}.ts`, desktop))
const E = await load('editor-formatting')
const { EditorState } = await import(new URL('node_modules/@codemirror/state/dist/index.js', desktop))
const { history, undo, redo } = await import(new URL('node_modules/@codemirror/commands/dist/index.js', desktop))
const storage = new Map()
globalThis.localStorage = { getItem: key => storage.get(key) ?? null, setItem: (key, value) => storage.set(key, value) }
globalThis.location = { search: '', pathname: '/' }
globalThis.window = { addEventListener() {} }
globalThis.document = { addEventListener() {} }
const { mockBackend: mock } = await load('mock')
const invoke = (cmd, args = {}) => mock.invoke(cmd, args)
const state = await invoke('get_state')
const clipboard = await invoke('clipboard_list')
const mockState = { ...state, clipboard, apps: await invoke('list_apps'), activity: await invoke('activity_list'), backups: await invoke('list_backups'), update: await invoke('update_state'), sync: await invoke('sync_status'), agent: await invoke('agent_setup'), fixture: FIXTURE }
location.search = '?update'
mockState.updateAvailable = await invoke('update_state')
location.search = '?nosync'
mockState.syncUnavailable = await invoke('sync_status')
location.search = ''
const S = await load('search'), G = await load('groups'), T = await load('template'), C = await load('clipboard'), U = await load('updates'), K = await load('keys'), Stats = await load('stats'), F = await load('formatting')
mockState.media = Object.fromEntries(mock.mediaUrls)
const outputs = {}
mockState.permissionStates = Object.fromEntries(['missing','allowed','revoked','paused','secure','restart'].map(mode=>{mock.previewPermission(mode); return [mode,structuredClone(mock.state.status)]}))
mock.previewPermission('allowed')
outputs['mock-state'] = mockState
const library = state.library
const large = Array.from({length:5000}, (_, i) => ({ ...library.snippets[i % library.snippets.length], id: crypto.randomUUID(), name: `${library.snippets[i % library.snippets.length].name} ${i} ${['café','العربية','İstanbul','😀'][i%4]}`, triggers: [`;s${i}`], updatedAt: new Date(Date.now() - Math.floor(Math.random()*40)*86400000).toISOString() }))
outputs['search-inputs'] = { library, large }
outputs.search = [library.snippets, large].map((snippets, i) => {
 const index = S.buildIndex(snippets, Stats.usageById(library))
 return { dataset:i ? 'large':'preview', cases:['',';sig',';s42','sig','em sg','clipboard','thanks','café','العربية','İ','😀','no-such-match','x'.repeat(80)].map(query => ({ query, limit:200, hits:S.search(index,query).map(h=>({id:h.snippet.id, score:h.score, field:h.field, nameHits:h.nameHits})) })) }
})
outputs.groups = { collapsed:G.flatten(G.buildTree(library.groups),new Set([library.groups[0].id])),  tree:G.buildTree(library.groups), flattened:G.flatten(G.buildTree(library.groups)), cases:library.groups.map(g=>({id:g.id,path:G.groupPath(library.groups,g.id),descendants:[...G.descendantIds(library.groups,g.id)], enabled:G.isGroupEnabled(library.groups,g.id)})), counts:[...G.recursiveCounts(library.groups,library.snippets)] }
outputs['group-edge-cases'] = [
 [{id:'a',name:'A',parentID:'b',sortIndex:0,isEnabled:true},{id:'b',name:'B',parentID:'a',sortIndex:1,isEnabled:false}],
 [{id:'a',name:'Orphan',parentID:'missing',sortIndex:0,isEnabled:true}],
 [{id:'a',name:'First',sortIndex:0,isEnabled:true},{id:'a',name:'Duplicate',sortIndex:1,isEnabled:false}]
].map(groups=>({groups,tree:G.buildTree(groups),paths:groups.map(g=>({id:g.id,path:G.groupPath(groups,g.id),descendants:[...G.descendantIds(groups,g.id)],enabled:G.isGroupEnabled(groups,g.id)})),counts:[...G.recursiveCounts(groups,library.snippets)]}))
outputs.stats = { usage:[...Stats.usageById(library)], totals:[20,40,80].map(wpm=>({wpm,result:Stats.totals(library,Stats.usageById(library),wpm)})), counts:[0,1,999,1000,999999,1000000].map(input=>({input,output:Stats.formatCount(input)})), durations:[0,0.1,1,59,60,1440].map(input=>({input,output:Stats.formatDuration(input)})), relative:[0,30000,3600000,86400000,86400000*40].map(age=>({age,output:Stats.relativeTime(Date.now()-age)})) }
outputs.clipboard = { queries:['','app:slack','is:image','is:img','design Thursday','app:"Visual Studio Code"','is:code const','APP:Safari','is:unknown','العربية'].map(query=>({query,parsed:C.parseQuery(query),filters:C.FILTERS.map(f=>({filter:f.value,ids:C.searchClips(clipboard.entries,f.value,query).map(e=>e.id)}))})), rows:[false,true].flatMap(pinnedFirst=>[false,true].map(sections=>({pinnedFirst,sections,result:C.buildRows(clipboard.entries,{pinnedFirst,sections})}))), entries:clipboard.entries.map(e=>({id:e.id,description:C.describe(e),spoken:C.spokenLabel(e),concealed:C.spokenLabel(e,true),time:C.clipTime(e.copiedAt),datedTime:C.clipTime(e.copiedAt,undefined,true),visibility:[false,true].map(showSecrets=>({showSecrets,concealed:C.isSecret(e)&&!showSecrets,spoken:C.spokenLabel(e,C.isSecret(e)&&!showSecrets)})),exact:C.exactTime(e.copiedAt),expiry:[0,1,7,365].map(days=>({days,label:C.retentionLabel(days),output:C.expiresIn(e,days)}))})), ranges:['banana BANANA','😀 café العربية','x'.repeat(200)].map(text=>({text,terms:['an','a','é','الع','x'],ranges:C.matchRanges(text,['an','a','é','الع','x'])})), bytes:[0,1,1024,1280,2304,1310720,1000000,38400000].map(n=>({n,label:C.formatBytes(n)})) }
// Extra clipboard inputs call the same frozen TypeScript implementation.
const linkUrls = [
 'https://example.com', 'https://example.com/', 'https://example.com?',
 'https://example.com/path?', 'https://example.com/?#fragment',
 'https://example.com:443/path', 'http://example.com:80/path',
 'ftp://example.com:21/path', 'https://example.com:8443/path',
 'https://user:pass@example.com/a', 'https://user@example.com/a',
 'https://example.com/a/../b', 'https://example.com/a/./b',
 'https://example.com/a/%2e%2e/b', 'https://example.com/a/.',
 'https://example.com/a/..', 'https://example.com/a//b',
 'https://example.com/../../b', 'https://example.com/caf%C3%A9',
 'https://example.com/a%2Fb?q=%3F', 'https://example.com/a?x=1#hash',
 'https://example.com/a?x=1/', 'https://WWW.example.com:443/',
 'www.example.com/path', 'mailto:hello@example.com?subject=Hi',
 'HTTPS://EXAMPLE.COM/path', 'https://example.com/%zz',
 'https://example.com/a\\b', 'https:\\example.com\\a',
 'https://exam\tple.com/a', 'https://example.com/a\nb',
 'https://example.com/a\rb', 'https://example.com/a\tb',
 'https://例え.テスト/道', 'https://münich.example/café',
]
outputs.clipboard.links = linkUrls.map(url=>({url,description:C.describe({...clipboard.entries[0],kind:'link',text:url})}))
outputs.clipboard.ranges.push(...[
 {text:'İstanbul İ X',terms:['i','x']},
 {text:'İİABC',terms:['abc']},
 {text:'𐐀𐐁 ABC 😀',terms:['𐐨','𐐩','abc']},
].map(({text,terms})=>({text,terms,ranges:C.matchRanges(text,terms)})))
outputs.clipboard.days = ['2025-11-11T12:00:00.000Z'].map(iso=>({iso,label:C.dayLabel(Date.parse(iso))}))
const tags = ['{{date}}','{{time}}','{{clipboard trim=true history=2}}','{{cursor}}',`{{snippet id="${library.snippets[0].id}"}}`,'{{field name="Name" default="Friend"}}','{{choice name="Platform" options="Mac|Linux"}}','{{selection trim=true}}','{{uuid upper=true}}','{{random options="A|B"}}','{{key name="tab" count=2}}','{{delay ms=300}}','{{unknown a="x"}}','{{field}}','\\{{literal}}','😀 {{date format="yyyy-MM-dd"}}','x'.repeat(150)]
const snippetName=id=>library.snippets.find(s=>s.id===id)?.name
outputs.template = { presets:T.DATE_FORMAT_PRESETS,cases:tags.map(source=>({source,tokens:T.tokenize(source),chips:T.tokenize(source).filter(t=>t.kind==='tag').map(t=>({label:T.chipLabel(t,snippetName),tone:T.chipTone(t),canonical:T.buildTag(t.name,t.attrs)})),facts:T.analyze(source),summary:T.plainSummary(source,snippetName)})) }
outputs.keys = { labels:['mod+k','mod+shift+p','Control+Alt+Space','CommandOrControl+Shift+F12','Ctrl+Alt+V'].map(input=>({input,keys:K.keyLabels(input),accelerator:K.acceleratorLabels(input)})),recorder:[{key:'a'},{key:'a',shiftKey:true},{key:'a',metaKey:true},{key:'F5'},{key:'Shift',shiftKey:true},{key:' ',ctrlKey:true,altKey:true},{key:'Escape'},{key:'ArrowLeft',ctrlKey:true}].map(event=>({event:{...event,code:event.key==='a'?'KeyA':event.key===' '?'Space':event.key},result:K.acceleratorFromEvent({ctrlKey:false,altKey:false,metaKey:false,shiftKey:false,code:event.key==='a'?'KeyA':event.key===' '?'Space':event.key,...event})})) }
outputs['release-notes'] = [null,'',mockState.updateAvailable.notes,'# Release\n\n### Fixes\n- **bold** and [link](https://example.com) (`abcdef1`)\n* second item'].map(input=>({input,result:U.parseNotes(input)}))
const transactions=[]
for (const text of ['Hello 😀 {{date}} world','first\nsecond\nthird']) {
 let current=EditorState.create({doc:text,selection:{anchor:0,head:text.length},extensions:[E.formattingField,E.formattingHistory,history()]})
 const view={get state(){return current},composing:false,focus(){},dispatch(spec){ current=spec.state ?? current.update(spec).state }}
 const snap=action=>transactions.push({text,action,doc:current.doc.toString(),selection:current.selection.toJSON(),spans:current.field(E.formattingField)})
 view.dispatch({effects:E.setFormatting.of([])}); snap('enable')
 E.changeFormat(view,{kind:'bold'}); snap('bold')
 E.changeFormat(view,{kind:'italic'}); snap('italic')
 E.changeFormat(view,{kind:'underline'}); snap('underline')
 E.changeFormat(view,{kind:'link',url:'https://example.com/first'}); snap('link')
 E.changeFormat(view,{kind:'link',url:'https://example.com/edited'}); snap('edit-link')
 E.changeFormat(view,'clear'); snap('clear')
 const image={kind:'image',dataUrl:'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a6x8AAAAASUVORK5CYII=',width:32}
 view.dispatch({changes:{from:0,insert:'[Dot]'},effects:E.setFormatting.of([{start:0,end:5,style:image}])}); snap('insert-image')
 view.dispatch({selection:{anchor:0,head:5}})
 E.changeFormat(view,'clear'); snap('remove-image-formatting')
 view.dispatch({changes:{from:0,insert:'Hi '}}); snap('insert')
 E.toggleList(view,false); snap('bullets')
 E.toggleList(view,true); snap('numbers')
 E.toggleList(view,true); snap('remove-numbers')
 undo({state:current,dispatch:tr=>current=tr.state}); snap('undo')
 redo({state:current,dispatch:tr=>current=tr.state}); snap('redo')
}
outputs['editor-formatting']={transactions,safety:['https://example.com','javascript:alert(1)','mailto:a@b.com','https://bad x'].map(input=>({input,result:F.safeLink(input)})),utf16:F.validSpans('😀 abc',[{start:1,end:3,style:{kind:'bold'}},{start:0,end:2,style:{kind:'italic'}}])}
// Private colour helpers are evaluated from the actual source, never reimplemented.
const ts=require('typescript')
const colorSource=readFileSync(new URL('components/app/clipboard/clip-content.tsx',desktop),'utf8')
const colorFunctions=colorSource.slice(colorSource.indexOf('function srgbToLinear'),colorSource.indexOf('function CopyValue'))
const formats=Function(ts.transpileModule(colorFunctions,{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText+'; return formats')()
const colorInputs=JSON.parse(readFileSync(new URL('fixtures/color-inputs.json',import.meta.url),'utf8'))
outputs.color=colorInputs.map(({css,rgba})=>({css,rgba,result:formats(rgba)}))
for(const [name,value] of Object.entries(outputs)) test(`fixture ${name}`,()=>{
 const path=new URL(`fixtures/${name}.json`,import.meta.url)
 const bytes=JSON.stringify(value,null,2)+'\n'
 if(process.env.UPDATE_FIXTURES==='1') writeFileSync(path,bytes)
 else assert.equal(readFileSync(path,'utf8'),bytes)
})

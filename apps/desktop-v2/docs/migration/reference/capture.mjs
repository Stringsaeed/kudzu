// pnpm dlx --package playwright@1.65.0 -c 'node apps/desktop-v2/docs/migration/reference/capture.mjs'
import { createRequire } from 'node:module'
import { existsSync, mkdirSync, writeFileSync, readFileSync } from 'node:fs'
import { resolve, delimiter } from 'node:path'
import { FIXTURE, installDeterminism } from './determinism.mjs'
import { landmarkColumns, dictionaries, authoredTokenSources, extractLandmarks, packLandmarks, resolveTokens, intern } from './landmarks.mjs'
const bin = process.env.PATH.split(delimiter).find(p => existsSync(resolve(p,'playwright')))
if (!bin) throw Error('Run through pnpm dlx --package playwright@1.65.0 -c ...')
const { chromium } = createRequire(resolve(bin,'../package.json'))('playwright')
const root = new URL('./',import.meta.url)
const browser = await chromium.launch({headless:true})
const base = process.env.REFERENCE_URL ?? 'http://localhost:3042'
const metrics = process.env.ONLY ? JSON.parse(readFileSync(new URL('metrics.json',root))) : { schemaVersion:2, landmarkColumns, styleColumns:['font','fontSize','fontWeight','radius','padding','color','background','shadow'], tables:dictionaries(), tokens:{}, fixture:FIXTURE, browser:browser.version(), scale:2, states:[] }
const mock = existsSync(new URL('fixtures/mock-state.json',root)) ? JSON.parse(readFileSync(new URL('fixtures/mock-state.json',root))) : null
const tokenSources=authoredTokenSources()
const stateList=[]
const add=(window,state,path='/',action=null,size=null)=>stateList.push({window,state,path,action,size})
add('main','library')
for(const pane of ['general','expansion','activity','clipboard','agents','apps','data','about']) add('main',`settings-${pane}`,`/?view=settings:${pane}`)
add('main','activity-empty','/?view=settings:activity&emptyactivity')
add('main','insights','/?view=insights')
add('main','clipboard-history','/?view=clipboard')
add('main','clips-5000','/?view=clipboard&clips=5000')
add('main','clipboard-empty','/?view=clipboard&emptyclips')
add('main','update','/?view=settings:about&update')
const onboardingImport=async p=>{await p.getByRole('button',{name:'Get started'}).click();await p.getByRole('button',{name:'Continue',exact:true}).click()}
add('main','onboarding-welcome','/?onboarding')
add('main','onboarding-access','/?onboarding',p=>p.getByRole('button',{name:'Get started'}).click())
add('main','onboarding-import','/?onboarding',onboardingImport)
add('main','onboarding-espanso','/?onboarding&espanso',onboardingImport)
add('main','onboarding-espanso-retry','/?onboarding&espanso=retry',async p=>{await onboardingImport(p);await p.getByRole('button',{name:'Import Espanso snippets'}).click();await p.getByRole('button',{name:'Retry Espanso snippets'}).waitFor()})
add('main','onboarding-practice','/?onboarding',async p=>{await onboardingImport(p);await p.getByRole('button',{name:'Skip for now'}).click()})
add('main','onboarding-finish-retry','/?onboarding&finish=retry',async p=>{await onboardingImport(p);await p.getByRole('button',{name:'Skip for now'}).click();await p.getByRole('button',{name:'Skip',exact:true}).click()})
add('palette','search','/palette')
add('picker','history','/clipboard')
add('picker','clips-5000','/clipboard?clips=5000')
add('picker','empty','/clipboard?emptyclips')
add('form','demo','/form?demo')
add('permissions','missing','/permissions')
for(const mode of ['allowed','revoked','paused','secure','restart']) add('permissions',mode,'/permissions',async p=>p.evaluate(mode=>window.dispatchEvent(new CustomEvent('kudzu:preview-permission',{detail:mode})),mode))
add('main','command-menu','/',p=>p.keyboard.press('Meta+k'))
add('main','cheat-sheet','/',p=>p.keyboard.press('Meta+/'))
add('main','library-empty','/',null,null)
const groupMenu=p=>p.getByRole('button',{name:/^Work 5/}).click({button:'right'})
add('main','group-context-menu','/',groupMenu)
add('main','group-dialog','/',async p=>{await groupMenu(p);await p.getByRole('menuitem',{name:'Group settings…'}).click()})
add('main','snippet-actions','/',p=>p.getByRole('button',{name:'More actions',exact:true}).click())
add('main','snippet-bulk','/',async p=>{await p.getByRole('option').nth(0).click();await p.getByRole('option').nth(1).click({modifiers:['Meta']})})
add('main','snippet-deleted-toast','/',async p=>{await p.getByRole('option').nth(0).click();await p.keyboard.press('Meta+Backspace')})
add('main','formatting-toolbar','/',p=>p.getByRole('radio',{name:'Formatted',exact:true}).click())
add('main','insert-menu','/',p=>p.getByRole('button',{name:/^Insert(?:\s*\/)?$/}).click())
add('main','clipboard-options-menu','/?view=clipboard',async p=>{await p.getByRole('button',{name:'Clipboard options'}).click();await p.getByRole('menu').waitFor()})
add('main','clipboard-bulk','/?view=clipboard',async p=>{await p.getByRole('option').nth(0).click();await p.getByRole('option').nth(1).click({modifiers:['Meta']})})
for(const [tag,content] of Object.entries({date:'{{date}}',time:'{{time}}',clipboard:'{{clipboard}}',cursor:'{{cursor}}',snippet:`{{snippet id="${mock?.library.snippets[0].id}"}}`,field:'{{field name="Name"}}',choice:'{{choice name="Platform" options="Mac|Linux"}}',selection:'{{selection}}',uuid:'{{uuid}}',random:'{{random options="A|B"}}',key:'{{key name="tab"}}',delay:'{{delay ms=300}}'})) {
 add('main',`chip-${tag}`,'/',async p=>{await p.locator('.kz-chip').first().click();await p.locator('[data-slot=popover-content]').waitFor()},null)
 stateList.at(-1).content=content
}
for(const [kind,label] of Object.entries({image:'Images',link:'Links',file:'Files',code:'Code',color:'Colors'})) add('main',`clipboard-${kind}`,'/?view=clipboard',p=>p.getByRole('button',{name:new RegExp('^'+label+' \\d')}).click())
add('main','clipboard-no-match','/?view=clipboard',p=>p.locator('input').first().fill('no-such-reference-item'))
add('palette','no-match','/palette',p=>p.locator('input').first().fill('no-such-reference-snippet'))
const sizes={main:[1180,760],palette:[760,480],picker:[800,520],form:[440,640],permissions:[420,660]}
if(process.env.ONLY) {
 if(metrics.schemaVersion!==2) throw Error('Run the full capture before selecting states')
 metrics.states=metrics.states.filter(s=>!process.env.ONLY.split(',').some(term=>s.scene.includes(term)))
}
for(const spec of stateList.filter(s=>!process.env.ONLY || process.env.ONLY.split(',').some(term=>s.state.includes(term)))) for(const theme of ['dark','light']) {
 const [width,height]=spec.size??sizes[spec.window]
 const context=await browser.newContext({viewport:{width,height},deviceScaleFactor:2,locale:FIXTURE.locale,timezoneId:FIXTURE.timezone,colorScheme:theme,reducedMotion:'reduce'})
 await context.addInitScript(installDeterminism,{...FIXTURE,uuidSkip:22})
 if(mock) await context.addInitScript(library=>localStorage.setItem('kudzu-mock',JSON.stringify(library)),mock.library)
 if(spec.content) await context.addInitScript(({library,content})=>{library.snippets[0].content=content;localStorage.setItem('kudzu-mock',JSON.stringify(library))},{library:structuredClone(mock.library),content:spec.content})
 if(spec.state==='library-empty') await context.addInitScript(()=>localStorage.setItem('kudzu-mock',JSON.stringify({formatVersion:1,snippets:[],groups:[],usage:[]})))
 const p=await context.newPage()
 p.setDefaultTimeout(5000)
 const url=base+spec.path+(theme==='light'?(spec.path.includes('?')?'&':'?')+'light':'')
 try {
 await p.goto(url,{waitUntil:'networkidle'})
 await p.locator('nextjs-portal').evaluateAll(nodes=>nodes.forEach(n=>n.remove()))
 await p.evaluate(()=>document.fonts.ready)
 await p.waitForTimeout(250)
 if(spec.action) await spec.action(p)
 await p.waitForTimeout(250)
 if(spec.window==='permissions') await p.locator('aside').filter({hasText:'Preview'}).evaluateAll(nodes=>nodes.forEach(n=>n.remove()))
 if(spec.window==='form') { await p.getByRole('dialog').evaluate(e=>{e.parentElement.style.padding='0'}); const h=Math.max(160,Math.min(680,Math.ceil(await p.getByRole('dialog').evaluate(e=>e.getBoundingClientRect().height)))); await p.setViewportSize({width,height:h}) }
 if(process.env.INSPECT==='1') { console.log(spec.state,await p.locator('body').innerText()); console.log(await p.locator('button,input,[role="option"]').evaluateAll(es=>es.map(e=>({tag:e.tagName,role:e.getAttribute('role'),label:e.getAttribute('aria-label'),text:e.textContent?.slice(0,60)})))); break }
 const landmarks=packLandmarks(await p.evaluate(extractLandmarks),metrics.tables)
 if(!metrics.tokens[theme]) metrics.tokens[theme]=Object.fromEntries((await p.evaluate(resolveTokens,tokenSources[theme])).map(([name,source,color])=>[name,{source:intern(metrics.tables.colorSources,source),rgba:intern(metrics.tables.colors,color)}]))
 mkdirSync(new URL(spec.window+'/',root),{recursive:true})
 const file=`${spec.window}/${spec.state}.${theme}.png`
 await p.screenshot({path:new URL(file,root).pathname,animations:'disabled',caret:'hide',scale:'device'})
 metrics.states.push({window:spec.window,scene:spec.state,theme,file,url:spec.path,viewport:p.viewportSize(),...(spec.window==='form'?{dialogHeight:await p.getByRole('dialog').evaluate(e=>Math.ceil(e.getBoundingClientRect().height))}:{}),landmarks})
 console.log(file,landmarks.length)
 if(!existsSync(new URL('fixtures/color-inputs.json',root))) {
 const inputs=await p.evaluate(()=>['#000','#fff','#6366f1','#ff000080','rebeccapurple','hsl(120 100% 50%)','oklch(0.7 0.1 200)','transparent','invalid-color','rgb(255, 128, 0)','rgba(0, 128, 255, 0.4)','oklch(0.8 0.4 150)'].map(css=>{const c=document.createElement('canvas').getContext('2d');c.fillStyle='#000';c.fillStyle=css;c.fillRect(0,0,1,1);return {css,rgba:[...c.getImageData(0,0,1,1).data].map(v=>v/255)}}))
 writeFileSync(new URL('fixtures/color-inputs.json',root),JSON.stringify(inputs,null,2)+'\n')
 }
 } catch(error) { metrics.states.push({window:spec.window,scene:spec.state,theme,error:String(error)}); console.error(spec.state,error) }
 await context.close()
}
await browser.close()
writeFileSync(new URL('metrics.json',root),JSON.stringify(metrics)+'\n')
writeFileSync(new URL('STATES.md',root),'# Reference scenes\n\nAll dimensions are logical points; PNGs are 2× and live outside git (README Screenshots). Scene names are scoped by window. metrics.json v2 keys landmarks by role, accessible name and 1-based occurrence; packed columns and intern dictionaries are documented in README. Token tables are shared per theme and store authored CSS plus resolved sRGB.\n\n| Window | Scene | Preview path | Size | Themes |\n|---|---|---|---|---|\n'+stateList.map(s=>`| ${s.window} | ${s.state} | \`${s.path}\` | ${s.window==='form'?'440 × dialog (clamped 160–680)':sizes[s.window].join(' × ')} | light, dark |`).join('\n')+'\n')

if(metrics.states.some(s=>s.error)) process.exitCode=1

// pnpm dlx --package playwright@1.65.0 -c 'node apps/desktop-v2/docs/migration/reference/capture.mjs'
import { createRequire } from 'node:module'
import { existsSync, mkdirSync, writeFileSync, readFileSync } from 'node:fs'
import { resolve, delimiter } from 'node:path'
import { FIXTURE, installDeterminism } from './determinism.mjs'
const bin = process.env.PATH.split(delimiter).find(p => existsSync(resolve(p,'playwright')))
if (!bin) throw Error('Run through pnpm dlx --package playwright@1.65.0 -c ...')
const { chromium } = createRequire(resolve(bin,'../package.json'))('playwright')
const root = new URL('./',import.meta.url)
const browser = await chromium.launch({headless:true})
const base = process.env.REFERENCE_URL ?? 'http://localhost:3042'
const metrics = { schemaVersion:1, fixture:FIXTURE, browser:browser.version(), scale:2, states:[] }
const mock = existsSync(new URL('fixtures/mock-state.json',root)) ? JSON.parse(readFileSync(new URL('fixtures/mock-state.json',root))) : null
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
 if(spec.window==='form') { const h=Math.min(640,Math.ceil(await p.getByRole('dialog').evaluate(e=>e.getBoundingClientRect().height))+24); await p.setViewportSize({width,height:h}) }
 if(process.env.INSPECT==='1') { console.log(spec.state,await p.locator('body').innerText()); console.log(await p.locator('button,input,[role="option"]').evaluateAll(es=>es.map(e=>({tag:e.tagName,role:e.getAttribute('role'),label:e.getAttribute('aria-label'),text:e.textContent?.slice(0,60)})))); break }
 const landmarks=await p.evaluate(()=>{
 const selector='button,input,textarea,[role],header,aside,nav,h1,h2,h3,.cm-editor,.kz-chip,[data-slot]'
 return [...document.querySelectorAll(selector)].flatMap((e,i)=>{
 const r=e.getBoundingClientRect(); if(!r.width||!r.height||r.bottom<0||r.top>innerHeight) return []
 const c=getComputedStyle(e)
 return [{key:`${e.tagName.toLowerCase()}:${i}`,role:e.getAttribute('role'),slot:e.getAttribute('data-slot'),label:e.getAttribute('aria-label')||e.getAttribute('placeholder')||e.textContent?.trim().slice(0,100),rect:{x:r.x,y:r.y,width:r.width,height:r.height},color:c.color,background:c.backgroundColor,fontSize:c.fontSize,fontFamily:c.fontFamily,fontWeight:c.fontWeight,borderRadius:c.borderRadius,padding:c.padding,boxShadow:c.boxShadow}]
 })
 })
 const tokens=await p.evaluate(()=>{const c=getComputedStyle(document.documentElement);return Object.fromEntries(['background','foreground','primary','secondary','muted','muted-foreground','accent','border','input','ring','sidebar','iris','surface','hairline'].map(k=>[k,c.getPropertyValue('--'+k).trim()]))})
 mkdirSync(new URL(spec.window+'/',root),{recursive:true})
 const file=`${spec.window}/${spec.state}.${theme}.png`
 await p.screenshot({path:new URL(file,root).pathname,animations:'disabled',caret:'hide',scale:'device'})
 metrics.states.push({window:spec.window,scene:spec.state,theme,file,url:spec.path,viewport:p.viewportSize(),landmarks,tokens})
 console.log(file,landmarks.length)
 if(!existsSync(new URL('fixtures/color-inputs.json',root))) {
 const inputs=await p.evaluate(()=>['#000','#fff','#6366f1','#ff000080','rebeccapurple','hsl(120 100% 50%)','oklch(0.7 0.1 200)','transparent','invalid-color'].map(css=>{const c=document.createElement('canvas').getContext('2d');c.fillStyle='#000';c.fillStyle=css;c.fillRect(0,0,1,1);return {css,rgba:[...c.getImageData(0,0,1,1).data].map(v=>v/255)}}))
 writeFileSync(new URL('fixtures/color-inputs.json',root),JSON.stringify(inputs,null,2)+'\n')
 }
 } catch(error) { metrics.states.push({window:spec.window,scene:spec.state,theme,error:String(error)}); console.error(spec.state,error) }
 await context.close()
}
await browser.close()
if(process.env.ONLY && existsSync(new URL('metrics.json',root))) {
 const previous=JSON.parse(readFileSync(new URL('metrics.json',root)))
 const replaced=new Set(metrics.states.map(s=>`${s.window}/${s.scene}/${s.theme}`))
 metrics.states=[...previous.states.filter(s=>!replaced.has(`${s.window}/${s.scene}/${s.theme}`)),...metrics.states]
}
writeFileSync(new URL('metrics.json',root),JSON.stringify(metrics,null,2)+'\n')
writeFileSync(new URL('STATES.md',root),'# Reference scenes\n\nAll dimensions are logical points; PNGs are 2×. Scene names are scoped by window.\n\n| Window | Scene | Preview path | Size | Themes |\n|---|---|---|---|---|\n'+stateList.map(s=>`| ${s.window} | ${s.state} | \`${s.path}\` | ${s.window==='form'?'440 × content (max 640)':sizes[s.window].join(' × ')} | light, dark |`).join('\n')+'\n')

if(metrics.states.some(s=>s.error)) process.exitCode=1

import { readFileSync } from 'node:fs'

export const landmarkColumns = ['role', 'name', 'occurrence', 'rect', 'style']
export const dictionaries = () => ({ colors: [], shadows: [], fonts: [], colorSources: [], styles: [] })
export function intern(table, value) {
  const encoded = JSON.stringify(value)
  const found = table.findIndex(item => JSON.stringify(item) === encoded)
  if (found !== -1) return found
  table.push(value)
  return table.length - 1
}

// Only the base themes are used in browser preview; vibrant overrides belong to native QA.
export function authoredTokenSources() {
  const css = readFileSync(new URL('../../../../desktop/app/globals.css', import.meta.url), 'utf8')
  const declarations = selector => {
    const body = css.match(new RegExp(`^${selector}\\s*\\{([\\s\\S]*?)\\n\\}`, 'm'))?.[1]
    if (!body) throw Error(`Missing CSS theme ${selector}`)
    return Object.fromEntries([...body.matchAll(/(--[\w-]+):\s*([^;]+);/g)].map(([, name, text]) => [name, text.trim()]))
  }
  const light = declarations(':root')
  return { light, dark: { ...light, ...declarations('\\.dark') } }
}

// Runs in the page. No DOM indices or implementation-only containers enter the result.
export function extractLandmarks() {
  const normalize = text => (text ?? '').replace(/\s+/g, ' ').trim()
  const ownText = e => normalize([...e.childNodes].filter(n => n.nodeType === Node.TEXT_NODE).map(n => n.textContent).join(' '))
  const nameOf = e => {
    const aria = e.getAttribute('aria-label')
    if (aria) return normalize(aria)
    const labelled = e.getAttribute('aria-labelledby')
    if (labelled) return normalize(labelled.split(/\s+/).map(id => document.getElementById(id)?.textContent ?? '').join(' '))
    if (e.labels?.length) return normalize([...e.labels].map(l => l.textContent).join(' '))
    const direct = ownText(e)
    if (direct) return direct
    // Button labels commonly wrap their text in a span beside an icon/count/keycap.
    // Read the single label span, never the aggregate text of the control's descendants.
    const spans = [...e.children].filter(c => c.tagName === 'SPAN' && !c.matches('[aria-hidden="true"], [data-slot="kbd"], [data-slot="shortcut"]'))
    return spans.map(ownText).find(Boolean) ?? ''
  }
  const round = value => Math.round(value * 1000) / 1000
  const length = value => value.endsWith('px') ? round(parseFloat(value)) : value
  const rgba = css => {
    const context = document.createElement('canvas').getContext('2d', { willReadFrequently: true })
    context.clearRect(0, 0, 1, 1)
    context.fillStyle = css
    context.fillRect(0, 0, 1, 1)
    const [r,g,b,a] = context.getImageData(0,0,1,1).data
    return [r,g,b,round(a/255)]
  }
  const landmarks = [], occurrences = new Map()
  const namedRoles = new Set(['region','navigation','banner','complementary','main','contentinfo','dialog','form','search'])
  const selector = 'button,input,textarea,select,h1,h2,h3,h4,h5,h6,header,aside,nav,main,footer,section,form,[role],.kz-chip'
  for (const e of document.querySelectorAll(selector)) {
    if (e.closest('[hidden]')) continue
    const rect = e.getBoundingClientRect(), style = getComputedStyle(e)
    if (!rect.width || !rect.height || rect.bottom <= 0 || rect.top >= innerHeight || rect.right <= 0 || rect.left >= innerWidth || style.visibility === 'hidden') continue
    let role = e.getAttribute('role')
    const tag = e.tagName.toLowerCase()
    if (e.matches('.kz-chip')) role = 'template-chip'
    else if (/^h[1-6]$/.test(tag)) role = 'heading'
    else if (tag === 'button') role ||= 'button'
    else if (['input','textarea','select'].includes(tag)) role ||= tag === 'select' ? 'combobox' : e.type === 'search' ? 'searchbox' : ['checkbox','radio','range'].includes(e.type) ? (e.type === 'range' ? 'slider' : e.type) : 'textbox'
    else {
      role ||= ({header:'banner',aside:'complementary',nav:'navigation',main:'main',footer:'contentinfo',section:'region',form:'form'})[tag]
      if (!namedRoles.has(role) && !['button','textbox','searchbox','checkbox','radio','slider','switch','combobox','spinbutton','heading'].includes(role)) continue
    }
    const name = nameOf(e)
    if (!name) continue
    const identity = JSON.stringify([role,name])
    const occurrence = (occurrences.get(identity) ?? 0) + 1
    occurrences.set(identity, occurrence)
    landmarks.push({ role, name, occurrence, rect:[rect.x,rect.y,rect.width,rect.height].map(round), type:[style.fontFamily,length(style.fontSize),Number(style.fontWeight)], radius:[style.borderTopLeftRadius,style.borderTopRightRadius,style.borderBottomRightRadius,style.borderBottomLeftRadius].map(length), padding:[style.paddingTop,style.paddingRight,style.paddingBottom,style.paddingLeft].map(length), paint:[rgba(style.color),rgba(style.backgroundColor),style.boxShadow] })
  }
  return landmarks
}

export function packLandmarks(landmarks, strings) {
  return landmarks.map(l => {
    const style = [intern(strings.fonts,l.type[0]),...l.type.slice(1),l.radius,l.padding,intern(strings.colors,l.paint[0]),intern(strings.colors,l.paint[1]),intern(strings.shadows,l.paint[2])]
    return [l.role,l.name,l.occurrence,l.rect,intern(strings.styles,style)]
  })
}

export function resolveTokens(sources) {
  const computed = getComputedStyle(document.documentElement)
  const context = document.createElement('canvas').getContext('2d', { willReadFrequently:true })
  return Object.entries(sources).flatMap(([name,source]) => {
    const color = computed.getPropertyValue(name).trim()
    if (!CSS.supports('color',color)) return []
    context.clearRect(0,0,1,1)
    context.fillStyle = color
    context.fillRect(0,0,1,1)
    const [r,g,b,a] = context.getImageData(0,0,1,1).data
    return [[name,source,[r,g,b,Math.round(a/255*1000)/1000]]]
  })
}

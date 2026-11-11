// Records orderings from the runtime's own ICU `String.prototype.localeCompare`, the function the
// frozen Tauri app uses to break search ties and order sibling groups. S21's Rust collator
// (`icu_collator`, root locale, tertiary strength) must reproduce them.
//
//   node apps/desktop-v2/docs/migration/reference/collation.mjs
//   node apps/desktop-v2/docs/migration/reference/collation.mjs --check   # fails if the file is stale
//
// Deterministic: a fixed corpus, an explicit locale, a stable sort, no clock or randomness. The
// output depends only on the ICU/CLDR version of the Node that runs it, recorded in the file.
// Copy the result into crates/kudzu-core/tests/fixtures/ts-parity/collation.json.
import { readFileSync, writeFileSync } from 'node:fs'
import assert from 'node:assert/strict'

const OUT = new URL('./fixtures/collation.json', import.meta.url)
const LOCALE = 'en-US'

const groups = {
  ascii: ['', ' ', 'a', 'A', 'b', 'B', 'ab', 'aB', 'Ab', 'AB', 'a b', 'a-b', 'a_b', 'a.b', 'a,b', 'a/b', 'a1', 'a10', 'a2', 'a 1', 'x', 'X', 'xx', 'Xx', 'xX', 'XX', 'x x', 'x\tx', 'x\nx', 'x\r\nx', 'zebra', 'Zebra'],
  digits: ['0', '1', '2', '9', '10', '11', '100', '1a', '1A', '٣', '٣٤', '３', '３４', '²', '½', '①', '१२', '0 0', '00', '007', '7'],
  punctuationAndSymbols: ['_under', '-dash', ';semi', ':colon', '!bang', '?q', '.dot', "'single'", '"quoted"', '(paren)', '[brack]', '{brace}', '@home', '*star', '\\back', '/slash', '&amp', '#tag', '%pct', '`tick', '^caret', '+plus', '<lt', '=eq', '>gt', '|pipe', '~tilde', '$5', '£5', '€5', '¥5', '¢5', '©', '®', '™', '°', '§', '¶', '•', '…', '—', '–', '‑', '«', '»', '“q”', '‘q’', '¿', '¡', '×', '÷', '±', '←', '→', '∞', '√', '★', '☆', '♥', '✓', '✔', '№', '¬', '¦', 'a…b', 'a—b', 'a–b', 'a-b-c', 'a - b'],
  emoji: ['😀', '😀 smile', '😃', '🎉', '👍', '👍🏽', '👍🏿', '👨‍👩‍👧', '👩‍💻', '🇺🇸', '🇩🇪', '❤️', '❤', '🔥', '🚀', 'a😀', 'a😀b', 'a🎉b', '😀a', '☺', '☺️', '🙂'],
  latinAccents: ['cafe', 'café', 'Café', 'cafe 1', 'café 1', 'cafés', 'resume', 'résumé', 'Resume', 'rèsume', 'resumé', 'Zebra', 'Ångström', 'angstrom', 'Angstrom', 'ångström', 'naïve', 'naive', 'Naïve', 'e', 'é', 'è', 'ê', 'ë', 'ē', 'ě', 'ẽ', 'ȩ', 'E', 'É', 'a', 'á', 'à', 'â', 'ä', 'ã', 'å', 'ā', 'ă', 'ą', 'o', 'ó', 'ò', 'ô', 'ö', 'õ', 'ø', 'ō', 'ő', 'u', 'ú', 'ù', 'û', 'ü', 'ū', 'ů', 'ű', 'c', 'ç', 'ć', 'č', 'n', 'ñ', 'ń', 'ň', 's', 'ś', 'š', 'ş', 'z', 'ź', 'ž', 'ż', 'l', 'ł', 'd', 'đ', 'ď', 'ð', 'þ', 'thorn', 'æ', 'Æ', 'œ', 'Œ', 'ae', 'oe', 'aether', 'æther', 'ether', 'ß', 'ẞ', 'ss', 'sz', 'st', 'sa', 'sb', 'straße', 'strasse', 'Strasse', 'STRASSE', 'strasze', 'Straße', 'strassf', 'ﬁ', 'fi', 'ﬂ', 'fl', 'ǆ', 'dž', 'Ǆ', 'ĳ', 'ij', 'IJ', 'é', 'é', 'è', 'Å', 'Å', 'Å', 'ñ', 'ñ', 'ǭ', 'ǭ'],
  german: ['Müller', 'Mueller', 'Muller', 'Möller', 'Moller', 'Mäder', 'Mader', 'Schön', 'Schoen', 'Schon', 'Äpfel', 'Apfel', 'Apple', 'Übung', 'Ubung', 'Uebung', 'Österreich', 'Osterreich', 'Oster', 'Fuß', 'Fuss', 'Fuse', 'groß', 'gross', 'Gross', 'grosse', 'große', 'Große', 'weiß', 'weiss', 'Weiß', 'Maße', 'Masse', 'Maße 2', 'Strauß', 'Strauss', 'Strausse', 'Straße 1', 'Strasse 1'],
  french: ['côte', 'cote', 'côté', 'coté', 'Côte', 'COTE', 'Cote', 'élève', 'eleve', 'Élève', 'à la carte', 'a la carte', 'où', 'ou', 'Où', 'ça', 'ca', 'Ça', 'naïveté', 'naivete', 'fiancé', 'fiancée', 'fiance', 'œuf', 'oeuf', 'Œuf', 'cœur', 'coeur', 'sœur', 'soeur', 'être', 'etre', 'Être', 'garçon', 'garcon', 'reçu', 'recu', 'forêt', 'foret', 'hôtel', 'hotel', 'coupé', 'coupe', 'coupée'],
  turkish: ['i', 'I', 'ı', 'İ', 'istanbul', 'Istanbul', 'İstanbul', 'ıstanbul', 'ISTANBUL', 'İSTANBUL', 'Işık', 'ışık', 'isik', 'Isik', 'ısı', 'isi', 'İsim', 'Isim', 'ğ', 'Ğ', 'g', 'G', 'gül', 'güneş', 'ğül', 'ş', 'Ş', 'ç', 'Ç', 'ö', 'Ö', 'ü', 'Ü', 'çay', 'cay', 'çöp', 'şeker', 'seker', 'Şeker', 'i̇', 'İ', 'dotted İ', 'dotless ı'],
  cjk: ['中文', '中国', '日本語', '日本', '漢字', '汉字', '漢', '汉', '你好', '世界', '世', '界', '一', '二', '三', '十', '人', '大', '小', '山', '川', 'ひらがな', 'ヒラガナ', 'カタカナ', 'かたかな', 'ｶﾀｶﾅ', 'あ', 'ア', 'ｱ', 'い', 'イ', 'う', 'か', 'カ', 'が', 'ガ', 'か゛', 'ー', '・', 'っ', 'ッ', 'つ', 'ツ', '한국어', '한글', '조선', '가', '나', '다', '각', '갂', '힣', 'ᄀ', 'ᅡ', '漢字a', 'a漢字', '日本語 text', 'text 日本語', '１２３', 'ａｂｃ', 'ＡＢＣ', 'abc', 'ABC'],
  otherScripts: ['α', 'β', 'γ', 'Ω', 'ω', 'ά', 'Α', 'я', 'Я', 'ж', 'а', 'б', 'в', 'ё', 'е', 'ё́', 'א', 'ב', 'ת', 'ا', 'ب', 'ت', 'ع', 'ي', 'ء', 'ة', 'ه', 'العربية', 'عربي', 'مرحبا', 'ก', 'ข', 'ไทย', 'अ', 'क', 'हिन्दी', 'ա', 'ბ', 'ሀ', 'ᚠ'],
  names: ['Email signature 0 café', 'Email signature 10 العربية', 'Email signature 1008 café', 'Email signature 102 😀', 'Email signature 1026 İstanbul', 'Thanks 12 café', 'Thanks 120 café', 'Thanks 1002 İstanbul', 'Thanks 102 İstanbul', 'Thanks 1020 café', 'Sign-up form', 'Sign up form', 'Signup form', 'Sign_up', 'Bug report 1015 😀', 'Bug report 1033 العربية', 'Git commit co-author', 'Git commit coauthor', 'Git commit co author', 'Untitled', 'untitled', 'Quote the selection', 'Request ID', 'request id', 'Lorem ipsum', 'Home address', 'home Address'],
  ignorable: ['a​b', 'ab', 'a­b', 'a\u0000b', 'a\u0007b', 'a‍b', 'a﻿b', '​', '­', 'a⁠b', 'a b', 'a b', 'a b', 'a　b', 'a b'],
}

const corpus = []
const seen = new Set()
for (const [group, words] of Object.entries(groups)) {
  for (const w of words) if (!seen.has(w)) { seen.add(w); corpus.push(w) }
}

const cmp = (a, b) => a.localeCompare(b, LOCALE)
const sorted = [...corpus].sort(cmp) // stable, so equal strings keep corpus order
let rank = 0
const entries = sorted.map((s, i) => {
  if (i > 0 && cmp(sorted[i - 1], s) !== 0) rank++
  return [s, rank]
})
// localeCompare must be a consistent total preorder for the ranks to mean anything.
for (let i = 0; i < sorted.length; i++) {
  for (let j = i; j < sorted.length; j++) {
    const c = Math.sign(cmp(sorted[i], sorted[j]))
    assert.equal(c, Math.sign(entries[i][1] - entries[j][1]), `${JSON.stringify(sorted[i])} vs ${JSON.stringify(sorted[j])}`)
  }
}

const out = JSON.stringify({
  comment: 'Generated by docs/migration/reference/collation.mjs from String.prototype.localeCompare. entries: [string, rank], ascending; equal ranks compare equal.',
  locale: LOCALE,
  icu: process.versions.icu,
  unicode: process.versions.unicode,
  cldr: process.versions.cldr,
  groups: Object.fromEntries(Object.entries(groups).map(([k, v]) => [k, v.length])),
  entries,
}, null, 0).replace(/\],\[/g, '],\n[').replace('"entries":[[', '"entries":[\n[') + '\n'

if (process.argv.includes('--check')) {
  assert.equal(readFileSync(OUT, 'utf8'), out, 'collation.json is stale: rerun collation.mjs')
  console.log('collation.json is current')
} else {
  writeFileSync(OUT, out)
  console.log(`wrote ${entries.length} strings in ${rank + 1} ranks (ICU ${process.versions.icu})`)
}

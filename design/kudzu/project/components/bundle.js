/* @ds-bundle: {"format":4,"namespace":"Kudzu","components":[{"name":"Icon"},{"name":"Kbd"},{"name":"Button"},{"name":"IconButton"},{"name":"ToolbarGroup"},{"name":"SearchField"},{"name":"SidebarItem"},{"name":"SidebarSection"},{"name":"TriggerChip"},{"name":"SnippetRow"},{"name":"ListHeader"},{"name":"Field"},{"name":"TextInput"},{"name":"TriggerInput"},{"name":"Editor"},{"name":"Segmented"},{"name":"Select"},{"name":"Toggle"},{"name":"Stepper"},{"name":"SettingsGroup"},{"name":"SettingsRow"},{"name":"StatTile"},{"name":"UsageBar"},{"name":"Menu"},{"name":"Badge"},{"name":"ListRow"},{"name":"EmptyState"},{"name":"Disclosure"},{"name":"AppShell"}]} */
(function () {
  var React = window.React;
  var h = React.createElement;
  var useState = React.useState;

  function cx() {
    var out = [];
    for (var i = 0; i < arguments.length; i++) if (arguments[i]) out.push(arguments[i]);
    return out.join(' ');
  }
  function rest(props, keys) {
    var o = {};
    for (var k in props) if (Object.prototype.hasOwnProperty.call(props, k) && keys.indexOf(k) < 0) o[k] = props[k];
    return o;
  }

  /* 16 x 16 grid, 1.5 stroke, round caps and joins. Drawn for Kudzu. */
  var ICONS = {
    search: ['c:7:7:4.5', 'M10.5 10.5 14 14'],
    plus: ['M8 3v10', 'M3 8h10'],
    star: ['M8 1.9l1.85 3.76 4.15.6-3 2.93.71 4.13L8 11.38l-3.71 1.94.71-4.13-3-2.93 4.15-.6z'],
    'star-fill': ['F:M8 1.9l1.85 3.76 4.15.6-3 2.93.71 4.13L8 11.38l-3.71 1.94.71-4.13-3-2.93 4.15-.6z'],
    clock: ['c:8:8:6', 'M8 4.75V8l2.25 1.5'],
    chart: ['M3 13.25V8.5', 'M8 13.25V2.75', 'M13 13.25V6'],
    folder: ['M2 4.5A1.5 1.5 0 0 1 3.5 3h2.6l1.4 1.5h5A1.5 1.5 0 0 1 14 6v5.5a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 11.5z'],
    'folder-plus': ['M2 4.5A1.5 1.5 0 0 1 3.5 3h2.6l1.4 1.5h5A1.5 1.5 0 0 1 14 6v5.5a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 11.5z', 'M8 6.75v4', 'M6 8.75h4'],
    stack: ['M8 2.25 2.25 5 8 7.75 13.75 5z', 'M2.25 8 8 10.75 13.75 8', 'M2.25 11 8 13.75 13.75 11'],
    sliders: ['M2.75 5h6', 'M12.25 5h1', 'M2.75 11h1', 'M7.25 11h6', 'c:10.5:5:1.75', 'c:5.5:11:1.75'],
    cloud: ['M4.75 12.75h6.75a3 3 0 0 0 .35-5.98A4 4 0 0 0 4.3 7.7a2.55 2.55 0 0 0 .45 5.05z'],
    history: ['M2.75 8a5.25 5.25 0 1 0 1.55-3.72', 'M2.5 2.5v2.75h2.75', 'M8 5.25V8l2 1.25'],
    sort: ['M5 2.75v10.5', 'M2.5 5.25 5 2.75l2.5 2.5', 'M11 13.25V2.75', 'M8.5 10.75 11 13.25l2.5-2.5'],
    tray: ['M8 2.5v7', 'M5.25 6.75 8 9.5l2.75-2.75', 'M2.75 10.5v1.75a1.25 1.25 0 0 0 1.25 1.25h8a1.25 1.25 0 0 0 1.25-1.25V10.5'],
    info: ['c:8:8:6', 'M8 7.25V11', 'M8 5h.01'],
    'chevron-down': ['M4.5 6.25 8 9.75l3.5-3.5'],
    'chevron-right': ['M6.25 4.5 9.75 8l-3.5 3.5'],
    'chevrons-v': ['M5.5 6 8 3.5 10.5 6', 'M5.5 10 8 12.5l2.5-2.5'],
    x: ['M4.5 4.5l7 7', 'M11.5 4.5l-7 7'],
    check: ['M3.5 8.25 6.5 11.25 12.5 5'],
    minus: ['M3.5 8h9'],
    compose: ['M7.25 3H4.5A1.5 1.5 0 0 0 3 4.5v7A1.5 1.5 0 0 0 4.5 13h7a1.5 1.5 0 0 0 1.5-1.5V8.75', 'M11.25 2.5l2.25 2.25L8.25 10H6V7.75z'],
    bug: ['M5.5 7a2.5 2.5 0 0 1 5 0v2.5a2.5 2.5 0 0 1-5 0z', 'M6.25 4.75 5.25 3.25', 'M9.75 4.75l1-1.5', 'M2.75 8.25H5.5', 'M10.5 8.25h2.75', 'M3.25 12l2.4-1.2', 'M12.75 12l-2.4-1.2'],
    sidebar: ['M3.5 3h9A1.5 1.5 0 0 1 14 4.5v7a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 11.5v-7A1.5 1.5 0 0 1 3.5 3z', 'M6.25 3v10'],
    bolt: ['M9.25 1.75 3.75 9H8l-1.25 5.25L12.25 7H8z'],
    type: ['M2.75 4.25h7', 'M6.25 4.25v8.5', 'M9.75 8.5h3.5', 'M11.5 8.5v4.25'],
    timer: ['c:8:9.25:5', 'M8 6.75v2.5', 'M6.5 2.25h3'],
    braces: ['M5.75 2.75c-1.25 0-1.75.5-1.75 1.75v1.25c0 1-.5 1.75-1.5 2.25 1 .5 1.5 1.25 1.5 2.25v1.25c0 1.25.5 1.75 1.75 1.75', 'M10.25 2.75c1.25 0 1.75.5 1.75 1.75v1.25c0 1 .5 1.75 1.5 2.25-1 .5-1.5 1.25-1.5 2.25v1.25c0 1.25-.5 1.75-1.75 1.75'],
    keyboard: ['M3 4h10a1.25 1.25 0 0 1 1.25 1.25v5.5A1.25 1.25 0 0 1 13 12H3a1.25 1.25 0 0 1-1.25-1.25v-5.5A1.25 1.25 0 0 1 3 4z', 'M4.5 6.5h.01', 'M7 6.5h.01', 'M9.5 6.5h.01', 'M12 6.5h-.5', 'M5 9.5h6'],
    trash: ['M2.75 4.5h10.5', 'M6.25 4.5V3h3.5v1.5', 'M4.25 4.5l.6 8.1a1.5 1.5 0 0 0 1.5 1.4h3.3a1.5 1.5 0 0 0 1.5-1.4l.6-8.1'],
    leaf: ['M2.75 13.25C2.75 7.25 6.5 2.75 13.25 2.75c0 6.75-4.5 10.5-10.5 10.5z', 'M2.75 13.25 9.25 6.75'],
    more: ['M3.75 8h.01', 'M8 8h.01', 'M12.25 8h.01'],
    app: ['M3.5 2.75h9a1.25 1.25 0 0 1 1.25 1.25v8a1.25 1.25 0 0 1-1.25 1.25h-9A1.25 1.25 0 0 1 2.25 12V4A1.25 1.25 0 0 1 3.5 2.75z', 'M2.25 5.75h11.5']
  };

  function Icon(props) {
    var size = props.size || 16;
    var parts = ICONS[props.name] || ICONS.info;
    var kids = parts.map(function (d, i) {
      if (d.indexOf('c:') === 0) {
        var a = d.split(':');
        return h('circle', { key: i, cx: a[1], cy: a[2], r: a[3] });
      }
      if (d.indexOf('F:') === 0) return h('path', { key: i, d: d.slice(2), className: 'kz-fill' });
      return h('path', { key: i, d: d });
    });
    return h('svg', {
      className: cx('kz-icon', props.className),
      width: size, height: size, viewBox: '0 0 16 16',
      'aria-hidden': props.label ? undefined : true,
      'aria-label': props.label, role: props.label ? 'img' : undefined,
      style: props.style
    }, kids);
  }
  Icon.names = Object.keys(ICONS);

  function Kbd(props) {
    return h('kbd', { className: cx('kz-kbd', props.className) }, props.children);
  }

  function Button(props) {
    var variant = props.variant || 'secondary';
    var o = rest(props, ['variant', 'size', 'icon', 'iconEnd', 'children', 'className']);
    return h('button', Object.assign({ type: 'button' }, o, {
      className: cx('kz-btn', 'kz-btn-' + variant, props.size === 'sm' && 'kz-btn-sm', props.className)
    }),
      props.icon ? h(Icon, { name: props.icon, size: props.size === 'sm' ? 14 : 16 }) : null,
      props.children != null ? h('span', null, props.children) : null,
      props.iconEnd ? h(Icon, { name: props.iconEnd, size: 12, className: 'kz-btn-end' }) : null);
  }

  function IconButton(props) {
    var o = rest(props, ['icon', 'label', 'menu', 'active', 'className', 'size', 'tone']);
    return h('button', Object.assign({ type: 'button', 'aria-label': props.label, title: props.label, 'aria-pressed': props.active ? true : undefined }, o, {
      className: cx('kz-iconbtn', props.menu && 'kz-iconbtn-menu', props.active && 'is-active', props.size === 'sm' && 'kz-iconbtn-sm', props.tone === 'bloom' && 'kz-iconbtn-bloom', props.className)
    }),
      h(Icon, { name: props.icon, size: props.size === 'sm' ? 14 : 16 }),
      props.menu ? h(Icon, { name: 'chevron-down', size: 10, className: 'kz-iconbtn-caret' }) : null);
  }

  function ToolbarGroup(props) {
    return h('div', { className: cx('kz-toolbar', props.className), role: 'toolbar' }, props.children);
  }

  function SearchField(props) {
    var o = rest(props, ['shortcut', 'className', 'placeholder']);
    return h('label', { className: cx('kz-search', props.className) },
      h(Icon, { name: 'search', size: 14 }),
      h('input', Object.assign({ type: 'search', placeholder: props.placeholder || 'Search snippets' }, o)),
      props.shortcut === false ? null : h(Kbd, null, props.shortcut || '⌘F'));
  }

  function SidebarItem(props) {
    var o = rest(props, ['icon', 'label', 'count', 'active', 'shortcut', 'className']);
    return h('button', Object.assign({ type: 'button', 'aria-current': props.active ? 'page' : undefined }, o, {
      className: cx('kz-side-item', props.active && 'is-active', props.className)
    }),
      props.icon ? h(Icon, { name: props.icon }) : null,
      h('span', { className: 'kz-side-label' }, props.label),
      props.count != null ? h('span', { className: 'kz-side-count' }, props.count) : null,
      props.shortcut ? h(Kbd, null, props.shortcut) : null);
  }

  function SidebarSection(props) {
    return h('div', { className: cx('kz-side-section', props.className) },
      props.label ? h('div', { className: 'kz-side-heading' }, h('span', null, props.label), props.action || null) : null,
      h('div', { className: 'kz-side-items' }, props.children));
  }

  function TriggerChip(props) {
    var text = String(props.children == null ? '' : props.children);
    var lead = /^[^A-Za-z0-9]/.test(text) ? text.charAt(0) : '';
    var body = lead ? text.slice(1) : text;
    return h('span', { className: cx('kz-chip', props.removable && 'is-removable', props.className) },
      lead ? h('span', { className: 'kz-chip-lead' }, lead) : null,
      h('span', null, body),
      props.removable ? h('button', { type: 'button', className: 'kz-chip-x', 'aria-label': 'Remove ' + text, onClick: props.onRemove }, h(Icon, { name: 'x', size: 10 })) : null);
  }

  function SnippetRow(props) {
    var triggers = props.triggers || [];
    var o = rest(props, ['name', 'triggers', 'preview', 'group', 'uses', 'favorite', 'selected', 'className']);
    return h('div', Object.assign({ role: 'option', 'aria-selected': !!props.selected, tabIndex: 0 }, o, {
      className: cx('kz-snip', props.selected && 'is-selected', props.className)
    }),
      h('div', { className: 'kz-snip-top' },
        h('span', { className: 'kz-snip-name' }, props.name),
        props.favorite ? h(Icon, { name: 'star-fill', size: 12, className: 'kz-fav', label: 'Favorite' }) : null,
        h('span', { className: 'kz-grow' }),
        props.group ? h('span', { className: 'kz-snip-group' }, props.group) : null,
        props.uses ? h('span', { className: 'kz-snip-uses', title: props.uses + ' expansions' }, h(Icon, { name: 'bolt', size: 11 }), props.uses) : null),
      h('div', { className: 'kz-snip-bottom' },
        triggers.map(function (t) { return h(TriggerChip, { key: t }, t); }),
        props.preview ? h('span', { className: 'kz-snip-preview' }, props.preview) : null));
  }

  function ListHeader(props) {
    return h('div', { className: cx('kz-listhead', props.className) },
      h('div', { className: 'kz-listhead-text' },
        h('div', { className: 'kz-listhead-title' }, props.title),
        props.subtitle ? h('div', { className: 'kz-listhead-sub' }, props.subtitle) : null),
      props.actions ? h('div', { className: 'kz-listhead-actions' }, props.actions) : null);
  }

  function Field(props) {
    return h('div', { className: cx('kz-field', props.className) },
      h('div', { className: 'kz-field-head' },
        h('label', { className: 'kz-field-label', htmlFor: props.htmlFor }, props.label),
        props.aside ? h('span', { className: 'kz-field-aside' }, props.aside) : null),
      props.children,
      props.hint ? h('div', { className: 'kz-field-hint' }, props.hint) : null);
  }

  function TextInput(props) {
    var o = rest(props, ['mono', 'className']);
    return h('input', Object.assign({ type: 'text' }, o, { className: cx('kz-input', props.mono && 'is-mono', props.className) }));
  }

  function TriggerInput(props) {
    var init = props.triggers || [];
    var st = useState(init);
    var list = st[0], setList = st[1];
    var d = useState('');
    var draft = d[0], setDraft = d[1];
    return h('div', { className: cx('kz-input', 'kz-trigger-input', props.className) },
      list.map(function (t) {
        return h(TriggerChip, { key: t, removable: true, onRemove: function () { setList(list.filter(function (x) { return x !== t; })); } }, t);
      }),
      h('input', {
        value: draft, placeholder: props.placeholder || 'Add trigger…', 'aria-label': 'Add trigger',
        onChange: function (e) { setDraft(e.target.value); },
        onKeyDown: function (e) {
          if ((e.key === 'Enter' || e.key === ' ') && draft.trim()) { e.preventDefault(); if (list.indexOf(draft.trim()) < 0) setList(list.concat([draft.trim()])); setDraft(''); }
          if (e.key === 'Backspace' && !draft && list.length) setList(list.slice(0, -1));
        }
      }));
  }

  function Editor(props) {
    var parts = props.value || [];
    return h('div', { className: cx('kz-editor', props.className) },
      h('div', { className: 'kz-editor-bar' },
        h(Button, { variant: 'ghost', size: 'sm', icon: 'plus', iconEnd: 'chevron-down' }, 'Insert'),
        h('span', { className: 'kz-grow' }),
        h('span', { className: 'kz-editor-hint' }, 'Type ', h(Kbd, null, '/'), ' to insert a tag')),
      h('div', { className: 'kz-editor-body', contentEditable: props.readOnly ? undefined : true, suppressContentEditableWarning: true, style: props.minHeight ? { minHeight: props.minHeight } : undefined },
        parts.map(function (p, i) {
          if (typeof p === 'string') return h(React.Fragment, { key: i }, p);
          return h('span', { key: i, className: 'kz-tag', contentEditable: false }, p.tag);
        })));
  }

  function Segmented(props) {
    var opts = props.options || [];
    var st = useState(props.value != null ? props.value : (opts[0] && (opts[0].value || opts[0])));
    var val = st[0], set = st[1];
    return h('div', { className: cx('kz-seg', props.className), role: 'radiogroup', 'aria-label': props.label },
      opts.map(function (o) {
        var v = o.value || o, l = o.label || o;
        return h('button', { key: v, type: 'button', role: 'radio', 'aria-checked': val === v, className: cx('kz-seg-opt', val === v && 'is-on'), onClick: function () { set(v); if (props.onChange) props.onChange(v); } }, l);
      }));
  }

  function Select(props) {
    var o = rest(props, ['value', 'className']);
    return h('button', Object.assign({ type: 'button', 'aria-haspopup': 'listbox' }, o, { className: cx('kz-select', props.className) }),
      h('span', null, props.value),
      h(Icon, { name: 'chevrons-v', size: 14 }));
  }

  function Toggle(props) {
    var st = useState(!!props.checked);
    var on = st[0], set = st[1];
    return h('button', {
      type: 'button', role: 'switch', 'aria-checked': on, 'aria-label': props.label,
      className: cx('kz-toggle', on && 'is-on', props.className),
      onClick: function () { set(!on); if (props.onChange) props.onChange(!on); }
    }, h('span', { className: 'kz-toggle-knob' }));
  }

  function Stepper(props) {
    var st = useState(props.value != null ? props.value : 0);
    var v = st[0], set = st[1];
    var step = props.step || 1, min = props.min != null ? props.min : -Infinity, max = props.max != null ? props.max : Infinity;
    return h('div', { className: cx('kz-stepper', props.className) },
      h('button', { type: 'button', 'aria-label': 'Decrease', onClick: function () { set(Math.max(min, v - step)); } }, h(Icon, { name: 'minus', size: 12 })),
      h('span', { className: 'kz-stepper-val' }, v, props.unit ? h('span', { className: 'kz-stepper-unit' }, ' ' + props.unit) : null),
      h('button', { type: 'button', 'aria-label': 'Increase', onClick: function () { set(Math.min(max, v + step)); } }, h(Icon, { name: 'plus', size: 12 })));
  }

  function SettingsGroup(props) {
    return h('section', { className: cx('kz-setgroup', props.className) },
      props.title ? h('div', { className: 'kz-setgroup-head' },
        h('div', null,
          h('h3', { className: 'kz-setgroup-title' }, props.title),
          props.description ? h('p', { className: 'kz-setgroup-desc' }, props.description) : null),
        props.action || null) : null,
      h('div', { className: 'kz-setgroup-card' }, props.children));
  }

  function SettingsRow(props) {
    return h('div', { className: cx('kz-setrow', props.className) },
      props.icon ? h(Icon, { name: props.icon, className: 'kz-setrow-icon' }) : null,
      h('div', { className: 'kz-setrow-text' },
        h('div', { className: 'kz-setrow-label' }, props.label),
        props.description ? h('div', { className: 'kz-setrow-desc' }, props.description) : null),
      h('div', { className: 'kz-setrow-control' }, props.children));
  }

  function StatTile(props) {
    return h('div', { className: cx('kz-stat', props.className) },
      h('div', { className: 'kz-stat-label' }, props.icon ? h(Icon, { name: props.icon, size: 14 }) : null, props.label),
      h('div', { className: 'kz-stat-value' }, props.value),
      props.note ? h('div', { className: 'kz-stat-note' }, props.note) : null);
  }

  function UsageBar(props) {
    var pct = props.max ? Math.max(2, Math.round((props.value / props.max) * 100)) : 0;
    return h('div', { className: cx('kz-usage', props.className) },
      h('span', { className: 'kz-usage-label' }, h(TriggerChip, null, props.label)),
      h('span', { className: 'kz-usage-track' }, h('span', { className: 'kz-usage-fill', style: { width: pct + '%' } })),
      h('span', { className: 'kz-usage-val' }, props.value));
  }

  function Menu(props) {
    return h('div', { className: cx('kz-menu', props.className), role: 'menu' },
      props.title ? h('div', { className: 'kz-menu-title' }, props.title) : null,
      (props.items || []).map(function (it, i) {
        if (it === '-' || it.separator) return h('div', { key: i, className: 'kz-menu-sep', role: 'separator' });
        return h('button', { key: i, type: 'button', role: it.checked != null ? 'menuitemradio' : 'menuitem', 'aria-checked': it.checked != null ? !!it.checked : undefined, className: cx('kz-menu-item', it.active && 'is-active', it.danger && 'is-danger') },
          h('span', { className: 'kz-menu-check' }, it.checked ? h(Icon, { name: 'check', size: 14 }) : (it.icon ? h(Icon, { name: it.icon, size: 14 }) : null)),
          h('span', { className: 'kz-grow' }, it.label),
          it.shortcut ? h('span', { className: 'kz-menu-key' }, it.shortcut) : null);
      }));
  }

  function Badge(props) {
    return h('span', { className: cx('kz-badge', 'kz-badge-' + (props.tone || 'neutral'), props.className) }, props.children);
  }

  function ListRow(props) {
    var o = rest(props, ['title', 'meta', 'aside', 'badge', 'selected', 'className']);
    return h('div', Object.assign({ role: 'option', 'aria-selected': !!props.selected, tabIndex: 0 }, o, { className: cx('kz-lrow', props.selected && 'is-selected', props.className) }),
      h('div', { className: 'kz-lrow-text' },
        h('div', { className: 'kz-lrow-title' }, props.title),
        props.meta ? h('div', { className: 'kz-lrow-meta' }, props.meta) : null),
      props.aside ? h('span', { className: 'kz-lrow-aside' }, props.aside) : null,
      props.badge || null);
  }

  function EmptyState(props) {
    return h('div', { className: cx('kz-empty', props.className) },
      h(Icon, { name: props.icon || 'leaf', size: 28, className: 'kz-empty-mark' }),
      h('div', { className: 'kz-empty-title' }, props.title),
      props.children ? h('div', { className: 'kz-empty-body' }, props.children) : null,
      props.action || null);
  }

  function Disclosure(props) {
    var st = useState(props.open != null ? props.open : true);
    var open = st[0], set = st[1];
    return h('div', { className: cx('kz-disc', open && 'is-open', props.className) },
      h('button', { type: 'button', className: 'kz-disc-head', 'aria-expanded': open, onClick: function () { set(!open); } },
        h(Icon, { name: 'chevron-right', size: 12, className: 'kz-disc-caret' }),
        h('span', null, props.title),
        props.summary && !open ? h('span', { className: 'kz-disc-summary' }, props.summary) : null),
      open ? h('div', { className: 'kz-disc-body' }, props.children) : null);
  }

  function AppShell(props) {
    return h('div', { className: cx('kz-window', props.className), style: props.style },
      h('aside', { className: 'kz-sidebar' },
        h('div', { className: 'kz-titlebar kz-sidebar-top' },
          h('span', { className: 'kz-lights', 'aria-hidden': true }, h('i', null), h('i', null), h('i', null)),
          h('span', { className: 'kz-grow' }),
          h(IconButton, { icon: 'sidebar', label: 'Hide sidebar', size: 'sm' })),
        h('div', { className: 'kz-sidebar-body' }, props.sidebar),
        props.sidebarFooter ? h('div', { className: 'kz-sidebar-foot' }, props.sidebarFooter) : null),
      h('section', { className: 'kz-listpane' }, props.list),
      h('main', { className: 'kz-detail' }, props.detail));
  }

  window.Kudzu = Object.assign(window.Kudzu || {}, {
    Icon: Icon, Kbd: Kbd, Button: Button, IconButton: IconButton, ToolbarGroup: ToolbarGroup, SearchField: SearchField,
    SidebarItem: SidebarItem, SidebarSection: SidebarSection, TriggerChip: TriggerChip, SnippetRow: SnippetRow,
    ListHeader: ListHeader, Field: Field, TextInput: TextInput, TriggerInput: TriggerInput, Editor: Editor,
    Segmented: Segmented, Select: Select, Toggle: Toggle, Stepper: Stepper, SettingsGroup: SettingsGroup,
    SettingsRow: SettingsRow, StatTile: StatTile, UsageBar: UsageBar, Menu: Menu, Badge: Badge, ListRow: ListRow,
    EmptyState: EmptyState, Disclosure: Disclosure, AppShell: AppShell
  });
})();

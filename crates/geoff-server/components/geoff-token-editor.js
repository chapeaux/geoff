/**
 * <geoff-token-editor> — Main token editor panel.
 *
 * Fetches tokens from /api/theme/tokens, groups them by category,
 * renders <geoff-token-group> elements, and PUTs changes back.
 * Search/filter, expand/collapse all, deprecation toggle,
 * state preservation across renders (scroll, focus, expanded groups).
 */
class GeoffTokenEditor extends HTMLElement {
  constructor() {
    super();
    const internals = this.attachInternals?.();
    if (!internals?.shadowRoot) {
      this.attachShadow({ mode: 'open' });
    }
    Object.assign(this, {
      _tokens: {}, _resolved: {}, _errors: [], _valid: true,
      _dirty: false, _saving: false, _offline: false, _saveTimer: null,
      _expandedGroups: new Set(), _focusedPath: null, _scrollTop: 0,
      _searchQuery: '', _activeTypeFilters: new Set(), _showDeprecated: false,
      _solidPanelOpen: false, _prefix: '',
    });
  }

  connectedCallback() {
    this.render();
    if (typeof window === 'undefined') return;
    this._fetchTokens();
    this.shadowRoot.addEventListener('token-change', (e) => this._onTokenChange(e));
    this.shadowRoot.addEventListener('add-token', () => { this._dirty = true; this._updateStatusBar(); });
    this.shadowRoot.addEventListener('rename-group', (e) => this._onRenameGroup(e));
    this.shadowRoot.addEventListener('move-token', (e) => this._onMoveToken(e));
    this.shadowRoot.addEventListener('solid-tokens-loaded', (e) => {
      this._tokens = e.detail.tokens; this._dirty = true; this.render(); this._scheduleAutoSave();
    });
    this.shadowRoot.addEventListener('keydown', (e) => {
      const tgt = e.composedPath()[0];
      const inInput = tgt && ['INPUT','SELECT','TEXTAREA'].includes(tgt.tagName);
      if (e.key === '/' && !inInput) { e.preventDefault(); this.shadowRoot.querySelector('.search-input')?.focus(); }
      if (e.key === 'Escape') {
        const si = this.shadowRoot.querySelector('.search-input');
        if (si && this.shadowRoot.activeElement === si) { si.value = ''; this._searchQuery = ''; this._applyFilters(); }
      }
    });
  }

  disconnectedCallback() { clearTimeout(this._saveTimer); }

  async _fetchTokens() {
    try {
      const res = await fetch('/api/theme/tokens');
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();
      this._tokens = data.tokens || {}; this._resolved = data.resolved || {};
      this._prefix = data.prefix || ''; this._offline = false;
    } catch { this._offline = true; this._tokens = this._sampleTokens(); }
    this.render();
  }

  _sampleTokens() {
    return {
      'color-critical': { $type: 'color',
        text: { $value: '#1a1a1a', $type: 'color', $description: 'Primary text color' },
        background: { $value: '#ffffff', $type: 'color', $description: 'Page background' },
        accent: { $value: '#0066cc', $type: 'color', $description: 'Accent / link color' } },
      spacing: { $type: 'dimension',
        small: { $value: '4px', $type: 'dimension' }, medium: { $value: '8px', $type: 'dimension' },
        large: { $value: '16px', $type: 'dimension' } },
      typography: { $type: 'fontFamily',
        body: { $value: 'system-ui, sans-serif', $type: 'fontFamily' },
        heading: { $value: 'system-ui, sans-serif', $type: 'fontFamily' },
        'body-weight': { $value: '400', $type: 'fontWeight' } },
    };
  }

  _groupTokens() {
    const groups = {};
    for (const [key, val] of Object.entries(this._tokens)) {
      if (typeof val === 'object' && val !== null && !val.$value) {
        const type = val.$type || this._inferType(key);
        const children = {};
        this._flattenTokens(val, key, children);
        groups[key] = { type, tokens: children, extensions: val.$extensions || null };
      } else {
        const group = key.split('.')[0] || '_ungrouped';
        if (!groups[group]) groups[group] = { type: '', tokens: {} };
        groups[group].tokens[key] = val;
      }
    }
    return groups;
  }

  _flattenTokens(obj, prefix, out) {
    for (const [k, v] of Object.entries(obj)) {
      if (k.startsWith('$')) continue;
      // "_" is Style Dictionary's convention for a group-level root token — use the parent path
      const path = k === '_' ? prefix : `${prefix}.${k}`;
      if (typeof v === 'object' && v !== null && v.$value !== undefined) {
        out[path] = v;
      } else if (typeof v === 'object' && v !== null) {
        this._flattenTokens(v, path, out);
      } else {
        out[path] = v;
      }
    }
  }

  _inferType(n) {
    if (/color|colour/i.test(n)) return 'color';
    if (/spac|size|gap|margin|padding|radius/i.test(n)) return 'dimension';
    if (/font|typo/i.test(n)) return 'fontFamily';
    if (/shadow/i.test(n)) return 'shadow';
    if (/border/i.test(n)) return 'border';
    if (/duration|delay|transition/i.test(n)) return 'duration';
    return '';
  }

  _totalTokenCount() {
    let n = 0;
    const groups = this._groupTokens();
    for (const g of Object.values(groups)) { n += Object.keys(g.tokens).length; }
    return n;
  }

  _deprecatedTokenCount() {
    let n = 0;
    const walk = (obj) => { for (const [k, v] of Object.entries(obj)) {
      if (k.startsWith('$')) continue;
      if (typeof v === 'object' && v !== null) { if (v.$deprecated) n++; else if (!v.$value) walk(v); }
    }};
    walk(this._tokens); return n;
  }

  _onTokenChange(e) {
    const { path, value } = e.detail;
    this._focusedPath = path;
    this._setNestedValue(this._tokens, path, value);
    this._dirty = true; this._updateStatusBar();
    clearTimeout(this._saveTimer);
    this._saveTimer = setTimeout(() => this._save(), 800);
  }

  _onRenameGroup(e) {
    const { oldName, newName } = e.detail;
    if (!this._tokens[oldName] || this._tokens[newName]) return;
    this._tokens[newName] = this._tokens[oldName]; delete this._tokens[oldName];
    if (this._expandedGroups.has(oldName)) { this._expandedGroups.delete(oldName); this._expandedGroups.add(newName); }
    this._dirty = true; this.render(); this._scheduleAutoSave();
  }

  _onMoveToken(e) {
    const { path, fromGroup, toGroup } = e.detail;
    const tokenKey = path.split('.').slice(1).join('.');
    if (!tokenKey || !this._tokens[fromGroup] || !this._tokens[toGroup]) return;
    const val = this._tokens[fromGroup][tokenKey]; if (val === undefined) return;
    this._tokens[toGroup][tokenKey] = val; delete this._tokens[fromGroup][tokenKey];
    this._dirty = true; this.render(); this._scheduleAutoSave();
  }

  _onCreateGroup(name, type) {
    if (!name || this._tokens[name]) return;
    this._tokens[name] = { $type: type || '' };
    this._expandedGroups.add(name);
    this._dirty = true; this.render(); this._scheduleAutoSave();
  }

  async _savePrefix(prefix) {
    try {
      await fetch('/api/theme/prefix', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ prefix }),
      });
    } catch { /* offline — prefix only affects CSS generation, saved on next full save */ }
    this._emitCSSUpdate();
  }

  _scheduleAutoSave() {
    this._updateStatusBar(); clearTimeout(this._saveTimer);
    this._saveTimer = setTimeout(() => this._save(), 800);
  }

  _setNestedValue(obj, path, value) {
    const parts = path.split('.'); let cur = obj;
    for (let i = 0; i < parts.length - 1; i++) {
      if (!cur[parts[i]] || typeof cur[parts[i]] !== 'object') cur[parts[i]] = {};
      cur = cur[parts[i]];
    }
    const last = parts[parts.length - 1];
    if (typeof cur[last] === 'object' && cur[last] !== null) cur[last].$value = value;
    else cur[last] = value;
  }

  async _save() {
    if (this._offline) { this._dirty = false; this._updateStatusBar(); this._emitCSSUpdate(); return; }
    this._saving = true; this._updateStatusBar();
    try {
      const res = await fetch('/api/theme/tokens', { method: 'PUT',
        headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ tokens: this._tokens }) });
      const data = await res.json();
      this._valid = data.valid !== false; this._errors = data.errors || [];
      if (data.css) this._emitCSSUpdate(data.css); this._applyErrors();
    } catch { this._emitCSSUpdate(); }
    this._saving = false; this._dirty = false; this._updateStatusBar();
  }

  _emitCSSUpdate(css) {
    this.dispatchEvent(new CustomEvent('css-update', { bubbles: true, composed: true,
      detail: { css: css || this._buildFallbackCSS(), tokens: this._tokens } }));
  }

  _buildFallbackCSS() {
    const lines = [':root {']; const walk = (obj, pfx) => {
      for (const [k, v] of Object.entries(obj)) { if (k.startsWith('$')) continue;
        if (typeof v === 'object' && v !== null) { if (v.$value !== undefined) lines.push(`  --${pfx}${k}: ${v.$value};`);
          else walk(v, `${pfx}${k}-`); } else lines.push(`  --${pfx}${k}: ${v};`); }
    }; walk(this._tokens, ''); lines.push('}'); return lines.join('\n');
  }

  _applyErrors() {
    this.shadowRoot.querySelectorAll('geoff-token-field').forEach(f => f.removeAttribute('error'));
    for (const err of this._errors) { if (err.path) {
      const f = this.shadowRoot.querySelector(`geoff-token-field[path="${err.path}"]`);
      if (f) f.setAttribute('error', err.message || 'Invalid');
    }}
  }

  _updateStatusBar() {
    const bar = this.shadowRoot.querySelector('.status-bar'); if (!bar) return;
    const s = this._saving ? 'Saving...' : this._dirty ? 'Unsaved changes' : 'Saved';
    const v = this._valid ? '<span class="valid">Valid</span>'
      : `<span class="invalid">${this._errors.length} error${this._errors.length !== 1 ? 's' : ''}</span>`;
    bar.innerHTML = `${this._offline ? '<span class="offline">Offline</span>' : ''}
      <span class="save-status">${s}</span><span class="spacer"></span>${v}`;
  }

  _buildFilterFn() {
    const q = this._searchQuery.toLowerCase().trim(), types = this._activeTypeFilters;
    if (!q && types.size === 0) return null;
    return (path, tok) => {
      if (types.size > 0) { const t = (typeof tok === 'object' ? tok.$type : '') || ''; if (!types.has(t)) return false; }
      if (q) {
        const cssVar = `--${path.replace(/\./g, '-')}`;
        const val = typeof tok === 'object' ? String(tok.$value ?? tok.value ?? '') : String(tok);
        const desc = (typeof tok === 'object' ? tok.$description : '') || '';
        const r = this._resolved[path];
        if (![path, cssVar, val, desc, r?.value, r?.description].join(' ').toLowerCase().includes(q)) return false;
      }
      return true;
    };
  }

  _matchingTokenCount() {
    const fn = this._buildFilterFn(); if (!fn) return this._totalTokenCount();
    let n = 0;
    for (const g of Object.values(this._groupTokens()))
      for (const [p, t] of Object.entries(g.tokens)) {
        if (!this._showDeprecated && typeof t === 'object' && t.$deprecated) continue;
        if (fn(p, t)) n++;
      }
    return n;
  }

  _applyFilters() {
    const fn = this._buildFilterFn();
    this.shadowRoot.querySelectorAll('geoff-token-group').forEach(el => { el.filter = fn; });
    this._updateSearchCount();
  }

  _updateSearchCount() {
    const el = this.shadowRoot.querySelector('.search-count'); if (!el) return;
    const total = this._totalTokenCount(), dep = this._deprecatedTokenCount();
    const vis = this._showDeprecated ? total : total - dep;
    const fn = this._buildFilterFn();
    el.textContent = (fn || !this._showDeprecated) ? `${this._matchingTokenCount()} of ${vis} tokens` : `${vis} tokens`;
  }

  _saveState() {
    this._expandedGroups = new Set();
    this.shadowRoot.querySelectorAll('geoff-token-group').forEach(el => {
      if (el.expanded) this._expandedGroups.add(el.getAttribute('name'));
    });
    const gd = this.shadowRoot.querySelector('.groups');
    if (gd) this._scrollTop = gd.scrollTop;
  }

  _restoreState() {
    this.shadowRoot.querySelectorAll('geoff-token-group').forEach(el => {
      if (this._expandedGroups.has(el.getAttribute('name'))) el.setAttribute('expanded', '');
    });
    const gd = this.shadowRoot.querySelector('.groups');
    if (gd && this._scrollTop) requestAnimationFrame(() => { gd.scrollTop = this._scrollTop; });
    if (this._focusedPath) requestAnimationFrame(() => {
      for (const g of this.shadowRoot.querySelectorAll('geoff-token-group')) {
        const f = g.shadowRoot?.querySelector(`geoff-token-field[path="${this._focusedPath}"]`);
        if (f) { f.shadowRoot?.querySelector('input, select')?.focus(); break; }
      }
    });
    const si = this.shadowRoot.querySelector('.search-input');
    if (si && this._searchQuery) si.value = this._searchQuery;
  }

  _esc(s) { return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/"/g, '&quot;'); }

  render() {
    this._saveState();
    const groups = this._groupTokens(), gk = Object.keys(groups);
    const depCount = this._deprecatedTokenCount();
    const types = ['color', 'dimension', 'fontFamily', 'shadow', 'fontWeight'];

    this.shadowRoot.innerHTML = `<style>
      :host { display:block; height:100%; overflow-y:auto; font-family:system-ui,sans-serif; color:#333; background:#f5f5f5; }
      .header { padding:12px; border-bottom:1px solid #e0e0e0; background:#fff; position:sticky; top:0; z-index:2; }
      .header-row { display:flex; align-items:center; gap:8px; }
      h2 { margin:0; font-size:16px; font-weight:600; flex:1; }
      .header-actions { display:flex; align-items:center; gap:6px; }
      .header-actions button { font-size:16px; width:28px; height:28px; border:1px solid #e0e0e0; border-radius:4px;
        background:#fff; cursor:pointer; display:flex; align-items:center; justify-content:center; color:#555; }
      .header-actions button:hover { background:#f0f0f0; }
      .download-btn, .create-btn { width:auto !important; font-size:12px !important; padding:4px 10px;
        height:auto !important; white-space:nowrap; }
      .create-btn { background:#0066cc !important; color:#fff !important; border-color:#0066cc !important; }
      .create-btn:hover { background:#0052a3 !important; }
      .dep-toggle { display:flex; align-items:center; gap:4px; font-size:12px; color:#666; cursor:pointer; user-select:none; }
      .dep-toggle input { margin:0; cursor:pointer; }
      .dep-badge { font-size:11px; padding:1px 6px; border-radius:8px; background:#fff3cd; color:#856404; }
      .search-bar { padding:8px 12px; background:#fff; border-bottom:1px solid #e0e0e0; position:sticky; top:49px; z-index:1; }
      .search-input { width:100%; font-size:13px; padding:6px 8px; border:1px solid #e0e0e0; border-radius:4px; box-sizing:border-box; }
      .search-input:focus { outline:none; border-color:#0066cc; box-shadow:0 0 0 2px rgba(0,102,204,.15); }
      .search-meta { display:flex; align-items:center; gap:8px; margin-top:6px; flex-wrap:wrap; }
      .search-count { font-size:11px; color:#888; }
      .type-filters { display:flex; gap:4px; flex-wrap:wrap; }
      .type-pill { font-size:11px; padding:2px 8px; border-radius:10px; border:1px solid #e0e0e0;
        background:#fff; cursor:pointer; color:#555; user-select:none; }
      .type-pill:hover { background:#f0f0f0; }
      .type-pill.active { background:#0066cc; color:#fff; border-color:#0066cc; }
      .offline-banner { background:#fff3cd; color:#856404; padding:8px 12px; font-size:13px; border-bottom:1px solid #ffc107; }
      .groups { padding:8px; overflow-y:auto; }
      .status-bar { display:flex; align-items:center; gap:8px; padding:6px 12px; background:#fff;
        border-top:1px solid #e0e0e0; font-size:12px; position:sticky; bottom:0; z-index:1; }
      .spacer { flex:1; } .save-status { color:#666; } .valid { color:#28a745; }
      .invalid { color:#dc3545; font-weight:500; } .offline { color:#856404; font-weight:500; }
      .create-group-bar { display:flex; gap:4px; padding:8px; align-items:center; }
      .create-group-bar input { font-size:13px; padding:4px 6px; border:1px solid #e0e0e0; border-radius:3px; flex:1; }
      .create-group-bar input:focus { outline:none; border-color:#0066cc; }
      .create-group-bar select { font-size:13px; padding:4px; border:1px solid #e0e0e0; border-radius:3px; }
      .create-group-bar button { font-size:12px; padding:4px 10px; border:1px solid #e0e0e0; border-radius:3px; background:#fff; cursor:pointer; }
      .create-group-bar button:hover { background:#f0f0f0; }
      .hint { font-size:11px; color:#888; padding:4px 8px; }
      .solid-btn { width:auto !important; font-size:12px !important; padding:4px 10px;
        height:auto !important; white-space:nowrap; }
      .solid-panel { padding:8px; border-bottom:1px solid #e0e0e0; background:#f9f9f9; }
      .solid-panel-header { display:flex; align-items:center; justify-content:space-between; margin-bottom:4px; }
      .solid-panel-header span { font-size:12px; font-weight:600; color:#555; }
      .solid-panel-close { border:none; background:none; cursor:pointer; font-size:16px; color:#888; padding:0 4px; }
      .solid-panel-close:hover { color:#333; }
      .solid-status { font-size:12px; padding:4px 8px; color:#28a745; }
      .prefix-field { display:flex; align-items:center; gap:4px; font-size:12px; color:#666; }
      .prefix-field input { width:60px; font-size:12px; padding:3px 6px; border:1px solid #e0e0e0;
        border-radius:3px; font-family:monospace; }
      .prefix-field input:focus { outline:none; border-color:#0066cc; box-shadow:0 0 0 2px rgba(0,102,204,.15); }
      .prefix-field input::placeholder { color:#bbb; }
      .prefix-clear { border:none; background:none; cursor:pointer; font-size:14px; color:#999; padding:0 2px; }
      .prefix-clear:hover { color:#c00; }
    </style>
    <div class="header"><div class="header-row"><h2>Theme Tokens</h2>
      <div class="prefix-field" title="CSS variable prefix (e.g. 'rh' → --rh-color-primary)">
        <label>Prefix:</label>
        <input type="text" class="prefix-input" value="${this._esc(this._prefix || '')}"
          placeholder="none" />
        ${this._prefix ? '<button class="prefix-clear" title="Remove prefix">&times;</button>' : ''}
      </div>
      <div class="header-actions">
        <button class="download-btn" data-action="download-json" title="Download tokens.json">&#11015; JSON</button>
        <button class="download-btn" data-action="download-css" title="Download CSS">&#11015; CSS</button>
        <button class="create-btn" data-action="create-theme" title="Create new theme">+ Create Theme</button>
        <label class="dep-toggle"><input type="checkbox" class="dep-checkbox" ${this._showDeprecated ? 'checked' : ''} />
          Show deprecated ${depCount > 0 ? `<span class="dep-badge">&#9888; ${depCount}</span>` : ''}</label>
        <button class="solid-btn" title="Solid Pod">Solid</button>
        <button class="collapse-all-btn" title="Collapse all">&minus;</button>
        <button class="expand-all-btn" title="Expand all">+</button>
      </div></div></div>
    ${this._solidPanelOpen ? `<div class="solid-panel">
      <div class="solid-panel-header"><span>Solid Pod</span>
        <button class="solid-panel-close" title="Close">&times;</button></div>
      <geoff-solid-auth></geoff-solid-auth>
    </div>` : ''}
    <div class="search-bar">
      <input type="search" placeholder="Search tokens... (/ to focus)" class="search-input" />
      <div class="search-meta"><span class="search-count"></span>
        <div class="type-filters">${types.map(t =>
          `<button class="type-pill${this._activeTypeFilters.has(t) ? ' active' : ''}" data-type="${t}">${t}</button>`
        ).join('')}</div></div></div>
    ${this._offline ? '<div class="offline-banner">Connect to geoff serve to enable live editing</div>' : ''}
    <div class="groups">
      ${gk.map(key => { const g = groups[key];
        const ea = g.extensions ? ` extensions="${this._esc(JSON.stringify(g.extensions))}"` : '';
        return `<geoff-token-group name="${this._esc(key)}" type="${this._esc(g.type)}"${ea}></geoff-token-group>`;
      }).join('')}
      <div class="create-group-bar">
        <input type="text" placeholder="New group name (e.g. color-critical)" class="new-group-name" />
        <select class="new-group-type"><option value="">Type...</option>
          ${['color','dimension','fontFamily','fontWeight','duration','shadow','border','number','typography']
            .map(t => `<option value="${t}">${t}</option>`).join('')}</select>
        <button class="create-group-btn">+ Create Group</button>
      </div>
      <div class="hint">Groups with <code>-critical</code> in the name are inlined in &lt;head&gt; for the critical rendering path.</div>
    </div><div class="status-bar"></div>`;

    const filterFn = this._buildFilterFn();
    const groupEls = this.shadowRoot.querySelectorAll('geoff-token-group');
    gk.forEach((key, i) => {
      const el = groupEls[i];
      el.showDeprecated = this._showDeprecated; el.filter = filterFn;
      el.groups = gk; el.resolvedTokens = this._resolved;
      el.prefix = this._prefix;
      el.tokens = groups[key].tokens;
    });
    this._restoreState();
    // Expand / Collapse all
    this.shadowRoot.querySelector('.expand-all-btn')?.addEventListener('click', () => {
      this.shadowRoot.querySelectorAll('geoff-token-group').forEach(g => g.expanded = true); });
    this.shadowRoot.querySelector('.collapse-all-btn')?.addEventListener('click', () => {
      this.shadowRoot.querySelectorAll('geoff-token-group').forEach(g => g.expanded = false); });
    // Deprecation toggle
    this.shadowRoot.querySelector('.dep-checkbox')?.addEventListener('change', (e) => {
      this._showDeprecated = e.target.checked;
      this.shadowRoot.querySelectorAll('geoff-token-group').forEach(g => { g.showDeprecated = this._showDeprecated; });
      this._updateSearchCount(); });
    // Search
    const si = this.shadowRoot.querySelector('.search-input');
    if (si) si.addEventListener('input', () => { this._searchQuery = si.value; this._applyFilters(); });
    // Type pills
    this.shadowRoot.querySelectorAll('.type-pill').forEach(pill => { pill.addEventListener('click', () => {
      const t = pill.dataset.type;
      this._activeTypeFilters.has(t) ? (this._activeTypeFilters.delete(t), pill.classList.remove('active'))
        : (this._activeTypeFilters.add(t), pill.classList.add('active'));
      this._applyFilters(); }); });
    // Create group
    this.shadowRoot.querySelector('.create-group-btn')?.addEventListener('click', () => {
      const n = this.shadowRoot.querySelector('.new-group-name')?.value?.trim();
      const t = this.shadowRoot.querySelector('.new-group-type')?.value || '';
      if (n) this._onCreateGroup(n, t); });

    // Solid panel toggle
    this.shadowRoot.querySelector('.solid-btn')?.addEventListener('click', () => {
      this._solidPanelOpen = !this._solidPanelOpen; this.render();
    });
    this.shadowRoot.querySelector('.solid-panel-close')?.addEventListener('click', () => {
      this._solidPanelOpen = false; this.render();
    });
    // Solid save handler: when the auth component requests a save, prompt for name and save
    const solidAuth = this.shadowRoot.querySelector('geoff-solid-auth');
    if (solidAuth) {
      solidAuth.addEventListener('solid-save-requested', async () => {
        const name = prompt('Theme name to save as:', 'my-theme');
        if (!name) return;
        try {
          await solidAuth.saveTokens(name, this._tokens);
          const bar = this.shadowRoot.querySelector('.solid-status');
          if (!bar) {
            const s = document.createElement('div');
            s.className = 'solid-status';
            s.textContent = `Saved "${name}" to Solid pod`;
            solidAuth.after(s);
            setTimeout(() => s.remove(), 3000);
          }
        } catch (e) { alert('Save to Solid failed: ' + e.message); }
      });
      solidAuth.addEventListener('solid-tokens-loaded', (e) => {
        this._tokens = e.detail.tokens;
        this._dirty = true;
        this.render();
        this._scheduleAutoSave();
      });
    }

    // Prefix input
    const prefixInput = this.shadowRoot.querySelector('.prefix-input');
    if (prefixInput) {
      prefixInput.addEventListener('change', () => {
        const val = prefixInput.value.trim().replace(/[^a-z0-9-]/gi, '').toLowerCase();
        prefixInput.value = val;
        this._prefix = val;
        this._savePrefix(val);
      });
    }
    this.shadowRoot.querySelector('.prefix-clear')?.addEventListener('click', () => {
      this._prefix = '';
      this._savePrefix('');
      this.render();
    });

    // Download buttons
    this.shadowRoot.querySelector('[data-action="download-json"]')?.addEventListener('click', () => this._downloadTokens());
    this.shadowRoot.querySelector('[data-action="download-css"]')?.addEventListener('click', () => this._downloadCSS());
    // Create theme wizard
    this.shadowRoot.querySelector('[data-action="create-theme"]')?.addEventListener('click', () => this._showCreateTheme());

    this._updateStatusBar(); this._updateSearchCount();
  }

  _downloadTokens() {
    const json = JSON.stringify(this._tokens, null, 2);
    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url; a.download = 'tokens.json'; a.click();
    URL.revokeObjectURL(url);
  }

  _downloadCSS() {
    const css = this._buildFallbackCSS();
    const blob = new Blob([css], { type: 'text/css' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url; a.download = 'tokens.css'; a.click();
    URL.revokeObjectURL(url);
  }

  _showCreateTheme() {
    const existing = this.shadowRoot.querySelector('geoff-create-theme');
    if (existing) existing.remove();
    const wizard = document.createElement('geoff-create-theme');
    wizard.addEventListener('theme-created', (e) => {
      this._tokens = e.detail.tokens;
      wizard.remove();
      this._dirty = true;
      this.render();
      this._scheduleAutoSave();
    });
    wizard.addEventListener('theme-cancelled', () => { wizard.remove(); });
    this.shadowRoot.appendChild(wizard);
  }

  /** Navigate to a group by name — expand it and scroll into view.
   *  Handles both exact matches (e.g., "font") and nested paths (e.g., "font.family"). */
  navigateToGroup(groupName) {
    const groups = this.shadowRoot.querySelectorAll('geoff-token-group');
    // Try exact match first
    for (const g of groups) {
      if (g.getAttribute('name') === groupName) {
        g.expanded = true;
        g.scrollIntoView({ behavior: 'smooth', block: 'start' });
        return;
      }
    }
    // Try matching the top-level group (e.g., "font" from "font.family.body-text")
    const topLevel = groupName.split('.')[0];
    for (const g of groups) {
      if (g.getAttribute('name') === topLevel) {
        g.expanded = true;
        g.scrollIntoView({ behavior: 'smooth', block: 'start' });
        return;
      }
    }
  }

  /** Navigate to a specific token by path — expand its group and highlight. */
  navigateToToken(path) {
    const groupName = path.split('.')[0];
    this.navigateToGroup(groupName);
    setTimeout(() => {
      const field = this.shadowRoot.querySelector(`geoff-token-field[path="${path}"]`);
      if (!field) {
        // Try inside groups' shadow roots
        const groups = this.shadowRoot.querySelectorAll('geoff-token-group');
        for (const g of groups) {
          const f = g.shadowRoot?.querySelector(`geoff-token-field[path="${path}"]`);
          if (f) { f.scrollIntoView({ behavior: 'smooth', block: 'center' }); return; }
        }
      } else {
        field.scrollIntoView({ behavior: 'smooth', block: 'center' });
      }
    }, 100);
  }
}
customElements.define('geoff-token-editor', GeoffTokenEditor);

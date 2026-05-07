/**
 * <geoff-token-group> — Collapsible group of design tokens.
 *
 * Groups related tokens under a header that can be toggled open/closed.
 * Contains <geoff-token-field> elements for each token in the group.
 *
 * Attributes:
 *   name      — group name (e.g. "color-critical")
 *   type      — DTCG $type for the group (color, dimension, etc.)
 *   expanded  — boolean attribute; present = expanded
 *   extensions — JSON string of $extensions metadata for the group
 *
 * Properties:
 *   tokens    — object map of { path: { value, $type, $description } }
 *
 * Events:
 *   add-token — { detail: { group, property } } when a new token is added
 */

const DTCG_ATTRIBUTES = {
  typography: ['fontFamily', 'fontSize', 'fontWeight', 'lineHeight', 'letterSpacing'],
  shadow: ['color', 'offsetX', 'offsetY', 'blur', 'spread'],
  border: ['color', 'width', 'style'],
  transition: ['duration', 'delay', 'timingFunction'],
  _meta: ['$value', '$type', '$description', '$deprecated'],
};

class GeoffTokenGroup extends HTMLElement {
  static get observedAttributes() { return ['name', 'type', 'expanded', 'extensions']; }

  constructor() {
    super();
    const internals = this.attachInternals?.();
    if (!internals?.shadowRoot) {
      this.attachShadow({ mode: 'open' });
    }
    this._tokens = {};
    this._showAdd = false;
    this._renaming = false;
    this._allGroups = [];
    this._resolvedTokens = {};
    this._showDeprecated = false;
    this._filter = null;
    this._prefix = '';
    this._moveOpenPath = null;
  }

  get resolvedTokens() { return this._resolvedTokens; }
  set resolvedTokens(val) { this._resolvedTokens = val || {}; }

  get prefix() { return this._prefix; }
  set prefix(val) { this._prefix = val || ''; }

  get showDeprecated() { return this._showDeprecated; }
  set showDeprecated(val) {
    const changed = this._showDeprecated !== !!val;
    this._showDeprecated = !!val;
    if (changed && this.shadowRoot.children.length) this.render();
  }

  get filter() { return this._filter; }
  set filter(fn) {
    this._filter = fn;
    if (this.shadowRoot.children.length) this.render();
  }

  get tokens() { return this._tokens; }
  set tokens(val) { this._tokens = val || {}; this.render(); }

  get groups() { return this._allGroups; }
  set groups(val) { this._allGroups = val || []; }

  get expanded() { return this.hasAttribute('expanded'); }
  set expanded(v) { v ? this.setAttribute('expanded', '') : this.removeAttribute('expanded'); }

  get _name() { return this.getAttribute('name') || ''; }
  get _type() { return this.getAttribute('type') || ''; }

  /** Render group-level extension subtitle in the header. */
  _renderGroupExtensions() {
    const raw = this.getAttribute('extensions');
    if (!raw) return '';
    let ext;
    try { ext = JSON.parse(raw); } catch { return ''; }
    if (!ext || typeof ext !== 'object') return '';
    const title = ext['com.redhat.ux.title'] || (ext['com.redhat.ux'] && ext['com.redhat.ux'].title);
    if (!title) return '';
    return `<span class="group-subtitle">${this._esc(String(title))}</span>`;
  }

  connectedCallback() { this.render(); }
  attributeChangedCallback() { if (this.shadowRoot.children.length) this.render(); }

  _suggestionsForType(type) {
    return DTCG_ATTRIBUTES[type] || DTCG_ATTRIBUTES._meta;
  }

  /** Count deprecated tokens in this group. */
  _deprecatedCount() {
    let n = 0;
    for (const tok of Object.values(this._tokens)) {
      if (typeof tok === 'object' && tok !== null && tok.$deprecated) n++;
    }
    return n;
  }

  /** Filter entries based on showDeprecated and filter function. */
  _visibleEntries() {
    let entries = Object.entries(this._tokens);
    if (!this._showDeprecated) {
      entries = entries.filter(([, tok]) => {
        if (typeof tok === 'object' && tok !== null && tok.$deprecated) return false;
        return true;
      });
    }
    if (typeof this._filter === 'function') {
      entries = entries.filter(([path, tok]) => this._filter(path, tok));
    }
    return entries;
  }

  render() {
    const expanded = this.expanded;
    const allEntries = Object.entries(this._tokens);
    const visibleEntries = this._visibleEntries();
    const totalCount = allEntries.length;
    const visibleCount = visibleEntries.length;
    const depCount = this._deprecatedCount();

    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; margin-bottom: 4px; }
        .header {
          display: flex; align-items: center; gap: 8px;
          padding: 8px; cursor: pointer; user-select: none;
          background: #f0f0f0; border: 1px solid #e0e0e0;
          border-radius: 4px;
        }
        .header:hover { background: #e8e8e8; }
        .arrow { font-size: 10px; width: 16px; text-align: center;
          transition: transform 0.15s; color: #666; }
        .arrow.open { transform: rotate(90deg); }
        .group-name { font-size: 15px; font-weight: 600; color: #333; }
        .badge { font-size: 11px; padding: 1px 6px; border-radius: 8px;
          background: #0066cc; color: #fff; }
        .dep-count { font-size: 11px; padding: 1px 6px; border-radius: 8px;
          background: #fff3cd; color: #856404; }
        .count { font-size: 11px; color: #888; margin-left: auto; }
        .body { padding: 4px 8px 8px; display: ${expanded ? 'block' : 'none'}; }
        .add-bar { display: flex; gap: 4px; margin-top: 6px; align-items: center; }
        .add-bar input { flex: 1; font-size: 13px; padding: 4px 6px;
          border: 1px solid #e0e0e0; border-radius: 3px; }
        .add-bar input:focus { outline: none; border-color: #0066cc;
          box-shadow: 0 0 0 2px rgba(0,102,204,.15); }
        .add-bar button, .add-btn {
          font-size: 12px; padding: 4px 10px; border: 1px solid #e0e0e0;
          border-radius: 3px; background: #fff; color: #333; cursor: pointer; }
        .add-btn { margin-top: 6px; }
        .add-btn:hover, .add-bar button:hover { background: #f0f0f0; }
        .suggestions { display: flex; flex-wrap: wrap; gap: 4px; margin-top: 4px; }
        .suggestion { font-size: 11px; padding: 2px 8px; border-radius: 10px;
          background: #e8f0fe; color: #0066cc; cursor: pointer; border: none; }
        .suggestion:hover { background: #d0e0fc; }
        .edit-group-btn { background: none; border: none; cursor: pointer;
          font-size: 14px; color: #888; padding: 2px 6px; border-radius: 3px; }
        .edit-group-btn:hover { background: #e0e0e0; color: #333; }
        .rename-bar { display: flex; gap: 4px; padding: 8px; align-items: center;
          background: #fafafa; border: 1px solid #e0e0e0; border-radius: 4px; margin-bottom: 4px; }
        .rename-bar input { flex: 1; font-size: 13px; padding: 4px 6px;
          border: 1px solid #e0e0e0; border-radius: 3px; }
        .rename-bar input:focus { outline: none; border-color: #0066cc; }
        .rename-bar button { font-size: 12px; padding: 4px 10px; border: 1px solid #e0e0e0;
          border-radius: 3px; background: #fff; color: #333; cursor: pointer; }
        .rename-bar button:hover { background: #f0f0f0; }
        .move-select { font-size: 13px; padding: 3px 6px; border: 1px solid #e0e0e0;
          border-radius: 3px; background: #fff; cursor: pointer; margin-left: 4px; }
        .move-icon { background: none; border: 1px solid transparent; cursor: pointer;
          font-size: 16px; color: #999; padding: 4px; border-radius: 3px; line-height: 1; margin-top: 2px; }
        .move-icon:hover { color: #0066cc; background: #f0f0f0; border-color: #e0e0e0; }
        .group-subtitle { font-size: 11px; font-style: italic; color: #999; font-weight: 400; }
      </style>
      <div class="header" part="header">
        <span class="arrow ${expanded ? 'open' : ''}">&#9654;</span>
        <span class="group-name">${this._esc(this._name)}</span>
        ${this._renderGroupExtensions()}
        ${this._type ? `<span class="badge">${this._esc(this._type)}</span>` : ''}
        ${depCount > 0 ? `<span class="dep-count">&#9888; ${depCount}</span>` : ''}
        <span class="count">${visibleCount !== totalCount ? `${visibleCount} / ` : ''}${totalCount} token${totalCount !== 1 ? 's' : ''}</span>
        <button class="edit-group-btn" title="Rename group">&#9998;</button>
      </div>
      <div class="body">
        ${this._renaming ? this._renderRenameForm() : ''}
        <slot></slot>
        ${this._renderFields(visibleEntries)}
        ${this._showAdd ? this._renderAddForm() : `<button class="add-btn">+ Add Token</button>`}
      </div>`;

    this._attachListeners();
    this._passResolvedTokensToFields();
  }

  /** Pass resolvedTokens to all child geoff-token-field elements. */
  _passResolvedTokensToFields() {
    if (!this._resolvedTokens || Object.keys(this._resolvedTokens).length === 0) return;
    this.shadowRoot.querySelectorAll('geoff-token-field').forEach(field => {
      field.resolvedTokens = this._resolvedTokens;
    });
  }

  _renderFields(entries) {
    const otherGroups = this._allGroups.filter(g => g !== this._name);
    return entries.map(([path, tok]) => {
      const type = tok.$type || this._type || 'default';
      const desc = tok.$description || '';
      const pfx = this._prefix ? `${this._prefix}-` : '';
      const variable = `--${pfx}${path.replace(/\./g, '-')}`;
      const val = typeof tok === 'object' ? (tok.$value ?? tok.value ?? '') : tok;
      const depMsg = (typeof tok === 'object' && tok.$deprecated) ? tok.$deprecated : null;
      const isMoving = this._moveOpenPath === path;
      const moveUI = otherGroups.length > 0
        ? (isMoving
          ? `<select class="move-select" data-path="${this._esc(path)}">
              <option value="">Select group…</option>
              ${otherGroups.map(g => `<option value="${this._esc(g)}">${this._esc(g)}</option>`).join('')}
            </select>`
          : `<button class="move-icon" data-path="${this._esc(path)}" title="Move to another group">&#8596;</button>`)
        : '';
      const extAttr = (typeof tok === 'object' && tok.$extensions)
        ? `extensions="${this._esc(JSON.stringify(tok.$extensions))}"` : '';
      const depAttr = depMsg !== null
        ? `deprecated="${this._esc(typeof depMsg === 'string' ? depMsg : '')}"` : '';
      return `<div style="display:flex;align-items:flex-start;gap:4px" data-token-path="${this._esc(path)}">
        <geoff-token-field style="flex:1"
          path="${this._esc(path)}" type="${this._esc(type)}"
          value="${this._esc(String(val))}" variable="${this._esc(variable)}"
          ${desc ? `description="${this._esc(desc)}"` : ''}
          ${extAttr}
          ${depAttr}
        ></geoff-token-field>
        ${moveUI}
      </div>`;
    }).join('');
  }

  _renderRenameForm() {
    return `<div class="rename-bar">
      <input type="text" value="${this._esc(this._name)}" placeholder="Group name" />
      <button data-action="confirm-rename">Rename</button>
      <button data-action="cancel-rename">Cancel</button>
    </div>`;
  }

  _renderAddForm() {
    const suggestions = this._suggestionsForType(this._type);
    return `
      <div class="add-bar">
        <input type="text" placeholder="Property name" list="suggestions-${this._name}" />
        <button data-action="confirm-add">Add</button>
        <button data-action="cancel-add">Cancel</button>
      </div>
      <div class="suggestions">
        ${suggestions.map(s => `<button class="suggestion">${s}</button>`).join('')}
      </div>`;
  }

  _esc(s) { return String(s).replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;'); }

  _attachListeners() {
    const root = this.shadowRoot;

    root.querySelector('.header').addEventListener('click', (e) => {
      if (e.target.closest('.edit-group-btn')) return;
      this.expanded = !this.expanded;
    });

    const editBtn = root.querySelector('.edit-group-btn');
    if (editBtn) {
      editBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        this._renaming = true;
        this.expanded = true;
        this.render();
      });
    }

    const confirmRename = root.querySelector('[data-action="confirm-rename"]');
    if (confirmRename) {
      confirmRename.addEventListener('click', () => {
        const input = root.querySelector('.rename-bar input');
        const newName = input?.value?.trim();
        if (newName && newName !== this._name) {
          this.dispatchEvent(new CustomEvent('rename-group', {
            bubbles: true, composed: true,
            detail: { oldName: this._name, newName },
          }));
        }
        this._renaming = false;
        this.render();
      });
    }

    const cancelRename = root.querySelector('[data-action="cancel-rename"]');
    if (cancelRename) {
      cancelRename.addEventListener('click', () => { this._renaming = false; this.render(); });
    }

    root.querySelectorAll('.move-icon').forEach(btn => {
      btn.addEventListener('click', () => {
        this._moveOpenPath = btn.dataset.path;
        this.render();
      });
    });
    root.querySelectorAll('.move-select').forEach(sel => {
      sel.addEventListener('change', () => {
        const targetGroup = sel.value;
        const tokenPath = sel.dataset.path;
        this._moveOpenPath = null;
        if (targetGroup && tokenPath) {
          this.dispatchEvent(new CustomEvent('move-token', {
            bubbles: true, composed: true,
            detail: { path: tokenPath, fromGroup: this._name, toGroup: targetGroup },
          }));
        }
      });
      sel.addEventListener('blur', () => { this._moveOpenPath = null; this.render(); });
      requestAnimationFrame(() => sel.focus());
    });

    const addBtn = root.querySelector('.add-btn');
    if (addBtn) {
      addBtn.addEventListener('click', () => { this._showAdd = true; this.render(); });
    }

    const confirmBtn = root.querySelector('[data-action="confirm-add"]');
    if (confirmBtn) {
      confirmBtn.addEventListener('click', () => this._doAdd());
    }

    const cancelBtn = root.querySelector('[data-action="cancel-add"]');
    if (cancelBtn) {
      cancelBtn.addEventListener('click', () => { this._showAdd = false; this.render(); });
    }

    root.querySelectorAll('.suggestion').forEach(btn => {
      btn.addEventListener('click', () => {
        const input = root.querySelector('.add-bar input');
        if (input) input.value = btn.textContent;
      });
    });

    const input = root.querySelector('.add-bar input');
    if (input) {
      input.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') this._doAdd();
        if (e.key === 'Escape') { this._showAdd = false; this.render(); }
      });
      requestAnimationFrame(() => input.focus());
    }
  }

  _doAdd() {
    const input = this.shadowRoot.querySelector('.add-bar input');
    const prop = input?.value?.trim();
    if (!prop) return;
    const path = `${this._name}.${prop}`;
    this._tokens[path] = { $value: '', $type: this._type };
    this._showAdd = false;
    this.dispatchEvent(new CustomEvent('add-token', {
      bubbles: true, composed: true,
      detail: { group: this._name, property: prop, path },
    }));
    this.render();
  }
}

customElements.define('geoff-token-group', GeoffTokenGroup);

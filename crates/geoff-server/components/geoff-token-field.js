/**
 * <geoff-token-field> — Individual design token input.
 *
 * Adapts its UI based on the token's $type (color, dimension,
 * duration, fontFamily, fontWeight, number, or text fallback).
 *
 * Attributes:
 *   path        — dot-separated token path (e.g. "color-critical.text")
 *   type        — DTCG $type: color | dimension | duration | fontFamily | fontWeight | number
 *   value       — current token value
 *   description — optional help text
 *   variable    — CSS custom property name (e.g. "--color-critical-text")
 *   error       — error message (empty = valid)
 *   extensions  — JSON string of $extensions metadata
 *   deprecated  — deprecation message (present = deprecated)
 *
 * Properties:
 *   resolvedTokens — object map of { "path": { value, type } } for resolving references
 *
 * Events:
 *   token-change      — { detail: { path, value } }
 *   follow-reference  — { detail: { path } } when a reference link is clicked
 */
class GeoffTokenField extends HTMLElement {
  static get observedAttributes() {
    return ['path', 'type', 'value', 'description', 'variable', 'error', 'extensions', 'deprecated'];
  }

  constructor() {
    super();
    const internals = this.attachInternals?.();
    if (!internals?.shadowRoot) {
      this.attachShadow({ mode: 'open' });
    }
    this._resolvedTokens = {};
  }

  get resolvedTokens() { return this._resolvedTokens; }
  set resolvedTokens(val) { this._resolvedTokens = val || {}; if (this.shadowRoot.children.length) this.render(); }

  connectedCallback() {
    this.render();
  }

  attributeChangedCallback() {
    if (this.shadowRoot.children.length) this.render();
  }

  get _path() { return this.getAttribute('path') || ''; }
  get _type() { return this.getAttribute('type') || 'default'; }
  get _value() { return this.getAttribute('value') || ''; }
  get _desc() { return this.getAttribute('description') || ''; }
  get _variable() { return this.getAttribute('variable') || ''; }
  get _error() { return this.getAttribute('error') || ''; }
  get _deprecated() { return this.getAttribute('deprecated'); }
  get isDeprecated() { return this._deprecated !== null; }

  /** Convert any color value to #rrggbb for the color picker. Non-hex values get a fallback. */
  _toHex(val) {
    if (!val) return '#000000';
    const s = String(val).trim();
    if (/^#[0-9a-fA-F]{6}$/.test(s)) return s;
    if (/^#[0-9a-fA-F]{3}$/.test(s)) return '#' + s[1]+s[1]+s[2]+s[2]+s[3]+s[3];
    if (/^#[0-9a-fA-F]{8}$/.test(s)) return s.slice(0, 7);
    // Try parsing rgb(r,g,b) or "r g b" formats
    const nums = s.match(/(\d+)\s*[\s,]\s*(\d+)\s*[\s,]\s*(\d+)/);
    if (nums) {
      const [, r, g, b] = nums;
      return '#' + [r,g,b].map(n => parseInt(n).toString(16).padStart(2,'0')).join('');
    }
    return '#000000';
  }

  /** Render basic markdown as HTML (bold, italic, code, links). */
  _renderMarkdown(text) {
    if (!text) return '';
    return this._escAttr(text)
      .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
      .replace(/\*(.+?)\*/g, '<em>$1</em>')
      .replace(/`(.+?)`/g, '<code style="font-size:0.9em;background:#f0f0f0;padding:1px 4px;border-radius:2px">$1</code>')
      .replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" target="_blank" style="color:#0066cc">$1</a>');
  }

  /** Build the input HTML for a given $type. */
  _inputHTML() {
    const v = this._escAttr(this._value);
    switch (this._type) {
      case 'color': {
        const hex = this._toHex(this._value);
        return `
          <div class="input-row">
            <input type="color" value="${hex}" data-role="picker" />
            <input type="text" value="${v}" data-role="text" placeholder="#000000" />
          </div>`;
      }
      case 'dimension':
        return this._numWithUnit(v, ['px', 'rem', 'em', '%']);
      case 'duration':
        return this._numWithUnit(v, ['ms', 's']);
      case 'fontFamily':
        return `<input type="text" value="${v}" placeholder="system-ui, sans-serif" />`;
      case 'fontWeight':
        return this._fontWeightSelect(v);
      case 'number':
        return `<input type="number" value="${v}" step="any" />`;
      default:
        return `<input type="text" value="${v}" />`;
    }
  }

  _numWithUnit(raw, units) {
    const num = parseFloat(raw) || 0;
    const unit = units.find(u => String(raw).endsWith(u)) || units[0];
    const opts = units.map(u =>
      `<option value="${u}"${u === unit ? ' selected' : ''}>${u}</option>`
    ).join('');
    return `
      <div class="input-row">
        <input type="number" value="${num}" step="any" data-role="num" />
        <select data-role="unit">${opts}</select>
      </div>`;
  }

  _fontWeightSelect(current) {
    const weights = [
      ['100', 'Thin'], ['200', 'Extra Light'], ['300', 'Light'],
      ['400', 'Normal'], ['500', 'Medium'], ['600', 'Semi Bold'],
      ['700', 'Bold'], ['800', 'Extra Bold'], ['900', 'Black'],
    ];
    const opts = weights.map(([val, lbl]) =>
      `<option value="${val}"${val === String(current) ? ' selected' : ''}>${lbl} (${val})</option>`
    ).join('');
    return `<select>${opts}</select>`;
  }

  _escAttr(s) {
    return String(s).replace(/&/g, '&amp;').replace(/"/g, '&quot;');
  }

  _emitChange(value) {
    this.setAttribute('value', value);
    this.dispatchEvent(new CustomEvent('token-change', {
      bubbles: true, composed: true,
      detail: { path: this._path, value },
    }));
  }

  /** Check if the current value is a DTCG reference like {color.blue.50}. */
  _isReference() {
    return /^\{[^{}]+\}$/.test(this._value);
  }

  /** Extract the reference path from a DTCG reference value. */
  _extractRefPath() {
    const m = this._value.match(/^\{([^{}]+)\}$/);
    return m ? m[1] : '';
  }

  /** Render reference info UI (link icon and resolved value). */
  _renderReferenceInfo() {
    if (!this._isReference()) return '';
    const refPath = this._extractRefPath();
    const resolved = this._resolvedTokens[refPath];
    const unresolved = !resolved;
    const resolvedLabel = resolved
      ? `<span class="ref-resolved">→ ${this._escAttr(String(resolved.value))}</span>`
      : '';
    return `
      <div class="ref-info">
        <button class="ref-link-btn${unresolved ? ' unresolved' : ''}"
          data-ref-path="${this._escAttr(refPath)}"
          title="${unresolved ? 'Unresolved reference' : 'Follow reference to ' + this._escAttr(refPath)}"
        >🔗</button>
        ${resolvedLabel}
      </div>`;
  }

  /** Render extension metadata badges and subtitle. */
  _renderExtensions() {
    const raw = this.getAttribute('extensions');
    if (!raw) return '';
    let ext;
    try { ext = JSON.parse(raw); } catch { return ''; }
    if (!ext || typeof ext !== 'object' || Object.keys(ext).length === 0) return '';

    const parts = [];
    const title = ext['com.redhat.ux.title'] || (ext['com.redhat.ux'] && ext['com.redhat.ux'].title);
    if (title) {
      parts.push(`<div class="ext-title">${this._escAttr(String(title))}</div>`);
    }

    const badges = [];
    for (const [k, v] of Object.entries(ext)) {
      if (k === 'com.redhat.ux.title') continue;
      if (typeof v === 'object' && v !== null) {
        for (const [sk, sv] of Object.entries(v)) {
          if (k === 'com.redhat.ux' && sk === 'title') continue;
          badges.push(`<span class="ext-badge">${this._escAttr(sk)}: ${this._escAttr(String(sv))}</span>`);
        }
      } else {
        badges.push(`<span class="ext-badge">${this._escAttr(k)}: ${this._escAttr(String(v))}</span>`);
      }
    }
    if (badges.length) {
      parts.push(`<div class="ext-badges">${badges.join('')}</div>`);
    }
    return parts.join('');
  }

  render() {
    const hasError = !!this._error;
    const dep = this.isDeprecated;
    const depMsg = (this._deprecated || 'Deprecated');

    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; padding: 4px 0; }
        :host([deprecated]) { opacity: 0.5; }
        .field { padding: 6px 8px; border-radius: 4px;
          border: 1px solid ${hasError ? '#dc3545' : 'transparent'}; }
        .field:hover { background: #fafafa; }
        label { display: block; font-size: 14px; font-weight: 500;
          color: #333; margin-bottom: 2px; }
        label.deprecated { text-decoration: line-through; }
        .dep-badge { font-size: 12px; color: #856404; margin-left: 6px; cursor: help; }
        .dep-msg { font-size: 12px; color: #856404; margin-top: 2px; }
        .var-name { font-family: monospace; font-size: 12px;
          color: #666; margin-bottom: 4px; }
        .help { font-size: 12px; color: #888; margin-top: 2px; }
        .error-msg { font-size: 12px; color: #dc3545; margin-top: 2px; }
        .input-row { display: flex; gap: 4px; align-items: center; }
        input, select {
          font-size: 14px; padding: 4px 6px; border: 1px solid #e0e0e0;
          border-radius: 3px; background: #fff; color: #333; }
        input[type="text"], input[type="number"] { flex: 1; min-width: 0; }
        input[type="color"] { width: 32px; height: 28px; padding: 2px;
          border: 1px solid #e0e0e0; cursor: pointer; }
        select { min-width: 60px; }
        input:focus, select:focus { outline: none;
          border-color: #0066cc; box-shadow: 0 0 0 2px rgba(0,102,204,.15); }
        .ref-info { display: flex; align-items: center; gap: 6px; margin-top: 2px; }
        .ref-link-btn { background: none; border: none; cursor: pointer;
          font-size: 14px; padding: 2px 4px; border-radius: 3px; }
        .ref-link-btn:hover { background: #e8f0fe; }
        .ref-link-btn.unresolved { color: #dc3545; }
        .ref-resolved { font-size: 12px; font-style: italic; color: #666; }
        .ext-title { font-size: 12px; font-style: italic; color: #999;
          margin-top: 1px; margin-bottom: 2px; }
        .ext-badges { display: flex; flex-wrap: wrap; gap: 4px; margin-top: 2px; }
        .ext-badge { font-size: 10px; padding: 1px 6px; border-radius: 8px;
          background: #e8f0fe; color: #0066cc; }
      </style>
      <div class="field">
        <label class="${dep ? 'deprecated' : ''}"${dep ? ` title="${this._escAttr(depMsg)}"` : ''}>
          ${this._escAttr(this._path)}${dep ? `<span class="dep-badge" title="${this._escAttr(depMsg)}">&#9888;&#65039;</span>` : ''}
        </label>
        ${this._renderExtensions()}
        ${this._variable ? `<div class="var-name">${this._escAttr(this._variable)}</div>` : ''}
        ${this._inputHTML()}
        ${this._renderReferenceInfo()}
        ${this._desc ? `<div class="help">${this._renderMarkdown(this._desc)}</div>` : ''}
        ${dep ? `<div class="dep-msg">${this._escAttr(depMsg)}</div>` : ''}
        ${hasError ? `<div class="error-msg">${this._escAttr(this._error)}</div>` : ''}
      </div>`;

    this._attachInputListeners();
  }

  _attachInputListeners() {
    const root = this.shadowRoot;
    const type = this._type;

    // Reference link button
    const refBtn = root.querySelector('.ref-link-btn');
    if (refBtn) {
      refBtn.addEventListener('click', () => {
        this.dispatchEvent(new CustomEvent('follow-reference', {
          bubbles: true, composed: true,
          detail: { path: refBtn.dataset.refPath },
        }));
      });
    }

    if (type === 'color') {
      const picker = root.querySelector('[data-role="picker"]');
      const text = root.querySelector('[data-role="text"]');
      picker.addEventListener('input', () => {
        text.value = picker.value;
        this._emitChange(picker.value);
      });
      text.addEventListener('change', () => {
        const hex = this._toHex(text.value);
        picker.value = hex;
        this._emitChange(text.value);
      });
    } else if (type === 'dimension' || type === 'duration') {
      const num = root.querySelector('[data-role="num"]');
      const unit = root.querySelector('[data-role="unit"]');
      const emit = () => this._emitChange(`${num.value}${unit.value}`);
      num.addEventListener('input', emit);
      unit.addEventListener('change', emit);
    } else {
      const input = root.querySelector('input, select');
      if (input) {
        const ev = input.tagName === 'SELECT' ? 'change' : 'input';
        input.addEventListener(ev, () => this._emitChange(input.value));
      }
    }
  }
}

customElements.define('geoff-token-field', GeoffTokenField);

/**
 * <geoff-color-palette> — Visual color swatch grid.
 *
 * Displays color tokens as swatches grouped by scale (numeric suffix)
 * or as a flex grid for non-scale colors.
 *
 * Properties:
 *   tokens    — object map of { "color.blue.10": { $value: "#e0f0ff", $type: "color" }, ... }
 *   groupName — name of the parent group (e.g. "color")
 *
 * Events:
 *   palette-select — { path, value } when a swatch is clicked
 *   token-change   — { path, value } when the detail editor value changes
 */
class GeoffColorPalette extends HTMLElement {
  constructor() {
    super();
    const internals = this.attachInternals?.();
    if (!internals?.shadowRoot) {
      this.attachShadow({ mode: 'open' });
    }
    this._tokens = {};
    this._groupName = '';
    this._selected = null;
  }

  get tokens() { return this._tokens; }
  set tokens(val) { this._tokens = val || {}; this.render(); }

  get groupName() { return this._groupName; }
  set groupName(val) { this._groupName = val || ''; }

  connectedCallback() { this.render(); }

  _esc(s) { return String(s).replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;'); }

  _resolve(tok) {
    if (typeof tok === 'object' && tok !== null) return tok.$value ?? tok.value ?? '';
    return String(tok);
  }

  _cssVar(path) { return `--${path.replace(/\./g, '-')}`; }

  /** Check if a hex color is light enough to need a border. */
  _isLight(hex) {
    const c = hex.replace('#', '');
    if (c.length < 6) return true;
    const r = parseInt(c.slice(0, 2), 16);
    const g = parseInt(c.slice(2, 4), 16);
    const b = parseInt(c.slice(4, 6), 16);
    return (r * 299 + g * 587 + b * 114) / 1000 > 200;
  }

  /** Separate tokens into scale rows and non-scale items. */
  _categorize() {
    const scales = {};
    const nonScale = [];
    for (const [path, tok] of Object.entries(this._tokens)) {
      const parts = path.split('.');
      const last = parts[parts.length - 1];
      if (/^\d+$/.test(last)) {
        const prefix = parts.slice(0, -1).join('.');
        if (!scales[prefix]) scales[prefix] = [];
        scales[prefix].push({ path, step: parseInt(last, 10), value: this._resolve(tok), tok });
      } else {
        nonScale.push({ path, value: this._resolve(tok), tok });
      }
    }
    for (const key of Object.keys(scales)) {
      scales[key].sort((a, b) => a.step - b.step);
    }
    return { scales, nonScale };
  }

  _swatchHTML(path, value, step) {
    const sel = this._selected === path;
    const border = this._isLight(value) ? '1px solid #ddd' : '1px solid transparent';
    const ring = sel ? '; outline: 2px solid #0066cc; outline-offset: 1px' : '';
    const label = step !== undefined ? step : path.split('.').pop();
    return `<div class="swatch-wrap" data-path="${this._esc(path)}" data-value="${this._esc(value)}">
      <div class="swatch" style="background:${this._esc(value)}; border:${border}${ring}"
        title="${this._esc(this._cssVar(path))}\n${this._esc(value)}\n${this._esc(path)}"></div>
      <span class="step-label">${this._esc(String(label))}</span>
    </div>`;
  }

  _detailHTML() {
    if (!this._selected) return '';
    const tok = this._tokens[this._selected];
    const value = this._resolve(tok);
    const desc = (tok && tok.$description) || '';
    const cssVar = this._cssVar(this._selected);
    return `<div class="detail">
      <div class="detail-row">
        <input type="color" value="${this._esc(value)}" data-role="d-picker" />
        <input type="text" value="${this._esc(value)}" data-role="d-text"
          pattern="^#[0-9a-fA-F]{3,8}$" placeholder="#000000" />
      </div>
      <div class="detail-meta">
        <span class="detail-var">${this._esc(cssVar)}</span>
        <span class="detail-path">${this._esc(this._selected)}</span>
      </div>
      ${desc ? `<div class="detail-desc">${this._esc(desc)}</div>` : ''}
    </div>`;
  }

  render() {
    const { scales, nonScale } = this._categorize();

    const scaleRows = Object.entries(scales).map(([prefix, items]) => {
      const label = prefix.split('.').pop();
      return `<div class="scale-row">
        <span class="scale-label">${this._esc(label)}</span>
        <div class="scale-swatches">
          ${items.map(s => this._swatchHTML(s.path, s.value, s.step)).join('')}
        </div>
      </div>`;
    }).join('');

    const nonScaleHTML = nonScale.length ? `<div class="non-scale">
      ${nonScale.map(s => this._swatchHTML(s.path, s.value)).join('')}
    </div>` : '';

    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; }
        .palette { background: #fff; padding: 12px; border-radius: 4px;
          border: 1px solid #e0e0e0; }
        .scale-row { display: flex; align-items: flex-start; gap: 8px;
          margin-bottom: 12px; }
        .scale-label { font: 12px monospace; color: #666; min-width: 64px;
          padding-top: 8px; text-align: right; }
        .scale-swatches { display: flex; gap: 4px; flex-wrap: wrap; }
        .non-scale { display: flex; gap: 4px; flex-wrap: wrap; margin-top: 8px; }
        .swatch-wrap { display: flex; flex-direction: column; align-items: center;
          cursor: pointer; }
        .swatch { width: 32px; height: 32px; border-radius: 2px;
          transition: transform 0.12s ease; }
        .swatch-wrap:hover .swatch { transform: scale(1.1); }
        .step-label { font-size: 10px; color: #888; margin-top: 2px;
          text-align: center; max-width: 32px; overflow: hidden;
          text-overflow: ellipsis; white-space: nowrap; }
        .detail { margin-top: 12px; padding: 10px; background: #fafafa;
          border: 1px solid #e0e0e0; border-radius: 4px; }
        .detail-row { display: flex; gap: 6px; align-items: center; }
        .detail-row input[type="color"] { width: 32px; height: 28px; padding: 2px;
          border: 1px solid #e0e0e0; border-radius: 3px; cursor: pointer; }
        .detail-row input[type="text"] { flex: 1; font-size: 13px; padding: 4px 6px;
          border: 1px solid #e0e0e0; border-radius: 3px; font-family: monospace; }
        .detail-row input:focus { outline: none; border-color: #0066cc;
          box-shadow: 0 0 0 2px rgba(0,102,204,.15); }
        .detail-meta { margin-top: 6px; font-size: 11px; color: #666;
          font-family: monospace; display: flex; gap: 12px; }
        .detail-desc { margin-top: 4px; font-size: 11px; color: #888; }
        .empty { font-size: 13px; color: #888; padding: 16px; text-align: center; }
      </style>
      <div class="palette">
        ${scaleRows || ''}${nonScaleHTML || ''}
        ${!scaleRows && !nonScaleHTML ? '<div class="empty">No color tokens</div>' : ''}
        ${this._detailHTML()}
      </div>`;

    this._attachListeners();
  }

  _attachListeners() {
    const root = this.shadowRoot;

    root.querySelectorAll('.swatch-wrap').forEach(el => {
      el.addEventListener('click', () => {
        const path = el.dataset.path;
        const value = el.dataset.value;
        this._selected = path;
        this.dispatchEvent(new CustomEvent('palette-select', {
          bubbles: true, composed: true, detail: { path, value },
        }));
        this.render();
      });
    });

    const picker = root.querySelector('[data-role="d-picker"]');
    const text = root.querySelector('[data-role="d-text"]');
    if (picker && text) {
      picker.addEventListener('input', () => {
        text.value = picker.value;
        this._emitChange(picker.value);
      });
      text.addEventListener('change', () => {
        if (/^#[0-9a-fA-F]{3,8}$/.test(text.value)) {
          picker.value = text.value.length === 7 ? text.value : picker.value;
          this._emitChange(text.value);
        }
      });
    }
  }

  _emitChange(value) {
    if (!this._selected) return;
    const tok = this._tokens[this._selected];
    if (typeof tok === 'object' && tok !== null) tok.$value = value;
    this.dispatchEvent(new CustomEvent('token-change', {
      bubbles: true, composed: true,
      detail: { path: this._selected, value },
    }));
  }
}

customElements.define('geoff-color-palette', GeoffColorPalette);

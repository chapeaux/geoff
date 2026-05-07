/**
 * <geoff-create-theme> — Theme creation wizard (modal).
 *
 * Three-step flow: choose source, name the theme, confirm.
 * Dispatches `theme-created` with { name, tokens } on completion,
 * or `theme-cancelled` when dismissed.
 */
class GeoffCreateTheme extends HTMLElement {
  constructor() {
    super();
    const internals = this.attachInternals?.();
    if (!internals?.shadowRoot) {
      this.attachShadow({ mode: 'open' });
    }
    this._step = 1;
    this._source = null; // 'upload' | 'scratch' | 'template'
    this._tokens = null;
    this._themeName = '';
    this._includeCritical = true;
    this._error = '';
  }

  connectedCallback() { this.render(); }

  _scratchTokens() {
    const t = {
      'color-critical': { $type: 'color',
        primary: { $value: '#0066cc', $type: 'color', $description: 'Primary brand color' },
        background: { $value: '#ffffff', $type: 'color', $description: 'Page background' },
        text: { $value: '#1a1a1a', $type: 'color', $description: 'Body text color' } },
      spacing: { $type: 'dimension',
        sm: { $value: '4px', $type: 'dimension' },
        md: { $value: '8px', $type: 'dimension' },
        lg: { $value: '16px', $type: 'dimension' } },
      typography: { $type: 'fontFamily',
        body: { $value: 'system-ui, sans-serif', $type: 'fontFamily' },
        heading: { $value: 'system-ui, sans-serif', $type: 'fontFamily' } },
    };
    if (!this._includeCritical) { delete t['color-critical']; }
    return t;
  }

  _templateTokens(name) {
    const base = this._scratchTokens();
    const extras = {
      default: {},
      blog: { 'color-critical': { ...base['color-critical'],
        link: { $value: '#0066cc', $type: 'color', $description: 'Link color' } } },
      docs: { spacing: { ...base.spacing,
        xl: { $value: '32px', $type: 'dimension' },
        xxl: { $value: '64px', $type: 'dimension' } } },
      portfolio: { 'color-critical': { ...base['color-critical'],
        accent: { $value: '#ff6600', $type: 'color', $description: 'Portfolio accent' } } },
    };
    return { ...base, ...(extras[name] || {}) };
  }

  _tokenCount(tokens) {
    let n = 0;
    const walk = (obj) => { for (const [k, v] of Object.entries(obj)) {
      if (k.startsWith('$')) continue;
      if (typeof v === 'object' && v !== null) { if (v.$value !== undefined) n++; else walk(v); }
    }};
    walk(tokens); return n;
  }

  _groupNames(tokens) {
    return Object.keys(tokens).filter(k => !k.startsWith('$'));
  }

  _validateName(name) {
    if (!name) return 'Theme name is required';
    if (!/^[a-z0-9]+(-[a-z0-9]+)*$/.test(name)) return 'Use kebab-case (e.g. my-theme)';
    return '';
  }

  _onFileSelected(file) {
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      try {
        this._tokens = JSON.parse(reader.result);
        this._error = '';
        this._step = 2;
        this.render();
      } catch (e) {
        this._error = 'Invalid JSON file: ' + e.message;
        this.render();
      }
    };
    reader.readAsText(file);
  }

  _dispatch(name, detail) {
    this.dispatchEvent(new CustomEvent(name, { bubbles: true, composed: true, detail }));
  }

  _cancel() { this._dispatch('theme-cancelled'); }

  _create() {
    const err = this._validateName(this._themeName);
    if (err) { this._error = err; this.render(); return; }
    this._dispatch('theme-created', { name: this._themeName, tokens: this._tokens });
  }

  _esc(s) { return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/"/g, '&quot;'); }

  render() {
    const step = this._step;
    const dots = [1, 2, 3].map(i =>
      `<span class="dot${i === step ? ' active' : ''}"></span>`
    ).join('');

    let body = '';
    if (step === 1) {
      body = `
        <h3>Choose a starting point</h3>
        <div class="source-options">
          ${[['upload','&#128193;','Upload tokens.json'],['scratch','&#9997;','Start from scratch'],
            ['template','&#128196;','Start from template']].map(([s,i,l]) =>
            `<button class="source-btn" data-source="${s}"><span class="source-icon">${i}</span><span class="source-label">${l}</span></button>`
          ).join('')}
        </div>
        ${this._source === 'upload' ? `
          <div class="upload-zone" id="drop-zone">
            <p>Drag & drop a .json file here</p>
            <p class="upload-or">or</p>
            <button class="pick-btn">Choose file</button>
            <input type="file" accept=".json" class="file-input" style="display:none" />
          </div>` : ''}
        ${this._source === 'template' ? `
          <div class="template-options">
            ${['default', 'blog', 'docs', 'portfolio'].map(t =>
              `<button class="tmpl-btn" data-tmpl="${t}">${t}</button>`
            ).join('')}
          </div>` : ''}
        ${this._error ? `<p class="error">${this._esc(this._error)}</p>` : ''}`;
    } else if (step === 2) {
      const count = this._tokens ? this._tokenCount(this._tokens) : 0;
      const groups = this._tokens ? this._groupNames(this._tokens) : [];
      body = `
        <h3>Name your theme</h3>
        <label class="field-label">Theme name (kebab-case)</label>
        <input type="text" class="name-input" placeholder="my-theme"
               value="${this._esc(this._themeName)}" />
        <label class="checkbox-label">
          <input type="checkbox" class="critical-check" ${this._includeCritical ? 'checked' : ''} />
          Include -critical groups
        </label>
        <div class="preview-info">
          <span>${count} token${count !== 1 ? 's' : ''}</span>
          <span class="sep">|</span>
          <span>${groups.length} group${groups.length !== 1 ? 's' : ''}: ${groups.join(', ')}</span>
        </div>
        ${this._error ? `<p class="error">${this._esc(this._error)}</p>` : ''}
        <div class="step-actions">
          <button class="secondary-btn back-btn">Back</button>
          <button class="primary-btn next-btn">Next</button>
        </div>`;
    } else if (step === 3) {
      const count = this._tokens ? this._tokenCount(this._tokens) : 0;
      const groups = this._tokens ? this._groupNames(this._tokens) : [];
      body = `
        <h3>Confirm</h3>
        <dl class="summary">
          <dt>Name</dt><dd>${this._esc(this._themeName)}</dd>
          <dt>Tokens</dt><dd>${count}</dd>
          <dt>Groups</dt><dd>${groups.join(', ')}</dd>
        </dl>
        ${this._error ? `<p class="error">${this._esc(this._error)}</p>` : ''}
        <div class="step-actions">
          <button class="secondary-btn back-btn">Back</button>
          <button class="primary-btn create-btn">Create</button>
        </div>`;
    }

    this.shadowRoot.innerHTML = `<style>
      :host { position:fixed; inset:0; z-index:9999; display:flex; align-items:center; justify-content:center; }
      .overlay { position:absolute; inset:0; background:rgba(0,0,0,0.45); }
      .card { position:relative; background:#fff; border-radius:8px; max-width:500px; width:90%;
        padding:24px; box-shadow:0 8px 32px rgba(0,0,0,0.18); font-family:system-ui,sans-serif; color:#333; z-index:1; }
      .close-btn { position:absolute; top:12px; right:12px; border:none; background:none;
        font-size:20px; cursor:pointer; color:#999; line-height:1; }
      .close-btn:hover { color:#333; }
      .dots { text-align:center; margin-bottom:16px; display:flex; gap:8px; justify-content:center; }
      .dot { width:10px; height:10px; border-radius:50%; background:#ddd; display:inline-block; }
      .dot.active { background:#0066cc; }
      h3 { margin:0 0 16px; font-size:18px; font-weight:600; }
      .source-options { display:flex; flex-direction:column; gap:8px; margin-bottom:12px; }
      .source-btn { display:flex; align-items:center; gap:12px; padding:12px; border:1px solid #e0e0e0;
        border-radius:6px; background:#fafafa; cursor:pointer; font-size:14px; text-align:left; }
      .source-btn:hover { background:#f0f0f0; border-color:#0066cc; }
      .source-icon { font-size:20px; }
      .source-label { font-weight:500; }
      .upload-zone { border:2px dashed #ccc; border-radius:6px; padding:24px; text-align:center;
        margin-top:12px; transition:border-color 0.2s; }
      .upload-zone.dragover { border-color:#0066cc; background:#f0f7ff; }
      .upload-or { font-size:12px; color:#888; margin:8px 0; }
      .pick-btn { padding:6px 16px; border:1px solid #0066cc; border-radius:4px; background:#fff;
        color:#0066cc; cursor:pointer; font-size:13px; }
      .pick-btn:hover { background:#f0f7ff; }
      .template-options { display:flex; gap:8px; flex-wrap:wrap; margin-top:12px; }
      .tmpl-btn { padding:8px 16px; border:1px solid #e0e0e0; border-radius:4px; background:#fff;
        cursor:pointer; font-size:13px; text-transform:capitalize; }
      .tmpl-btn:hover { border-color:#0066cc; background:#f0f7ff; }
      .field-label { display:block; font-size:13px; color:#555; margin-bottom:4px; }
      .name-input { width:100%; padding:8px; border:1px solid #e0e0e0; border-radius:4px;
        font-size:14px; box-sizing:border-box; margin-bottom:12px; }
      .name-input:focus { outline:none; border-color:#0066cc; box-shadow:0 0 0 2px rgba(0,102,204,.15); }
      .checkbox-label { display:flex; align-items:center; gap:6px; font-size:13px; color:#555; margin-bottom:12px; cursor:pointer; }
      .preview-info { font-size:12px; color:#888; margin-bottom:16px; } .sep { margin:0 6px; }
      .step-actions { display:flex; justify-content:flex-end; gap:8px; margin-top:16px; }
      .primary-btn { padding:8px 20px; border:none; border-radius:4px; background:#0066cc; color:#fff; cursor:pointer; font-size:14px; }
      .primary-btn:hover { background:#0052a3; }
      .secondary-btn { padding:8px 20px; border:1px solid #e0e0e0; border-radius:4px; background:#fff; color:#333; cursor:pointer; font-size:14px; }
      .secondary-btn:hover { background:#f0f0f0; }
      .error { color:#dc3545; font-size:13px; margin:8px 0 0; }
      .summary { margin:0 0 8px; } .summary dt { font-size:12px; color:#888; margin:8px 0 2px; }
      .summary dd { margin:0; font-size:14px; }
    </style>
    <div class="overlay"></div>
    <div class="card">
      <button class="close-btn" title="Cancel">&times;</button>
      <div class="dots">${dots}</div>
      ${body}
    </div>`;

    this._bind();
  }

  _bind() {
    const sr = this.shadowRoot;
    sr.querySelector('.overlay')?.addEventListener('click', () => this._cancel());
    sr.querySelector('.close-btn')?.addEventListener('click', () => this._cancel());

    // Step 1
    sr.querySelectorAll('.source-btn').forEach(btn => {
      btn.addEventListener('click', () => {
        const src = btn.dataset.source;
        if (src === 'scratch') {
          this._source = 'scratch';
          this._tokens = this._scratchTokens();
          this._step = 2; this.render();
        } else if (src === 'upload') {
          this._source = 'upload'; this.render();
        } else if (src === 'template') {
          this._source = 'template'; this.render();
        }
      });
    });

    // Upload zone
    const zone = sr.querySelector('#drop-zone');
    if (zone) {
      const fi = sr.querySelector('.file-input');
      sr.querySelector('.pick-btn')?.addEventListener('click', () => fi?.click());
      fi?.addEventListener('change', () => { if (fi.files[0]) this._onFileSelected(fi.files[0]); });
      zone.addEventListener('dragover', (e) => { e.preventDefault(); zone.classList.add('dragover'); });
      zone.addEventListener('dragleave', () => zone.classList.remove('dragover'));
      zone.addEventListener('drop', (e) => {
        e.preventDefault(); zone.classList.remove('dragover');
        const f = e.dataTransfer?.files?.[0];
        if (f) this._onFileSelected(f);
      });
    }

    // Template buttons
    sr.querySelectorAll('.tmpl-btn').forEach(btn => {
      btn.addEventListener('click', () => {
        this._tokens = this._templateTokens(btn.dataset.tmpl);
        this._step = 2; this.render();
      });
    });

    // Step 2
    const nameInput = sr.querySelector('.name-input');
    if (nameInput) {
      nameInput.addEventListener('input', () => { this._themeName = nameInput.value.trim(); });
      nameInput.focus();
    }
    sr.querySelector('.critical-check')?.addEventListener('change', (e) => {
      this._includeCritical = e.target.checked;
      if (this._source === 'scratch') this._tokens = this._scratchTokens();
      this.render();
    });
    sr.querySelector('.next-btn')?.addEventListener('click', () => {
      const err = this._validateName(this._themeName);
      if (err) { this._error = err; this.render(); return; }
      this._error = ''; this._step = 3; this.render();
    });

    // Step 3
    sr.querySelector('.create-btn')?.addEventListener('click', () => this._create());

    // Back
    sr.querySelector('.back-btn')?.addEventListener('click', () => {
      this._error = ''; this._step--; this.render();
    });
  }
}
customElements.define('geoff-create-theme', GeoffCreateTheme);

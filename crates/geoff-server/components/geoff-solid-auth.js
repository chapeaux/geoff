/** <geoff-solid-auth> — Solid pod authentication and storage. */
class GeoffSolidAuth extends HTMLElement {
  constructor() {
    super();
    const internals = this.attachInternals?.();
    if (!internals?.shadowRoot) {
      this.attachShadow({ mode: 'open' });
    }
    const ls = typeof localStorage !== 'undefined' ? localStorage : null;
    Object.assign(this, {
      _podUrl: ls?.getItem('geoff-solid-pod') || 'https://paa.pub',
      _authenticated: false, _checking: false, _error: null, _themes: [],
    });
  }

  get podUrl() { return this._podUrl; }
  set podUrl(v) { this._podUrl = v; this.render(); }
  get authenticated() { return this._authenticated; }

  connectedCallback() {
    if (typeof localStorage === 'undefined') { this.render(); return; }
    const token = localStorage.getItem('geoff-solid-token');
    const pod = localStorage.getItem('geoff-solid-pod');
    if (token && pod) { this._podUrl = pod; this._checkAccess(); }
    else this.render();
  }

  async _solidFetch(url, options = {}) {
    const token = localStorage.getItem('geoff-solid-token');
    const headers = { ...options.headers };
    if (token) headers['Authorization'] = `Bearer ${token}`;
    return fetch(url, { ...options, headers });
  }

  _normalizePodUrl(url) {
    let u = url.trim();
    if (!u.startsWith('http')) u = 'https://' + u;
    if (!u.endsWith('/')) u += '/';
    return u;
  }

  async _checkAccess() {
    this._checking = true; this._error = null; this.render();
    try {
      const base = this._normalizePodUrl(this._podUrl);
      const res = await this._solidFetch(base);
      if (res.ok || res.status === 403) {
        this._podUrl = base; this._authenticated = true;
        localStorage.setItem('geoff-solid-pod', base);
        this.dispatchEvent(new CustomEvent('solid-authenticated', {
          bubbles: true, composed: true, detail: { podUrl: base } }));
      } else { this._error = `Could not reach pod (HTTP ${res.status})`; }
    } catch (e) { this._error = `Connection failed: ${e.message}`; }
    this._checking = false; this.render();
  }

  async saveTokens(name, tokensJson) {
    const base = this._normalizePodUrl(this._podUrl);
    const url = `${base}geoff/themes/${encodeURIComponent(name)}/tokens.json`;
    const res = await this._solidFetch(url, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(tokensJson),
    });
    if (!res.ok) throw new Error(`Save failed (HTTP ${res.status})`);
    this.dispatchEvent(new CustomEvent('solid-tokens-saved', {
      bubbles: true, composed: true, detail: { name }
    }));
    return true;
  }

  async loadTokens(name) {
    const base = this._normalizePodUrl(this._podUrl);
    const url = `${base}geoff/themes/${encodeURIComponent(name)}/tokens.json`;
    const res = await this._solidFetch(url);
    if (!res.ok) throw new Error(`Load failed (HTTP ${res.status})`);
    const tokens = await res.json();
    this.dispatchEvent(new CustomEvent('solid-tokens-loaded', {
      bubbles: true, composed: true, detail: { name, tokens }
    }));
    return tokens;
  }

  async listThemes() {
    const base = this._normalizePodUrl(this._podUrl);
    const url = `${base}geoff/themes/`;
    try {
      const res = await this._solidFetch(url, {
        headers: { 'Accept': 'text/turtle, application/ld+json' }
      });
      if (!res.ok) return [];
      const text = await res.text();
      // Parse container listing — look for ldp:contains references
      const names = [];
      const containsRe = /ldp:contains\s+<([^>]+)>/g;
      let m;
      while ((m = containsRe.exec(text)) !== null) {
        const seg = m[1].replace(/\/$/, '').split('/').pop();
        if (seg) names.push(decodeURIComponent(seg));
      }
      // Fallback: look for href patterns in HTML directory listings
      if (names.length === 0) {
        const hrefRe = /href="([^"]+\/?)"/g;
        while ((m = hrefRe.exec(text)) !== null) {
          const seg = m[1].replace(/\/$/, '').split('/').pop();
          if (seg && seg !== '..' && seg !== '.') names.push(decodeURIComponent(seg));
        }
      }
      this._themes = names;
      return names;
    } catch { return []; }
  }

  _connect() {
    const input = this.shadowRoot.querySelector('.pod-input');
    const tokenInput = this.shadowRoot.querySelector('.token-input');
    if (input) this._podUrl = input.value;
    if (tokenInput && tokenInput.value.trim()) {
      localStorage.setItem('geoff-solid-token', tokenInput.value.trim());
    }
    this._checkAccess();
  }

  _disconnect() {
    this._authenticated = false; this._themes = []; this._error = null;
    localStorage.removeItem('geoff-solid-token'); localStorage.removeItem('geoff-solid-pod');
    this.render();
  }

  _esc(s) { return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/"/g, '&quot;'); }

  render() {
    const hasToken = !!localStorage.getItem('geoff-solid-token');
    this.shadowRoot.innerHTML = `<style>
      :host { display:block; font-family:system-ui,sans-serif; font-size:13px; }
      .panel { padding:12px; background:#fafafa; border:1px solid #e0e0e0; border-radius:6px; }
      label { display:block; font-weight:500; margin-bottom:4px; color:#555; }
      input { width:100%; padding:6px 8px; border:1px solid #ddd; border-radius:4px; font-size:13px;
              box-sizing:border-box; margin-bottom:8px; }
      input:focus { outline:none; border-color:#0066cc; box-shadow:0 0 0 2px rgba(0,102,204,.15); }
      button { padding:6px 14px; border:1px solid #ddd; border-radius:4px; background:#fff;
               cursor:pointer; font-size:13px; }
      button:hover { background:#f0f0f0; }
      button.primary { background:#0066cc; color:#fff; border-color:#0066cc; }
      button.primary:hover { background:#0055b3; }
      button.danger { color:#dc3545; border-color:#dc3545; }
      button.danger:hover { background:#dc3545; color:#fff; }
      .actions { display:flex; gap:6px; flex-wrap:wrap; margin-top:4px; }
      .webid { font-family:monospace; font-size:12px; color:#333; word-break:break-all;
                padding:6px 8px; background:#e8f4fd; border-radius:4px; margin-bottom:8px; }
      .error { color:#dc3545; font-size:12px; margin-bottom:8px; }
      .link { font-size:12px; color:#0066cc; text-decoration:none; }
      .link:hover { text-decoration:underline; }
      .hint { font-size:11px; color:#888; margin-top:2px; margin-bottom:8px; }
      .theme-list { margin-top:8px; }
      .theme-item { display:flex; align-items:center; justify-content:space-between; padding:4px 8px;
                     background:#fff; border:1px solid #e0e0e0; border-radius:3px; margin-bottom:4px; }
      .theme-item span { font-family:monospace; font-size:12px; }
    </style>
    <div class="panel">
    ${this._authenticated ? this._renderAuthenticated() : this._renderLogin(hasToken)}
    </div>`;

    this._bindEvents();
  }

  _renderLogin(hasToken) {
    return `
      <label for="pod-url">Solid Pod URL</label>
      <input class="pod-input" id="pod-url" type="url" value="${this._esc(this._podUrl)}"
             placeholder="https://paa.pub/username/" />
      <label for="bearer-token">Bearer Token${hasToken ? ' (stored)' : ''}</label>
      <input class="token-input" id="bearer-token" type="password"
             placeholder="${hasToken ? 'Using stored token' : 'Paste token from pod settings'}" />
      <div class="hint">Generate a token from your pod's settings page.</div>
      ${this._error ? `<div class="error">${this._esc(this._error)}</div>` : ''}
      <div class="actions">
        <button class="primary connect-btn" ${this._checking ? 'disabled' : ''}>
          ${this._checking ? 'Connecting...' : 'Connect to Solid'}
        </button>
      </div>
      <div style="margin-top:8px">
        <a class="link" href="https://paa.pub" target="_blank" rel="noopener">
          Don't have a pod? Create one at paa.pub
        </a>
      </div>`;
  }

  _renderAuthenticated() {
    const themes = this._themes.map(t =>
      `<div class="theme-item"><span>${this._esc(t)}</span>
       <button class="load-theme-btn" data-name="${this._esc(t)}">Load</button></div>`
    ).join('');
    return `
      <label>Connected to</label>
      <div class="webid">${this._esc(this._podUrl)}</div>
      ${this._error ? `<div class="error">${this._esc(this._error)}</div>` : ''}
      <div class="actions">
        <button class="primary save-btn">Save Tokens</button>
        <button class="list-btn">Load from Solid</button>
        <button class="danger disconnect-btn">Disconnect</button>
      </div>
      ${this._themes.length > 0 ? `<div class="theme-list">${themes}</div>` : ''}`;
  }

  _bindEvents() {
    this.shadowRoot.querySelector('.connect-btn')
      ?.addEventListener('click', () => this._connect());
    this.shadowRoot.querySelector('.disconnect-btn')
      ?.addEventListener('click', () => this._disconnect());
    this.shadowRoot.querySelector('.save-btn')
      ?.addEventListener('click', () => {
        this.dispatchEvent(new CustomEvent('solid-save-requested', {
          bubbles: true, composed: true
        }));
      });
    this.shadowRoot.querySelector('.list-btn')
      ?.addEventListener('click', async () => {
        this._error = null;
        try {
          await this.listThemes();
        } catch (e) { this._error = e.message; }
        this.render();
      });
    this.shadowRoot.querySelectorAll('.load-theme-btn').forEach(btn => {
      btn.addEventListener('click', async () => {
        const name = btn.dataset.name;
        try {
          await this.loadTokens(name);
        } catch (e) { this._error = e.message; this.render(); }
      });
    });
    // Allow Enter in inputs to connect
    const podInput = this.shadowRoot.querySelector('.pod-input');
    const tokenInput = this.shadowRoot.querySelector('.token-input');
    [podInput, tokenInput].forEach(el => {
      el?.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') this._connect();
      });
    });
  }
}
customElements.define('geoff-solid-auth', GeoffSolidAuth);

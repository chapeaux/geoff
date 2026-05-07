/**
 * <geoff-theme-preview> — Live preview panel for theme editing.
 *
 * Displays template pages in iframes and injects CSS updates
 * received via postMessage or the updateCSS() method.
 * Supports adding remote URLs and uploading HTML files as tabs.
 *
 * Attributes:
 *   templates — JSON array of { name, path } objects for the tab bar
 *
 * Methods:
 *   updateCSS(css) — inject CSS into all preview iframes
 */
class GeoffThemePreview extends HTMLElement {
  static get observedAttributes() { return ['templates']; }

  constructor() {
    super();
    const internals = this.attachInternals?.();
    if (!internals?.shadowRoot) {
      this.attachShadow({ mode: 'open' });
    }
    this._activeTab = 0;
    this._css = '';
    this._templates = [{ name: 'Home', path: '/' }];
    this._customTabs = [];
    this._showUrlInput = false;
    this._messageHandler = (e) => this._onMessage(e);
  }

  async connectedCallback() {
    this._parseTemplates();
    if (typeof window !== 'undefined') {
      await this._discoverPages();
      this.render();
      window.addEventListener('message', this._messageHandler);
    } else {
      this.render();
    }
  }

  disconnectedCallback() {
    if (typeof window !== 'undefined') {
      window.removeEventListener('message', this._messageHandler);
    }
  }

  attributeChangedCallback() {
    this._parseTemplates();
    if (this.shadowRoot.children.length) this.render();
  }

  _parseTemplates() {
    const attr = this.getAttribute('templates');
    if (attr) {
      try { this._templates = JSON.parse(attr); } catch { /* keep defaults */ }
    }
  }

  async _discoverPages() {
    if (this.getAttribute('templates')) return;
    try {
      const res = await fetch('/api/pages');
      if (!res.ok) return;
      const pages = await res.json();
      if (Array.isArray(pages) && pages.length > 0) {
        const seen = new Set();
        const tabs = [];
        for (const p of pages) {
          const url = '/' + p.path.replace(/\.md$/, '.html').replace(/\\/g, '/');
          const name = p.title || p.path;
          if (!seen.has(url)) { seen.add(url); tabs.push({ name, path: url }); }
          if (tabs.length >= 8) break;
        }
        if (tabs.length > 0) this._templates = tabs;
      }
    } catch { /* keep defaults */ }
  }

  _allTabs() { return [...this._templates, ...this._customTabs]; }

  /** Public method: inject CSS into all preview iframes. */
  updateCSS(css) {
    this._css = css || '';
    this._injectCSS();
  }

  _onMessage(e) {
    if (e.data && e.data.type === 'geoff-css-update' && e.data.css) {
      this.updateCSS(e.data.css);
    }
  }

  _injectCSS() {
    const iframes = this.shadowRoot.querySelectorAll('iframe');
    const tabs = this._allTabs();
    iframes.forEach((iframe, i) => {
      const tab = tabs[i];
      if (tab && tab.type === 'url') {
        // Proxied URLs: reload the iframe to re-fetch with updated CSS
        const src = iframe.getAttribute('src');
        if (src) iframe.src = src;
      } else {
        try {
          iframe.contentWindow.postMessage(
            { type: 'geoff-inject-css', css: this._css }, '*'
          );
        } catch { /* cross-origin, skip */ }
      }
    });
  }

  _switchTab(index) {
    this._activeTab = index;
    this._showUrlInput = false;
    this.render();
  }

  _removeTab(index) {
    const customIndex = index - this._templates.length;
    if (customIndex < 0 || customIndex >= this._customTabs.length) return;
    const tab = this._customTabs[customIndex];
    if (tab.type === 'upload' && tab.path.startsWith('blob:')) {
      URL.revokeObjectURL(tab.path);
    }
    this._customTabs.splice(customIndex, 1);
    if (this._activeTab >= this._allTabs().length) {
      this._activeTab = Math.max(0, this._allTabs().length - 1);
    }
    this.render();
  }

  _addUrlTab() {
    const input = this.shadowRoot.querySelector('.url-input');
    if (!input) return;
    const url = input.value.trim();
    if (!url) return;
    const proxyPath = `/api/theme/proxy?url=${encodeURIComponent(url)}`;
    this._customTabs.push({
      name: new URL(url, location.href).hostname || url.slice(0, 20),
      path: proxyPath, type: 'url', removable: true, originalUrl: url
    });
    this._activeTab = this._allTabs().length - 1;
    this._showUrlInput = false;
    this.render();
  }

  async _uploadHtml() {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.html,.htm';
    input.onchange = async () => {
      const file = input.files[0];
      if (!file) return;
      const text = await file.text();
      // Inject current theme CSS into the HTML before creating the blob
      let html = text;
      if (this._css) {
        const style = `<style data-geoff-theme>${this._css}</style>`;
        if (html.includes('</head>')) {
          html = html.replace('</head>', `${style}\n</head>`);
        } else if (html.includes('<body')) {
          html = html.replace('<body', `${style}\n<body`);
        } else {
          html = style + '\n' + html;
        }
      }
      const blob = new Blob([html], { type: 'text/html' });
      const blobUrl = URL.createObjectURL(blob);
      this._customTabs.push({
        name: file.name.replace(/\.(html?|htm)$/i, ''),
        path: blobUrl, type: 'upload', removable: true, rawHtml: text
      });
      this._activeTab = this._allTabs().length - 1;
      this.render();
    };
    input.click();
  }

  _esc(s) { return String(s).replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;'); }

  render() {
    const tabs = this._allTabs();
    const active = this._activeTab;
    const current = tabs[active];

    this.shadowRoot.innerHTML = `
      <style>
        :host { display: flex; flex-direction: column; height: 100%;
          background: #fff; font-family: system-ui, sans-serif; }
        .tab-bar {
          display: flex; gap: 0; background: #f5f5f5;
          border-bottom: 1px solid #e0e0e0; padding: 0 8px;
          flex-shrink: 0; flex-wrap: wrap; align-items: center;
        }
        .tab {
          padding: 8px 16px; border: none; background: transparent;
          font-size: 13px; color: #666; cursor: pointer;
          border-bottom: 2px solid transparent;
          transition: color 0.15s, border-color 0.15s;
          display: inline-flex; align-items: center; gap: 4px;
        }
        .tab:hover { color: #333; }
        .tab.active { color: #0066cc; border-bottom-color: #0066cc; font-weight: 500; }
        .tab .close {
          font-size: 11px; color: #999; cursor: pointer; margin-left: 4px;
          border: none; background: none; padding: 0 2px; line-height: 1;
        }
        .tab .close:hover { color: #c00; }
        .action-btn {
          padding: 6px 10px; border: 1px dashed #ccc; background: transparent;
          font-size: 12px; color: #888; cursor: pointer; border-radius: 3px;
          margin-left: 4px;
        }
        .action-btn:hover { color: #333; border-color: #999; }
        .url-bar {
          display: flex; gap: 6px; padding: 6px 8px; background: #fafafa;
          border-bottom: 1px solid #e0e0e0; align-items: center;
        }
        .url-bar input {
          flex: 1; padding: 4px 8px; border: 1px solid #ccc; border-radius: 3px;
          font-size: 13px;
        }
        .url-bar button {
          padding: 4px 12px; background: #0066cc; color: #fff; border: none;
          border-radius: 3px; font-size: 13px; cursor: pointer;
        }
        .url-bar button:hover { background: #0052a3; }
        .url-bar .cors-note { font-size: 11px; color: #999; }
        .preview-container { flex: 1; position: relative; overflow: hidden; }
        iframe {
          position: absolute; top: 0; left: 0;
          width: 100%; height: 100%; border: none; background: #fff;
        }
        .no-preview {
          display: flex; align-items: center; justify-content: center;
          height: 100%; color: #888; font-size: 14px;
        }
      </style>
      <div class="tab-bar">
        ${tabs.map((t, i) => `
          <button class="tab ${i === active ? 'active' : ''}" data-index="${i}">
            ${this._esc(t.name)}${t.removable
              ? `<span class="close" data-remove="${i}">×</span>` : ''}
          </button>`).join('')}
        <button class="action-btn" data-action="url">+ URL</button>
        <button class="action-btn" data-action="upload">+ Upload HTML</button>
      </div>
      ${this._showUrlInput ? `
        <div class="url-bar">
          <input class="url-input" type="url" placeholder="https://example.com" />
          <button class="url-add">Add</button>
          <span class="cors-note">Note: some sites may block iframe embedding</span>
        </div>` : ''}
      <div class="preview-container">
        ${current
          ? `<iframe src="${this._esc(current.path)}" sandbox="allow-same-origin allow-scripts"></iframe>`
          : '<div class="no-preview">No preview templates available</div>'}
      </div>`;

    // Tab click handlers
    this.shadowRoot.querySelectorAll('.tab').forEach(tab => {
      tab.addEventListener('click', (e) => {
        if (e.target.classList.contains('close')) return;
        this._switchTab(Number(tab.dataset.index));
      });
    });

    // Close button handlers
    this.shadowRoot.querySelectorAll('.close').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        this._removeTab(Number(btn.dataset.remove));
      });
    });

    // Action button handlers
    this.shadowRoot.querySelectorAll('.action-btn').forEach(btn => {
      btn.addEventListener('click', () => {
        if (btn.dataset.action === 'url') {
          this._showUrlInput = !this._showUrlInput;
          this.render();
        } else if (btn.dataset.action === 'upload') {
          this._uploadHtml();
        }
      });
    });

    // URL input handlers
    const urlAdd = this.shadowRoot.querySelector('.url-add');
    if (urlAdd) {
      urlAdd.addEventListener('click', () => this._addUrlTab());
      const urlInput = this.shadowRoot.querySelector('.url-input');
      if (urlInput) {
        urlInput.addEventListener('keydown', (e) => {
          if (e.key === 'Enter') this._addUrlTab();
        });
        requestAnimationFrame(() => urlInput.focus());
      }
    }

    // Inject CSS once iframe loads
    const iframe = this.shadowRoot.querySelector('iframe');
    if (iframe && this._css) {
      iframe.addEventListener('load', () => this._injectCSS());
    }
  }
}

customElements.define('geoff-theme-preview', GeoffThemePreview);

/**
 * <geoff-theme-editor-app> — App shell for the visual theme editor.
 *
 * Three-pane layout: sidebar tree | token editor | live preview.
 * Manages WebSocket connection for live reload and routes events
 * between panes.
 */
class GeoffThemeEditorApp extends HTMLElement {
  constructor() {
    super();
    const internals = this.attachInternals?.();
    if (!internals?.shadowRoot) {
      this.attachShadow({ mode: 'open' });
    }
    this._splitRatio = 0.45;
    this._dragging = false;
    this._ws = null;
    this._wsRetryDelay = 1000;
    this._treeWidth = 240;
  }

  connectedCallback() {
    this.render();
    if (typeof window === 'undefined') return;
    this._setupDragHandle();
    this._setupCSSBridge();
    this._setupTreeBridge();
    this._connectWebSocket();
  }

  disconnectedCallback() {
    if (this._ws) { this._ws.close(); this._ws = null; }
  }

  _connectWebSocket() {
    try {
      const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
      this._ws = new WebSocket(`${proto}//${location.host}/ws`);
      this._ws.addEventListener('message', (e) => {
        let msg;
        try { msg = JSON.parse(e.data); } catch { return; }
        if (msg.type === 'reload' || msg.type === 'css-update') {
          const preview = this.shadowRoot.querySelector('geoff-theme-preview');
          if (msg.css && preview) preview.updateCSS(msg.css);
          if (msg.type === 'reload') {
            const iframe = preview?.shadowRoot?.querySelector('iframe');
            if (iframe) iframe.src = iframe.src;
          }
        }
      });
      this._ws.addEventListener('close', () => {
        setTimeout(() => this._connectWebSocket(), this._wsRetryDelay);
        this._wsRetryDelay = Math.min(this._wsRetryDelay * 2, 30000);
      });
      this._ws.addEventListener('open', () => { this._wsRetryDelay = 1000; });
      this._ws.addEventListener('error', () => {});
    } catch {}
  }

  _setupCSSBridge() {
    this.shadowRoot.addEventListener('css-update', (e) => {
      const preview = this.shadowRoot.querySelector('geoff-theme-preview');
      if (preview && e.detail?.css) preview.updateCSS(e.detail.css);
    });
  }

  _setupTreeBridge() {
    this.shadowRoot.addEventListener('tree-navigate', (e) => {
      const editor = this.shadowRoot.querySelector('geoff-token-editor');
      if (editor && e.detail?.group) {
        editor.navigateToGroup(e.detail.group);
      }
    });
    this.shadowRoot.addEventListener('tree-select', (e) => {
      const editor = this.shadowRoot.querySelector('geoff-token-editor');
      if (editor && e.detail?.path) {
        editor.navigateToToken(e.detail.path);
      }
    });
  }

  _setupDragHandle() {
    const handle = this.shadowRoot.querySelector('.drag-handle');
    const container = this.shadowRoot.querySelector('.main-area');
    if (!handle || !container) return;

    const onMove = (e) => {
      if (!this._dragging) return;
      e.preventDefault();
      const clientX = e.touches ? e.touches[0].clientX : e.clientX;
      const rect = container.getBoundingClientRect();
      let ratio = (clientX - rect.left) / rect.width;
      ratio = Math.max(0.2, Math.min(0.8, ratio));
      this._splitRatio = ratio;
      this._applyRatio();
    };

    const onUp = () => {
      this._dragging = false;
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
      document.removeEventListener('touchmove', onMove);
      document.removeEventListener('touchend', onUp);
    };

    handle.addEventListener('mousedown', (e) => {
      e.preventDefault();
      this._dragging = true;
      document.body.style.cursor = 'col-resize';
      document.body.style.userSelect = 'none';
      document.addEventListener('mousemove', onMove);
      document.addEventListener('mouseup', onUp);
    });

    handle.addEventListener('touchstart', () => {
      this._dragging = true;
      document.addEventListener('touchmove', onMove, { passive: false });
      document.addEventListener('touchend', onUp);
    });
  }

  _applyRatio() {
    const editor = this.shadowRoot.querySelector('.editor-pane');
    const preview = this.shadowRoot.querySelector('.preview-pane');
    if (editor && preview) {
      editor.style.width = `${this._splitRatio * 100}%`;
      preview.style.width = `${(1 - this._splitRatio) * 100}%`;
    }
  }

  render() {
    const editorW = this._splitRatio * 100;
    const previewW = (1 - this._splitRatio) * 100;

    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; width: 100%; height: 100%; font-family: system-ui, sans-serif; }
        .container { display: flex; height: 100%; overflow: hidden; }
        .tree-pane {
          width: ${this._treeWidth}px; min-width: 200px; max-width: 360px;
          height: 100%; overflow: hidden; border-right: 1px solid #e0e0e0;
          flex-shrink: 0;
        }
        .main-area { display: flex; flex: 1; height: 100%; overflow: hidden; }
        .editor-pane {
          width: ${editorW}%; height: 100%; overflow: hidden;
          display: flex; flex-direction: column;
          border-right: 1px solid #e0e0e0;
        }
        .preview-pane {
          width: ${previewW}%; height: 100%; overflow: hidden;
          display: flex; flex-direction: column;
        }
        .drag-handle {
          width: 5px; cursor: col-resize; background: #e0e0e0;
          flex-shrink: 0; transition: background 0.15s;
        }
        .drag-handle:hover, .drag-handle:active { background: #0066cc; }
        geoff-token-tree { display: block; height: 100%; }
        geoff-token-editor { flex: 1; min-height: 0; }
        geoff-theme-preview { flex: 1; min-height: 0; }
      </style>
      <div class="container">
        <div class="tree-pane">
          <geoff-token-tree></geoff-token-tree>
        </div>
        <div class="main-area">
          <div class="editor-pane">
            <geoff-token-editor></geoff-token-editor>
          </div>
          <div class="drag-handle" title="Drag to resize"></div>
          <div class="preview-pane">
            <geoff-theme-preview></geoff-theme-preview>
          </div>
        </div>
      </div>`;

    // Sync tokens to the tree whenever the editor's tokens change
    const editor = this.shadowRoot.querySelector('geoff-token-editor');
    const tree = this.shadowRoot.querySelector('geoff-token-tree');
    if (editor && tree) {
      const syncTree = () => {
        if (editor._tokens && Object.keys(editor._tokens).length > 0) {
          tree.tokens = editor._tokens;
        }
      };
      // Watch for renders (which happen on token load, upload, create, etc.)
      const observer = new MutationObserver(syncTree);
      observer.observe(editor.shadowRoot, { childList: true, subtree: true });
      // Also listen for explicit token change events
      this.shadowRoot.addEventListener('css-update', syncTree);
      this.shadowRoot.addEventListener('theme-created', syncTree);
      // Initial sync after fetch completes
      setTimeout(syncTree, 500);
    }
  }
}

customElements.define('geoff-theme-editor-app', GeoffThemeEditorApp);

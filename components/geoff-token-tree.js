/**
 * <geoff-token-tree> — Sidebar tree navigation for design tokens.
 *
 * Displays the full token hierarchy as an expandable/collapsible tree.
 *
 * Properties:
 *   tokens      — the full nested token object
 *   activeGroup — currently highlighted group path
 *
 * Events:
 *   tree-navigate — { group } when a group node is clicked
 *   tree-select   — { path } when a leaf token is clicked
 */
class GeoffTokenTree extends HTMLElement {
  constructor() {
    super();
    const internals = this.attachInternals?.();
    if (!internals?.shadowRoot) {
      this.attachShadow({ mode: 'open' });
    }
    this._tokens = {};
    this._activeGroup = '';
    this._expanded = new Set();
  }

  get tokens() { return this._tokens; }
  set tokens(val) { this._tokens = val || {}; this.render(); }

  get activeGroup() { return this._activeGroup; }
  set activeGroup(val) { this._activeGroup = val || ''; this.render(); }

  connectedCallback() { this.render(); }

  _esc(s) { return String(s).replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;'); }

  /** Count leaf tokens (nodes with $value) under an object. */
  _countLeaves(obj) {
    let count = 0;
    for (const [k, v] of Object.entries(obj)) {
      if (k.startsWith('$')) continue;
      if (typeof v === 'object' && v !== null && !v.$value && v.$value !== '') {
        count += this._countLeaves(v);
      } else {
        count++;
      }
    }
    return count;
  }

  /** Check if a node is a leaf token (has $value or is a primitive). */
  _isLeaf(val) {
    if (typeof val !== 'object' || val === null) return true;
    return val.$value !== undefined;
  }

  /** Get resolved color value for a token if it's a color type. */
  _colorDot(val) {
    if (typeof val !== 'object' || val === null) return '';
    if (val.$type !== 'color' && !(/^#[0-9a-fA-F]{3,8}$/.test(val.$value || ''))) return '';
    const color = val.$value || '';
    if (!color) return '';
    return `<span class="color-dot" style="background:${this._esc(color)}"></span>`;
  }

  /** Build tree HTML recursively. */
  _buildTree(obj, prefix, depth) {
    const entries = Object.entries(obj).filter(([k]) => !k.startsWith('$'));
    if (!entries.length) return '';

    return entries.map(([key, val]) => {
      // "_" is Style Dictionary's group-level root token — use parent path as the token path
      const path = key === '_' ? (prefix || key) : (prefix ? `${prefix}.${key}` : key);
      const displayName = key === '_' ? '(default)' : key;

      if (this._isLeaf(val)) {
        const isActive = this._activeGroup === path;
        return `<div class="node leaf" data-path="${this._esc(path)}"
          style="padding-left:${depth * 16 + 20}px"
          ${isActive ? 'data-active' : ''}>
          ${this._colorDot(val)}
          <span class="node-name">${this._esc(displayName)}</span>
        </div>`;
      }

      const count = this._countLeaves(val);
      const open = this._expanded.has(path);
      const isActive = this._activeGroup === path;
      const children = open ? this._buildTree(val, path, depth + 1) : '';

      return `<div class="node group" data-path="${this._esc(path)}"
          style="padding-left:${depth * 16}px"
          ${isActive ? 'data-active' : ''}>
        <span class="arrow${open ? ' open' : ''}">&#9654;</span>
        <span class="node-name">${this._esc(displayName)}</span>
        <span class="count">${count}</span>
      </div>${children}`;
    }).join('');
  }

  _expandAll(obj, prefix) {
    for (const [k, v] of Object.entries(obj)) {
      if (k.startsWith('$')) continue;
      const path = k === '_' ? (prefix || k) : (prefix ? `${prefix}.${k}` : k);
      if (!this._isLeaf(v)) {
        this._expanded.add(path);
        this._expandAll(v, path);
      }
    }
  }

  render() {
    const treeHTML = this._buildTree(this._tokens, '', 0);

    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; height: 100%; overflow-y: auto;
          background: #fafafa; font-family: system-ui, sans-serif; }
        .toolbar { display: flex; gap: 4px; padding: 6px 8px;
          border-bottom: 1px solid #e0e0e0; background: #fff;
          position: sticky; top: 0; z-index: 1; }
        .toolbar button { font-size: 12px; padding: 2px 8px;
          border: 1px solid #e0e0e0; border-radius: 3px;
          background: #fff; color: #555; cursor: pointer; }
        .toolbar button:hover { background: #f0f0f0; }
        .tree { padding: 4px 0; }
        .node { display: flex; align-items: center; gap: 4px;
          height: 24px; padding-right: 8px; cursor: pointer;
          font-size: 14px; color: #333; user-select: none;
          white-space: nowrap; }
        .node:hover { background: #e8e8e8; }
        .node[data-active] { background: #e0ecff; }
        .arrow { font-size: 12px; width: 16px; text-align: center;
          color: #888; transition: transform 0.12s; flex-shrink: 0; }
        .arrow.open { transform: rotate(90deg); }
        .node-name { font-family: monospace; overflow: hidden;
          text-overflow: ellipsis; }
        .leaf .node-name { color: #555; }
        .count { font-size: 12px; color: #999; margin-left: auto;
          flex-shrink: 0; }
        .color-dot { width: 8px; height: 8px; border-radius: 50%;
          border: 1px solid #ddd; flex-shrink: 0; display: inline-block; }
        .empty { font-size: 14px; color: #888; padding: 16px;
          text-align: center; }
      </style>
      <div class="toolbar">
        <button data-action="expand-all">Expand all</button>
        <button data-action="collapse-all">Collapse all</button>
      </div>
      <div class="tree">
        ${treeHTML || '<div class="empty">No tokens loaded</div>'}
      </div>`;

    this._attachListeners();
  }

  _attachListeners() {
    const root = this.shadowRoot;

    root.querySelector('[data-action="expand-all"]')?.addEventListener('click', () => {
      this._expandAll(this._tokens, '');
      this.render();
    });

    root.querySelector('[data-action="collapse-all"]')?.addEventListener('click', () => {
      this._expanded.clear();
      this.render();
    });

    root.querySelectorAll('.node.group').forEach(el => {
      el.addEventListener('click', () => {
        const path = el.dataset.path;
        if (this._expanded.has(path)) {
          this._expanded.delete(path);
        } else {
          this._expanded.add(path);
        }
        this.dispatchEvent(new CustomEvent('tree-navigate', {
          bubbles: true, composed: true, detail: { group: path },
        }));
        this.render();
      });
    });

    root.querySelectorAll('.node.leaf').forEach(el => {
      el.addEventListener('click', () => {
        this.dispatchEvent(new CustomEvent('tree-select', {
          bubbles: true, composed: true, detail: { path: el.dataset.path },
        }));
      });
    });
  }
}

customElements.define('geoff-token-tree', GeoffTokenTree);

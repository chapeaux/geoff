/**
 * geoff-editor
 *
 * Markdown + RDF-aware frontmatter editor with live preview.
 * Split pane: frontmatter form on top/left, markdown editor on bottom/right.
 */
class GeoffEditor extends HTMLElement {
  constructor() {
    super();
    this.attachShadow({ mode: 'open' });
    this.currentPath = null;
    this.originalData = null;
  }

  connectedCallback() {
    this.render();
    this.setupEventListeners();
    this.connectWebSocket();

    const path = this.getAttribute('path');
    if (path) {
      this.loadPage(path);
    }
  }

  render() {
    this.shadowRoot.innerHTML = `
      <style>
        :host {
          display: block;
          height: 100%;
          font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
          --border-color: #ddd;
          --bg-primary: #fff;
          --bg-secondary: #f5f5f5;
          --text-primary: #333;
          --text-secondary: #666;
          --accent: #0066cc;
          --accent-hover: #0052a3;
          --success: #28a745;
          --error: #dc3545;
        }

        .container {
          display: flex;
          flex-direction: column;
          height: 100%;
          gap: 1px;
          background: var(--border-color);
        }

        .toolbar {
          display: flex;
          align-items: center;
          gap: 8px;
          padding: 8px 12px;
          background: var(--bg-secondary);
          border-bottom: 1px solid var(--border-color);
        }

        .toolbar button {
          padding: 6px 12px;
          border: 1px solid var(--border-color);
          background: var(--bg-primary);
          color: var(--text-primary);
          border-radius: 4px;
          cursor: pointer;
          font-size: 14px;
        }

        .toolbar button:hover {
          background: var(--bg-secondary);
        }

        .toolbar button.primary {
          background: var(--accent);
          color: white;
          border-color: var(--accent);
        }

        .toolbar button.primary:hover {
          background: var(--accent-hover);
        }

        .toolbar .status {
          margin-left: auto;
          color: var(--text-secondary);
          font-size: 14px;
        }

        .toolbar .status.success {
          color: var(--success);
        }

        .toolbar .status.error {
          color: var(--error);
        }

        .split-pane {
          display: flex;
          flex: 1;
          min-height: 0;
          background: var(--bg-primary);
        }

        .frontmatter-pane,
        .content-pane {
          flex: 1;
          display: flex;
          flex-direction: column;
          overflow: hidden;
        }

        .frontmatter-pane {
          border-right: 1px solid var(--border-color);
          max-width: 400px;
        }

        .pane-header {
          padding: 12px;
          background: var(--bg-secondary);
          border-bottom: 1px solid var(--border-color);
          font-weight: 500;
          font-size: 14px;
        }

        .pane-content {
          flex: 1;
          overflow-y: auto;
          padding: 12px;
        }

        .form-group {
          margin-bottom: 16px;
        }

        .form-group label {
          display: block;
          margin-bottom: 4px;
          font-size: 13px;
          font-weight: 500;
          color: var(--text-primary);
        }

        .form-group input,
        .form-group select,
        .form-group textarea {
          width: 100%;
          padding: 8px;
          border: 1px solid var(--border-color);
          border-radius: 4px;
          font-family: inherit;
          font-size: 14px;
          box-sizing: border-box;
        }

        .form-group textarea {
          resize: vertical;
          min-height: 60px;
          font-family: monospace;
        }

        .markdown-toolbar {
          display: flex;
          gap: 4px;
          padding: 8px;
          background: var(--bg-secondary);
          border-bottom: 1px solid var(--border-color);
        }

        .markdown-toolbar button {
          padding: 4px 8px;
          border: 1px solid var(--border-color);
          background: var(--bg-primary);
          border-radius: 3px;
          cursor: pointer;
          font-size: 12px;
          font-family: monospace;
        }

        .markdown-toolbar button:hover {
          background: var(--bg-secondary);
        }

        .editor-wrapper {
          flex: 1;
          display: flex;
          flex-direction: column;
          overflow: hidden;
        }

        #markdown-editor {
          flex: 1;
          width: 100%;
          padding: 12px;
          border: none;
          font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
          font-size: 14px;
          line-height: 1.6;
          resize: none;
          box-sizing: border-box;
        }

        #markdown-editor:focus {
          outline: none;
        }

        .preview-pane {
          border-top: 1px solid var(--border-color);
          max-height: 40%;
          overflow-y: auto;
        }

        .preview-content {
          padding: 12px;
          line-height: 1.6;
        }

        .preview-content h1,
        .preview-content h2,
        .preview-content h3 {
          margin-top: 1em;
          margin-bottom: 0.5em;
        }

        .preview-content code {
          background: var(--bg-secondary);
          padding: 2px 4px;
          border-radius: 3px;
          font-family: monospace;
        }

        .preview-content pre {
          background: var(--bg-secondary);
          padding: 12px;
          border-radius: 4px;
          overflow-x: auto;
        }

        @media (max-width: 768px) {
          .split-pane {
            flex-direction: column;
          }

          .frontmatter-pane {
            max-width: none;
            border-right: none;
            border-bottom: 1px solid var(--border-color);
          }
        }
      </style>

      <div class="container">
        <div class="toolbar">
          <button class="primary" id="save-btn" aria-label="Save page">Save</button>
          <button id="reload-btn" aria-label="Reload page">Reload</button>
          <span class="status" id="status"></span>
        </div>

        <div class="split-pane">
          <div class="frontmatter-pane">
            <div class="pane-header">Frontmatter</div>
            <div class="pane-content">
              <form id="frontmatter-form"></form>
            </div>
          </div>

          <div class="content-pane">
            <div class="pane-header">Markdown</div>
            <div class="editor-wrapper">
              <div class="markdown-toolbar">
                <button data-action="bold" title="Bold" aria-label="Bold">**B**</button>
                <button data-action="italic" title="Italic" aria-label="Italic">*I*</button>
                <button data-action="heading" title="Heading" aria-label="Heading">H</button>
                <button data-action="link" title="Link" aria-label="Insert link">[L]</button>
                <button data-action="list" title="List" aria-label="Insert list">• List</button>
                <button data-action="code" title="Code" aria-label="Code block">\`C\`</button>
              </div>
              <textarea id="markdown-editor" aria-label="Markdown content"></textarea>
            </div>
            <div class="preview-pane">
              <div class="pane-header">Preview</div>
              <div class="preview-content" id="preview"></div>
            </div>
          </div>
        </div>
      </div>
    `;
  }

  setupEventListeners() {
    const saveBtn = this.shadowRoot.getElementById('save-btn');
    const reloadBtn = this.shadowRoot.getElementById('reload-btn');
    const editor = this.shadowRoot.getElementById('markdown-editor');
    const toolbar = this.shadowRoot.querySelector('.markdown-toolbar');

    saveBtn.addEventListener('click', () => this.savePage());
    reloadBtn.addEventListener('click', () => this.loadPage(this.currentPath));
    editor.addEventListener('input', () => this.updatePreview());
    toolbar.addEventListener('click', (e) => this.handleMarkdownAction(e));
  }

  async loadPage(path) {
    this.currentPath = path;
    this.setStatus('Loading...', 'info');

    try {
      const response = await fetch(`/api/pages/${encodeURIComponent(path)}`);
      if (!response.ok) throw new Error(`Failed to load page: ${response.statusText}`);

      const data = await response.json();
      this.originalData = data;

      this.populateFrontmatter(data.frontmatter || {});
      const editor = this.shadowRoot.getElementById('markdown-editor');
      editor.value = data.content || '';
      this.updatePreview();

      this.setStatus('Loaded', 'success');
      setTimeout(() => this.setStatus('', ''), 2000);
    } catch (error) {
      console.error('Failed to load page:', error);
      this.setStatus(`Error: ${error.message}`, 'error');
    }
  }

  populateFrontmatter(data) {
    const form = this.shadowRoot.getElementById('frontmatter-form');
    form.innerHTML = '';

    // Common fields
    const fields = [
      { name: 'title', type: 'text', label: 'Title' },
      { name: 'date', type: 'date', label: 'Date' },
      { name: 'type', type: 'text', label: 'Type' },
      { name: 'author', type: 'text', label: 'Author' },
      { name: 'template', type: 'text', label: 'Template' },
      { name: 'language', type: 'text', label: 'Language' },
      { name: 'tags', type: 'textarea', label: 'Tags (comma-separated)' },
      { name: 'about', type: 'textarea', label: 'About (comma-separated)' },
    ];

    fields.forEach(field => {
      const group = document.createElement('div');
      group.className = 'form-group';

      const label = document.createElement('label');
      label.textContent = field.label;
      label.htmlFor = field.name;

      let input;
      if (field.type === 'textarea') {
        input = document.createElement('textarea');
        const value = data[field.name];
        if (Array.isArray(value)) {
          input.value = value.join(', ');
        } else if (value) {
          input.value = value;
        }
      } else {
        input = document.createElement('input');
        input.type = field.type;
        input.value = data[field.name] || '';
      }

      input.id = field.name;
      input.name = field.name;

      group.appendChild(label);
      group.appendChild(input);
      form.appendChild(group);
    });

    // Add any extra fields not in the common set
    Object.keys(data).forEach(key => {
      if (!fields.find(f => f.name === key)) {
        const group = document.createElement('div');
        group.className = 'form-group';

        const label = document.createElement('label');
        label.textContent = key;
        label.htmlFor = key;

        const input = document.createElement('input');
        input.type = 'text';
        input.id = key;
        input.name = key;
        input.value = data[key] || '';

        group.appendChild(label);
        group.appendChild(input);
        form.appendChild(group);
      }
    });
  }

  getFrontmatterData() {
    const form = this.shadowRoot.getElementById('frontmatter-form');
    const formData = new FormData(form);
    const data = {};

    for (let [key, value] of formData.entries()) {
      if (value) {
        // Handle arrays (tags, about)
        if (key === 'tags' || key === 'about') {
          data[key] = value.split(',').map(s => s.trim()).filter(Boolean);
        } else {
          data[key] = value;
        }
      }
    }

    return data;
  }

  async savePage() {
    if (!this.currentPath) {
      this.setStatus('No page loaded', 'error');
      return;
    }

    this.setStatus('Saving...', 'info');

    const frontmatter = this.getFrontmatterData();
    const content = this.shadowRoot.getElementById('markdown-editor').value;

    try {
      const response = await fetch(`/api/pages/${encodeURIComponent(this.currentPath)}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ frontmatter, content })
      });

      if (!response.ok) throw new Error(`Failed to save: ${response.statusText}`);

      this.setStatus('Saved', 'success');
      setTimeout(() => this.setStatus('', ''), 2000);

      // Emit custom event for other components
      this.dispatchEvent(new CustomEvent('geoff-page-saved', {
        bubbles: true,
        composed: true,
        detail: { path: this.currentPath }
      }));
    } catch (error) {
      console.error('Failed to save page:', error);
      this.setStatus(`Error: ${error.message}`, 'error');
    }
  }

  updatePreview() {
    const markdown = this.shadowRoot.getElementById('markdown-editor').value;
    const preview = this.shadowRoot.getElementById('preview');

    // Basic markdown-to-HTML conversion (simplified)
    let html = markdown
      .replace(/^### (.*$)/gim, '<h3>$1</h3>')
      .replace(/^## (.*$)/gim, '<h2>$1</h2>')
      .replace(/^# (.*$)/gim, '<h1>$1</h1>')
      .replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>')
      .replace(/\*(.*?)\*/g, '<em>$1</em>')
      .replace(/\[(.*?)\]\((.*?)\)/g, '<a href="$2">$1</a>')
      .replace(/`(.*?)`/g, '<code>$1</code>')
      .replace(/\n\n/g, '</p><p>')
      .replace(/\n/g, '<br>');

    preview.innerHTML = '<p>' + html + '</p>';
  }

  handleMarkdownAction(event) {
    const button = event.target.closest('button');
    if (!button) return;

    const action = button.dataset.action;
    const editor = this.shadowRoot.getElementById('markdown-editor');
    const start = editor.selectionStart;
    const end = editor.selectionEnd;
    const selectedText = editor.value.substring(start, end);
    let replacement = '';

    switch (action) {
      case 'bold':
        replacement = `**${selectedText || 'bold text'}**`;
        break;
      case 'italic':
        replacement = `*${selectedText || 'italic text'}*`;
        break;
      case 'heading':
        replacement = `## ${selectedText || 'Heading'}`;
        break;
      case 'link':
        replacement = `[${selectedText || 'link text'}](url)`;
        break;
      case 'list':
        replacement = `- ${selectedText || 'list item'}`;
        break;
      case 'code':
        replacement = selectedText ? `\`${selectedText}\`` : '\`code\`';
        break;
    }

    editor.value = editor.value.substring(0, start) + replacement + editor.value.substring(end);
    editor.focus();
    editor.setSelectionRange(start, start + replacement.length);
    this.updatePreview();
  }

  connectWebSocket() {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const ws = new WebSocket(`${protocol}//${window.location.host}/ws`);

    ws.addEventListener('message', (event) => {
      const data = JSON.parse(event.data);
      if (data.type === 'reload' && this.currentPath) {
        this.loadPage(this.currentPath);
      }
    });

    ws.addEventListener('error', (error) => {
      console.error('WebSocket error:', error);
    });
  }

  setStatus(message, type = 'info') {
    const status = this.shadowRoot.getElementById('status');
    status.textContent = message;
    status.className = `status ${type}`;
  }

  disconnectedCallback() {
    // Cleanup if needed
  }
}

customElements.define('geoff-editor', GeoffEditor);

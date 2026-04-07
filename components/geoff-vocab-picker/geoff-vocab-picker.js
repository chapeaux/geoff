/**
 * geoff-vocab-picker
 *
 * Vocabulary term browser for exploring loaded ontologies.
 * Search, browse, and select terms from schema.org, Dublin Core, FOAF, etc.
 */
class GeoffVocabPicker extends HTMLElement {
  constructor() {
    super();
    this.attachShadow({ mode: 'open' });
    this.vocabularies = [];
    this.searchResults = [];
    this.selectedTerm = null;
  }

  connectedCallback() {
    this.render();
    this.setupEventListeners();
    this.loadVocabularies();
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
          --bg-hover: #f0f0f0;
          --text-primary: #333;
          --text-secondary: #666;
          --accent: #0066cc;
          --accent-light: #e6f2ff;
          --vocab-schema: #3498db;
          --vocab-dc: #e74c3c;
          --vocab-foaf: #2ecc71;
          --vocab-sioc: #f39c12;
          --vocab-geoff: #9b59b6;
        }

        .container {
          display: flex;
          flex-direction: column;
          height: 100%;
          background: var(--bg-primary);
        }

        .search-bar {
          padding: 12px;
          border-bottom: 1px solid var(--border-color);
          background: var(--bg-secondary);
        }

        .search-input {
          width: 100%;
          padding: 8px 12px;
          border: 1px solid var(--border-color);
          border-radius: 4px;
          font-size: 14px;
          box-sizing: border-box;
        }

        .search-input:focus {
          outline: none;
          border-color: var(--accent);
        }

        .content {
          flex: 1;
          display: flex;
          overflow: hidden;
        }

        .sidebar {
          width: 200px;
          border-right: 1px solid var(--border-color);
          overflow-y: auto;
          background: var(--bg-secondary);
        }

        .vocab-list {
          list-style: none;
          margin: 0;
          padding: 8px;
        }

        .vocab-item {
          padding: 8px 12px;
          cursor: pointer;
          border-radius: 4px;
          font-size: 13px;
          margin-bottom: 4px;
          display: flex;
          align-items: center;
          gap: 8px;
        }

        .vocab-item:hover {
          background: var(--bg-hover);
        }

        .vocab-item.active {
          background: var(--accent-light);
          color: var(--accent);
          font-weight: 500;
        }

        .vocab-badge {
          width: 12px;
          height: 12px;
          border-radius: 2px;
          flex-shrink: 0;
        }

        .vocab-badge.schema { background: var(--vocab-schema); }
        .vocab-badge.dc { background: var(--vocab-dc); }
        .vocab-badge.foaf { background: var(--vocab-foaf); }
        .vocab-badge.sioc { background: var(--vocab-sioc); }
        .vocab-badge.geoff { background: var(--vocab-geoff); }

        .main-panel {
          flex: 1;
          display: flex;
          flex-direction: column;
          overflow: hidden;
        }

        .results-header {
          padding: 12px;
          border-bottom: 1px solid var(--border-color);
          background: var(--bg-secondary);
          font-size: 13px;
          color: var(--text-secondary);
        }

        .results-list {
          flex: 1;
          overflow-y: auto;
          padding: 8px;
        }

        .term-card {
          padding: 12px;
          margin-bottom: 8px;
          border: 1px solid var(--border-color);
          border-radius: 4px;
          background: var(--bg-primary);
          cursor: pointer;
          transition: all 0.2s;
        }

        .term-card:hover {
          border-color: var(--accent);
          box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
        }

        .term-card.selected {
          border-color: var(--accent);
          background: var(--accent-light);
        }

        .term-header {
          display: flex;
          align-items: center;
          gap: 8px;
          margin-bottom: 6px;
        }

        .term-label {
          font-weight: 500;
          font-size: 14px;
          color: var(--text-primary);
          flex: 1;
        }

        .term-vocab {
          font-size: 11px;
          padding: 2px 6px;
          border-radius: 3px;
          background: var(--bg-secondary);
          color: var(--text-secondary);
        }

        .term-iri {
          font-family: monospace;
          font-size: 11px;
          color: var(--text-secondary);
          margin-bottom: 6px;
          word-break: break-all;
        }

        .term-description {
          font-size: 13px;
          color: var(--text-primary);
          line-height: 1.5;
        }

        .term-alt-labels {
          margin-top: 6px;
          font-size: 12px;
          color: var(--text-secondary);
        }

        .details-panel {
          width: 350px;
          border-left: 1px solid var(--border-color);
          background: var(--bg-secondary);
          overflow-y: auto;
          padding: 16px;
        }

        .details-panel.hidden {
          display: none;
        }

        .details-panel h3 {
          margin: 0 0 16px 0;
          font-size: 16px;
          color: var(--text-primary);
        }

        .detail-section {
          margin-bottom: 20px;
        }

        .detail-label {
          font-size: 12px;
          font-weight: 500;
          color: var(--text-secondary);
          margin-bottom: 4px;
        }

        .detail-value {
          font-size: 13px;
          color: var(--text-primary);
          line-height: 1.6;
        }

        .detail-value code {
          background: var(--bg-primary);
          padding: 2px 6px;
          border-radius: 3px;
          font-family: monospace;
          font-size: 12px;
          word-break: break-all;
          display: inline-block;
        }

        .action-buttons {
          display: flex;
          gap: 8px;
          margin-top: 16px;
        }

        .action-buttons button {
          flex: 1;
          padding: 8px 12px;
          border: 1px solid var(--border-color);
          background: var(--accent);
          color: white;
          border-radius: 4px;
          cursor: pointer;
          font-size: 13px;
        }

        .action-buttons button:hover {
          background: var(--accent);
          filter: brightness(1.1);
        }

        .empty-state {
          display: flex;
          flex-direction: column;
          align-items: center;
          justify-content: center;
          height: 100%;
          color: var(--text-secondary);
          font-size: 14px;
          text-align: center;
          padding: 20px;
        }

        .loading {
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 40px;
          color: var(--text-secondary);
        }

        @media (max-width: 768px) {
          .sidebar {
            width: 150px;
          }

          .details-panel {
            position: absolute;
            top: 0;
            right: 0;
            bottom: 0;
            z-index: 10;
            box-shadow: -2px 0 8px rgba(0, 0, 0, 0.1);
          }
        }
      </style>

      <div class="container">
        <div class="search-bar">
          <input
            type="search"
            class="search-input"
            id="search-input"
            placeholder="Search vocabulary terms..."
            aria-label="Search vocabulary terms"
          >
        </div>

        <div class="content">
          <div class="sidebar">
            <ul class="vocab-list" id="vocab-list">
              <li class="vocab-item active" data-vocab="all">
                <div class="vocab-badge" style="background: linear-gradient(135deg, var(--vocab-schema), var(--vocab-dc));"></div>
                <span>All Vocabularies</span>
              </li>
            </ul>
          </div>

          <div class="main-panel">
            <div class="results-header" id="results-header">
              Loading vocabularies...
            </div>
            <div class="results-list" id="results-list">
              <div class="loading">Loading...</div>
            </div>
          </div>

          <div class="details-panel hidden" id="details-panel">
            <h3 id="details-title">Term Details</h3>
            <div id="details-content"></div>
          </div>
        </div>
      </div>
    `;
  }

  setupEventListeners() {
    const searchInput = this.shadowRoot.getElementById('search-input');
    const vocabList = this.shadowRoot.getElementById('vocab-list');

    let searchTimeout;
    searchInput.addEventListener('input', (e) => {
      clearTimeout(searchTimeout);
      searchTimeout = setTimeout(() => this.search(e.target.value), 300);
    });

    vocabList.addEventListener('click', (e) => {
      const item = e.target.closest('.vocab-item');
      if (item) {
        this.shadowRoot.querySelectorAll('.vocab-item').forEach(el => {
          el.classList.remove('active');
        });
        item.classList.add('active');
        const vocab = item.dataset.vocab;
        this.filterByVocab(vocab);
      }
    });
  }

  async loadVocabularies() {
    try {
      const response = await fetch('/api/vocabs');
      if (!response.ok) throw new Error(`Failed to load vocabularies: ${response.statusText}`);

      const data = await response.json();
      this.vocabularies = data.vocabularies || [];

      this.renderVocabList();
      this.showAllTerms();
    } catch (error) {
      console.error('Failed to load vocabularies:', error);
      this.showError('Failed to load vocabularies');
    }
  }

  renderVocabList() {
    const vocabList = this.shadowRoot.getElementById('vocab-list');

    this.vocabularies.forEach(vocab => {
      const li = document.createElement('li');
      li.className = 'vocab-item';
      li.dataset.vocab = vocab.id;

      const badge = document.createElement('div');
      badge.className = `vocab-badge ${vocab.id}`;

      const label = document.createElement('span');
      label.textContent = vocab.label;

      li.appendChild(badge);
      li.appendChild(label);
      vocabList.appendChild(li);
    });
  }

  async search(query) {
    if (!query.trim()) {
      this.showAllTerms();
      return;
    }

    try {
      const response = await fetch(`/api/vocabs/search?q=${encodeURIComponent(query)}`);
      if (!response.ok) throw new Error(`Search failed: ${response.statusText}`);

      const data = await response.json();
      this.searchResults = data.results || [];
      this.renderResults();
    } catch (error) {
      console.error('Search failed:', error);
      this.showError('Search failed');
    }
  }

  showAllTerms() {
    // Flatten all terms from all vocabularies
    this.searchResults = [];
    this.vocabularies.forEach(vocab => {
      vocab.terms?.forEach(term => {
        this.searchResults.push({
          ...term,
          vocabulary: vocab.id,
          vocabularyLabel: vocab.label
        });
      });
    });
    this.renderResults();
  }

  filterByVocab(vocabId) {
    if (vocabId === 'all') {
      this.showAllTerms();
      return;
    }

    const vocab = this.vocabularies.find(v => v.id === vocabId);
    if (vocab) {
      this.searchResults = vocab.terms?.map(term => ({
        ...term,
        vocabulary: vocab.id,
        vocabularyLabel: vocab.label
      })) || [];
      this.renderResults();
    }
  }

  renderResults() {
    const header = this.shadowRoot.getElementById('results-header');
    const list = this.shadowRoot.getElementById('results-list');

    header.textContent = `${this.searchResults.length} term${this.searchResults.length !== 1 ? 's' : ''}`;

    if (this.searchResults.length === 0) {
      list.innerHTML = `
        <div class="empty-state">
          <p>No terms found</p>
        </div>
      `;
      return;
    }

    // Group by vocabulary
    const grouped = {};
    this.searchResults.forEach(term => {
      const vocab = term.vocabulary || 'unknown';
      if (!grouped[vocab]) {
        grouped[vocab] = [];
      }
      grouped[vocab].push(term);
    });

    let html = '';
    Object.entries(grouped).forEach(([vocab, terms]) => {
      terms.forEach(term => {
        html += this.renderTermCard(term);
      });
    });

    list.innerHTML = html;

    // Add click handlers
    list.querySelectorAll('.term-card').forEach((card, index) => {
      card.addEventListener('click', () => {
        const term = this.searchResults[parseInt(card.dataset.index)];
        this.selectTerm(term, card);
      });
    });
  }

  renderTermCard(term) {
    const index = this.searchResults.indexOf(term);
    return `
      <div class="term-card" data-index="${index}">
        <div class="term-header">
          <div class="term-label">${term.label || term.id}</div>
          <div class="term-vocab">${term.vocabularyLabel || term.vocabulary}</div>
        </div>
        <div class="term-iri">${term.iri || term.id}</div>
        ${term.description ? `<div class="term-description">${term.description}</div>` : ''}
        ${term.altLabels ? `<div class="term-alt-labels">Also: ${term.altLabels.join(', ')}</div>` : ''}
      </div>
    `;
  }

  selectTerm(term, element) {
    // Deselect previous
    this.shadowRoot.querySelectorAll('.term-card.selected').forEach(el => {
      el.classList.remove('selected');
    });

    // Select new
    element.classList.add('selected');
    this.selectedTerm = term;

    // Show details
    this.showTermDetails(term);
  }

  showTermDetails(term) {
    const panel = this.shadowRoot.getElementById('details-panel');
    const title = this.shadowRoot.getElementById('details-title');
    const content = this.shadowRoot.getElementById('details-content');

    panel.classList.remove('hidden');
    title.textContent = term.label || term.id;

    let html = '';

    html += `<div class="detail-section">
      <div class="detail-label">IRI</div>
      <div class="detail-value"><code>${term.iri || term.id}</code></div>
    </div>`;

    if (term.description) {
      html += `<div class="detail-section">
        <div class="detail-label">Description</div>
        <div class="detail-value">${term.description}</div>
      </div>`;
    }

    if (term.vocabularyLabel) {
      html += `<div class="detail-section">
        <div class="detail-label">Vocabulary</div>
        <div class="detail-value">${term.vocabularyLabel}</div>
      </div>`;
    }

    if (term.altLabels && term.altLabels.length > 0) {
      html += `<div class="detail-section">
        <div class="detail-label">Alternative Labels</div>
        <div class="detail-value">${term.altLabels.join(', ')}</div>
      </div>`;
    }

    if (term.type) {
      html += `<div class="detail-section">
        <div class="detail-label">Type</div>
        <div class="detail-value">${term.type}</div>
      </div>`;
    }

    if (term.domain) {
      html += `<div class="detail-section">
        <div class="detail-label">Domain</div>
        <div class="detail-value"><code>${term.domain}</code></div>
      </div>`;
    }

    if (term.range) {
      html += `<div class="detail-section">
        <div class="detail-label">Range</div>
        <div class="detail-value"><code>${term.range}</code></div>
      </div>`;
    }

    html += `<div class="action-buttons">
      <button id="copy-iri-btn">Copy IRI</button>
      <button id="insert-btn">Insert</button>
    </div>`;

    content.innerHTML = html;

    // Add action handlers
    const copyBtn = content.querySelector('#copy-iri-btn');
    const insertBtn = content.querySelector('#insert-btn');

    copyBtn.addEventListener('click', () => this.copyIRI(term));
    insertBtn.addEventListener('click', () => this.insertTerm(term));
  }

  copyIRI(term) {
    const iri = term.iri || term.id;
    navigator.clipboard.writeText(iri).then(() => {
      // Visual feedback
      const btn = this.shadowRoot.querySelector('#copy-iri-btn');
      const originalText = btn.textContent;
      btn.textContent = 'Copied!';
      setTimeout(() => {
        btn.textContent = originalText;
      }, 1500);
    });
  }

  insertTerm(term) {
    // Emit custom event for parent components to handle
    this.dispatchEvent(new CustomEvent('geoff-term-selected', {
      bubbles: true,
      composed: true,
      detail: {
        term,
        iri: term.iri || term.id,
        label: term.label || term.id
      }
    }));
  }

  showError(message) {
    const list = this.shadowRoot.getElementById('results-list');
    list.innerHTML = `
      <div class="empty-state">
        <p>${message}</p>
      </div>
    `;
  }
}

customElements.define('geoff-vocab-picker', GeoffVocabPicker);

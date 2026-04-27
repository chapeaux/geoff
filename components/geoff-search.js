/**
 * <geoff-search> — Client-side SPARQL search using Oxigraph WASM.
 *
 * Loads a pre-built N-Triples search index and runs SPARQL queries
 * in the browser using the same engine that built the site.
 *
 * Usage:
 *   <geoff-search index="/search.nt"></geoff-search>
 *
 * Attributes:
 *   index  — URL of the N-Triples search index (default: "/search.nt")
 *   limit  — Maximum results to show (default: "20")
 */
class GeoffSearch extends HTMLElement {
  constructor() {
    super();
    this._store = null;
    this._ox = null;
    this._loading = false;
    this._loaded = false;
  }

  connectedCallback() {
    this.innerHTML = `
      <form role="search" class="geoff-search-form">
        <input type="search" placeholder="Search…" aria-label="Search" />
        <div class="geoff-search-status" aria-live="polite"></div>
      </form>
      <div class="geoff-search-results" role="list"></div>
    `;

    const input = this.querySelector('input');
    let debounce;
    input.addEventListener('input', () => {
      clearTimeout(debounce);
      debounce = setTimeout(() => this._search(input.value), 200);
    });
    input.addEventListener('focus', () => this._ensureLoaded(), { once: true });
  }

  async _ensureLoaded() {
    if (this._loaded || this._loading) return;
    this._loading = true;
    this._setStatus('Loading search…');

    try {
      const ox = await import('https://esm.sh/oxigraph@0.5');
      this._ox = ox.default || ox;
      this._store = new this._ox.Store();

      const indexUrl = this.getAttribute('index') || '/search.nt';
      const response = await fetch(indexUrl);
      if (!response.ok) throw new Error(`Failed to fetch ${indexUrl}`);
      const nt = await response.text();

      this._store.load(nt, { format: 'application/n-triples' });
      this._loaded = true;
      this._setStatus('');
    } catch (e) {
      this._setStatus('Search unavailable');
      console.error('[geoff-search]', e);
    } finally {
      this._loading = false;
    }
  }

  async _search(query) {
    const results = this.querySelector('.geoff-search-results');
    if (!query.trim()) {
      results.innerHTML = '';
      this._setStatus('');
      return;
    }

    await this._ensureLoaded();
    if (!this._loaded) return;

    const escaped = query.replace(/\\/g, '\\\\').replace(/"/g, '\\"');
    const limit = parseInt(this.getAttribute('limit') || '20', 10);

    const sparql = `
      SELECT ?title ?url ?desc ?date ?type WHERE {
        ?s <http://schema.org/name> ?title .
        OPTIONAL { ?s <http://schema.org/url> ?url }
        OPTIONAL { ?s <http://schema.org/description> ?desc }
        OPTIONAL { ?s <http://schema.org/datePublished> ?date }
        OPTIONAL { ?s <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> ?type }
        FILTER(
          CONTAINS(LCASE(?title), LCASE("${escaped}"))
          || CONTAINS(LCASE(COALESCE(?desc, "")), LCASE("${escaped}"))
        )
      }
      ORDER BY DESC(?date)
      LIMIT ${limit}
    `;

    try {
      const bindings = this._store.query(sparql);
      this._renderResults(bindings, query);
    } catch (e) {
      console.error('[geoff-search] query error:', e);
      this._setStatus('Search error');
    }
  }

  _renderResults(bindings, query) {
    const container = this.querySelector('.geoff-search-results');

    if (!bindings || bindings.length === 0) {
      container.innerHTML = '';
      this._setStatus(`No results for "${query}"`);
      return;
    }

    this._setStatus(`${bindings.length} result${bindings.length === 1 ? '' : 's'}`);

    container.innerHTML = bindings.map(row => {
      const title = this._esc(row.get('title')?.value || 'Untitled');
      const url = row.get('url')?.value || '#';
      const desc = this._esc(row.get('desc')?.value || '');
      const date = row.get('date')?.value || '';

      return `<a href="${url}" class="geoff-search-result" role="listitem">
        <strong>${title}</strong>
        ${date ? `<time>${date}</time>` : ''}
        ${desc ? `<small>${desc}</small>` : ''}
      </a>`;
    }).join('');
  }

  _setStatus(text) {
    const el = this.querySelector('.geoff-search-status');
    if (el) el.textContent = text;
  }

  _esc(str) {
    const d = document.createElement('div');
    d.textContent = str;
    return d.innerHTML;
  }
}

customElements.define('geoff-search', GeoffSearch);

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
    this._loading = false;
    this._loaded = false;
  }

  connectedCallback() {
    if (typeof window === 'undefined') return;
    if (!this.querySelector('input')) {
      this.innerHTML = `
        <form role="search" class="geoff-search-form">
          <input type="search" placeholder="Search…" aria-label="Search" />
          <div class="geoff-search-status" aria-live="polite"></div>
        </form>
        <div class="geoff-search-results" role="list"></div>
      `;
    }

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
      await ox.default();
      this._store = new ox.Store();

      const indexUrl = this.getAttribute('index') || '/search.nt';
      const response = await fetch(indexUrl);
      if (!response.ok) throw new Error(`Failed to fetch ${indexUrl}`);
      const nt = await response.text();

      this._store.load(nt, { format: 'nt' });
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

    const tokens = this._parseQuery(query.trim());
    if (tokens.length === 0) {
      results.innerHTML = '';
      this._setStatus('');
      return;
    }

    const filter = this._buildFilter(tokens);
    const limit = parseInt(this.getAttribute('limit') || '20', 10);

    const sparql = `
      SELECT ?s ?title ?desc WHERE {
        ?s <https://schema.org/name> ?title .
        OPTIONAL { ?s <https://schema.org/description> ?desc }
        FILTER(${filter})
      }
      ORDER BY ?title
      LIMIT ${limit}
    `;

    try {
      const bindings = this._store.query(sparql);
      const arr = (bindings && typeof bindings[Symbol.iterator] === 'function')
        ? [...bindings]
        : bindings;
      this._renderResults(arr, query);
    } catch (e) {
      console.error('[geoff-search] query error:', e);
      this._setStatus('Search error');
    }
  }

  /**
   * Parse a search query into tokens.
   *
   * Supports:
   * - Multiple terms: `foo bar` (implicit AND — both must match)
   * - Quoted phrases: `"exact phrase"` (case-insensitive exact match)
   * - OR operator: `foo OR bar` (either must match)
   * - AND operator: `foo AND bar` (explicit AND, same as space)
   *
   * OR binds looser than AND: `a b OR c` → `(a AND b) OR c`
   */
  _parseQuery(input) {
    const tokens = [];
    let i = 0;
    while (i < input.length) {
      if (input[i] === ' ') { i++; continue; }

      if (input[i] === '"') {
        const end = input.indexOf('"', i + 1);
        if (end !== -1) {
          tokens.push({ type: 'term', value: input.slice(i + 1, end) });
          i = end + 1;
          continue;
        }
      }

      const wordEnd = input.indexOf(' ', i);
      const word = wordEnd === -1 ? input.slice(i) : input.slice(i, wordEnd);
      i = wordEnd === -1 ? input.length : wordEnd;

      if (word === 'OR') {
        tokens.push({ type: 'OR' });
      } else if (word === 'AND') {
        continue;
      } else {
        tokens.push({ type: 'term', value: word });
      }
    }
    return tokens.filter(t => t.type !== 'term' || t.value.length > 0);
  }

  _buildFilter(tokens) {
    const groups = [[]];
    for (const token of tokens) {
      if (token.type === 'OR') {
        groups.push([]);
      } else {
        groups[groups.length - 1].push(token);
      }
    }

    const groupFilters = groups
      .filter(g => g.length > 0)
      .map(group => {
        const termFilters = group.map(t => {
          const escaped = t.value.replace(/\\/g, '\\\\').replace(/"/g, '\\"');
          const lower = escaped.toLowerCase();
          return `(CONTAINS(LCASE(?title), "${lower}") || CONTAINS(LCASE(COALESCE(?desc, "")), "${lower}"))`;
        });
        return termFilters.length === 1 ? termFilters[0] : `(${termFilters.join(' && ')})`;
      });

    return groupFilters.length === 1 ? groupFilters[0] : `(${groupFilters.join(' || ')})`;
  }

  _renderResults(bindings, query) {
    const container = this.querySelector('.geoff-search-results');

    if (!bindings || bindings.length === 0) {
      container.innerHTML = '';
      this._setStatus(`No results for "${query}"`);
      return;
    }

    const seen = new Set();
    const results = [];
    for (const row of bindings) {
      const title = row.get('title')?.value || 'Untitled';
      const s = row.get('s')?.value || '';
      let url = '#';
      if (s.startsWith('urn:geoff:content:')) {
        url = '/' + s.replace('urn:geoff:content:', '').replace(/\.md$/, '/').replace(/index\/$/, '');
      }
      if (seen.has(url)) continue;
      seen.add(url);
      const desc = row.get('desc')?.value || '';
      const parts = url.replace(/^\//, '').replace(/\/$/, '').split('/');
      const context = parts.length > 1
        ? parts.slice(0, -1).map(p => p.replace(/-/g, ' ')).map(p => p.charAt(0).toUpperCase() + p.slice(1)).join(' › ')
        : '';
      results.push({ title, url, desc, context });
    }

    this._setStatus(`${results.length} result${results.length === 1 ? '' : 's'}`);

    container.innerHTML = results.map(({ title, url, desc, context }) => {
      const t = this._esc(title);
      const c = this._esc(context);
      const d = this._esc(desc);
      return `<a href="${url}" class="geoff-search-result" role="listitem">
        <strong>${t}</strong>
        ${c ? `<small class="geoff-search-context">${c}</small>` : ''}
        ${d ? `<small>${d}</small>` : ''}
      </a>`;
    }).join('');
  }

  _setStatus(text) {
    const el = this.querySelector('.geoff-search-status');
    if (el) el.textContent = text;
  }

  _esc(str) {
    if (typeof document === 'undefined') return str.replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);
    const d = document.createElement('div');
    d.textContent = str;
    return d.innerHTML;
  }
}

customElements.define('geoff-search', GeoffSearch);

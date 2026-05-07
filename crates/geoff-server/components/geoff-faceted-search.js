/**
 * <geoff-faceted-search> — Full-page faceted search with on-demand graph loading.
 *
 * Discovers available search partitions from the manifest in search.nt,
 * displays facet buttons, and loads partition graphs on demand.
 *
 * Attributes:
 *   index       — URL of the main search index (default: "/search.nt")
 *   limit       — Maximum results per query (default: "50")
 *   placeholder — Input placeholder text (default: "Search…")
 */

const FACETED_STYLES = `
geoff-faceted-search {
  display: block;
}
.gfs-layout {
  display: flex;
  gap: 1.5rem;
  align-items: flex-start;
}
.gfs-sidebar {
  flex: 0 0 200px;
  position: sticky;
  top: 1rem;
}
.gfs-main {
  flex: 1;
  min-width: 0;
}
.gfs-facets {
  list-style: none;
  margin: 0;
  padding: 0;
}
.gfs-facet {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  width: 100%;
  padding: 0.4rem 0.5rem;
  margin-bottom: 0.25rem;
  border-radius: var(--gfs-radius, 4px);
  cursor: pointer;
  font: inherit;
  font-size: 0.9em;
}
.gfs-facet:hover {
  background: var(--gfs-facet-hover, #f5f5f5);
}
.gfs-facet input[type="checkbox"] {
  accent-color: var(--gfs-facet-active-border, #4285f4);
}
.gfs-facet-label {
  flex: 1;
}
.gfs-facet-count {
  font-size: 0.8em;
  opacity: 0.6;
  min-width: 1.5em;
  text-align: right;
}
.gfs-facet-count:empty {
  display: none;
}
.gfs-facet-all {
  margin-top: 0.5rem;
  padding-top: 0.5rem;
  border-top: 1px solid var(--gfs-divider, #eee);
  font-size: 0.85em;
  opacity: 0.8;
}
.gfs-input {
  width: 100%;
  padding: 0.75rem 1rem;
  font-size: 1.1em;
  border: 1px solid var(--gfs-border, #ddd);
  border-radius: var(--gfs-radius, 4px);
  box-sizing: border-box;
}
.gfs-input:focus {
  outline: 2px solid var(--gfs-focus, #4285f4);
  outline-offset: 1px;
}
.gfs-status {
  padding: 0.5rem 0;
  font-size: 0.9em;
  opacity: 0.7;
}
.gfs-results {
  list-style: none;
  margin: 1rem 0 0;
  padding: 0;
}
.gfs-result {
  display: block;
  padding: 0.75rem 0;
  border-bottom: 1px solid var(--gfs-divider, #eee);
  text-decoration: none;
  color: inherit;
}
.gfs-result:last-child {
  border-bottom: none;
}
.gfs-result:hover {
  background: var(--gfs-result-hover, #fafafa);
}
.gfs-result strong {
  display: block;
  font-size: 1.05em;
}
.gfs-result small {
  display: block;
  opacity: 0.7;
  font-size: 0.85em;
  margin-top: 0.25rem;
}
.gfs-result time {
  font-size: 0.8em;
  opacity: 0.5;
}
.gfs-result .gfs-context {
  font-size: 0.8em;
  opacity: 0.5;
}
.gfs-loading {
  opacity: 0.6;
  font-style: italic;
}
@media (max-width: 600px) {
  .gfs-layout { flex-direction: column; }
  .gfs-sidebar { flex: none; position: static; }
  .gfs-facets { display: flex; flex-wrap: wrap; gap: 0.25rem; }
  .gfs-facet { width: auto; }
}
`;

let facetedStylesInjected = false;

class GeoffFacetedSearch extends HTMLElement {
  constructor() {
    super();
    this._store = null;
    this._ox = null;
    this._loadedGraphs = new Set();
    this._facets = [];
    this._activeFacets = new Set();
    this._initialized = false;
  }

  async connectedCallback() {
    if (typeof window === 'undefined') return;
    if (this._initialized) return;
    this._initialized = true;

    if (!facetedStylesInjected) {
      const style = document.createElement('style');
      style.textContent = FACETED_STYLES;
      document.head.appendChild(style);
      facetedStylesInjected = true;
    }

    const placeholder = this.getAttribute('placeholder') || 'Search…';
    this.innerHTML = `
      <div class="gfs-layout">
        <aside class="gfs-sidebar">
          <h3 style="margin-top:0">Facets</h3>
          <div class="gfs-facets gfs-loading">Loading…</div>
        </aside>
        <div class="gfs-main">
          <form role="search">
            <input type="search" class="gfs-input" placeholder="${placeholder}"
                   aria-label="Search" autocomplete="off" />
          </form>
          <div class="gfs-status" aria-live="polite"></div>
          <ul class="gfs-results" role="list"></ul>
        </div>
      </div>
    `;

    const input = this.querySelector('.gfs-input');
    const form = this.querySelector('form');
    let debounce;

    input.addEventListener('input', () => {
      clearTimeout(debounce);
      debounce = setTimeout(() => this._search(input.value), 250);
    });

    form.addEventListener('submit', (e) => e.preventDefault());

    // Read query params
    const params = new URLSearchParams(location.search);
    const q = params.get('q');
    if (q) {
      input.value = q;
    }

    await this._init();

    if (q) {
      this._search(q);
    }
  }

  async _init() {
    try {
      const wasmSrc = this.getAttribute('wasm-src');
      if (wasmSrc) {
        try {
          const mod = await import(wasmSrc);
          await mod.default();
          this._store = new mod.GeoffSparql();
          this._store._geoff = true;
        } catch {
          const ox = await import('https://esm.sh/oxigraph@0.5');
          await ox.default();
          this._store = new ox.Store();
        }
      } else {
        const ox = await import('https://esm.sh/oxigraph@0.5');
        await ox.default();
        this._store = new ox.Store();
      }

      // Load main graph (includes manifest)
      const indexUrl = this.getAttribute('index') || '/search.nt';
      await this._loadGraph(indexUrl);

      // Discover facets from manifest
      this._discoverFacets();
    } catch (e) {
      this._setStatus('Search unavailable');
      console.error('[geoff-faceted-search]', e);
    }
  }

  async _loadGraph(url) {
    if (this._loadedGraphs.has(url)) return;
    try {
      const response = await fetch(url);
      if (!response.ok) throw new Error(`Failed to fetch ${url}`);
      const nt = await response.text();
      if (this._store._geoff) {
        this._store.load(nt);
      } else {
        this._store.load(nt, { format: 'nt' });
      }
      this._loadedGraphs.add(url);
    } catch (e) {
      console.error(`[geoff-faceted-search] Failed to load ${url}:`, e);
    }
  }

  _discoverFacets() {
    const sparql = `
      SELECT ?url ?name WHERE {
        ?url <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <urn:geoff:SearchGraph> .
        ?url <urn:geoff:graphName> ?name .
      }
      ORDER BY ?name
    `;

    try {
      let arr;
      if (this._store._geoff) {
        arr = JSON.parse(this._store.query(sparql));
      } else {
        arr = [...this._store.query(sparql)];
      }
      const v = (row, key) => row.get ? row.get(key)?.value : row[key];
      this._facets = arr.map(row => ({
        name: v(row, 'name') || '',
        url: v(row, 'url') || '',
        loaded: false,
      }));
    } catch (e) {
      console.error('[geoff-faceted-search] facet discovery error:', e);
    }

    this._renderFacets();
  }

  _renderFacets() {
    const container = this.querySelector('.gfs-facets');
    if (this._facets.length === 0) {
      container.innerHTML = '<em>No facets available</em>';
      container.classList.remove('gfs-loading');
      return;
    }

    container.classList.remove('gfs-loading');
    container.innerHTML = `
      ${this._facets.map(f => `
        <label class="gfs-facet">
          <input type="checkbox" data-facet="${this._esc(f.name)}" />
          <span class="gfs-facet-label">${this._titleCase(f.name)}</span>
          <span class="gfs-facet-count" data-count-facet="${this._esc(f.name)}"></span>
        </label>
      `).join('')}
      <label class="gfs-facet gfs-facet-all">
        <input type="checkbox" data-facet="all" />
        <span class="gfs-facet-label">Select all</span>
      </label>
    `;

    container.querySelectorAll('input[data-facet]').forEach(cb => {
      cb.addEventListener('change', () => this._toggleFacet(cb));
    });

    // Restore facet state from URL
    const params = new URLSearchParams(location.search);
    const facetParam = params.get('facets');
    if (facetParam) {
      if (facetParam === 'all') {
        this.querySelector('[data-facet="all"]').checked = true;
        this._toggleFacet(this.querySelector('[data-facet="all"]'));
      } else {
        for (const name of facetParam.split(',')) {
          const cb = this.querySelector(`[data-facet="${name}"]`);
          if (cb) {
            cb.checked = true;
            this._toggleFacet(cb);
          }
        }
      }
    }
  }

  async _toggleFacet(cb) {
    const facet = cb.dataset.facet;
    const allCb = this.querySelector('[data-facet="all"]');

    if (facet === 'all') {
      if (cb.checked) {
        // Select all: check all facets and load their graphs
        this.querySelectorAll('input[data-facet]').forEach(c => { c.checked = true; });
        for (const f of this._facets) {
          this._activeFacets.add(f.name);
          if (!f.loaded) {
            await this._loadGraph(`/search/${f.name}.nt`);
            f.loaded = true;
          }
        }
      } else {
        // Deselect all
        this._activeFacets.clear();
        this.querySelectorAll('input[data-facet]').forEach(c => { c.checked = false; });
      }
    } else {
      if (cb.checked) {
        this._activeFacets.add(facet);
        const facetData = this._facets.find(f => f.name === facet);
        if (facetData && !facetData.loaded) {
          cb.parentElement.classList.add('gfs-loading');
          await this._loadGraph(`/search/${facet}.nt`);
          facetData.loaded = true;
          cb.parentElement.classList.remove('gfs-loading');
        }
      } else {
        this._activeFacets.delete(facet);
      }

      // Update "all" checkbox state
      if (allCb) {
        allCb.checked = this._activeFacets.size === this._facets.length;
      }
    }

    // Update URL with facet state
    this._updateUrl();

    // Re-run search (always, even with empty query — facet change should update results)
    const input = this.querySelector('.gfs-input');
    this._search(input.value);
  }

  _updateUrl() {
    const url = new URL(location.href);
    if (this._activeFacets.size === 0) {
      url.searchParams.delete('facets');
    } else if (this._activeFacets.size === this._facets.length) {
      url.searchParams.set('facets', 'all');
    } else {
      url.searchParams.set('facets', [...this._activeFacets].sort().join(','));
    }
    history.replaceState(null, '', url);
  }

  _search(query) {
    const results = this.querySelector('.gfs-results');
    const hasQuery = query && query.trim().length > 0;
    const hasFacets = this._activeFacets.size > 0;

    if (!hasQuery && !hasFacets) {
      results.innerHTML = '';
      this._setStatus('');
      return;
    }

    if (!this._store) return;

    const limit = parseInt(this.getAttribute('limit') || '50', 10);
    const filters = [];

    // Text search filter
    if (hasQuery) {
      const tokens = this._parseQuery(query.trim());
      if (tokens.length > 0) {
        filters.push(this._buildFilter(tokens));
      }
    }

    // Facet filter: restrict to pages whose URL starts with a selected section
    if (hasFacets && this._activeFacets.size < this._facets.length) {
      const sectionFilters = [...this._activeFacets].map(f =>
        `STRSTARTS(STR(?url), "/${f}/") || STRSTARTS(STR(?url), "/${f}.")`
      );
      filters.push(`(${sectionFilters.join(' || ')})`);
    }

    const filterClause = filters.length > 0
      ? `FILTER(${filters.join(' && ')})`
      : '';

    const sparql = `
      SELECT ?s ?title ?url ?desc ?date WHERE {
        ?s <https://schema.org/name> ?title .
        OPTIONAL { ?s <https://schema.org/url> ?url }
        OPTIONAL { ?s <https://schema.org/description> ?desc }
        OPTIONAL { ?s <https://schema.org/datePublished> ?date }
        ${filterClause}
      }
      ORDER BY DESC(?date) ?title
      LIMIT ${limit}
    `;

    try {
      let arr;
      if (this._store._geoff) {
        arr = JSON.parse(this._store.query(sparql));
      } else {
        arr = [...this._store.query(sparql)];
      }
      this._renderResults(arr, query);
      this._updateFacetCounts(hasQuery ? query : null);

      // Update URL params
      const url = new URL(location.href);
      if (hasQuery) {
        url.searchParams.set('q', query);
      } else {
        url.searchParams.delete('q');
      }
      history.replaceState(null, '', url);
    } catch (e) {
      console.error('[geoff-faceted-search] query error:', e);
      this._setStatus('Search error');
    }
  }

  _updateFacetCounts(query) {
    if (!this._store) return;

    for (const f of this._facets) {
      const el = this.querySelector(`[data-count-facet="${f.name}"]`);
      if (!el) continue;

      if (!f.loaded) {
        el.textContent = '';
        continue;
      }

      // Count matching results in this facet's section
      const textFilter = query ? this._buildFilter(this._parseQuery(query)) : '';
      const sectionFilter = `STRSTARTS(STR(?url), "/${f.name}/") || STRSTARTS(STR(?url), "/${f.name}.")`;
      const filters = [sectionFilter];
      if (textFilter) filters.push(textFilter);

      const sparql = `
        SELECT (COUNT(DISTINCT ?s) AS ?count) WHERE {
          ?s <https://schema.org/name> ?title .
          OPTIONAL { ?s <https://schema.org/url> ?url }
          OPTIONAL { ?s <https://schema.org/description> ?desc }
          FILTER(${filters.join(' && ')})
        }
      `;

      try {
        let count = 0;
        if (this._store._geoff) {
          const result = JSON.parse(this._store.query(sparql));
          count = parseInt(result[0]?.count || '0', 10);
        } else {
          const result = [...this._store.query(sparql)];
          count = parseInt(this._v(result[0], 'count') || '0', 10);
        }
        el.textContent = count > 0 ? `(${count})` : '';
      } catch {
        el.textContent = '';
      }
    }
  }

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
      if (word === 'OR') tokens.push({ type: 'OR' });
      else if (word !== 'AND') tokens.push({ type: 'term', value: word });
    }
    return tokens.filter(t => t.type !== 'term' || t.value.length > 0);
  }

  _buildFilter(tokens) {
    const groups = [[]];
    for (const token of tokens) {
      if (token.type === 'OR') groups.push([]);
      else groups[groups.length - 1].push(token);
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

  _v(row, key) {
    if (row.get) return row.get(key)?.value;
    return row[key];
  }

  _resolveUrl(row) {
    const url = this._v(row, 'url');
    if (url) return url;
    const s = this._v(row, 's') || '';
    if (s.startsWith('urn:geoff:content:'))
      return '/' + s.replace('urn:geoff:content:', '').replace(/\.md$/, '.html').replace(/index\.html$/, '');
    return '#';
  }

  _renderResults(bindings, query) {
    const container = this.querySelector('.gfs-results');
    if (!bindings || bindings.length === 0) {
      container.innerHTML = '';
      this._setStatus(`No results for "${query}"`);
      return;
    }

    const seen = new Set();
    const results = [];
    for (const row of bindings) {
      const title = this._v(row, 'title') || 'Untitled';
      const url = this._resolveUrl(row);
      if (seen.has(url)) continue;
      seen.add(url);
      const desc = this._v(row, 'desc') || '';
      const date = this._v(row, 'date') || '';
      const parts = url.replace(/^\//, '').replace(/\/$/, '').replace(/\.html$/, '').split('/');
      const context = parts.length > 1
        ? parts.slice(0, -1).map(p => p.replace(/-/g, ' ')).map(p => p.charAt(0).toUpperCase() + p.slice(1)).join(' › ')
        : '';
      results.push({ title, url, desc, date, context });
    }

    this._setStatus(`${results.length} result${results.length === 1 ? '' : 's'}`);
    container.innerHTML = results.map(({ title, url, desc, date, context }) => `
      <li class="gfs-result">
        <a href="${this._esc(url)}">
          <strong>${this._esc(title)}</strong>
          ${date ? `<time>${date}</time>` : ''}
          ${context ? `<span class="gfs-context">${this._esc(context)}</span>` : ''}
          ${desc ? `<small>${this._esc(desc)}</small>` : ''}
        </a>
      </li>
    `).join('');
  }

  _setStatus(text) {
    const el = this.querySelector('.gfs-status');
    if (el) el.textContent = text;
  }

  _titleCase(s) {
    return s.split('-').map(w => w.charAt(0).toUpperCase() + w.slice(1)).join(' ');
  }

  _esc(str) {
    const d = document.createElement('div');
    d.textContent = str;
    return d.innerHTML;
  }
}

customElements.define('geoff-faceted-search', GeoffFacetedSearch);

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
 *
 * Search syntax:
 *   foo bar         — implicit AND (both must match)
 *   "exact phrase"  — quoted exact match (case-insensitive)
 *   foo OR bar      — either term matches
 *   foo AND bar     — explicit AND (same as space)
 *
 * Keyboard:
 *   ArrowDown / ArrowUp — navigate results
 *   Enter               — go to selected result
 *   Escape              — close results
 */

const STYLES = `
geoff-search {
  display: block;
}
.geoff-search-form {
  position: relative;
}
.geoff-search-dropdown {
  position: absolute;
  top: 100%;
  left: 0;
  right: 0;
  margin-top: 4px;
  z-index: 9999;
  background: var(--geoff-search-bg, #fff);
  border: 1px solid var(--geoff-search-border, #ccc);
  border-radius: var(--geoff-search-radius, 4px);
  box-shadow: var(--geoff-search-shadow, 0 4px 12px rgba(0,0,0,.15));
  display: none;
}
.geoff-search-dropdown.is-open {
  display: block;
}
.geoff-search-status {
  font-size: 0.85em;
  padding: 0.5rem 0.75rem;
  border-bottom: 1px solid var(--geoff-search-divider, #eee);
}
.geoff-search-status:empty {
  display: none;
}
.geoff-search-results {
  margin: 0;
  padding: 0;
  list-style: none;
  max-height: var(--geoff-search-max-height, 50vh);
  overflow-y: auto;
  overscroll-behavior: contain;
}
.geoff-search-result {
  display: block;
  padding: 8px 12px;
  text-decoration: none;
  color: inherit;
  border-bottom: 1px solid var(--geoff-search-divider, #eee);
  outline: none;
}
.geoff-search-result:last-child {
  border-bottom: none;
}
.geoff-search-result[aria-selected="true"],
.geoff-search-result:focus {
  background: var(--geoff-search-highlight, #f0f4ff);
}
.geoff-search-result strong {
  display: block;
}
.geoff-search-result small {
  display: block;
  opacity: 0.7;
  font-size: 0.85em;
}
.geoff-search-result time {
  display: block;
  font-size: 0.8em;
  opacity: 0.6;
}
`;

let stylesInjected = false;

class GeoffSearch extends HTMLElement {
  constructor() {
    super();
    this._store = null;
    this._loading = false;
    this._loaded = false;
    this._activeIndex = -1;
    this._resultCount = 0;
    this._listboxId = `geoff-search-listbox-${Math.random().toString(36).slice(2, 8)}`;
  }

  connectedCallback() {
    if (typeof window === 'undefined') return;

    if (!stylesInjected) {
      const style = document.createElement('style');
      style.textContent = STYLES;
      document.head.appendChild(style);
      stylesInjected = true;
    }

    if (!this.querySelector('input')) {
      this.innerHTML = `
        <form role="search" class="geoff-search-form">
          <input type="search"
            placeholder="Search…"
            aria-label="Search"
            role="combobox"
            aria-expanded="false"
            aria-autocomplete="list"
            aria-controls="${this._listboxId}"
            autocomplete="off" />
          <div class="geoff-search-dropdown">
            <div class="geoff-search-status" aria-live="polite"></div>
            <ul class="geoff-search-results"
              id="${this._listboxId}"
              role="listbox"
              aria-label="Search results"></ul>
          </div>
        </form>
      `;
    }

    const input = this.querySelector('input');
    const form = this.querySelector('form');
    let debounce;

    input.addEventListener('input', () => {
      clearTimeout(debounce);
      debounce = setTimeout(() => this._search(input.value), 200);
    });

    input.addEventListener('focus', () => this._ensureLoaded(), { once: true });

    input.addEventListener('keydown', (e) => this._onKeydown(e));

    form.addEventListener('submit', (e) => {
      e.preventDefault();
      this._activateSelected();
    });

    document.addEventListener('click', (e) => {
      if (!this.contains(e.target)) this._closeResults();
    });
  }

  _onKeydown(e) {
    const input = this.querySelector('input');

    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        this._moveSelection(1);
        break;
      case 'ArrowUp':
        e.preventDefault();
        this._moveSelection(-1);
        break;
      case 'Enter':
        if (this._activeIndex >= 0) {
          e.preventDefault();
          this._activateSelected();
        }
        break;
      case 'Escape':
        e.preventDefault();
        this._closeResults();
        input.blur();
        break;
      case 'Home':
        if (this._resultCount > 0) {
          e.preventDefault();
          this._setSelection(0);
        }
        break;
      case 'End':
        if (this._resultCount > 0) {
          e.preventDefault();
          this._setSelection(this._resultCount - 1);
        }
        break;
    }
  }

  _moveSelection(delta) {
    if (this._resultCount === 0) return;
    let next = this._activeIndex + delta;
    if (next < 0) next = this._resultCount - 1;
    if (next >= this._resultCount) next = 0;
    this._setSelection(next);
  }

  _setSelection(index) {
    const options = this.querySelectorAll('[role="option"]');
    const input = this.querySelector('input');

    options.forEach((opt, i) => {
      opt.setAttribute('aria-selected', i === index ? 'true' : 'false');
    });

    this._activeIndex = index;

    if (index >= 0 && options[index]) {
      input.setAttribute('aria-activedescendant', options[index].id);
      options[index].scrollIntoView({ block: 'nearest' });
    } else {
      input.removeAttribute('aria-activedescendant');
    }
  }

  _activateSelected() {
    const selected = this.querySelector('[aria-selected="true"]');
    if (selected) {
      const url = selected.getAttribute('data-url');
      if (url) window.location.href = url;
    }
  }

  _closeResults() {
    const container = this.querySelector('.geoff-search-results');
    const dropdown = this.querySelector('.geoff-search-dropdown');
    const input = this.querySelector('input');
    if (container) container.innerHTML = '';
    if (dropdown) dropdown.classList.remove('is-open');
    if (input) input.setAttribute('aria-expanded', 'false');
    input?.removeAttribute('aria-activedescendant');
    this._activeIndex = -1;
    this._resultCount = 0;
    this._setStatus('');
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
    const container = this.querySelector('.geoff-search-results');
    const input = this.querySelector('input');
    if (!query.trim()) {
      this._closeResults();
      return;
    }

    await this._ensureLoaded();
    if (!this._loaded) return;

    const tokens = this._parseQuery(query.trim());
    if (tokens.length === 0) {
      this._closeResults();
      return;
    }

    const filter = this._buildFilter(tokens);
    const limit = parseInt(this.getAttribute('limit') || '20', 10);

    const sparql = `
      SELECT ?s ?title ?url ?desc ?date WHERE {
        ?s <https://schema.org/name> ?title .
        OPTIONAL { ?s <https://schema.org/url> ?url }
        OPTIONAL { ?s <https://schema.org/description> ?desc }
        OPTIONAL { ?s <https://schema.org/datePublished> ?date }
        FILTER(${filter})
      }
      ORDER BY DESC(?date) ?title
      LIMIT ${limit}
    `;

    try {
      const bindings = this._store.query(sparql);
      const arr = (bindings && typeof bindings[Symbol.iterator] === 'function')
        ? [...bindings]
        : bindings;
      this._renderResults(arr, query);
      const dropdown = this.querySelector('.geoff-search-dropdown');
      if (dropdown) dropdown.classList.add('is-open');
      input.setAttribute('aria-expanded', this._resultCount > 0 ? 'true' : 'false');
    } catch (e) {
      console.error('[geoff-search] query error:', e);
      this._setStatus('Search error');
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

  _resolveUrl(row) {
    const url = row.get('url')?.value;
    if (url) return url;
    const s = row.get('s')?.value || '';
    if (s.startsWith('urn:geoff:content:')) {
      return '/' + s.replace('urn:geoff:content:', '').replace(/\.md$/, '.html').replace(/index\.html$/, '');
    }
    return '#';
  }

  _renderResults(bindings, query) {
    const container = this.querySelector('.geoff-search-results');

    if (!bindings || bindings.length === 0) {
      container.innerHTML = '';
      this._activeIndex = -1;
      this._resultCount = 0;
      this._setStatus(`No results for "${query}"`);
      return;
    }

    const seen = new Set();
    const results = [];
    for (const row of bindings) {
      const title = row.get('title')?.value || 'Untitled';
      const url = this._resolveUrl(row);
      if (seen.has(url)) continue;
      seen.add(url);
      const desc = row.get('desc')?.value || '';
      const date = row.get('date')?.value || '';
      const parts = url.replace(/^\//, '').replace(/\/$/, '').replace(/\.html$/, '').split('/');
      const context = parts.length > 1
        ? parts.slice(0, -1).map(p => p.replace(/-/g, ' ')).map(p => p.charAt(0).toUpperCase() + p.slice(1)).join(' › ')
        : '';
      results.push({ title, url, desc, date, context });
    }

    this._resultCount = results.length;
    this._activeIndex = -1;
    this._setStatus(`${results.length} result${results.length === 1 ? '' : 's'}`);

    container.innerHTML = results.map(({ title, url, desc, date, context }, i) => {
      const t = this._esc(title);
      const c = this._esc(context);
      const d = this._esc(desc);
      const optionId = `${this._listboxId}-opt-${i}`;
      return `<li role="option" id="${optionId}" aria-selected="false"
                  data-url="${this._esc(url)}" class="geoff-search-result"
                  tabindex="-1">
        <strong>${t}</strong>
        ${date ? `<time>${date}</time>` : ''}
        ${c ? `<small class="geoff-search-context">${c}</small>` : ''}
        ${d ? `<small>${d}</small>` : ''}
      </li>`;
    }).join('');

    container.querySelectorAll('[role="option"]').forEach((opt) => {
      opt.addEventListener('click', () => {
        const url = opt.getAttribute('data-url');
        if (url) window.location.href = url;
      });
    });
  }

  _setStatus(text) {
    const el = this.querySelector('.geoff-search-status');
    const dropdown = this.querySelector('.geoff-search-dropdown');
    if (el) el.textContent = text;
    if (dropdown) {
      if (text) dropdown.classList.add('is-open');
      else if (!this.querySelector('.geoff-search-result')) dropdown.classList.remove('is-open');
    }
  }

  _esc(str) {
    if (typeof document === 'undefined') return str.replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);
    const d = document.createElement('div');
    d.textContent = str;
    return d.innerHTML;
  }
}

customElements.define('geoff-search', GeoffSearch);

/**
 * geoff-shacl-panel
 *
 * SHACL validation dashboard showing validation results,
 * violations, and validation status per page.
 */
class GeoffShaclPanel extends HTMLElement {
  constructor() {
    super();
    this.attachShadow({ mode: 'open' });
    this.validationResults = null;
    this.filterSeverity = 'all';
    this.sortBy = 'severity';
  }

  connectedCallback() {
    this.render();
    this.setupEventListeners();
    this.loadValidation();
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
          --success: #28a745;
          --warning: #ffc107;
          --error: #dc3545;
          --info: #17a2b8;
        }

        .container {
          display: flex;
          flex-direction: column;
          height: 100%;
          background: var(--bg-primary);
        }

        .header {
          padding: 16px;
          background: var(--bg-secondary);
          border-bottom: 1px solid var(--border-color);
        }

        .summary {
          display: flex;
          gap: 16px;
          margin-bottom: 16px;
        }

        .stat-card {
          flex: 1;
          padding: 12px;
          background: var(--bg-primary);
          border: 1px solid var(--border-color);
          border-radius: 4px;
          text-align: center;
        }

        .stat-value {
          font-size: 24px;
          font-weight: 600;
          margin-bottom: 4px;
        }

        .stat-value.success { color: var(--success); }
        .stat-value.error { color: var(--error); }
        .stat-value.warning { color: var(--warning); }

        .stat-label {
          font-size: 12px;
          color: var(--text-secondary);
          text-transform: uppercase;
          letter-spacing: 0.5px;
        }

        .controls {
          display: flex;
          align-items: center;
          gap: 12px;
          flex-wrap: wrap;
        }

        .controls label {
          font-size: 13px;
          color: var(--text-secondary);
        }

        .controls select,
        .controls button {
          padding: 6px 12px;
          border: 1px solid var(--border-color);
          border-radius: 4px;
          font-size: 13px;
          background: var(--bg-primary);
          cursor: pointer;
        }

        .controls button {
          background: var(--accent);
          color: white;
          border-color: var(--accent);
        }

        .controls button:hover {
          filter: brightness(1.1);
        }

        .controls button:disabled {
          opacity: 0.5;
          cursor: not-allowed;
        }

        .content {
          flex: 1;
          overflow-y: auto;
          padding: 16px;
        }

        .section {
          margin-bottom: 24px;
        }

        .section-header {
          font-size: 16px;
          font-weight: 600;
          margin-bottom: 12px;
          color: var(--text-primary);
          display: flex;
          align-items: center;
          gap: 8px;
        }

        .badge {
          display: inline-flex;
          align-items: center;
          justify-content: center;
          min-width: 24px;
          height: 20px;
          padding: 0 6px;
          border-radius: 10px;
          font-size: 11px;
          font-weight: 600;
          color: white;
        }

        .badge.violation { background: var(--error); }
        .badge.warning { background: var(--warning); }
        .badge.info { background: var(--info); }

        .violation-list {
          list-style: none;
          margin: 0;
          padding: 0;
        }

        .violation-card {
          padding: 12px;
          margin-bottom: 8px;
          border: 1px solid var(--border-color);
          border-left: 4px solid;
          border-radius: 4px;
          background: var(--bg-primary);
          cursor: pointer;
          transition: all 0.2s;
        }

        .violation-card:hover {
          box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
        }

        .violation-card.violation {
          border-left-color: var(--error);
        }

        .violation-card.warning {
          border-left-color: var(--warning);
        }

        .violation-card.info {
          border-left-color: var(--info);
        }

        .violation-header {
          display: flex;
          align-items: flex-start;
          gap: 12px;
          margin-bottom: 8px;
        }

        .violation-severity {
          display: inline-block;
          padding: 2px 8px;
          border-radius: 3px;
          font-size: 11px;
          font-weight: 600;
          text-transform: uppercase;
          color: white;
          flex-shrink: 0;
        }

        .violation-severity.violation { background: var(--error); }
        .violation-severity.warning { background: var(--warning); }
        .violation-severity.info { background: var(--info); }

        .violation-message {
          flex: 1;
          font-size: 14px;
          color: var(--text-primary);
          line-height: 1.5;
        }

        .violation-details {
          display: grid;
          grid-template-columns: auto 1fr;
          gap: 8px;
          font-size: 13px;
          color: var(--text-secondary);
          margin-top: 8px;
        }

        .violation-label {
          font-weight: 500;
        }

        .violation-value {
          font-family: monospace;
          font-size: 12px;
        }

        .page-list {
          list-style: none;
          margin: 0;
          padding: 0;
        }

        .page-item {
          padding: 12px;
          margin-bottom: 8px;
          border: 1px solid var(--border-color);
          border-radius: 4px;
          background: var(--bg-primary);
          cursor: pointer;
          transition: all 0.2s;
        }

        .page-item:hover {
          background: var(--bg-hover);
        }

        .page-item.pass {
          border-left: 4px solid var(--success);
        }

        .page-item.fail {
          border-left: 4px solid var(--error);
        }

        .page-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          gap: 12px;
        }

        .page-path {
          font-size: 14px;
          font-weight: 500;
          color: var(--text-primary);
        }

        .page-status {
          font-size: 12px;
          padding: 2px 8px;
          border-radius: 3px;
          font-weight: 600;
        }

        .page-status.pass {
          background: var(--success);
          color: white;
        }

        .page-status.fail {
          background: var(--error);
          color: white;
        }

        .page-violations {
          margin-top: 8px;
          font-size: 12px;
          color: var(--text-secondary);
        }

        .empty-state {
          display: flex;
          flex-direction: column;
          align-items: center;
          justify-content: center;
          padding: 60px 20px;
          text-align: center;
          color: var(--text-secondary);
        }

        .empty-state-icon {
          font-size: 48px;
          margin-bottom: 16px;
        }

        .empty-state-title {
          font-size: 18px;
          font-weight: 600;
          margin-bottom: 8px;
          color: var(--text-primary);
        }

        .empty-state-message {
          font-size: 14px;
          max-width: 400px;
        }

        .loading {
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 60px 20px;
          color: var(--text-secondary);
          font-size: 14px;
        }

        .success-state {
          background: linear-gradient(135deg, #d4edda 0%, #c3e6cb 100%);
          border: 1px solid #c3e6cb;
          border-radius: 8px;
          padding: 24px;
          text-align: center;
        }

        .success-state-icon {
          font-size: 48px;
          margin-bottom: 12px;
        }

        .success-state-title {
          font-size: 18px;
          font-weight: 600;
          color: #155724;
          margin-bottom: 8px;
        }

        .success-state-message {
          font-size: 14px;
          color: #155724;
        }

        @media (max-width: 768px) {
          .summary {
            flex-direction: column;
          }

          .controls {
            flex-direction: column;
            align-items: stretch;
          }

          .controls select,
          .controls button {
            width: 100%;
          }
        }
      </style>

      <div class="container">
        <div class="header">
          <div class="summary">
            <div class="stat-card">
              <div class="stat-value" id="total-pages">-</div>
              <div class="stat-label">Total Pages</div>
            </div>
            <div class="stat-card">
              <div class="stat-value success" id="passing-pages">-</div>
              <div class="stat-label">Passing</div>
            </div>
            <div class="stat-card">
              <div class="stat-value error" id="failing-pages">-</div>
              <div class="stat-label">Failing</div>
            </div>
            <div class="stat-card">
              <div class="stat-value warning" id="total-violations">-</div>
              <div class="stat-label">Violations</div>
            </div>
          </div>

          <div class="controls">
            <label>
              Severity:
              <select id="severity-filter">
                <option value="all">All</option>
                <option value="Violation">Violations Only</option>
                <option value="Warning">Warnings Only</option>
                <option value="Info">Info Only</option>
              </select>
            </label>
            <label>
              Sort by:
              <select id="sort-by">
                <option value="severity">Severity</option>
                <option value="page">Page</option>
                <option value="property">Property</option>
              </select>
            </label>
            <button id="validate-btn">Re-validate</button>
          </div>
        </div>

        <div class="content" id="content">
          <div class="loading">Loading validation results...</div>
        </div>
      </div>
    `;
  }

  setupEventListeners() {
    const validateBtn = this.shadowRoot.getElementById('validate-btn');
    const severityFilter = this.shadowRoot.getElementById('severity-filter');
    const sortBy = this.shadowRoot.getElementById('sort-by');

    validateBtn.addEventListener('click', () => this.validate());
    severityFilter.addEventListener('change', (e) => {
      this.filterSeverity = e.target.value;
      this.renderResults();
    });
    sortBy.addEventListener('change', (e) => {
      this.sortBy = e.target.value;
      this.renderResults();
    });
  }

  async loadValidation() {
    try {
      const response = await fetch('/api/validate');
      if (!response.ok) throw new Error(`Failed to load validation: ${response.statusText}`);

      this.validationResults = await response.json();
      this.renderResults();
    } catch (error) {
      console.error('Failed to load validation:', error);
      this.showError('Failed to load validation results');
    }
  }

  async validate() {
    const btn = this.shadowRoot.getElementById('validate-btn');
    btn.disabled = true;
    btn.textContent = 'Validating...';

    try {
      const response = await fetch('/api/validate', { method: 'POST' });
      if (!response.ok) throw new Error(`Validation failed: ${response.statusText}`);

      this.validationResults = await response.json();
      this.renderResults();
    } catch (error) {
      console.error('Validation failed:', error);
      this.showError('Validation failed');
    } finally {
      btn.disabled = false;
      btn.textContent = 'Re-validate';
    }
  }

  renderResults() {
    if (!this.validationResults) return;

    this.updateSummary();

    const violations = this.getFilteredViolations();

    if (violations.length === 0 && this.filterSeverity === 'all') {
      this.showSuccess();
      return;
    }

    if (violations.length === 0) {
      this.showEmptyFilter();
      return;
    }

    this.renderViolations(violations);
  }

  updateSummary() {
    const results = this.validationResults;

    const totalPages = results.pages?.length || 0;
    const passingPages = results.pages?.filter(p => p.conforms).length || 0;
    const failingPages = totalPages - passingPages;
    const totalViolations = results.violations?.length || 0;

    this.shadowRoot.getElementById('total-pages').textContent = totalPages;
    this.shadowRoot.getElementById('passing-pages').textContent = passingPages;
    this.shadowRoot.getElementById('failing-pages').textContent = failingPages;
    this.shadowRoot.getElementById('total-violations').textContent = totalViolations;
  }

  getFilteredViolations() {
    let violations = this.validationResults.violations || [];

    // Filter by severity
    if (this.filterSeverity !== 'all') {
      violations = violations.filter(v => v.severity === this.filterSeverity);
    }

    // Sort
    violations.sort((a, b) => {
      if (this.sortBy === 'severity') {
        const severityOrder = { Violation: 0, Warning: 1, Info: 2 };
        return severityOrder[a.severity] - severityOrder[b.severity];
      } else if (this.sortBy === 'page') {
        return (a.focusNode || '').localeCompare(b.focusNode || '');
      } else if (this.sortBy === 'property') {
        return (a.path || '').localeCompare(b.path || '');
      }
      return 0;
    });

    return violations;
  }

  renderViolations(violations) {
    const content = this.shadowRoot.getElementById('content');

    // Group violations by severity
    const grouped = {
      Violation: violations.filter(v => v.severity === 'Violation'),
      Warning: violations.filter(v => v.severity === 'Warning'),
      Info: violations.filter(v => v.severity === 'Info')
    };

    let html = '';

    // Render each severity group
    Object.entries(grouped).forEach(([severity, items]) => {
      if (items.length === 0) return;

      html += `
        <div class="section">
          <div class="section-header">
            ${severity}s
            <span class="badge ${severity.toLowerCase()}">${items.length}</span>
          </div>
          <ul class="violation-list">
            ${items.map(v => this.renderViolationCard(v)).join('')}
          </ul>
        </div>
      `;
    });

    // Also render page list
    html += this.renderPageList();

    content.innerHTML = html;

    // Add click handlers
    content.querySelectorAll('.violation-card').forEach((card, index) => {
      card.addEventListener('click', () => {
        const violation = violations[parseInt(card.dataset.index)];
        this.navigateToPage(violation.focusNode);
      });
    });

    content.querySelectorAll('.page-item').forEach(item => {
      item.addEventListener('click', () => {
        this.navigateToPage(item.dataset.path);
      });
    });
  }

  renderViolationCard(violation) {
    const index = (this.validationResults.violations || []).indexOf(violation);
    const severity = violation.severity || 'Info';

    return `
      <li class="violation-card ${severity.toLowerCase()}" data-index="${index}">
        <div class="violation-header">
          <span class="violation-severity ${severity.toLowerCase()}">${severity}</span>
          <div class="violation-message">${violation.message || 'Validation error'}</div>
        </div>
        <div class="violation-details">
          ${violation.focusNode ? `
            <span class="violation-label">Page:</span>
            <span class="violation-value">${this.getPagePath(violation.focusNode)}</span>
          ` : ''}
          ${violation.path ? `
            <span class="violation-label">Property:</span>
            <span class="violation-value">${this.getShortName(violation.path)}</span>
          ` : ''}
          ${violation.value ? `
            <span class="violation-label">Value:</span>
            <span class="violation-value">${violation.value}</span>
          ` : ''}
        </div>
      </li>
    `;
  }

  renderPageList() {
    const pages = this.validationResults.pages || [];

    if (pages.length === 0) return '';

    return `
      <div class="section">
        <div class="section-header">Pages</div>
        <ul class="page-list">
          ${pages.map(page => this.renderPageItem(page)).join('')}
        </ul>
      </div>
    `;
  }

  renderPageItem(page) {
    const status = page.conforms ? 'pass' : 'fail';
    const violationCount = page.violationCount || 0;

    return `
      <li class="page-item ${status}" data-path="${page.path || ''}">
        <div class="page-header">
          <span class="page-path">${page.path || 'Unknown'}</span>
          <span class="page-status ${status}">${status.toUpperCase()}</span>
        </div>
        ${violationCount > 0 ? `
          <div class="page-violations">
            ${violationCount} violation${violationCount !== 1 ? 's' : ''}
          </div>
        ` : ''}
      </li>
    `;
  }

  showSuccess() {
    const content = this.shadowRoot.getElementById('content');
    content.innerHTML = `
      <div class="success-state">
        <div class="success-state-icon">✓</div>
        <div class="success-state-title">All pages valid!</div>
        <div class="success-state-message">
          No SHACL violations found. Your content conforms to all defined shapes.
        </div>
      </div>
    `;
  }

  showEmptyFilter() {
    const content = this.shadowRoot.getElementById('content');
    content.innerHTML = `
      <div class="empty-state">
        <div class="empty-state-icon">🔍</div>
        <div class="empty-state-title">No results</div>
        <div class="empty-state-message">
          No violations match the current filter. Try changing the severity filter.
        </div>
      </div>
    `;
  }

  showError(message) {
    const content = this.shadowRoot.getElementById('content');
    content.innerHTML = `
      <div class="empty-state">
        <div class="empty-state-icon">⚠️</div>
        <div class="empty-state-title">Error</div>
        <div class="empty-state-message">${message}</div>
      </div>
    `;
  }

  navigateToPage(focusNode) {
    const path = this.getPagePath(focusNode);
    if (path) {
      this.dispatchEvent(new CustomEvent('geoff-navigate', {
        bubbles: true,
        composed: true,
        detail: { path }
      }));
    }
  }

  getPagePath(uri) {
    if (!uri) return '';
    // Extract path from URN like urn:geoff:content:blog/post.md
    const match = uri.match(/urn:geoff:content:(.+)/);
    return match ? match[1] : uri;
  }

  getShortName(uri) {
    if (!uri) return '';
    const parts = uri.split(/[/#]/);
    return parts[parts.length - 1] || uri;
  }
}

customElements.define('geoff-shacl-panel', GeoffShaclPanel);

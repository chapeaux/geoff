/**
 * geoff-graph-view
 *
 * RDF graph visualization with interactive force-directed layout.
 * Displays nodes (RDF subjects/objects) and edges (predicates).
 */
class GeoffGraphView extends HTMLElement {
  constructor() {
    super();
    this.attachShadow({ mode: 'open' });
    this.nodes = [];
    this.edges = [];
    this.selectedNode = null;
    this.animationFrame = null;
    this.zoom = 1;
    this.pan = { x: 0, y: 0 };
    this.isDragging = false;
    this.dragStart = { x: 0, y: 0 };
  }

  connectedCallback() {
    this.render();
    this.setupEventListeners();
    this.loadGraph();
  }

  disconnectedCallback() {
    if (this.animationFrame) {
      cancelAnimationFrame(this.animationFrame);
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
          --node-page: #3498db;
          --node-person: #e74c3c;
          --node-concept: #2ecc71;
          --node-default: #95a5a6;
        }

        .container {
          display: flex;
          flex-direction: column;
          height: 100%;
          background: var(--bg-primary);
        }

        .toolbar {
          display: flex;
          align-items: center;
          gap: 8px;
          padding: 8px 12px;
          background: var(--bg-secondary);
          border-bottom: 1px solid var(--border-color);
        }

        .toolbar label {
          font-size: 13px;
          color: var(--text-secondary);
        }

        .toolbar select,
        .toolbar input {
          padding: 4px 8px;
          border: 1px solid var(--border-color);
          border-radius: 4px;
          font-size: 13px;
        }

        .toolbar button {
          padding: 4px 12px;
          border: 1px solid var(--border-color);
          background: var(--bg-primary);
          border-radius: 4px;
          cursor: pointer;
          font-size: 13px;
        }

        .toolbar button:hover {
          background: var(--bg-secondary);
        }

        .view-container {
          flex: 1;
          display: flex;
          overflow: hidden;
          position: relative;
        }

        .graph-canvas {
          flex: 1;
          position: relative;
          overflow: hidden;
          background: var(--bg-primary);
          cursor: grab;
        }

        .graph-canvas.dragging {
          cursor: grabbing;
        }

        svg {
          width: 100%;
          height: 100%;
        }

        .edge {
          stroke: #ccc;
          stroke-width: 1.5;
          fill: none;
          marker-end: url(#arrowhead);
        }

        .edge.highlighted {
          stroke: var(--accent);
          stroke-width: 2;
        }

        .edge-label {
          font-size: 10px;
          fill: var(--text-secondary);
          pointer-events: none;
        }

        .node {
          cursor: pointer;
          transition: r 0.2s;
        }

        .node circle {
          stroke: #fff;
          stroke-width: 2;
        }

        .node.page circle {
          fill: var(--node-page);
        }

        .node.person circle {
          fill: var(--node-person);
        }

        .node.concept circle {
          fill: var(--node-concept);
        }

        .node.default circle {
          fill: var(--node-default);
        }

        .node.selected circle {
          stroke: var(--accent);
          stroke-width: 3;
        }

        .node:hover circle {
          filter: brightness(1.1);
        }

        .node-label {
          font-size: 11px;
          fill: var(--text-primary);
          pointer-events: none;
          text-anchor: middle;
        }

        .details-panel {
          width: 300px;
          border-left: 1px solid var(--border-color);
          background: var(--bg-secondary);
          overflow-y: auto;
          padding: 16px;
        }

        .details-panel.hidden {
          display: none;
        }

        .details-panel h3 {
          margin: 0 0 12px 0;
          font-size: 16px;
          color: var(--text-primary);
        }

        .property {
          margin-bottom: 12px;
        }

        .property-name {
          font-size: 12px;
          font-weight: 500;
          color: var(--text-secondary);
          margin-bottom: 2px;
        }

        .property-value {
          font-size: 13px;
          color: var(--text-primary);
          word-break: break-word;
        }

        .property-value code {
          background: var(--bg-primary);
          padding: 2px 4px;
          border-radius: 3px;
          font-family: monospace;
          font-size: 12px;
        }

        .legend {
          position: absolute;
          top: 12px;
          right: 12px;
          background: rgba(255, 255, 255, 0.95);
          border: 1px solid var(--border-color);
          border-radius: 4px;
          padding: 12px;
          font-size: 12px;
        }

        .legend-item {
          display: flex;
          align-items: center;
          gap: 8px;
          margin-bottom: 6px;
        }

        .legend-item:last-child {
          margin-bottom: 0;
        }

        .legend-color {
          width: 12px;
          height: 12px;
          border-radius: 50%;
          border: 1px solid #fff;
        }
      </style>

      <div class="container">
        <div class="toolbar">
          <label>
            Filter:
            <select id="filter-type">
              <option value="all">All Graph</option>
              <option value="page">By Page</option>
            </select>
          </label>
          <input type="text" id="filter-path" placeholder="Page path..." style="display: none;">
          <button id="reload-btn">Reload</button>
          <button id="reset-view-btn">Reset View</button>
        </div>

        <div class="view-container">
          <div class="graph-canvas" id="canvas">
            <svg id="graph-svg">
              <defs>
                <marker id="arrowhead" markerWidth="10" markerHeight="10" refX="20" refY="3" orient="auto">
                  <polygon points="0 0, 10 3, 0 6" fill="#ccc" />
                </marker>
              </defs>
              <g id="graph-group"></g>
            </svg>

            <div class="legend">
              <div class="legend-item">
                <div class="legend-color" style="background: var(--node-page);"></div>
                <span>Page</span>
              </div>
              <div class="legend-item">
                <div class="legend-color" style="background: var(--node-person);"></div>
                <span>Person</span>
              </div>
              <div class="legend-item">
                <div class="legend-color" style="background: var(--node-concept);"></div>
                <span>Concept</span>
              </div>
              <div class="legend-item">
                <div class="legend-color" style="background: var(--node-default);"></div>
                <span>Other</span>
              </div>
            </div>
          </div>

          <div class="details-panel hidden" id="details-panel">
            <h3 id="details-title">Node Details</h3>
            <div id="details-content"></div>
          </div>
        </div>
      </div>
    `;
  }

  setupEventListeners() {
    const filterType = this.shadowRoot.getElementById('filter-type');
    const filterPath = this.shadowRoot.getElementById('filter-path');
    const reloadBtn = this.shadowRoot.getElementById('reload-btn');
    const resetViewBtn = this.shadowRoot.getElementById('reset-view-btn');
    const canvas = this.shadowRoot.getElementById('canvas');

    filterType.addEventListener('change', (e) => {
      filterPath.style.display = e.target.value === 'page' ? 'inline-block' : 'none';
      if (e.target.value === 'page' && filterPath.value) {
        this.loadGraph(filterPath.value);
      } else {
        this.loadGraph();
      }
    });

    filterPath.addEventListener('input', (e) => {
      if (e.target.value) {
        this.loadGraph(e.target.value);
      }
    });

    reloadBtn.addEventListener('click', () => {
      const path = filterType.value === 'page' ? filterPath.value : null;
      this.loadGraph(path);
    });

    resetViewBtn.addEventListener('click', () => {
      this.zoom = 1;
      this.pan = { x: 0, y: 0 };
      this.updateTransform();
    });

    canvas.addEventListener('mousedown', (e) => this.handleMouseDown(e));
    canvas.addEventListener('mousemove', (e) => this.handleMouseMove(e));
    canvas.addEventListener('mouseup', (e) => this.handleMouseUp(e));
    canvas.addEventListener('wheel', (e) => this.handleWheel(e));
  }

  async loadGraph(path = null) {
    try {
      const url = path ? `/api/graph/${encodeURIComponent(path)}` : '/api/graph';
      const response = await fetch(url);
      if (!response.ok) throw new Error(`Failed to load graph: ${response.statusText}`);

      const data = await response.json();
      this.processGraphData(data);
      this.renderGraph();
      this.startSimulation();
    } catch (error) {
      console.error('Failed to load graph:', error);
    }
  }

  processGraphData(data) {
    // Convert RDF triples to nodes and edges
    const nodeMap = new Map();
    const edges = [];

    data.triples?.forEach(triple => {
      // Add subject node
      if (!nodeMap.has(triple.subject)) {
        nodeMap.set(triple.subject, {
          id: triple.subject,
          label: this.getNodeLabel(triple.subject),
          type: this.getNodeType(triple.subject),
          properties: {},
          x: Math.random() * 800,
          y: Math.random() * 600,
          vx: 0,
          vy: 0
        });
      }

      // Add object node (if it's a URI)
      if (triple.object.type === 'uri') {
        if (!nodeMap.has(triple.object.value)) {
          nodeMap.set(triple.object.value, {
            id: triple.object.value,
            label: this.getNodeLabel(triple.object.value),
            type: this.getNodeType(triple.object.value),
            properties: {},
            x: Math.random() * 800,
            y: Math.random() * 600,
            vx: 0,
            vy: 0
          });
        }

        edges.push({
          source: triple.subject,
          target: triple.object.value,
          predicate: this.getPredicateLabel(triple.predicate)
        });
      } else {
        // Store literal as property
        const node = nodeMap.get(triple.subject);
        node.properties[this.getPredicateLabel(triple.predicate)] = triple.object.value;
      }
    });

    this.nodes = Array.from(nodeMap.values());
    this.edges = edges;
  }

  getNodeLabel(uri) {
    const parts = uri.split(/[/#]/);
    return parts[parts.length - 1] || uri;
  }

  getNodeType(uri) {
    if (uri.includes('content/') || uri.includes('/page')) return 'page';
    if (uri.includes('Person') || uri.includes('author')) return 'person';
    if (uri.includes('schema:') || uri.includes('concept')) return 'concept';
    return 'default';
  }

  getPredicateLabel(uri) {
    const parts = uri.split(/[/#]/);
    return parts[parts.length - 1] || uri;
  }

  renderGraph() {
    const group = this.shadowRoot.getElementById('graph-group');
    group.innerHTML = '';

    // Render edges
    this.edges.forEach((edge, i) => {
      const line = document.createElementNS('http://www.w3.org/2000/svg', 'line');
      line.classList.add('edge');
      line.dataset.edgeIndex = i;
      group.appendChild(line);

      const label = document.createElementNS('http://www.w3.org/2000/svg', 'text');
      label.classList.add('edge-label');
      label.textContent = edge.predicate;
      group.appendChild(label);
    });

    // Render nodes
    this.nodes.forEach((node, i) => {
      const g = document.createElementNS('http://www.w3.org/2000/svg', 'g');
      g.classList.add('node', node.type);
      g.dataset.nodeIndex = i;

      const circle = document.createElementNS('http://www.w3.org/2000/svg', 'circle');
      circle.setAttribute('r', 8);
      g.appendChild(circle);

      const label = document.createElementNS('http://www.w3.org/2000/svg', 'text');
      label.classList.add('node-label');
      label.textContent = node.label;
      label.setAttribute('dy', 20);
      g.appendChild(label);

      g.addEventListener('click', () => this.selectNode(node, g));
      group.appendChild(g);
    });

    this.updatePositions();
  }

  selectNode(node, element) {
    // Deselect previous
    this.shadowRoot.querySelectorAll('.node.selected').forEach(el => {
      el.classList.remove('selected');
    });

    // Select new
    element.classList.add('selected');
    this.selectedNode = node;

    // Show details
    const panel = this.shadowRoot.getElementById('details-panel');
    const title = this.shadowRoot.getElementById('details-title');
    const content = this.shadowRoot.getElementById('details-content');

    panel.classList.remove('hidden');
    title.textContent = node.label;

    let html = `<div class="property">
      <div class="property-name">URI</div>
      <div class="property-value"><code>${node.id}</code></div>
    </div>`;

    Object.entries(node.properties).forEach(([key, value]) => {
      html += `<div class="property">
        <div class="property-name">${key}</div>
        <div class="property-value">${value}</div>
      </div>`;
    });

    content.innerHTML = html;

    // Highlight connected edges
    this.shadowRoot.querySelectorAll('.edge').forEach(edge => {
      edge.classList.remove('highlighted');
    });

    this.edges.forEach((edge, i) => {
      if (edge.source === node.id || edge.target === node.id) {
        const edgeEl = this.shadowRoot.querySelector(`[data-edge-index="${i}"]`);
        if (edgeEl) edgeEl.classList.add('highlighted');
      }
    });
  }

  startSimulation() {
    const simulate = () => {
      this.updateForces();
      this.updatePositions();
      this.animationFrame = requestAnimationFrame(simulate);
    };
    simulate();
  }

  updateForces() {
    const k = 0.1; // Spring constant
    const repulsion = 5000;
    const damping = 0.9;

    // Apply spring forces between connected nodes
    this.edges.forEach(edge => {
      const source = this.nodes.find(n => n.id === edge.source);
      const target = this.nodes.find(n => n.id === edge.target);
      if (!source || !target) return;

      const dx = target.x - source.x;
      const dy = target.y - source.y;
      const distance = Math.sqrt(dx * dx + dy * dy) || 1;
      const force = k * (distance - 100); // Ideal distance: 100

      const fx = (dx / distance) * force;
      const fy = (dy / distance) * force;

      source.vx += fx;
      source.vy += fy;
      target.vx -= fx;
      target.vy -= fy;
    });

    // Apply repulsion between all nodes
    for (let i = 0; i < this.nodes.length; i++) {
      for (let j = i + 1; j < this.nodes.length; j++) {
        const a = this.nodes[i];
        const b = this.nodes[j];

        const dx = b.x - a.x;
        const dy = b.y - a.y;
        const distanceSq = dx * dx + dy * dy || 1;
        const force = repulsion / distanceSq;

        const distance = Math.sqrt(distanceSq);
        const fx = (dx / distance) * force;
        const fy = (dy / distance) * force;

        a.vx -= fx;
        a.vy -= fy;
        b.vx += fx;
        b.vy += fy;
      }
    }

    // Update positions and apply damping
    this.nodes.forEach(node => {
      node.x += node.vx;
      node.y += node.vy;
      node.vx *= damping;
      node.vy *= damping;
    });
  }

  updatePositions() {
    const group = this.shadowRoot.getElementById('graph-group');

    // Update node positions
    this.nodes.forEach((node, i) => {
      const g = group.querySelector(`[data-node-index="${i}"]`);
      if (g) {
        g.setAttribute('transform', `translate(${node.x}, ${node.y})`);
      }
    });

    // Update edge positions
    this.edges.forEach((edge, i) => {
      const source = this.nodes.find(n => n.id === edge.source);
      const target = this.nodes.find(n => n.id === edge.target);
      if (!source || !target) return;

      const line = group.querySelector(`line[data-edge-index="${i}"]`);
      const label = group.querySelectorAll('.edge-label')[i];

      if (line) {
        line.setAttribute('x1', source.x);
        line.setAttribute('y1', source.y);
        line.setAttribute('x2', target.x);
        line.setAttribute('y2', target.y);
      }

      if (label) {
        label.setAttribute('x', (source.x + target.x) / 2);
        label.setAttribute('y', (source.y + target.y) / 2);
      }
    });
  }

  handleMouseDown(e) {
    if (e.target.closest('.node')) return; // Don't pan when clicking nodes
    this.isDragging = true;
    this.dragStart = { x: e.clientX - this.pan.x, y: e.clientY - this.pan.y };
    this.shadowRoot.getElementById('canvas').classList.add('dragging');
  }

  handleMouseMove(e) {
    if (!this.isDragging) return;
    this.pan.x = e.clientX - this.dragStart.x;
    this.pan.y = e.clientY - this.dragStart.y;
    this.updateTransform();
  }

  handleMouseUp(e) {
    this.isDragging = false;
    this.shadowRoot.getElementById('canvas').classList.remove('dragging');
  }

  handleWheel(e) {
    e.preventDefault();
    const delta = e.deltaY > 0 ? 0.9 : 1.1;
    this.zoom *= delta;
    this.zoom = Math.max(0.1, Math.min(5, this.zoom));
    this.updateTransform();
  }

  updateTransform() {
    const group = this.shadowRoot.getElementById('graph-group');
    group.setAttribute('transform', `translate(${this.pan.x}, ${this.pan.y}) scale(${this.zoom})`);
  }
}

customElements.define('geoff-graph-view', GeoffGraphView);

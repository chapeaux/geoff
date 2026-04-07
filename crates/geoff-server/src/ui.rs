//! Embedded authoring UI shell served at `/__geoff__/`.

/// The authoring SPA shell HTML.
pub const AUTHORING_UI_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Geoff — Authoring UI</title>
    <style>
        :root {
            --bg: #1a1a2e;
            --surface: #16213e;
            --surface2: #0f3460;
            --accent: #e94560;
            --text: #eee;
            --text-muted: #aaa;
            --border: #333;
            --mono: 'Menlo', 'Monaco', 'Consolas', monospace;
            --sans: system-ui, -apple-system, sans-serif;
        }
        * { box-sizing: border-box; margin: 0; padding: 0; }
        body {
            font-family: var(--sans);
            background: var(--bg);
            color: var(--text);
            display: flex;
            flex-direction: column;
            height: 100vh;
        }
        header {
            display: flex;
            align-items: center;
            gap: 1rem;
            padding: 0.5rem 1rem;
            background: var(--surface);
            border-bottom: 1px solid var(--border);
        }
        header h1 {
            font-size: 1.1rem;
            font-weight: 600;
            color: var(--accent);
        }
        nav {
            display: flex;
            gap: 0.25rem;
        }
        nav a {
            color: var(--text-muted);
            text-decoration: none;
            padding: 0.4rem 0.8rem;
            border-radius: 4px;
            font-size: 0.85rem;
            transition: background 0.15s, color 0.15s;
        }
        nav a:hover { background: var(--surface2); color: var(--text); }
        nav a.active { background: var(--surface2); color: var(--accent); font-weight: 600; }
        .status {
            margin-left: auto;
            font-size: 0.75rem;
            color: var(--text-muted);
        }
        .status .dot {
            display: inline-block;
            width: 8px; height: 8px;
            border-radius: 50%;
            background: #4caf50;
            margin-right: 4px;
            vertical-align: middle;
        }
        main {
            flex: 1;
            overflow: auto;
            padding: 1rem;
        }
        .panel { display: none; }
        .panel.active { display: block; }

        .panel { height: 100%; }
        .panel > * { height: 100%; }
    </style>
</head>
<body>
    <header>
        <h1>Geoff</h1>
        <nav>
            <a href="#editor" data-panel="editor">Editor</a>
            <a href="#graph" data-panel="graph">Graph</a>
            <a href="#vocabs" data-panel="vocabs">Vocabs</a>
            <a href="#validation" data-panel="validation">Validation</a>
        </nav>
        <div class="status"><span class="dot"></span>Connected</div>
    </header>
    <main>
        <!-- Editor Panel -->
        <div id="panel-editor" class="panel">
            <geoff-editor></geoff-editor>
        </div>

        <!-- Graph Panel -->
        <div id="panel-graph" class="panel">
            <geoff-graph-view></geoff-graph-view>
        </div>

        <!-- Vocabs Panel -->
        <div id="panel-vocabs" class="panel">
            <geoff-vocab-picker></geoff-vocab-picker>
        </div>

        <!-- Validation Panel -->
        <div id="panel-validation" class="panel">
            <geoff-shacl-panel></geoff-shacl-panel>
        </div>
    </main>

    <!-- Web Components -->
    <script src="/__geoff__/components/geoff-editor/geoff-editor.js"></script>
    <script src="/__geoff__/components/geoff-graph-view/geoff-graph-view.js"></script>
    <script src="/__geoff__/components/geoff-vocab-picker/geoff-vocab-picker.js"></script>
    <script src="/__geoff__/components/geoff-shacl-panel/geoff-shacl-panel.js"></script>

    <script>
    // ── Router ──────────────────────────────────────────────────────
    const panels = ['editor', 'graph', 'vocabs', 'validation'];
    function navigate(panel) {
        panels.forEach(p => {
            document.getElementById('panel-' + p).classList.toggle('active', p === panel);
        });
        document.querySelectorAll('nav a').forEach(a => {
            a.classList.toggle('active', a.dataset.panel === panel);
        });
    }

    document.querySelectorAll('nav a').forEach(a => {
        a.addEventListener('click', e => {
            e.preventDefault();
            location.hash = a.dataset.panel;
        });
    });

    window.addEventListener('hashchange', () => {
        const h = location.hash.slice(1) || 'editor';
        navigate(h);
    });

    // ── WebSocket (status indicator) ────────────────────────────────
    const statusDot = document.querySelector('.status .dot');
    function connectWs() {
        const ws = new WebSocket('ws://' + location.host + '/ws');
        ws.onopen = () => { statusDot.style.background = '#4caf50'; };
        ws.onclose = () => {
            statusDot.style.background = '#f44336';
            setTimeout(connectWs, 2000);
        };
        ws.onmessage = (e) => {
            if (e.data === 'reload') {
                const h = location.hash.slice(1) || 'editor';
                navigate(h);
            }
        };
    }
    connectWs();

    // ── Init ────────────────────────────────────────────────────────
    const hash = location.hash.slice(1) || 'editor';
    navigate(hash);
    </script>
</body>
</html>
"##;

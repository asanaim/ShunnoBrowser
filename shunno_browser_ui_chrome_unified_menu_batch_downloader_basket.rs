pub fn build_chrome_html() -> String {
    r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8" />
<meta name="viewport" content="width=device-width, initial-scale=1.0" />
<title>Shunno Browser</title>
<style>
  :root {
    --bg: #0f131a;
    --surface: #171d27;
    --surface-2: #1f2735;
    --border: rgba(255, 255, 255, 0.09);
    --text: #eef2f8;
    --muted: #8f9bb3;
    --accent: #3b82f6;
    --accent-2: #10b981;
    --danger: #ef4444;
    --shadow: 0 18px 42px rgba(0, 0, 0, 0.55);
  }
  body.light-theme {
    --bg: #f3f6fb;
    --surface: #ffffff;
    --surface-2: #eef2f8;
    --border: rgba(15, 23, 42, 0.12);
    --text: #0f172a;
    --muted: #5b677d;
    --accent: #2563eb;
    --accent-2: #059669;
    --danger: #dc2626;
    --shadow: 0 16px 36px rgba(15, 23, 42, 0.16);
  }
  * { box-sizing: border-box; margin: 0; padding: 0; font-family: "Segoe UI", system-ui, -apple-system, sans-serif; }
  html, body { width: 100%; height: 100%; background: var(--bg); color: var(--text); overflow: hidden; user-select: none; }

  /* Top Chrome Bar (88px total height) */
  #chrome-root { width: 100%; height: 88px; display: flex; flex-direction: column; background: var(--bg); border-bottom: 1px solid var(--border); }

  /* Tab Strip (38px) */
  .tab-strip {
    height: 38px; display: flex; align-items: center; gap: 6px; padding: 4px 10px 0 10px;
    background: var(--bg); overflow-x: auto; scrollbar-width: none;
  }
  .tab-strip::-webkit-scrollbar { display: none; }
  .brand-pill {
    display: inline-flex; align-items: center; gap: 6px; padding: 4px 10px; border-radius: 9px;
    background: linear-gradient(135deg, rgba(59,130,246,0.18), rgba(16,185,129,0.18));
    border: 1px solid var(--border); font-weight: 700; font-size: 12px; letter-spacing: 0.2px;
    color: var(--text); flex-shrink: 0;
  }
  .brand-dot {
    width: 12px; height: 12px; border-radius: 50%;
    border: 2.5px solid var(--accent); box-shadow: inset 0 0 0 2px var(--accent-2);
  }
  .tab {
    height: 30px; min-width: 120px; max-width: 210px; padding: 0 10px; border-radius: 9px 9px 0 0;
    background: transparent; border: 1px solid transparent; color: var(--muted);
    display: flex; align-items: center; justify-content: space-between; gap: 8px;
    font-size: 12px; cursor: pointer; transition: 0.14s ease; flex-shrink: 0;
  }
  .tab:hover { background: var(--surface); color: var(--text); }
  .tab.active {
    background: var(--surface); color: var(--text); border-color: var(--border);
    border-bottom-color: var(--surface); font-weight: 600;
  }
  .tab-title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1; }
  .tab-close {
    width: 18px; height: 18px; border-radius: 50%; border: none; background: transparent;
    color: var(--muted); font-size: 11px; display: inline-flex; align-items: center; justify-content: center; cursor: pointer;
  }
  .tab-close:hover { background: rgba(239,68,68,0.2); color: var(--danger); }
  .new-tab-btn {
    width: 26px; height: 26px; border-radius: 8px; border: 1px solid var(--border);
    background: var(--surface); color: var(--text); font-size: 15px; cursor: pointer;
    display: inline-flex; align-items: center; justify-content: center; flex-shrink: 0;
  }
  .new-tab-btn:hover { background: var(--surface-2); }

  /* Minimal Navigation Toolbar (50px) */
  .nav-bar {
    height: 50px; display: flex; align-items: center; gap: 8px; padding: 0 12px;
    background: var(--surface);
  }
  .icon-btn {
    height: 32px; min-width: 32px; padding: 0 9px; border-radius: 9px;
    border: 1px solid var(--border); background: var(--surface-2); color: var(--text);
    font-size: 13px; font-weight: 600; cursor: pointer; display: inline-flex; align-items: center; justify-content: center; gap: 5px;
    transition: 0.14s ease;
  }
  .icon-btn:hover { border-color: var(--accent); }
  .icon-btn.accent { background: linear-gradient(135deg, var(--accent), #2563eb); color: #fff; border: none; }

  .omnibox {
    flex: 1; height: 34px; display: flex; align-items: center; gap: 6px; padding: 0 10px;
    border-radius: 11px; background: var(--bg); border: 1px solid var(--border);
  }
  .omnibox:focus-within { border-color: var(--accent); box-shadow: 0 0 0 2px rgba(59,130,246,0.22); }
  .engine-select {
    background: var(--surface-2); color: var(--text); border: 1px solid var(--border);
    border-radius: 7px; padding: 3px 6px; font-size: 11.5px; font-weight: 600; outline: none; cursor: pointer;
  }
  #url-input {
    flex: 1; background: transparent; border: none; outline: none; color: var(--text);
    font-size: 13px; user-select: text;
  }
  .badge-count {
    background: var(--accent-2); color: #fff; font-size: 10.5px; font-weight: 700;
    padding: 1px 6px; border-radius: 999px;
  }

  /* Overlay Drawers (Expanded when opened) */
  .drawer {
    position: fixed; top: 88px; right: 12px; width: 410px; max-height: calc(100vh - 102px);
    background: var(--surface); border: 1px solid var(--border); border-radius: 14px;
    box-shadow: var(--shadow); display: none; flex-direction: column; overflow: hidden; z-index: 9999;
  }
  .drawer.open { display: flex; }
  .drawer-head {
    padding: 12px 14px; border-bottom: 1px solid var(--border);
    display: flex; align-items: center; justify-content: space-between;
  }
  .drawer-head h3 { font-size: 14px; font-weight: 700; }
  .drawer-body { padding: 12px 14px; overflow-y: auto; display: flex; flex-direction: column; gap: 12px; }

  .section-card {
    background: var(--bg); border: 1px solid var(--border); border-radius: 11px; padding: 10px 12px;
    display: flex; flex-direction: column; gap: 8px;
  }
  .section-title { font-size: 11px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.6px; color: var(--muted); }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 8px; font-size: 12.5px; }
  .grid-2 { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; }
  .chip-btn {
    padding: 7px 10px; border-radius: 8px; border: 1px solid var(--border); background: var(--surface);
    color: var(--text); font-size: 12px; font-weight: 600; cursor: pointer; text-align: left;
    display: flex; align-items: center; justify-content: space-between;
  }
  .chip-btn.on { border-color: var(--accent-2); background: rgba(16,185,129,0.14); }
  .chip-btn:hover { border-color: var(--accent); }

  input[type="text"], textarea, select.full {
    width: 100%; background: var(--surface); color: var(--text); border: 1px solid var(--border);
    border-radius: 8px; padding: 7px 9px; font-size: 12px; outline: none; user-select: text;
  }
  textarea { min-height: 68px; resize: vertical; font-family: Consolas, monospace; }

  .dl-item {
    background: var(--bg); border: 1px solid var(--border); border-radius: 10px; padding: 9px 11px;
    display: flex; flex-direction: column; gap: 6px;
  }
  .dl-top { display: flex; justify-content: space-between; align-items: center; gap: 6px; }
  .dl-name { font-size: 12.5px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 250px; }
  .dl-meta { font-size: 11px; color: var(--muted); display: flex; justify-content: space-between; }
  .progress-bar { width: 100%; height: 6px; border-radius: 999px; background: var(--surface-2); overflow: hidden; }
  .progress-fill { height: 100%; background: linear-gradient(90deg, var(--accent), var(--accent-2)); transition: width 0.25s ease; }

  .list-row {
    display: flex; align-items: center; justify-content: space-between; gap: 8px;
    padding: 6px 8px; border-radius: 8px; background: var(--surface); border: 1px solid var(--border); font-size: 12px;
  }
  .list-link { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; cursor: pointer; flex: 1; }
  .list-link:hover { color: var(--accent); text-decoration: underline; }
</style>
</head>
<body>
<div id="chrome-root">
  <div class="tab-strip" id="tab-strip">
    <div class="brand-pill"><span class="brand-dot"></span><span>Shunno Browser</span></div>
    <div id="tabs-container" style="display:flex;align-items:center;gap:5px;"></div>
    <button class="new-tab-btn" title="New Tab (Ctrl+T)" onclick="sendCmd('NEW_TAB')">+</button>
  </div>

  <div class="nav-bar">
    <button class="icon-btn" title="Back (Alt+Left)" onclick="sendCmd('GO_BACK')">‹</button>
    <button class="icon-btn" title="Forward (Alt+Right)" onclick="sendCmd('GO_FORWARD')">›</button>
    <button class="icon-btn" title="Reload (Ctrl+R)" onclick="sendCmd('RELOAD')">↻</button>

    <div class="omnibox">
      <select id="engine-select" class="engine-select" title="Search Engine" onchange="changeEngine(this.value)"></select>
      <input id="url-input" type="text" placeholder="Search or enter website address (Ctrl+L)..." onkeydown="handleUrlKey(event)" />
      <button class="icon-btn" style="height:24px;padding:0 7px;font-size:11px;" title="Bookmark Current Page (Ctrl+D)" onclick="sendCmd('ADD_BOOKMARK')">★</button>
    </div>

    <button class="icon-btn accent" title="Downloads & Media Sniffer (Ctrl+J)" onclick="toggleDrawer('downloads-drawer')">
      ⚡ Downloads <span id="dl-badge" class="badge-count" style="display:none;">0</span>
    </button>
    <button class="icon-btn" title="Shunno Browser Menu" onclick="toggleDrawer('menu-drawer')">☰ Menu</button>
  </div>
</div>

<!-- Downloads & Sniffer Drawer -->
<div id="downloads-drawer" class="drawer">
  <div class="drawer-head">
    <h3>⚡ Shunno Multi-Threaded Downloader</h3>
    <div style="display:flex;gap:6px;">
      <button class="icon-btn" style="height:26px;font-size:11px;" onclick="sendCmd('OPEN_DOWNLOAD_FOLDER')">📁 Folder</button>
      <button class="icon-btn" style="height:26px;font-size:11px;" onclick="sendCmd('CLEAR_COMPLETED_DOWNLOADS')">Clear Done</button>
      <button class="icon-btn" style="height:26px;" onclick="closeAllDrawers()">✕</button>
    </div>
  </div>
  <div class="drawer-body">
    <div class="section-card">
      <div class="section-title">Detected Page Media & DASH Streams</div>
      <div id="detected-media-list" style="display:flex;flex-direction:column;gap:6px;font-size:12px;color:var(--muted);">
        No active media streams detected on this tab yet.
      </div>
    </div>

    <div class="section-card">
      <div class="section-title">Direct / Multi-Line Batch URL Download</div>
      <textarea id="batch-urls-input" placeholder="Paste one or multiple URLs (one per line)..."></textarea>
      <div class="row">
        <span style="font-size:11px;color:var(--muted);">Forwards active tab cookies & Referer</span>
        <button class="icon-btn accent" style="height:28px;font-size:12px;" onclick="submitBatchDownload()">⬇ Start Batch</button>
      </div>
    </div>

    <div class="section-card">
      <div class="section-title">Active & Completed Queue</div>
      <div id="downloads-list" style="display:flex;flex-direction:column;gap:7px;"></div>
    </div>
  </div>
</div>

<!-- Unified Shunno Browser Menu Drawer -->
<div id="menu-drawer" class="drawer">
  <div class="drawer-head">
    <h3>Shunno Browser <span style="font-size:11px;color:var(--accent-2);font-weight:700;">v1.0.1</span></h3>
    <button class="icon-btn" style="height:26px;" onclick="closeAllDrawers()">✕</button>
  </div>
  <div class="drawer-body">
    <div class="section-card">
      <div class="section-title">Appearance & Workspace</div>
      <div class="grid-2">
        <button id="btn-full-dark" class="chip-btn" onclick="sendCmd('TOGGLE_FULL_DARK_MODE')"><span>🌙 Full Dark Mode</span><span id="lbl-full-dark">ON</span></button>
        <button id="btn-basket" class="chip-btn" onclick="sendCmd('TOGGLE_FLOATING_BASKET')"><span>🧺 Drop Basket</span><span id="lbl-basket">OFF</span></button>
        <button class="chip-btn" onclick="sendCmd('TOGGLE_READER_MODE')"><span>📖 Reader View</span><span>Alt+R</span></button>
        <button class="chip-btn" onclick="sendCmd('SAVE_OFFLINE_HTML')"><span>💾 Save Page HTML</span><span>Live</span></button>
      </div>
      <div class="row" style="margin-top:4px;">
        <span>Page Zoom (<span id="zoom-label">100%</span>)</span>
        <div style="display:flex;gap:5px;">
          <button class="icon-btn" style="height:26px;" onclick="sendCmd('ZOOM_OUT')">−</button>
          <button class="icon-btn" style="height:26px;" onclick="sendCmd('ZOOM_RESET')">100%</button>
          <button class="icon-btn" style="height:26px;" onclick="sendCmd('ZOOM_IN')">+</button>
        </div>
      </div>
    </div>

    <div class="section-card">
      <div class="section-title">Privacy Shields & Site Controls</div>
      <div class="grid-2">
        <button id="perm-adblock" class="chip-btn" onclick="toggleSitePerm('ad_block')"><span>🛡️ Ad & Tracker Shield</span><span id="val-adblock">ON</span></button>
        <button id="perm-ytads" class="chip-btn" onclick="sendCmd('TOGGLE_YT_AD_STRIPPER')"><span>🎬 YT Ad Stripper</span><span id="val-ytads">ON</span></button>
        <button id="perm-js" class="chip-btn" onclick="toggleSitePerm('javascript')"><span>⚡ JavaScript</span><span id="val-js">ON</span></button>
        <button id="perm-cookies" class="chip-btn" onclick="toggleSitePerm('cookies')"><span>🍪 Cookies</span><span id="val-cookies">ON</span></button>
        <button id="perm-images" class="chip-btn" onclick="toggleSitePerm('images')"><span>🖼️ Images</span><span id="val-images">ON</span></button>
        <button id="perm-ua" class="chip-btn" onclick="toggleSitePerm('desktop_ua')"><span>🖥️ Desktop UA</span><span id="val-ua">ON</span></button>
      </div>
      <div class="row">
        <button class="chip-btn" style="flex:1;" onclick="sendCmd('NEW_INCOGNITO_TAB')"><span>🕵️ New Private Tab</span><span>Ctrl+Shift+N</span></button>
        <button class="chip-btn" style="flex:1;" onclick="sendCmd('NUKE_SITE_DATA')"><span>🧹 Clear Site Data</span><span>Reset</span></button>
      </div>
    </div>

    <div class="section-card">
      <div class="section-title">Search Engines & Custom Provider</div>
      <div id="engines-list" style="display:flex;flex-direction:column;gap:5px;"></div>
      <div class="grid-2" style="margin-top:4px;">
        <input id="custom-engine-name" type="text" placeholder="Engine Name (e.g. Brave)" />
        <input id="custom-engine-url" type="text" placeholder="https://example.com/?q=%s" />
      </div>
      <button class="icon-btn" style="width:100%;height:28px;" onclick="addCustomEngine()">+ Add Custom Search Engine</button>
    </div>

    <div class="section-card">
      <div class="section-title">Download Engine Settings</div>
      <div class="row">
        <span>Parallel Segments: <strong id="seg-label">8</strong></span>
        <input type="range" min="1" max="16" value="8" id="seg-slider" onchange="sendCmd('SET_MAX_SEGMENTS:' + this.value)" />
      </div>
      <div class="row">
        <span id="dl-dir-label" style="font-size:11px;color:var(--muted);overflow:hidden;text-overflow:ellipsis;white-space:nowrap;max-width:250px;"></span>
        <button class="icon-btn" style="height:26px;font-size:11px;" onclick="sendCmd('CHOOSE_DOWNLOAD_DIR')">Change Folder</button>
      </div>
    </div>

    <div class="section-card">
      <div class="section-title">Bookmarks & Recent History</div>
      <div id="bookmarks-list" style="display:flex;flex-direction:column;gap:5px;max-height:110px;overflow-y:auto;"></div>
      <div class="row" style="margin-top:4px;">
        <span class="section-title">History</span>
        <button class="icon-btn" style="height:22px;font-size:10.5px;" onclick="sendCmd('CLEAR_HISTORY')">Clear History</button>
      </div>
      <div id="history-list" style="display:flex;flex-direction:column;gap:5px;max-height:110px;overflow-y:auto;"></div>
    </div>
  </div>
</div>

<script>
  let currentState = null;

  function sendCmd(cmd) {
    if (window.ipc) {
      window.ipc.postMessage(cmd);
    }
  }

  function toggleDrawer(id) {
    const el = document.getElementById(id);
    const wasOpen = el.classList.contains('open');
    closeAllDrawers();
    if (!wasOpen) {
      el.classList.add('open');
      sendCmd('DRAWER_OPENED');
    }
  }

  function closeAllDrawers() {
    document.querySelectorAll('.drawer').forEach(d => d.classList.remove('open'));
    sendCmd('DRAWER_CLOSED');
  }

  function handleUrlKey(e) {
    if (e.key === 'Enter') {
      const val = e.target.value.trim();
      if (val) sendCmd('NAVIGATE:' + val);
      e.target.blur();
    }
  }

  function changeEngine(id) {
    sendCmd('SET_SEARCH_ENGINE:' + id);
  }

  function toggleSitePerm(key) {
    sendCmd('TOGGLE_SITE_PERM:' + key);
  }

  function addCustomEngine() {
    const name = document.getElementById('custom-engine-name').value.trim();
    const url = document.getElementById('custom-engine-url').value.trim();
    if (!name || !url.includes('%s')) {
      alert('Please enter a name and a search URL containing %s');
      return;
    }
    sendCmd('ADD_SEARCH_ENGINE:' + JSON.stringify({ name, url }));
    document.getElementById('custom-engine-name').value = '';
    document.getElementById('custom-engine-url').value = '';
  }

  function submitBatchDownload() {
    const raw = document.getElementById('batch-urls-input').value.trim();
    if (!raw) return;
    sendCmd('BATCH_DOWNLOAD:' + raw);
    document.getElementById('batch-urls-input').value = '';
  }

  window.focusOmnibox = function() {
    const inp = document.getElementById('url-input');
    inp.focus();
    inp.select();
  };

  window.openDrawerById = function(id) {
    toggleDrawer(id);
  };

  window.renderChromeState = function(state) {
    currentState = state;

    // Theme
    if (state.full_dark_mode) {
      document.body.classList.remove('light-theme');
    } else {
      document.body.classList.add('light-theme');
    }

    // Tabs
    const tabsEl = document.getElementById('tabs-container');
    tabsEl.innerHTML = '';
    state.tabs.forEach(t => {
      const div = document.createElement('div');
      div.className = 'tab' + (t.id === state.active_tab_id ? ' active' : '');
      div.onclick = () => sendCmd('SWITCH_TAB:' + t.id);

      const title = document.createElement('span');
      title.className = 'tab-title';
      title.textContent = (t.is_incognito ? '🕵️ ' : '') + (t.title || 'New Tab');

      const closeBtn = document.createElement('button');
      closeBtn.className = 'tab-close';
      closeBtn.textContent = '✕';
      closeBtn.title = 'Close Tab (Ctrl+W)';
      closeBtn.onclick = (ev) => {
        ev.stopPropagation();
        sendCmd('CLOSE_TAB:' + t.id);
      };

      div.appendChild(title);
      div.appendChild(closeBtn);
      tabsEl.appendChild(div);
    });

    // Omnibox URL
    const urlInput = document.getElementById('url-input');
    if (document.activeElement !== urlInput) {
      urlInput.value = state.active_url || '';
    }

    // Search Engines dropdown + manager
    const sel = document.getElementById('engine-select');
    sel.innerHTML = '';
    const engList = document.getElementById('engines-list');
    engList.innerHTML = '';
    state.search_engines.forEach(eng => {
      const opt = document.createElement('option');
      opt.value = eng.id;
      opt.textContent = eng.name;
      if (eng.id === state.active_engine_id) opt.selected = true;
      sel.appendChild(opt);

      const row = document.createElement('div');
      row.className = 'list-row';
      row.innerHTML = `<span class="list-link" onclick="changeEngine('${eng.id}')">${eng.id === state.active_engine_id ? '✓ ' : ''}${eng.name}</span>`;
      if (state.search_engines.length > 1) {
        const del = document.createElement('button');
        del.className = 'tab-close';
        del.textContent = '✕';
        del.onclick = () => sendCmd('REMOVE_SEARCH_ENGINE:' + eng.id);
        row.appendChild(del);
      }
      engList.appendChild(row);
    });

    // Appearance & Shields toggles
    setChip('btn-full-dark', 'lbl-full-dark', state.full_dark_mode);
    setChip('btn-basket', 'lbl-basket', state.floating_basket_enabled);
    setChip('perm-adblock', 'val-adblock', state.permissions.ad_block);
    setChip('perm-ytads', 'val-ytads', state.strip_youtube_ads);
    setChip('perm-js', 'val-js', state.permissions.javascript);
    setChip('perm-cookies', 'val-cookies', state.permissions.cookies);
    setChip('perm-images', 'val-images', state.permissions.images);
    setChip('perm-ua', 'val-ua', state.permissions.desktop_ua);

    document.getElementById('zoom-label').textContent = Math.round((state.permissions.zoom_level || 1.0) * 100) + '%';
    document.getElementById('seg-label').textContent = state.max_segments;
    document.getElementById('seg-slider').value = state.max_segments;
    document.getElementById('dl-dir-label').textContent = state.download_dir;

    // Detected Media + DASH Muxer option
    const mediaEl = document.getElementById('detected-media-list');
    if (!state.detected_media || state.detected_media.length === 0) {
      mediaEl.innerHTML = 'No active media streams detected on this tab yet.';
    } else {
      mediaEl.innerHTML = '';
      if (state.dash_pair) {
        const muxRow = document.createElement('div');
        muxRow.className = 'list-row';
        muxRow.style.borderColor = 'var(--accent-2)';
        muxRow.innerHTML = `<span class="list-link">🎬 Split DASH HD Video + Audio Detected</span>`;
        const muxBtn = document.createElement('button');
        muxBtn.className = 'icon-btn accent';
        muxBtn.style.height = '25px';
        muxBtn.style.fontSize = '11px';
        muxBtn.textContent = 'Mux MP4';
        muxBtn.onclick = () => sendCmd('MUX_DASH_PAIR');
        muxRow.appendChild(muxBtn);
        mediaEl.appendChild(muxRow);
      }
      state.detected_media.slice(0, 8).forEach(m => {
        const row = document.createElement('div');
        row.className = 'list-row';
        const label = document.createElement('span');
        label.className = 'list-link';
        label.textContent = `[${m.kind}] ${m.label}`;
        label.title = m.url;
        const btn = document.createElement('button');
        btn.className = 'icon-btn';
        btn.style.height = '24px';
        btn.style.fontSize = '11px';
        btn.textContent = '⬇';
        btn.onclick = () => sendCmd('DOWNLOAD_URL:' + JSON.stringify({ url: m.url, title: m.label }));
        row.appendChild(label);
        row.appendChild(btn);
        mediaEl.appendChild(row);
      });
    }

    // Downloads List
    const dlList = document.getElementById('downloads-list');
    const activeCount = state.downloads.filter(d => d.status === 'Downloading' || d.status === 'Queued' || d.status === 'Muxing').length;
    const badge = document.getElementById('dl-badge');
    if (activeCount > 0) {
      badge.style.display = 'inline-block';
      badge.textContent = activeCount;
    } else {
      badge.style.display = 'none';
    }

    dlList.innerHTML = '';
    if (state.downloads.length === 0) {
      dlList.innerHTML = '<div style="font-size:12px;color:var(--muted);">No downloads yet.</div>';
    } else {
      state.downloads.forEach(d => {
        const pct = d.total_bytes > 0 ? Math.min(100, Math.round((d.downloaded_bytes / d.total_bytes) * 100)) : (d.status === 'Completed' ? 100 : 15);
        const statusText = typeof d.status === 'string' ? d.status : ('Failed: ' + (d.status.Failed || 'Error'));
        const item = document.createElement('div');
        item.className = 'dl-item';
        item.innerHTML = `
          <div class="dl-top">
            <span class="dl-name" title="${d.filename}">${d.filename}</span>
            <span style="font-size:11px;color:var(--accent-2);font-weight:700;">${statusText}</span>
          </div>
          <div class="progress-bar"><div class="progress-fill" style="width:${pct}%"></div></div>
          <div class="dl-meta">
            <span>${formatBytes(d.downloaded_bytes)} / ${formatBytes(d.total_bytes)} (${d.segments}x)</span>
            <span>${d.speed_bps > 0 ? formatBytes(d.speed_bps) + '/s' : d.category}</span>
          </div>
        `;
        const actions = document.createElement('div');
        actions.style.cssText = 'display:flex;gap:6px;justify-content:flex-end;';
        if (d.status === 'Downloading') {
          actions.appendChild(miniBtn('Pause', () => sendCmd('PAUSE_DOWNLOAD:' + d.id)));
        } else if (d.status === 'Paused' || typeof d.status === 'object') {
          actions.appendChild(miniBtn('Resume', () => sendCmd('RESUME_DOWNLOAD:' + d.id)));
        }
        actions.appendChild(miniBtn('Remove', () => sendCmd('REMOVE_DOWNLOAD:' + d.id)));
        item.appendChild(actions);
        dlList.appendChild(item);
      });
    }

    // Bookmarks & History
    const bmEl = document.getElementById('bookmarks-list');
    bmEl.innerHTML = state.bookmarks.length === 0 ? '<span style="font-size:11.5px;color:var(--muted);">No bookmarks saved.</span>' : '';
    state.bookmarks.slice(0, 12).forEach(b => {
      const row = document.createElement('div');
      row.className = 'list-row';
      row.innerHTML = `<span class="list-link" onclick="sendCmd('NAVIGATE:${b.url.replace(/'/g, "\\'")}')">★ ${b.title || b.url}</span>`;
      const del = document.createElement('button');
      del.className = 'tab-close';
      del.textContent = '✕';
      del.onclick = () => sendCmd('REMOVE_BOOKMARK:' + b.id);
      row.appendChild(del);
      bmEl.appendChild(row);
    });

    const histEl = document.getElementById('history-list');
    histEl.innerHTML = state.history.length === 0 ? '<span style="font-size:11.5px;color:var(--muted);">No history recorded.</span>' : '';
    state.history.slice(0, 12).forEach(h => {
      const row = document.createElement('div');
      row.className = 'list-row';
      row.innerHTML = `<span class="list-link" onclick="sendCmd('NAVIGATE:${h.url.replace(/'/g, "\\'")}')">${h.title || h.url}</span>`;
      histEl.appendChild(row);
    });
  };

  function setChip(btnId, lblId, isOn) {
    const b = document.getElementById(btnId);
    const l = document.getElementById(lblId);
    if (!b || !l) return;
    if (isOn) b.classList.add('on'); else b.classList.remove('on');
    l.textContent = isOn ? 'ON' : 'OFF';
  }

  function miniBtn(label, fn) {
    const b = document.createElement('button');
    b.className = 'icon-btn';
    b.style.cssText = 'height:22px;font-size:11px;padding:0 8px;';
    b.textContent = label;
    b.onclick = fn;
    return b;
  }

  function formatBytes(b) {
    if (!b || b <= 0) return '—';
    const units = ['B', 'KB', 'MB', 'GB'];
    let i = 0;
    let v = b;
    while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
    return v.toFixed(i === 0 ? 0 : 1) + ' ' + units[i];
  }
</script>
</body>
</html>"##
        .to_string()
}
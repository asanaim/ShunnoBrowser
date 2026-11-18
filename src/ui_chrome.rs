pub const CHROME_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<style>
  :root {
    --bg: #F2F2F7;
    --tab-bg: #E5E5EA;
    --surface: #FFFFFF;
    --border: #D8D8DF;
    --blue: #0A84FF;
    --blue-soft: #EBF4FF;
    --green: #30D158;
    --green-soft: #E8F8EE;
    --orange: #FF9F0A;
    --red: #FF3B30;
    --text: #1C1C1E;
    --muted: #6E6E73;
    --input-bg: #F2F2F7;
  }
  body.dark {
    --bg: #0B0B0D;
    --tab-bg: #141416;
    --surface: #1C1C1E;
    --border: #2C2C2E;
    --blue: #0A84FF;
    --blue-soft: #162942;
    --green: #30D158;
    --green-soft: #132E1C;
    --orange: #FF9F0A;
    --red: #FF453A;
    --text: #F5F5F7;
    --muted: #98989D;
    --input-bg: #2C2C2E;
  }
  * { box-sizing: border-box; margin: 0; padding: 0; font-family: system-ui, "Segoe UI Variable Display", "Segoe UI", sans-serif; user-select: none; }
  html, body { background: transparent; color: var(--text); overflow: hidden; display: flex; flex-direction: column; height: 100vh; }

  /* Top Chrome Strip */
  .chrome-top {
    background: var(--surface);
    box-shadow: 0 1px 4px rgba(0,0,0,0.08);
    z-index: 10;
  }

  /* Auto-Resizing Tab Strip */
  .tab-bar {
    display: flex; align-items: center; gap: 5px; padding: 5px 10px 4px 10px;
    background: var(--tab-bg); border-bottom: 1px solid var(--border); height: 38px; overflow: hidden;
  }
  .brand-pill {
    display: flex; align-items: center; gap: 6px; font-weight: 800; font-size: 12px; letter-spacing: -0.2px;
    background: linear-gradient(135deg, #0D47A1, #0A84FF); color: #fff;
    padding: 4px 10px; border-radius: 9px; flex-shrink: 0; box-shadow: 0 1px 3px rgba(10,132,255,0.3);
  }
  #tabs-container {
    display: flex; gap: 5px; align-items: center; flex: 1; min-width: 0; overflow: hidden;
  }
  .tab {
    display: flex; align-items: center; gap: 5px; padding: 4px 8px; border-radius: 9px;
    background: rgba(128,128,128,0.14); color: var(--text); font-size: 12px; cursor: pointer;
    flex: 1 1 0; min-width: 34px; max-width: 195px; overflow: hidden; transition: background 0.15s;
  }
  .tab.active { background: var(--surface); color: var(--text); font-weight: 600; box-shadow: 0 1px 4px rgba(0,0,0,0.15); }
  .tab.incognito.active { background: #2C2C2E; color: #FFFFFF; border: 1px solid #48484A; }
  .tab-title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1; min-width: 0; }
  .tab-close { font-size: 10.5px; color: var(--muted); padding: 1px 4px; border-radius: 4px; flex-shrink: 0; }
  .tab-close:hover { background: rgba(255,59,48,0.2); color: var(--red); }
  .tab-add {
    border: none; background: rgba(128,128,128,0.16); color: var(--blue); font-weight: 700;
    font-size: 12px; padding: 4px 9px; border-radius: 8px; cursor: pointer; flex-shrink: 0;
  }
  .tab-add:hover { background: var(--surface); }

  /* Minimalist Navigation Toolbar */
  .nav-bar {
    display: flex; align-items: center; gap: 7px; padding: 6px 12px;
    background: var(--surface); border-bottom: 1px solid var(--border); height: 48px;
  }
  .nav-btn {
    border: none; background: var(--input-bg); color: var(--text); font-size: 13.5px; font-weight: 600;
    padding: 6px 11px; border-radius: 10px; cursor: pointer; display: inline-flex; align-items: center; gap: 5px;
    white-space: nowrap; transition: all 0.12s;
  }
  .nav-btn:hover { filter: brightness(0.95); }
  .nav-btn.on { background: var(--blue-soft); color: var(--blue); }
  .nav-btn.green { background: var(--green-soft); color: #24B44B; }

  .search-picker {
    border: none; background: var(--input-bg); color: var(--text); font-size: 12px; font-weight: 600;
    padding: 6px 8px; border-radius: 10px; outline: none; cursor: pointer; max-width: 120px;
  }
  .url-wrap {
    flex: 1; display: flex; align-items: center; background: var(--input-bg);
    border-radius: 11px; padding: 0 12px; height: 35px; border: 1px solid transparent; min-width: 120px;
  }
  .url-wrap:focus-within { background: var(--surface); border-color: var(--blue); box-shadow: 0 0 0 3px rgba(10,132,255,0.18); }
  #url-input {
    border: none; background: transparent; outline: none; width: 100%;
    font-size: 13px; color: var(--text); user-select: text;
  }

  /* Compact Floating Popover Overlay (Keeps running webpage visible underneath!) */
  .popover-backdrop {
    flex: 1;
    display: none;
    justify-content: flex-end;
    align-items: flex-start;
    padding: 8px 12px 16px 12px;
    background: rgba(0, 0, 0, 0.18);
  }
  .popover-backdrop.open {
    display: flex;
  }
  .popover-sheet {
    width: 460px;
    max-width: calc(100vw - 24px);
    max-height: calc(100vh - 106px);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 16px;
    padding: 12px;
    overflow-y: auto;
    box-shadow: 0 18px 48px rgba(0, 0, 0, 0.28);
  }
  .popover-sheet.wide {
    width: 600px;
  }

  .card {
    background: var(--surface); border-radius: 13px; padding: 12px 14px; margin-bottom: 10px;
    border: 1px solid var(--border);
  }
  .card:last-child { margin-bottom: 0; }
  .card-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; gap: 8px; flex-wrap: wrap; }
  .card-title { font-size: 13.5px; font-weight: 700; }
  .grid-toggles { display: grid; grid-template-columns: repeat(2, 1fr); gap: 7px; }
  .toggle-tile {
    display: flex; justify-content: space-between; align-items: center; gap: 6px;
    padding: 8px 10px; border-radius: 10px; background: var(--input-bg); font-size: 12px; font-weight: 600;
  }
  .pill-btn {
    border: none; border-radius: 8px; padding: 4px 10px; font-size: 11.5px; font-weight: 600; cursor: pointer;
  }
  .pill-blue { background: var(--blue); color: #fff; }
  .pill-soft { background: var(--blue-soft); color: var(--blue); }
  .pill-green { background: var(--green-soft); color: #24B44B; }
  .pill-orange { background: rgba(255,159,10,0.16); color: var(--orange); }
  .pill-red { background: rgba(255,59,48,0.16); color: var(--red); }

  .input-row { display: flex; gap: 6px; margin-bottom: 8px; flex-wrap: wrap; }
  .text-box {
    flex: 1; min-width: 120px; padding: 7px 10px; border-radius: 9px; border: 1px solid var(--border);
    background: var(--input-bg); color: var(--text); font-size: 12.5px; outline: none; user-select: text;
  }
  .select-box {
    padding: 7px 9px; border-radius: 9px; border: 1px solid var(--border);
    font-size: 12px; background: var(--input-bg); color: var(--text);
  }

  .prog-track { width: 100%; height: 6px; background: var(--input-bg); border-radius: 4px; overflow: hidden; margin: 6px 0; }
  .prog-fill { height: 100%; background: var(--blue); transition: width 0.2s; }
  .prog-fill.done { background: var(--green); }
  .meta { font-size: 11px; color: var(--muted); }
  .list-row {
    display: flex; justify-content: space-between; align-items: center; padding: 7px 0;
    border-bottom: 1px solid var(--border); gap: 8px;
  }
  .list-row:last-child { border-bottom: none; }
</style>
</head>
<body>

  <div class="chrome-top">
    <!-- 1. Auto-Resizing Tab Strip -->
    <div class="tab-bar" id="tab-bar">
      <div class="brand-pill">
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none">
          <ellipse cx="12" cy="12" rx="6.8" ry="8.8" stroke="white" stroke-width="2.8"/>
          <line x1="7.5" y1="16.5" x2="16.5" y2="7.5" stroke="#30D158" stroke-width="2.6" stroke-linecap="round"/>
        </svg>
        <span>Shunno</span>
      </div>
      <div id="tabs-container"></div>
      <button class="tab-add" onclick="send('NEW_TAB')" title="New Tab (Ctrl+T)">+</button>
      <button class="tab-add" style="color:var(--text)" onclick="send('NEW_INCOGNITO')" title="New Private Tab">🕵</button>
    </div>

    <!-- 2. Minimalist Navigation Toolbar -->
    <div class="nav-bar">
      <button class="nav-btn" onclick="send('BACK')" title="Back">‹</button>
      <button class="nav-btn" onclick="send('FORWARD')" title="Forward">›</button>
      <button class="nav-btn" onclick="send('RELOAD')" title="Reload (Ctrl+R)">↻</button>

      <select class="search-picker" id="engine-select" onchange="send('SET_SEARCH_ENGINE:' + this.value)" title="Select Search Engine"></select>

      <div class="url-wrap">
        <input id="url-input" placeholder="Search or enter website URL..."
               onkeydown="if(event.key==='Enter') send('NAV:' + this.value)" />
      </div>

      <button class="nav-btn green" id="btn-streams" style="display:none" onclick="toggleDrawer('streams')" title="Detected Media Streams">🎯 0</button>
      <button class="nav-btn on" id="btn-downloads" onclick="toggleDrawer('downloads')" title="Shunno Get Downloads (Ctrl+J)">⚡ 0</button>
      <button class="nav-btn" id="btn-controls" onclick="toggleDrawer('controls')" title="Controls &amp; Settings">•••</button>
    </div>
  </div>

  <!-- 3. Compact Floating Popover Overlay (Clicking dimmed area closes popover) -->
  <div class="popover-backdrop" id="drawer" onclick="if(event.target===this) closeDrawer()">
    <div class="popover-sheet" id="popover-sheet">

      <!-- Panel A: Compact Controls & Settings Popover -->
      <div id="panel-controls" style="display:none">
        <div class="card">
          <div class="card-header">
            <span class="card-title">Shunno Browser</span>
            <div style="display:flex;gap:6px;align-items:center;">
              <span class="meta" id="version-badge" style="font-weight:700;">v1.0.0</span>
              <button class="pill-btn pill-soft" onclick="send('TOGGLE_FULLSCREEN')" title="Toggle Full Screen (F11)">⛶ F11</button>
              <button class="pill-btn pill-red" onclick="closeDrawer()">✕</button>
            </div>
          </div>

          <div style="display:flex;gap:6px;flex-wrap:wrap;margin-bottom:10px;">
            <button class="pill-btn pill-soft" onclick="toggleDrawer('find')">🔍 Find</button>
            <button class="pill-btn pill-soft" onclick="toggleDrawer('bookmarks')">⭐ Library</button>
            <button class="pill-btn pill-soft" onclick="send('READER_MODE'); closeDrawer();">📖 Reader</button>
            <button class="pill-btn pill-soft" id="menu-speed-btn" onclick="cycleSpeed()">⏩ 1.0×</button>
            <button class="pill-btn pill-green" onclick="send('SAVE_OFFLINE_HTML'); closeDrawer();">💾 Save Offline</button>
            <button class="pill-btn pill-soft" onclick="send('OPEN_SCRIPTS_FOLDER')">📜 Scripts</button>
          </div>

          <div class="grid-toggles" id="global-toggles-grid"></div>
        </div>

        <div class="card">
          <div class="card-header">
            <span class="card-title" id="site-domain-title">🌐 Site Controls</span>
          </div>
          <div class="grid-toggles" id="site-toggles-grid"></div>
        </div>

        <div class="card">
          <div class="card-header">
            <span class="card-title">🔎 Search Engines</span>
          </div>
          <div class="input-row">
            <input class="text-box" id="new-se-name" placeholder="Name (e.g. Qwant)" style="max-width:130px;" />
            <input class="text-box" id="new-se-url" placeholder="https://example.com/?q=%s" />
            <button class="pill-btn pill-blue" onclick="addSearchEngine()">Add</button>
          </div>
          <div id="search-engines-list"></div>
        </div>
      </div>

      <!-- Panel B: Find in Page -->
      <div id="panel-find" style="display:none">
        <div class="card">
          <div class="card-header">
            <span class="card-title">🔍 Find in Page</span>
            <button class="pill-btn pill-red" onclick="closeDrawer()">✕</button>
          </div>
          <div class="input-row" style="margin-bottom:0;">
            <input class="text-box" id="find-input" placeholder="Search text..."
                   oninput="send('FIND:' + this.value)" onkeydown="if(event.key==='Enter') send('FIND:' + this.value)" />
            <button class="pill-btn pill-soft" onclick="send('FIND_PREV')">↑</button>
            <button class="pill-btn pill-blue" onclick="send('FIND_NEXT')">↓</button>
          </div>
        </div>
      </div>

      <!-- Panel C: Detected Media Streams -->
      <div id="panel-streams" style="display:none">
        <div class="card">
          <div class="card-header">
            <span class="card-title">🎯 Detected Media Streams</span>
            <button class="pill-btn pill-red" onclick="closeDrawer()">✕</button>
          </div>
          <div id="streams-list"></div>
        </div>
      </div>

      <!-- Panel D: Bookmarks & History -->
      <div id="panel-bookmarks" style="display:none">
        <div class="card">
          <div class="card-header">
            <span class="card-title">⭐ Bookmarks</span>
            <div style="display:flex;gap:5px;">
              <button class="pill-btn pill-blue" onclick="send('ADD_BOOKMARK')">+ Save</button>
              <button class="pill-btn pill-soft" onclick="send('EXPORT_TXT_BOOKMARKS')">Export</button>
              <button class="pill-btn pill-soft" onclick="send('IMPORT_TXT_BOOKMARKS')">Import</button>
              <button class="pill-btn pill-red" onclick="closeDrawer()">✕</button>
            </div>
          </div>
          <div id="bookmarks-list"></div>
        </div>

        <div class="card">
          <div class="card-header">
            <span class="card-title">🕒 History</span>
            <button class="pill-btn pill-red" onclick="send('CLEAR_HISTORY')">Clear</button>
          </div>
          <div id="history-list"></div>
        </div>
      </div>

      <!-- Panel E: Shunno Get Downloads -->
      <div id="panel-downloads" style="display:none">
        <div class="card">
          <div class="card-header">
            <span class="card-title">⚡ Shunno Get</span>
            <div style="display:flex;gap:5px;align-items:center;flex-wrap:wrap;">
              <span class="meta" id="ffmpeg-status-badge" style="font-weight:700;">🎬 FFmpeg</span>
              <button class="pill-btn pill-soft" onclick="send('OPEN_BIN_FOLDER')" title="Place ffmpeg.exe here">bin/</button>
              <select class="select-box" id="speed-limit-select" onchange="send('SET_SPEED_LIMIT:' + this.value)">
                <option value="0">Unlimited</option>
                <option value="10485760">10 MB/s</option>
                <option value="5242880">5 MB/s</option>
                <option value="2097152">2 MB/s</option>
                <option value="524288">512 KB/s</option>
              </select>
              <button class="pill-btn pill-soft" onclick="send('OPEN_DOWNLOADS_FOLDER')">📂</button>
              <button class="pill-btn pill-red" onclick="closeDrawer()">✕</button>
            </div>
          </div>
          <div class="input-row">
            <input class="text-box" id="dl-url-input" placeholder="Paste link or track_[01-12].flac..." />
            <select class="select-box" id="dl-threads-select">
              <option value="1">1T</option>
              <option value="4">4T</option>
              <option value="8">8T</option>
              <option value="16" selected>16T</option>
              <option value="32">32T</option>
            </select>
            <button class="pill-btn pill-blue" onclick="startManualDownload()">⚡ Get</button>
          </div>
          <div class="input-row" style="margin-bottom:0;">
            <input class="text-box" id="mux-video-input" placeholder="DASH Video URL..." />
            <input class="text-box" id="mux-audio-input" placeholder="DASH Audio URL..." />
            <button class="pill-btn pill-green" onclick="startDashMux()">🎬 Mux</button>
          </div>
        </div>

        <div id="downloads-list"></div>
      </div>

    </div>
  </div>

<script>
  let activeDrawer = null;
  let currentSpeed = 1.0;
  const speeds = [1.0, 1.25, 1.5, 2.0, 3.0, 4.0, 0.75];

  function send(msg) {
    if (window.ipc) window.ipc.postMessage(msg);
  }

  function toggleDrawer(name) {
    if (activeDrawer === name) {
      closeDrawer();
      return;
    }
    activeDrawer = name;
    document.getElementById('drawer').classList.add('open');
    const sheet = document.getElementById('popover-sheet');
    if (name === 'downloads' || name === 'bookmarks') {
      sheet.classList.add('wide');
    } else {
      sheet.classList.remove('wide');
    }
    ['controls', 'find', 'streams', 'bookmarks', 'downloads'].forEach(p => {
      document.getElementById('panel-' + p).style.display = (p === name) ? 'block' : 'none';
    });
    send('DRAWER_OPEN:1');
  }

  function closeDrawer() {
    activeDrawer = null;
    document.getElementById('drawer').classList.remove('open');
    send('DRAWER_OPEN:0');
  }

  function cycleSpeed() {
    const idx = (speeds.indexOf(currentSpeed) + 1) % speeds.length;
    currentSpeed = speeds[idx];
    document.getElementById('menu-speed-btn').textContent = '⏩ ' + currentSpeed + '×';
    send('SET_PLAYBACK_SPEED:' + currentSpeed);
  }

  function startManualDownload() {
    const input = document.getElementById('dl-url-input');
    const threads = document.getElementById('dl-threads-select').value;
    if (!input.value.trim()) return;
    send('ENQUEUE_DL:' + threads + '|' + input.value.trim());
    input.value = '';
  }

  function startDashMux() {
    const v = document.getElementById('mux-video-input');
    const a = document.getElementById('mux-audio-input');
    if (!v.value.trim() || !a.value.trim()) return;
    send('MUX_DASH:' + v.value.trim() + '|' + a.value.trim());
    v.value = '';
    a.value = '';
  }

  function addSearchEngine() {
    const nameEl = document.getElementById('new-se-name');
    const urlEl = document.getElementById('new-se-url');
    const name = nameEl.value.trim();
    const url = urlEl.value.trim();
    if (!name || !url) return;
    send('ADD_SEARCH_ENGINE:' + name + '|' + url);
    nameEl.value = '';
    urlEl.value = '';
  }

  function fmtBytes(b) {
    if (!b || b <= 0) return '0 B';
    const k = b / 1024, m = k / 1024, g = m / 1024;
    if (g >= 1) return g.toFixed(2) + ' GB';
    if (m >= 1) return m.toFixed(1) + ' MB';
    if (k >= 1) return k.toFixed(0) + ' KB';
    return b + ' B';
  }

  window.renderChromeState = function(state) {
    if (state.global.full_dark_mode) {
      document.body.classList.add('dark');
    } else {
      document.body.classList.remove('dark');
    }

    document.getElementById('version-badge').textContent = state.version || 'v1.0.0';
    document.getElementById('ffmpeg-status-badge').textContent = state.ffmpeg_available
      ? '🎬 FFmpeg: Ready'
      : '🎬 FFmpeg: Optional';

    const tc = document.getElementById('tabs-container');
    tc.innerHTML = state.tabs.map((t, idx) => `
      <div class="tab ${idx === state.active_tab ? 'active' : ''} ${t.incognito ? 'incognito' : ''}"
           onclick="send('SWITCH_TAB:${idx}')" title="${escapeHtml(t.title || 'New Tab')}">
        <span class="tab-title">${t.incognito ? '🕵 ' : ''}${escapeHtml(t.title || 'New Tab')}</span>
        <span class="tab-close" onclick="event.stopPropagation(); send('CLOSE_TAB:${idx}')">✕</span>
      </div>
    `).join('');

    const seSelect = document.getElementById('engine-select');
    if (state.global.search_engines && state.global.search_engines.length > 0) {
      seSelect.innerHTML = state.global.search_engines.map((se, i) =>
        `<option value="${i}" ${i === state.global.active_search_engine ? 'selected' : ''}>${escapeHtml(se.name)}</option>`
      ).join('');
    }

    const urlBox = document.getElementById('url-input');
    if (document.activeElement !== urlBox) {
      urlBox.value = state.active_url || '';
    }

    const sBtn = document.getElementById('btn-streams');
    if (state.streams.length > 0) {
      sBtn.style.display = 'inline-flex';
      sBtn.textContent = `🎯 ${state.streams.length}`;
    } else {
      sBtn.style.display = 'none';
    }

    document.getElementById('btn-downloads').textContent = `⚡ ${state.downloads.length}`;
    document.getElementById('speed-limit-select').value = String(state.global.speed_limit_bps);

    document.getElementById('streams-list').innerHTML = state.streams.length === 0
      ? '<div class="meta">No media streams intercepted on this page yet.</div>'
      : state.streams.map(s => `
          <div class="list-row">
            <div style="overflow:hidden;">
              <div style="font-size:12px;font-weight:600;">${escapeHtml(s.name)} <span class="pill-btn pill-green" style="padding:1px 5px;font-size:10.5px;">${escapeHtml(s.size_label)}</span></div>
              <div class="meta" style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">${escapeHtml(s.url)}</div>
            </div>
            <button class="pill-btn pill-blue" onclick="send('ENQUEUE_DL:16|${encodeAttr(s.url)}'); toggleDrawer('downloads');">⚡</button>
          </div>
        `).join('');

    const globalItems = [
      ['full_dark', '🌙 Dark Mode', state.global.full_dark_mode],
      ['drop_basket', '🧺 Drop Basket', state.global.drop_basket_enabled],
      ['bg_audio', '🎧 Background Play', state.global.background_media_play],
      ['strip_tracking', '✂️ Strip Trackers', state.global.strip_tracking_params],
      ['block_popups', '🚫 Block Pop-Ups', state.global.block_popups]
    ];
    document.getElementById('global-toggles-grid').innerHTML = globalItems.map(([key, label, val]) => `
      <div class="toggle-tile">
        <span>${label}</span>
        <button class="pill-btn ${val ? 'pill-blue' : 'pill-soft'}" onclick="send('TOGGLE_GLOBAL:${key}')">${val ? 'ON' : 'OFF'}</button>
      </div>
    `).join('');

    document.getElementById('site-domain-title').textContent = `🌐 ${state.active_host || 'Site Controls'} (${state.blocked_count} blocked)`;
    const siteItems = [
      ['adblock', '🛡️ Ad Blocker', state.site.adblock],
      ['sponsorblock', '🎬 YT Ad Clean', state.site.sponsorblock],
      ['js_enabled', '⚙️ JavaScript', state.site.js_enabled],
      ['images_enabled', '🖼️ Images', state.site.images_enabled],
      ['desktop_ua', '💻 Desktop UA', state.site.desktop_ua],
      ['auto_reject_cookies', '🍪 Reject Cookies', state.site.auto_reject_cookies]
    ];
    document.getElementById('site-toggles-grid').innerHTML = siteItems.map(([key, label, val]) => `
      <div class="toggle-tile">
        <span>${label}</span>
        <button class="pill-btn ${val ? 'pill-blue' : 'pill-soft'}" onclick="send('TOGGLE_SITE:${key}')">${val ? 'ON' : 'OFF'}</button>
      </div>
    `).join('');

    document.getElementById('search-engines-list').innerHTML = (state.global.search_engines || []).map((se, i) => `
      <div class="list-row">
        <div style="overflow:hidden;">
          <span style="font-size:12px;font-weight:600;">${escapeHtml(se.name)}</span>
          <span class="meta" style="margin-left:6px;">${escapeHtml(se.query_url)}</span>
        </div>
        <div style="display:flex;gap:5px;flex-shrink:0;">
          <button class="pill-btn ${i === state.global.active_search_engine ? 'pill-blue' : 'pill-soft'}" onclick="send('SET_SEARCH_ENGINE:${i}')">
            ${i === state.global.active_search_engine ? 'Active' : 'Use'}
          </button>
          ${state.global.search_engines.length > 1 ? `<button class="pill-btn pill-red" onclick="send('DEL_SEARCH_ENGINE:${i}')">✕</button>` : ''}
        </div>
      </div>
    `).join('');

    document.getElementById('bookmarks-list').innerHTML = state.bookmarks.map((b, i) => `
      <div class="list-row">
        <div style="overflow:hidden;">
          <div style="font-size:12.5px;font-weight:600;">${escapeHtml(b.title)}</div>
          <div class="meta" style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">${escapeHtml(b.url)}</div>
        </div>
        <div style="display:flex;gap:5px;flex-shrink:0;">
          <button class="pill-btn pill-soft" onclick="send('NAV:${encodeAttr(b.url)}'); closeDrawer();">Open</button>
          <button class="pill-btn pill-red" onclick="send('DEL_BOOKMARK:${i}')">✕</button>
        </div>
      </div>
    `).join('');

    document.getElementById('history-list').innerHTML = state.history.length === 0
      ? '<div class="meta">No history recorded yet.</div>'
      : state.history.slice(0, 30).map(h => `
          <div class="list-row">
            <div style="overflow:hidden;">
              <div style="font-size:12px;font-weight:600;">${escapeHtml(h.title || h.url)}</div>
              <div class="meta" style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">${escapeHtml(h.url)}</div>
            </div>
            <button class="pill-btn pill-soft" onclick="send('NAV:${encodeAttr(h.url)}'); closeDrawer();">Visit</button>
          </div>
        `).join('');

    document.getElementById('downloads-list').innerHTML = state.downloads.length === 0
      ? '<div class="card"><div class="meta">No downloads yet. Hover over any video or file on a page to use the floating download pill.</div></div>'
      : state.downloads.map(d => {
          const pct = d.status === 'COMPLETED' ? 100 : (d.total_bytes > 0 ? Math.min(100, (d.downloaded_bytes * 100 / d.total_bytes)) : 0);
          const sizeTxt = d.total_bytes > 0 ? `${fmtBytes(d.downloaded_bytes)} / ${fmtBytes(d.total_bytes)} (${pct.toFixed(1)}%)` : fmtBytes(d.downloaded_bytes);
          const infoTxt = d.status === 'DOWNLOADING'
            ? `⚡ ${fmtBytes(d.speed_bps)}/s ${d.segment_map || ''}`
            : d.status === 'MUXING'
              ? '🎬 Muxing via FFmpeg...'
              : d.status === 'COMPLETED'
                ? `✓ Saved • SHA-256: ${(d.sha256 || '').slice(0, 14)}...`
                : d.status === 'FAILED'
                  ? `⚠ ${escapeHtml(d.error_message || 'Error')}`
                  : `Paused at ${fmtBytes(d.downloaded_bytes)}`;

          return `
            <div class="card">
              <div class="card-header" style="margin-bottom:4px;">
                <div style="overflow:hidden;">
                  <div style="font-size:13px;font-weight:700;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">${escapeHtml(d.file_name)}</div>
                  <div class="meta" style="color:#0A84FF;">${escapeHtml(d.category)} • ${d.supports_ranges ? d.threads + 'T' : '1T'} • ${d.status}</div>
                </div>
                <div class="meta" style="font-weight:600;flex-shrink:0;">${sizeTxt}</div>
              </div>
              <div class="prog-track"><div class="prog-fill ${d.status === 'COMPLETED' ? 'done' : ''}" style="width:${pct}%"></div></div>
              <div style="display:flex;justify-content:space-between;align-items:center;margin-top:5px;">
                <div class="meta">${infoTxt}</div>
                <div style="display:flex;gap:5px;">
                  ${d.status === 'DOWNLOADING'
                    ? `<button class="pill-btn pill-orange" onclick="send('DL_PAUSE:${d.id}')">Pause</button>`
                    : d.status !== 'COMPLETED' && d.status !== 'MUXING'
                      ? `<button class="pill-btn pill-soft" onclick="send('DL_RESUME:${d.id}')">Resume</button>
                         <button class="pill-btn pill-soft" onclick="const u=prompt('Paste fresh mirror URL:','${encodeAttr(d.url)}'); if(u) send('DL_FIX:${d.id}|'+u);">Fix</button>`
                      : d.status === 'COMPLETED'
                        ? `<button class="pill-btn pill-green" onclick="send('DL_SHOW:${encodeAttr(d.saved_path || '')}')">Show</button>`
                        : ''
                  }
                  <button class="pill-btn pill-red" onclick="send('DL_DEL:${d.id}')">✕</button>
                </div>
              </div>
            </div>
          `;
        }).join('');
  };

  function escapeHtml(str) {
    return String(str).replace(/[&<>"']/g, m => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[m]));
  }
  function encodeAttr(str) {
    return String(str).replace(/'/g, "\\'");
  }

  document.addEventListener('keydown', (e) => {
    if (e.key === 'F11') {
      e.preventDefault();
      send('TOGGLE_FULLSCREEN');
    } else if (e.ctrlKey && !e.shiftKey && !e.altKey) {
      const k = e.key.toLowerCase();
      if (k === 't') { e.preventDefault(); send('NEW_TAB'); }
      else if (k === 'w') { e.preventDefault(); send('CLOSE_ACTIVE_TAB'); }
      else if (k === 'l') { e.preventDefault(); const u = document.getElementById('url-input'); u.focus(); u.select(); }
      else if (k === 'f') { e.preventDefault(); toggleDrawer('find'); setTimeout(() => document.getElementById('find-input').focus(), 50); }
      else if (k === 'j') { e.preventDefault(); toggleDrawer('downloads'); }
      else if (k === 'r') { e.preventDefault(); send('RELOAD'); }
    } else if (e.key === 'Escape' && activeDrawer) {
      closeDrawer();
    }
  });
</script>
</body>
</html>
"##;

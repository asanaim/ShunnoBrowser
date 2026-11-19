(function() {
  const IPC_TOKEN = "__SHUNNO_IPC_TOKEN__";
  const ADBLOCK_ON = __SHUNNO_ADBLOCK__;
  const SPONSORBLOCK_ON = __SHUNNO_SPONSORBLOCK__;
  const REJECT_COOKIES_ON = __SHUNNO_REJECT_COOKIES__;
  const BASKET_ON = __SHUNNO_BASKET__;
  const BG_MEDIA_ON = __SHUNNO_BG_MEDIA__;
  const FULL_DARK_ON = __SHUNNO_FULL_DARK__;
  const JS_DISABLED = __SHUNNO_JS_DISABLED__;
  const IMAGES_DISABLED = __SHUNNO_IMAGES_DISABLED__;
  const DESKTOP_UA_ON = __SHUNNO_DESKTOP_UA__;

  function postSecure(msg) {
    if (window.ipc) {
      window.ipc.postMessage(IPC_TOKEN + "|" + msg);
    }
  }

  function formatBytesJS(bytes) {
    const b = Number(bytes);
    if (!b || isNaN(b) || b <= 0) return null;
    const kb = 1024, mb = kb * 1024, gb = mb * 1024;
    if (b >= gb) return (b / gb).toFixed(2) + ' GB';
    if (b >= mb) return (b / mb).toFixed(1) + ' MB';
    if (b >= kb) return (b / kb).toFixed(0) + ' KB';
    return b + ' B';
  }

  // 1. Background Media Playback (Page Visibility API lock)
  if (BG_MEDIA_ON) {
    try {
      Object.defineProperty(document, 'hidden', { get: () => false, configurable: true });
      Object.defineProperty(document, 'visibilityState', { get: () => 'visible', configurable: true });
      window.addEventListener('visibilitychange', e => e.stopImmediatePropagation(), true);
      document.addEventListener('visibilitychange', e => e.stopImmediatePropagation(), true);
    } catch (e) {}
  }

  // 2. Desktop User-Agent override
  if (DESKTOP_UA_ON) {
    try {
      Object.defineProperty(navigator, 'userAgent', {
        get: () => 'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:132.0) Gecko/20100101 Firefox/132.0'
      });
    } catch (e) {}
  }

  // 3. Full Dark Mode (Luminance inversion exempting media)
  if (FULL_DARK_ON) {
    const injectDark = () => {
      if (document.getElementById('shunno-full-dark')) return;
      const st = document.createElement('style');
      st.id = 'shunno-full-dark';
      st.textContent = 'html { filter: invert(0.92) hue-rotate(180deg) !important; background: #111 !important; } img, video, picture, canvas, svg, [style*="background-image"] { filter: invert(1) hue-rotate(180deg) !important; }';
      (document.head || document.documentElement).appendChild(st);
    };
    if (document.documentElement) injectDark();
    else window.addEventListener('DOMContentLoaded', injectDark, { once: true });
  }

  // 4. Image Blocker
  if (IMAGES_DISABLED) {
    const st = document.createElement('style');
    st.textContent = 'img, picture, svg, video[poster], [style*="background-image"] { display: none !important; background-image: none !important; }';
    (document.head || document.documentElement).appendChild(st);
  }

  // 5. YouTube Pre-Roll Ad Stripper + Direct Stream & Quality Harvester
  const isYouTube = /(^|\.)youtube\.com$|(^|\.)youtu\.be$/i.test(location.hostname);
  window.__shunnoYtStreams = [];
  const probedSizes = new Map();

  function harvestYtStreams(obj) {
    try {
      const sd = obj && obj.streamingData;
      if (!sd) return;
      const all = [].concat(sd.formats || [], sd.adaptiveFormats || []);
      const rawTitle = (obj.videoDetails && obj.videoDetails.title) || document.title.replace(' - YouTube', '') || 'YouTube_Video';
      const title = rawTitle.trim();
      const found = [];
      for (const f of all) {
        if (f && f.url && String(f.url).startsWith('http')) {
          const mime = String(f.mimeType || '');
          const isAudio = mime.startsWith('audio/');
          const ext = isAudio ? '.M4A' : (mime.includes('webm') ? '.WEBM' : '.MP4');
          const qual = f.qualityLabel || (isAudio ? (Math.round((f.bitrate || 128000) / 1000) + 'kbps Audio') : 'Video');
          if (f.contentLength) {
            const szLabel = formatBytesJS(f.contentLength);
            if (szLabel) probedSizes.set(f.url, szLabel);
          }
          found.push({
            url: f.url,
            ext: ext,
            name: qual + ' - ' + title + ext.toLowerCase()
          });
        }
      }
      if (found.length > 0) {
        window.__shunnoYtStreams = found;
        if (window.__shunnoSyncYtStreams) window.__shunnoSyncYtStreams(found);
      }
    } catch (e) {}
  }

  function stripAdFields(obj) {
    if (!obj || typeof obj !== 'object') return obj;
    try {
      if ('streamingData' in obj) harvestYtStreams(obj);
      if ('adPlacements' in obj) { obj.adPlacements = []; delete obj.adPlacements; }
      if ('playerAds' in obj) { obj.playerAds = []; delete obj.playerAds; }
      if ('adSlots' in obj) { obj.adSlots = []; delete obj.adSlots; }
      if ('adBreakHeartbeatParams' in obj) delete obj.adBreakHeartbeatParams;
      if (obj.auxiliaryUi && obj.auxiliaryUi.messageRenderers) delete obj.auxiliaryUi.messageRenderers;
      if (obj.playerConfig && obj.playerConfig.ssapConfig) delete obj.playerConfig.ssapConfig;
    } catch (e) {}
    return obj;
  }

  if (isYouTube && (ADBLOCK_ON || SPONSORBLOCK_ON)) {
    let _ytPlayerResp = undefined;
    try {
      Object.defineProperty(window, 'ytInitialPlayerResponse', {
        configurable: true,
        get() { return _ytPlayerResp; },
        set(val) { _ytPlayerResp = stripAdFields(val); }
      });
    } catch (e) {}

    const origParse = JSON.parse;
    JSON.parse = function(text, reviver) {
      const res = origParse.apply(this, arguments);
      if (res && typeof res === 'object' && ('adPlacements' in res || 'playerAds' in res || 'adSlots' in res || 'streamingData' in res)) {
        stripAdFields(res);
      }
      return res;
    };

    const origRespJson = Response.prototype.json;
    Response.prototype.json = function() {
      return origRespJson.apply(this, arguments).then(data => stripAdFields(data));
    };

    const origXhrText = Object.getOwnPropertyDescriptor(XMLHttpRequest.prototype, 'responseText');
    if (origXhrText && origXhrText.get) {
      Object.defineProperty(XMLHttpRequest.prototype, 'responseText', {
        configurable: true,
        get() {
          const txt = origXhrText.get.call(this);
          if (typeof txt === 'string' && (txt.includes('"adPlacements"') || txt.includes('"playerAds"') || txt.includes('"adSlots"') || txt.includes('"streamingData"'))) {
            try {
              const parsed = origParse(txt);
              stripAdFields(parsed);
              return JSON.stringify(parsed);
            } catch (e) {}
          }
          return txt;
        }
      });
    }

    // Re-harvest on YouTube SPA navigation & player updates
    window.addEventListener('yt-navigate-finish', () => {
      if (window.ytInitialPlayerResponse) stripAdFields(window.ytInitialPlayerResponse);
      const mp = document.getElementById('movie_player');
      if (mp && typeof mp.getPlayerResponse === 'function') {
        stripAdFields(mp.getPlayerResponse());
      }
    });
  }

  // 6. Lightweight Network Sniffer + Built-in IDM Floating Pill (Hover, Selection, Right-Click & Drop Basket)
  const AD_HOSTS = [
    'doubleclick.net', 'googlesyndication.com', 'googleadservices.com',
    'adnxs.com', 'adsrvr.org', 'taboola.com', 'outbrain.com',
    'popads.net', 'popcash.net', 'propellerads.com', 'exoclick.com',
    'scorecardresearch.com', 'criteo.com', 'pubmatic.com', 'rubiconproject.com',
    'youtube.com/pagead/', 'youtube.com/api/stats/ads', 'youtube.com/ptracking'
  ];
  const MEDIA_EXTS = ['.mp4', '.mkv', '.webm', '.mov', '.m3u8', '.mpd', '.flac', '.alac', '.m4a', '.wav', '.mp3', '.ogg', '.opus'];
  const FILE_EXTS = ['.zip', '.7z', '.rar', '.tar', '.gz', '.iso', '.exe', '.msi', '.pdf', '.dmg', '.deb'];
  const ALL_EXTS = MEDIA_EXTS.concat(FILE_EXTS);
  const capturedStreams = [];

  function requestSizeProbe(targetUrl) {
    if (!targetUrl || probedSizes.has(targetUrl) || targetUrl === location.href) return;
    probedSizes.set(targetUrl, 'Probing...');
    postSecure('PROBE_STREAM:' + JSON.stringify({
      url: targetUrl,
      referer: location.href,
      cookie: document.cookie || ''
    }));
  }

  function addCapturedStream(rawUrl, kindHint) {
    if (!rawUrl || !String(rawUrl).startsWith('http')) return;
    const urlStr = String(rawUrl);
    if (urlStr.startsWith('blob:')) return;
    if (capturedStreams.some(s => s.url === urlStr)) return;
    const cleanPath = urlStr.split('?')[0].toLowerCase();
    const ext = ALL_EXTS.find(e => cleanPath.endsWith(e)) || kindHint || '.mp4';
    let labelName = 'Stream ' + ext.toUpperCase();
    try {
      const seg = cleanPath.split('/').pop();
      if (seg && seg.length > 2) labelName = decodeURIComponent(seg);
    } catch (e) {}
    capturedStreams.unshift({ url: urlStr, ext: ext.toUpperCase(), name: labelName });
    if (capturedStreams.length > 20) capturedStreams.pop();
    requestSizeProbe(urlStr);
    postSecure('STREAM:' + urlStr);
  }

  window.__shunnoOnStreamProbed = function(info) {
    if (!info || !info.url) return;
    probedSizes.set(info.url, info.size_label || 'Stream');
    const item = capturedStreams.find(s => s.url === info.url);
    if (item && info.resolved_name) {
      item.name = info.resolved_name;
    }
    if (window.__shunnoRefreshFloatingPill) {
      window.__shunnoRefreshFloatingPill(info.url, info.size_label);
    }
  };

  function inspectUrl(raw) {
    if (!raw) return false;
    const str = String(raw);
    const lower = str.toLowerCase();
    const cleanLower = lower.split('?')[0];
    if (ADBLOCK_ON && AD_HOSTS.some(h => lower.includes(h))) {
      postSecure('AD_BLOCKED');
      return true;
    }
    if (ALL_EXTS.some(ext => cleanLower.endsWith(ext)) && str.startsWith('http')) {
      addCapturedStream(str);
    }
    return false;
  }

  const origFetch = window.fetch;
  window.fetch = function(input, init) {
    const u = (typeof input === 'string') ? input : (input && input.url);
    if (inspectUrl(u)) return Promise.reject(new Error('Blocked by Shunno AdBlock'));
    return origFetch.apply(this, arguments);
  };

  const origOpen = XMLHttpRequest.prototype.open;
  XMLHttpRequest.prototype.open = function(method, url) {
    if (inspectUrl(url)) {
      this._shunnoBlocked = true;
      return;
    }
    return origOpen.apply(this, arguments);
  };
  const origSend = XMLHttpRequest.prototype.send;
  XMLHttpRequest.prototype.send = function() {
    if (this._shunnoBlocked) return;
    return origSend.apply(this, arguments);
  };

  function triggerShunnoGet(targetUrl, threads) {
    if (!targetUrl) return;
    postSecure('FLOAT_DL:' + JSON.stringify({
      url: targetUrl,
      threads: threads || 16,
      referer: location.href,
      cookie: document.cookie || ''
    }));
  }

  window.__shunnoTogglePiP = async function() {
    try {
      if (document.pictureInPictureElement) {
        await document.exitPictureInPicture();
        return;
      }
      const vids = Array.from(document.querySelectorAll('video')).filter(v => v.readyState >= 1 && v.videoWidth > 0);
      if (vids.length > 0) {
        await vids[0].requestPictureInPicture();
      }
    } catch (e) {}
  };

  function installFloatingOverlay() {
    if (!document.documentElement || document.getElementById('shunno-float-host')) return;

    const host = document.createElement('div');
    host.id = 'shunno-float-host';
    host.style.cssText = 'position:fixed;top:0;left:0;width:0;height:0;z-index:2147483647;pointer-events:none;';
    const shadow = host.attachShadow({ mode: 'open' });

    const styleEl = document.createElement('style');
    styleEl.textContent = `
      @keyframes shunnoPopIn {
        from { opacity: 0; transform: translateY(6px) scale(0.95); }
        to   { opacity: 1; transform: translateY(0) scale(1); }
      }
      .pill-wrap {
        position: fixed;
        pointer-events: auto;
        display: none;
        align-items: center;
        gap: 4px;
        background: rgba(20, 20, 24, 0.95);
        color: #FFFFFF;
        border: 1px solid rgba(255, 255, 255, 0.25);
        border-radius: 10px;
        padding: 4px 8px;
        font-family: system-ui, "Segoe UI", sans-serif;
        font-size: 12px;
        font-weight: 600;
        box-shadow: 0 8px 24px rgba(0, 0, 0, 0.42);
        animation: shunnoPopIn 0.2s cubic-bezier(0.22, 1, 0.36, 1);
        transition: left 0.16s cubic-bezier(0.22, 1, 0.36, 1), top 0.16s cubic-bezier(0.22, 1, 0.36, 1);
      }
      .pill-main {
        display: inline-flex;
        align-items: center;
        gap: 6px;
        cursor: pointer;
        padding: 2px 5px;
        border-radius: 6px;
      }
      .pill-main:hover { background: #0A84FF; }
      .pip-btn {
        background: rgba(255,255,255,0.14);
        color: #fff;
        border: none;
        border-radius: 6px;
        padding: 3px 7px;
        font-size: 11px;
        font-weight: 700;
        cursor: pointer;
      }
      .pip-btn:hover { background: #30D158; color: #000; }
      .badge {
        background: #30D158;
        color: #000;
        font-size: 10px;
        font-weight: 800;
        padding: 1px 6px;
        border-radius: 5px;
      }
      .menu {
        position: fixed;
        pointer-events: auto;
        display: none;
        flex-direction: column;
        gap: 6px;
        min-width: 300px;
        max-width: 400px;
        max-height: 320px;
        overflow-y: auto;
        background: rgba(20, 20, 24, 0.97);
        color: #fff;
        border: 1px solid rgba(255, 255, 255, 0.25);
        border-radius: 12px;
        padding: 10px;
        font-family: system-ui, "Segoe UI", sans-serif;
        font-size: 12px;
        box-shadow: 0 14px 36px rgba(0, 0, 0, 0.50);
      }
      .menu-hdr {
        display: flex;
        justify-content: space-between;
        align-items: center;
        font-weight: 700;
        font-size: 12px;
        color: #30D158;
        border-bottom: 1px solid rgba(255,255,255,0.14);
        padding-bottom: 6px;
      }
      .row {
        display: flex;
        justify-content: space-between;
        align-items: center;
        gap: 8px;
        padding: 6px 7px;
        border-radius: 8px;
        background: rgba(255,255,255,0.08);
      }
      .row:hover { background: rgba(10,132,255,0.35); }
      .name {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        flex: 1;
        font-size: 11.5px;
      }
      .go {
        background: #0A84FF;
        color: #fff;
        border: none;
        border-radius: 6px;
        padding: 4px 9px;
        font-size: 11px;
        font-weight: 700;
        cursor: pointer;
        flex-shrink: 0;
      }
      .go:hover { background: #30D158; color: #000; }
      .basket {
        position: fixed;
        right: 16px;
        bottom: 16px;
        pointer-events: auto;
        display: ${BASKET_ON ? 'flex' : 'none'};
        align-items: center;
        gap: 6px;
        background: rgba(10, 132, 255, 0.92);
        color: #fff;
        border: 1.5px dashed rgba(255, 255, 255, 0.7);
        border-radius: 14px;
        padding: 7px 12px;
        font-family: system-ui, "Segoe UI", sans-serif;
        font-size: 11.5px;
        font-weight: 700;
        box-shadow: 0 8px 24px rgba(10, 132, 255, 0.35);
        cursor: pointer;
      }
      .basket.dragover {
        background: #30D158;
        color: #000;
        transform: scale(1.06);
        border-style: solid;
      }
    `;
    shadow.appendChild(styleEl);

    const pillWrap = document.createElement('div');
    pillWrap.className = 'pill-wrap';

    const pillMain = document.createElement('div');
    pillMain.className = 'pill-main';

    const badge = document.createElement('span');
    badge.className = 'badge';
    badge.textContent = '0↓';

    const pillText = document.createElement('span');
    pillText.textContent = 'Download';

    const caret = document.createElement('span');
    caret.style.opacity = '0.75';
    caret.textContent = '▾';

    pillMain.appendChild(badge);
    pillMain.appendChild(pillText);
    pillMain.appendChild(caret);

    const pipBtn = document.createElement('button');
    pipBtn.className = 'pip-btn';
    pipBtn.textContent = '📺 PiP';
    pipBtn.title = 'Picture-in-Picture';
    pipBtn.onclick = (e) => {
      e.stopPropagation();
      window.__shunnoTogglePiP();
    };

    pillWrap.appendChild(pillMain);
    pillWrap.appendChild(pipBtn);

    const menu = document.createElement('div');
    menu.className = 'menu';

    const basket = document.createElement('div');
    basket.className = 'basket';
    basket.title = 'Drag & drop any link or URL here to start a 16-thread download';
    const bIcon = document.createElement('span');
    bIcon.textContent = '0↓';
    const basketLabel = document.createElement('span');
    basketLabel.textContent = 'Drop Link';
    basket.appendChild(bIcon);
    basket.appendChild(basketLabel);

    shadow.appendChild(pillWrap);
    shadow.appendChild(menu);
    shadow.appendChild(basket);
    document.documentElement.appendChild(host);

    let activeItems = [];
    let activeBaseLabel = 'Download';
    let hideTimer = null;

    basket.addEventListener('dragover', (e) => {
      e.preventDefault();
      e.stopPropagation();
      basket.classList.add('dragover');
      basketLabel.textContent = 'Release to Get!';
    });
    basket.addEventListener('dragleave', (e) => {
      e.preventDefault();
      basket.classList.remove('dragover');
      basketLabel.textContent = 'Drop Link';
    });
    basket.addEventListener('drop', (e) => {
      e.preventDefault();
      e.stopPropagation();
      basket.classList.remove('dragover');
      const dropped = (e.dataTransfer && (e.dataTransfer.getData('text/uri-list') || e.dataTransfer.getData('text/plain'))) || '';
      const urls = dropped.split(/\r?\n/).map(s => s.trim()).filter(s => s.startsWith('http'));
      if (urls.length > 0) {
        urls.forEach(u => triggerShunnoGet(u, 16));
        basketLabel.textContent = 'Queued (' + urls.length + ')!';
        setTimeout(() => { basketLabel.textContent = 'Drop Link'; }, 1400);
      } else {
        basketLabel.textContent = 'Drop Link';
      }
    });
    basket.addEventListener('click', () => postSecure('KEY:j'));

    window.__shunnoRefreshFloatingPill = function(url, sizeLabel) {
      if (activeItems.length === 1 && activeItems[0].url === url && pillWrap.style.display !== 'none') {
        pillText.textContent = activeBaseLabel + ' (' + sizeLabel + ')';
      }
    };

    function scheduleHide() {
      clearTimeout(hideTimer);
      hideTimer = setTimeout(() => {
        if (!pillWrap.matches(':hover') && !menu.matches(':hover')) {
          pillWrap.style.display = 'none';
          menu.style.display = 'none';
        }
      }, 1100);
    }

    pillWrap.addEventListener('mouseenter', () => clearTimeout(hideTimer));
    pillWrap.addEventListener('mouseleave', scheduleHide);
    menu.addEventListener('mouseenter', () => clearTimeout(hideTimer));
    menu.addEventListener('mouseleave', scheduleHide);

    function showPillAt(x, y, baseLabel, items, isVideo) {
      if (!items || items.length === 0) return;
      clearTimeout(hideTimer);
      activeItems = items;
      activeBaseLabel = baseLabel;
      pipBtn.style.display = isVideo ? 'inline-block' : 'none';
      items.slice(0, 6).forEach(it => requestSizeProbe(it.url));

      if (items.length === 1) {
        const sz = probedSizes.get(items[0].url);
        pillText.textContent = sz ? (baseLabel + ' (' + sz + ')') : baseLabel;
      } else {
        pillText.textContent = baseLabel + ' (' + items.length + ')';
      }

      const left = Math.min(window.innerWidth - 270, Math.max(8, x));
      const top = Math.min(window.innerHeight - 48, Math.max(8, y));
      pillWrap.style.left = left + 'px';
      pillWrap.style.top = top + 'px';
      pillWrap.style.display = 'inline-flex';
    }

    pillMain.addEventListener('click', (e) => {
      e.stopPropagation();
      if (activeItems.length === 1) {
        triggerShunnoGet(activeItems[0].url, 16);
        pillWrap.style.display = 'none';
        return;
      }
      while (menu.firstChild) menu.removeChild(menu.firstChild);

      const hdr = document.createElement('div');
      hdr.className = 'menu-hdr';
      const hdrTitle = document.createElement('span');
      hdrTitle.textContent = '⚡ Shunno Get (' + activeItems.length + ' items)';
      const closeBtn = document.createElement('span');
      closeBtn.style.cssText = 'cursor:pointer;color:#aaa;padding:0 4px;';
      closeBtn.textContent = '✕';
      closeBtn.onclick = () => { menu.style.display = 'none'; };
      hdr.appendChild(hdrTitle);
      hdr.appendChild(closeBtn);
      menu.appendChild(hdr);

      activeItems.forEach((it) => {
        const row = document.createElement('div');
        row.className = 'row';
        const b = document.createElement('span');
        b.className = 'badge';
        const sz = probedSizes.get(it.url);
        b.textContent = it.ext.replace('.', '') + (sz ? (' • ' + sz) : '');
        const nm = document.createElement('span');
        nm.className = 'name';
        nm.title = it.url;
        nm.textContent = it.name;
        const go = document.createElement('button');
        go.className = 'go';
        go.textContent = '16T';
        go.onclick = () => {
          triggerShunnoGet(it.url, 16);
          menu.style.display = 'none';
          pillWrap.style.display = 'none';
        };
        row.appendChild(b);
        row.appendChild(nm);
        row.appendChild(go);
        menu.appendChild(row);
      });

      menu.style.left = pillWrap.style.left;
      menu.style.top = (parseFloat(pillWrap.style.top) + 36) + 'px';
      menu.style.display = 'flex';
    });

    function inspectHoverTarget(target) {
      if (!target || !target.closest) return;
      const playerContainer = target.closest('video, audio, ytd-player, .html5-video-player, #movie_player');
      if (playerContainer) {
        if (isYouTube && window.__shunnoYtStreams.length === 0) {
          if (window.ytInitialPlayerResponse) stripAdFields(window.ytInitialPlayerResponse);
          const mp = document.getElementById('movie_player');
          if (mp && typeof mp.getPlayerResponse === 'function') {
            stripAdFields(mp.getPlayerResponse());
          }
        }
        const videoEl = playerContainer.matches('video, audio')
          ? playerContainer
          : playerContainer.querySelector('video, audio');
        const rect = playerContainer.getBoundingClientRect();
        if (rect.width >= 140 && rect.height >= 50) {
          const items = [];
          if (videoEl) {
            const directSrc = videoEl.currentSrc || videoEl.src;
            if (directSrc && directSrc.startsWith('http')) {
              addCapturedStream(directSrc, videoEl.tagName === 'AUDIO' ? '.m4a' : '.mp4');
            }
            videoEl.querySelectorAll('source').forEach(s => {
              if (s.src && s.src.startsWith('http')) addCapturedStream(s.src);
            });
          }
          if (window.__shunnoYtStreams && window.__shunnoYtStreams.length > 0) {
            window.__shunnoYtStreams.forEach(ys => {
              if (!items.some(x => x.url === ys.url)) items.push(ys);
            });
          }
          capturedStreams.forEach(s => {
            if (!items.some(x => x.url === s.url)) items.push(s);
          });
          if (items.length === 0 && isYouTube) {
            items.push({
              url: location.href,
              ext: '.MP4',
              name: (document.title || 'YouTube Video').replace(' - YouTube', '')
            });
          }
          if (items.length > 0) {
            const isAud = videoEl && videoEl.tagName === 'AUDIO';
            showPillAt(rect.right - 265, Math.max(8, rect.top + 12), isAud ? 'Download audio' : 'Download video', items, !isAud);
            return;
          }
        }
      }

      const linkEl = target.closest('a[href]');
      if (linkEl && linkEl.href && linkEl.href.startsWith('http')) {
        const cleanHref = linkEl.href.split('?')[0].toLowerCase();
        const matchedExt = ALL_EXTS.find(ext => cleanHref.endsWith(ext));
        if (matchedExt) {
          const rect = linkEl.getBoundingClientRect();
          let fname = 'File ' + matchedExt;
          try {
            fname = decodeURIComponent(cleanHref.split('/').pop() || fname);
          } catch (e) {}
          showPillAt(
            rect.left,
            Math.max(8, rect.top - 36),
            'Download ' + matchedExt.toUpperCase(),
            [{ url: linkEl.href, ext: matchedExt.toUpperCase(), name: fname }],
            false
          );
        }
      }
    }

    // IDM-style "Download Selected Links" floating bar when highlighting text/links on any page
    document.addEventListener('mouseup', (e) => {
      setTimeout(() => {
        const sel = window.getSelection();
        if (!sel || sel.isCollapsed) return;
        const foundUrls = new Set();
        const text = sel.toString() || '';
        const urlMatches = text.match(/https?:\/\/[^\s"'<>]+/g);
        if (urlMatches) {
          urlMatches.forEach(u => foundUrls.add(u));
        }
        if (sel.rangeCount > 0) {
          const frag = sel.getRangeAt(0).cloneContents();
          frag.querySelectorAll('a[href]').forEach(a => {
            if (a.href && a.href.startsWith('http')) foundUrls.add(a.href);
          });
        }
        if (foundUrls.size > 0) {
          const items = Array.from(foundUrls).slice(0, 25).map(u => {
            const clean = u.split('?')[0];
            const seg = decodeURIComponent(clean.split('/').pop() || 'link');
            const ext = ALL_EXTS.find(ex => clean.toLowerCase().endsWith(ex)) || '.LINK';
            return { url: u, ext: ext.toUpperCase(), name: seg };
          });
          showPillAt(e.clientX + 10, e.clientY + 12, 'Download Selected', items, false);
        }
      }, 30);
    }, true);

    let lastHoverCheck = 0;
    document.addEventListener('mouseover', (e) => inspectHoverTarget(e.target), true);
    document.addEventListener('mousemove', (e) => {
      const now = Date.now();
      if (now - lastHoverCheck < 180) return;
      lastHoverCheck = now;
      inspectHoverTarget(e.target);
    }, { passive: true, capture: true });
    document.addEventListener('mouseout', (e) => {
      if (e.target && e.target.closest && e.target.closest('video, audio, ytd-player, .html5-video-player, #movie_player, a[href]')) {
        scheduleHide();
      }
    }, true);
  }

  // 7. Debounced MutationObserver for YouTube Ad Skipper & Cookie Auto-Rejector
  if (ADBLOCK_ON || SPONSORBLOCK_ON || REJECT_COOKIES_ON) {
    const styleSheet = document.createElement('style');
    if (ADBLOCK_ON || SPONSORBLOCK_ON) {
      styleSheet.textContent = 'ytd-reel-shelf-renderer, ytd-rich-section-renderer, ytd-ad-slot-renderer, ytd-banner-promo-renderer, #masthead-ad, .ad-showing .ytp-ad-player-overlay, .ytp-ad-module, ins.adsbygoogle, [id^="google_ads_"], [class*="taboola"], [class*="outbrain"] { display: none !important; }';
    }
    (document.head || document.documentElement).appendChild(styleSheet);

    let domScheduled = false;
    function runCleanPass() {
      domScheduled = false;
      if (SPONSORBLOCK_ON && isYouTube) {
        const sk = document.querySelector('.ytp-ad-skip-button, .ytp-ad-skip-button-modern, .ytp-skip-ad-button');
        if (sk) sk.click();
        const adShowing = document.querySelector('.ad-showing, .ad-interrupting');
        const v = document.querySelector('video');
        if (v && adShowing) {
          v.muted = true;
          if (isFinite(v.duration) && v.duration > 0) {
            v.currentTime = v.duration;
          } else {
            v.playbackRate = 16.0;
          }
        }
      }
      if (REJECT_COOKIES_ON) {
        document.querySelectorAll('#onetrust-reject-all-handler, .fc-cta-do-not-consent, [aria-label*="Reject all" i], button[id*="reject" i]').forEach(b => b.click());
      }
    }

    const observer = new MutationObserver(() => {
      if (!domScheduled) {
        domScheduled = true;
        requestAnimationFrame(runCleanPass);
      }
    });

    const startObserver = () => {
      if (document.body) {
        observer.observe(document.body, { childList: true, subtree: true });
        runCleanPass();
      }
    };
    if (document.body) startObserver();
    else window.addEventListener('DOMContentLoaded', startObserver, { once: true });
  }

  // 8. Audio Activity Reporter (drives Tab Audio Mute Indicator 🔊 / 🔇)
  let lastAudioPlaying = false;
  function reportAudioState() {
    const media = Array.from(document.querySelectorAll('video, audio'));
    const playing = media.some(m => !m.paused && !m.ended && m.readyState > 2);
    if (playing !== lastAudioPlaying) {
      lastAudioPlaying = playing;
      postSecure('AUDIO_STATE:' + (playing ? '1' : '0'));
    }
  }
  document.addEventListener('play', reportAudioState, true);
  document.addEventListener('pause', reportAudioState, true);
  document.addEventListener('ended', reportAudioState, true);

  window.__shunnoSyncYtStreams = function(list) {
    if (!Array.isArray(list)) return;
    list.forEach(it => {
      if (it && it.url && !capturedStreams.some(s => s.url === it.url)) {
        capturedStreams.push(it);
        requestSizeProbe(it.url);
      }
    });
  };

  window.__shunnoGrabAllPageLinks = function() {
    const found = [];
    const seen = new Set();
    function pushItem(url, nameHint, extHint) {
      if (!url || !String(url).startsWith('http') || String(url).startsWith('blob:')) return;
      const u = String(url);
      if (seen.has(u)) return;
      seen.add(u);
      const clean = u.split('?')[0];
      let name = nameHint || 'Media / File';
      try {
        const seg = decodeURIComponent(clean.split('/').pop() || '');
        if (seg && seg.length > 2) name = seg;
      } catch (e) {}
      found.push({ url: u, name: name, ext: extHint || 'FILE' });
    }

    if (window.__shunnoYtStreams && window.__shunnoYtStreams.length > 0) {
      window.__shunnoYtStreams.forEach(s => pushItem(s.url, s.name, s.ext));
    }
    capturedStreams.forEach(s => pushItem(s.url, s.name, s.ext));

    document.querySelectorAll('a[href], video[src], audio[src], source[src]').forEach(el => {
      const raw = el.href || el.src;
      if (!raw || !String(raw).startsWith('http')) return;
      const clean = String(raw).split('?')[0].toLowerCase();
      const matchedExt = ALL_EXTS.find(ext => clean.endsWith(ext));
      if (matchedExt || el.hasAttribute('download') || el.tagName === 'VIDEO' || el.tagName === 'AUDIO') {
        const label = (el.textContent || '').trim() || el.getAttribute('download') || '';
        pushItem(raw, label, (matchedExt || '.media').toUpperCase());
      }
    });

    postSecure('GRAB_RESULTS:' + JSON.stringify(found.slice(0, 60)));
  };

  function scanInlineScriptsForYt() {
    if (!isYouTube || window.__shunnoYtStreams.length > 0) return;
    try {
      const scripts = document.querySelectorAll('script');
      for (const s of scripts) {
        const txt = s.textContent || '';
        if (txt.includes('streamingData') && txt.includes('formats')) {
          const m = txt.match(/ytInitialPlayerResponse\s*=\s*(\{.+?\});/s);
          if (m && m[1]) {
            const parsed = JSON.parse(m[1]);
            stripAdFields(parsed);
            if (window.__shunnoYtStreams.length > 0) break;
          }
        }
      }
    } catch (e) {}
  }

  function sendPageMeta() {
    postSecure('META:' + JSON.stringify({
      title: document.title || location.hostname || location.href,
      url: location.href,
      cookie: document.cookie || ''
    }));
  }

  if (document.documentElement) installFloatingOverlay();
  window.addEventListener('DOMContentLoaded', () => {
    if (JS_DISABLED) {
      document.querySelectorAll('script').forEach(s => s.remove());
    }
    if (isYouTube) {
      if (window.ytInitialPlayerResponse) stripAdFields(window.ytInitialPlayerResponse);
      scanInlineScriptsForYt();
    }
    sendPageMeta();
    const titleEl = document.querySelector('title');
    if (titleEl) {
      new MutationObserver(sendPageMeta).observe(titleEl, { childList: true, characterData: true, subtree: true });
    }
    document.querySelectorAll('video source, audio source, video[src], audio[src]').forEach(el => {
      if (el.src) inspectUrl(el.src);
    });
    installFloatingOverlay();
  });
  window.addEventListener('load', sendPageMeta);

  document.addEventListener('keydown', (e) => {
    if (e.key === 'F11') {
      e.preventDefault();
      postSecure('KEY:f11');
    } else if (e.ctrlKey && !e.shiftKey && !e.altKey) {
      const k = e.key.toLowerCase();
      if (k === 't' || k === 'w' || k === 'j') {
        e.preventDefault();
        postSecure('KEY:' + k);
      } else if (k === '=' || k === '+') {
        e.preventDefault();
        postSecure('KEY:zoom_in');
      } else if (k === '-') {
        e.preventDefault();
        postSecure('KEY:zoom_out');
      } else if (k === '0') {
        e.preventDefault();
        postSecure('KEY:zoom_reset');
      }
    }
  });

  document.addEventListener('wheel', (e) => {
    if (e.ctrlKey) {
      e.preventDefault();
      postSecure(e.deltaY < 0 ? 'KEY:zoom_in' : 'KEY:zoom_out');
    }
  }, { passive: false });
})();

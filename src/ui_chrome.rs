pub const CHROME_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<style>
  :root {
    --bg: #F5F5F7;
    --tab-bg: #E8E8ED;
    --surface: #FFFFFF;
    --surface-solid: #FFFFFF;
    --glass: rgba(255, 255, 255, 0.78);
    --border: rgba(0, 0, 0, 0.09);
    --blue: #0A84FF;
    --blue-soft: rgba(10, 132, 255, 0.12);
    --green: #30D158;
    --green-soft: rgba(48, 209, 88, 0.14);
    --orange: #FF9F0A;
    --red: #FF3B30;
    --text: #1D1D1F;
    --muted: #6E6E73;
    --input-bg: rgba(118, 118, 128, 0.11);
    --ease-fluid: cubic-bezier(0.25, 1, 0.5, 1);
    --ease-spring: cubic-bezier(0.22, 1.15, 0.36, 1);
  }
  body.dark {
    --bg: #0D0D10;
    --tab-bg: #161619;
    --surface: #1C1C1E;
    --surface-solid: #1C1C1E;
    --glass: rgba(30, 30, 34, 0.80);
    --border: rgba(255, 255, 255, 0.11);
    --blue: #0A84FF;
    --blue-soft: rgba(10, 132, 255, 0.22);
    --green: #30D158;
    --green-soft: rgba(48, 209, 88, 0.20);
    --orange: #FF9F0A;
    --red: #FF453A;
    --text: #F5F5F7;
    --muted: #98989D;
    --input-bg: rgba(255, 255, 255, 0.09);
  }
  * {
    box-sizing: border-box; margin: 0; padding: 0;
    font-family: system-ui, "Segoe UI Variable Display", "Segoe UI", sans-serif;
    user-select: none;
  }
  html, body {
    background: transparent;
    color: var(--text);
    overflow: hidden;
    display: flex;
    flex-direction: column;
    height: 100vh;
    border-radius: 12px;
    transition: color 0.36s cubic-bezier(0.22, 1, 0.36, 1), background-color 0.36s cubic-bezier(0.22, 1, 0.36, 1);
  }
  .chrome-shell, .floating-sheet, .card, .home-canvas, .home-search-box, .shortcut-tile, .engine-menu, .nav-icon-btn, .tab, .bookmarks-strip {
    transition: background-color 0.36s cubic-bezier(0.22, 1, 0.36, 1), color 0.36s ease, border-color 0.36s ease, box-shadow 0.36s ease;
  }

  /* Unified Top Chrome Shell (112px total: 36px Tab Strip + 46px Firefox-Style Address Bar + 30px Bookmarks Bar) */
  .chrome-shell {
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    border-top-left-radius: 12px;
    border-top-right-radius: 12px;
    z-index: 20;
    transition: background 0.3s var(--ease-fluid);
  }

  /* 1. Borderless Tab Strip (No redundant "Shunno Browser" title bar!) */
  .tab-bar {
    display: flex; align-items: center; gap: 6px; padding: 5px 8px 4px 10px;
    background: var(--tab-bg); height: 36px; overflow: hidden;
    border-top-left-radius: 12px; border-top-right-radius: 12px;
  }

  /* Chrome/Safari-inspired Compass-Ring Logo */
  .brand-emblem {
    width: 28px; height: 28px; border-radius: 50%;
    display: inline-flex; align-items: center; justify-content: center;
    flex-shrink: 0; cursor: pointer;
    box-shadow: 0 2px 6px rgba(10, 132, 255, 0.28);
    transition: transform 0.28s var(--ease-spring);
  }
  .brand-emblem:hover { transform: scale(1.08) rotate(12deg); }

  #tabs-container {
    display: flex; gap: 5px; align-items: center; flex: 0 1 auto; min-width: 0; overflow: hidden;
  }

  /* Static, flicker-free tabs (only newly added tab animates in; switching tabs never re-creates DOM nodes!) */
  @keyframes tabEnter {
    from { opacity: 0; transform: scale(0.92); }
    to   { opacity: 1; transform: scale(1); }
  }
  .tab {
    display: flex; align-items: center; gap: 6px; padding: 4px 10px; border-radius: 10px;
    background: rgba(128, 128, 128, 0.11); color: var(--text); font-size: 12px; cursor: pointer;
    flex: 1 1 165px; min-width: 42px; max-width: 200px; overflow: hidden;
    transition: background 0.22s var(--ease-fluid), box-shadow 0.22s var(--ease-fluid), color 0.18s ease;
  }
  .tab.is-new { animation: tabEnter 0.24s var(--ease-fluid); }
  .tab:hover { background: rgba(128, 128, 128, 0.20); }
  .tab.active {
    background: var(--surface-solid); color: var(--text); font-weight: 600;
    box-shadow: 0 1px 6px rgba(0, 0, 0, 0.12);
  }
  .tab.incognito.active { background: #2C2C2E; color: #FFFFFF; }
  .tab.drag-over-left { box-shadow: -3px 0 0 0 var(--blue); }
  .tab.drag-over-right { box-shadow: 3px 0 0 0 var(--blue); }
  .folder-pills { display: flex; gap: 5px; flex-wrap: wrap; margin-bottom: 8px; }
  .folder-pill {
    border: none; border-radius: 8px; padding: 3px 9px; font-size: 11px; font-weight: 600;
    background: var(--input-bg); color: var(--text); cursor: pointer;
    transition: background 0.18s ease, color 0.18s ease;
  }
  .folder-pill.active { background: var(--blue); color: #fff; }

  .tab-audio {
    font-size: 11px; padding: 0 4px; border-radius: 5px; flex-shrink: 0;
    background: var(--blue-soft); color: var(--blue);
    transition: background 0.18s ease, color 0.18s ease;
  }
  .tab-audio:hover { background: var(--blue); color: #fff; }
  .tab-title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1; min-width: 0; }
  .tab-close {
    font-size: 10px; color: var(--muted); padding: 2px 5px; border-radius: 5px; flex-shrink: 0;
    transition: background 0.16s ease, color 0.16s ease;
  }
  .tab-close:hover { background: rgba(255, 59, 48, 0.18); color: var(--red); }

  .tab-add {
    border: none; background: rgba(128, 128, 128, 0.14); color: var(--blue); font-weight: 700;
    font-size: 12px; padding: 4px 9px; border-radius: 9px; cursor: pointer; flex-shrink: 0;
    transition: background 0.2s var(--ease-fluid), transform 0.18s var(--ease-spring);
  }
  .tab-add:hover { background: var(--surface-solid); }
  .tab-add:active { transform: scale(0.92); }

  /* Drag spacer + Native Window Controls (replaces wasted top title bar!) */
  .win-drag-spacer {
    flex: 1; height: 100%; min-width: 24px; cursor: default;
  }
  .win-controls {
    display: flex; align-items: center; gap: 4px; flex-shrink: 0;
  }
  .win-btn {
    width: 30px; height: 26px; border: none; border-radius: 7px;
    background: transparent; color: var(--text); font-size: 13px; line-height: 1; font-weight: 600;
    cursor: pointer; display: inline-flex; align-items: center; justify-content: center;
    transition: background 0.18s var(--ease-fluid), color 0.18s ease;
  }
  .win-btn:hover { background: rgba(128, 128, 128, 0.18); }
  .win-btn.close:hover { background: #FF3B30; color: #FFFFFF; }

  /* 2. Firefox-Inspired Centered Compact Address + Search Capsule */
  .nav-bar {
    display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 5px 12px;
    height: 46px;
  }
  .nav-group { display: flex; align-items: center; gap: 5px; flex-shrink: 0; }
  .nav-btn {
    border: none; background: var(--input-bg); color: var(--text); font-size: 13px; font-weight: 600;
    padding: 6px 10px; border-radius: 10px; cursor: pointer; display: inline-flex; align-items: center; gap: 5px;
    white-space: nowrap;
    transition: background 0.22s var(--ease-fluid), transform 0.18s var(--ease-spring), color 0.2s ease;
  }
  .nav-btn:hover { filter: brightness(0.95); }
  .nav-btn:active { transform: scale(0.94); }
  .nav-btn.on { background: var(--blue-soft); color: var(--blue); }
  .nav-btn.green { background: var(--green-soft); color: #24B44B; }

  /* Firefox-style Unified Search + URL Capsule (compact centered width) */
  .firefox-capsule {
    flex: 1; max-width: 500px; min-width: 200px; height: 32px;
    display: flex; align-items: center; gap: 6px;
    background: var(--input-bg); border-radius: 16px; padding: 2px 10px 2px 4px;
    border: 1.5px solid transparent;
    transition: background 0.34s var(--ease-fluid), border-color 0.25s var(--ease-fluid), box-shadow 0.28s var(--ease-fluid), max-width 0.3s var(--ease-fluid);
  }
  .firefox-capsule:focus-within {
    background: var(--surface-solid);
    border-color: var(--blue);
    box-shadow: 0 0 0 3.5px rgba(10, 132, 255, 0.18);
    max-width: 560px;
  }
  .engine-logo-btn {
    border: none; background: var(--surface-solid); color: var(--text);
    width: 28px; height: 26px; border-radius: 8px;
    display: inline-flex; align-items: center; justify-content: center;
    cursor: pointer; flex-shrink: 0; box-shadow: 0 1px 3px rgba(0,0,0,0.10);
    transition: transform 0.18s var(--ease-spring), background 0.2s ease;
  }
  .engine-logo-btn:hover { transform: scale(1.06); }
  .engine-menu {
    position: fixed; top: 82px; left: 50%; transform: translateX(-280px);
    background: var(--surface-solid); border: 1px solid var(--border);
    border-radius: 13px; padding: 6px; min-width: 170px;
    box-shadow: 0 12px 32px rgba(0,0,0,0.20); z-index: 90;
    display: none; flex-direction: column; gap: 3px;
  }
  .engine-menu.open { display: flex; }
  .engine-opt {
    display: flex; align-items: center; gap: 8px; padding: 6px 10px;
    border-radius: 9px; border: none; background: transparent; color: var(--text);
    font-size: 12.5px; font-weight: 600; cursor: pointer; text-align: left;
  }
  .engine-opt:hover { background: var(--input-bg); }
  .engine-opt.active { background: var(--blue-soft); color: var(--blue); }
  #url-input {
    border: none; background: transparent; outline: none; flex: 1; min-width: 60px;
    font-size: 13px; color: var(--text); user-select: text;
  }
  .zoom-pill {
    font-size: 11px; font-weight: 700; color: var(--blue); background: var(--blue-soft);
    padding: 2px 7px; border-radius: 7px; cursor: pointer; flex-shrink: 0;
    transition: transform 0.16s var(--ease-spring);
  }
  .zoom-pill:hover { transform: scale(1.06); }

  /* 3. Chrome-Inspired Bookmarks & Library Strip Beneath Address Bar */
  .bookmarks-strip {
    display: flex; align-items: center; gap: 6px; padding: 0 12px;
    height: 30px; border-top: 1px solid var(--border); overflow-x: auto;
  }
  .bookmarks-strip::-webkit-scrollbar { display: none; }
  .bm-chip {
    display: inline-flex; align-items: center; gap: 5px;
    padding: 3px 9px; border-radius: 8px; font-size: 11.5px; font-weight: 500;
    color: var(--text); background: transparent; border: none; cursor: pointer;
    max-width: 170px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; flex-shrink: 0;
    transition: background 0.2s var(--ease-fluid), transform 0.16s var(--ease-spring);
  }
  .bm-chip:hover { background: var(--input-bg); }
  .bm-chip:active { transform: scale(0.95); }
  .bm-chip.action { color: var(--blue); font-weight: 600; background: var(--blue-soft); }

  /* Clean Home Page Canvas (shown when active tab URL is shunno://newtab) */
  .home-canvas {
    flex: 1; display: none; flex-direction: column; align-items: center; justify-content: center;
    padding: 24px; background: var(--bg); overflow-y: auto;
    border-bottom-left-radius: 12px; border-bottom-right-radius: 12px;
  }
  .home-canvas.active { display: flex; }
  .home-hero {
    display: flex; flex-direction: column; align-items: center; gap: 18px;
    width: 100%; max-width: 640px;
  }
  .home-search-box {
    width: 100%; display: flex; align-items: center; gap: 10px;
    background: var(--surface-solid); border: 1px solid var(--border);
    border-radius: 16px; padding: 10px 16px;
    box-shadow: 0 8px 28px rgba(0, 0, 0, 0.07);
    transition: box-shadow 0.28s var(--ease-fluid), border-color 0.25s ease;
  }
  .home-search-box:focus-within {
    border-color: var(--blue);
    box-shadow: 0 10px 34px rgba(10, 132, 255, 0.18);
  }
  .home-search-input {
    border: none; background: transparent; outline: none; flex: 1;
    font-size: 15px; color: var(--text); user-select: text;
  }
  .shortcuts-grid {
    display: grid; grid-template-columns: repeat(auto-fill, minmax(112px, 1fr));
    gap: 12px; width: 100%; margin-top: 8px;
  }
  .shortcut-tile {
    position: relative; display: flex; flex-direction: column; align-items: center; gap: 8px;
    padding: 14px 10px; border-radius: 16px; background: var(--surface-solid);
    border: 1px solid var(--border); cursor: pointer;
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.04);
    transition: transform 0.24s var(--ease-spring), box-shadow 0.24s var(--ease-fluid);
  }
  .shortcut-tile:hover {
    transform: translateY(-3px);
    box-shadow: 0 10px 24px rgba(0, 0, 0, 0.10);
  }
  .shortcut-icon {
    width: 40px; height: 40px; border-radius: 12px;
    background: var(--blue-soft); color: var(--blue);
    display: flex; align-items: center; justify-content: center;
    font-size: 17px; font-weight: 800;
  }
  .shortcut-label {
    font-size: 12px; font-weight: 600; color: var(--text);
    max-width: 96px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .shortcut-del {
    position: absolute; top: 6px; right: 6px; width: 18px; height: 18px;
    border-radius: 50%; border: none; background: rgba(128,128,128,0.18);
    color: var(--muted); font-size: 10px; cursor: pointer; opacity: 0;
    transition: opacity 0.18s ease, background 0.18s ease;
  }
  .shortcut-tile:hover .shortcut-del { opacity: 1; }
  .shortcut-del:hover { background: var(--red); color: #fff; }

  /* Translucent Frosted-Glass Floating Overlays (Menu & Downloads float over webpage!) */
  .overlay-backdrop {
    position: fixed; top: 112px; left: 0; right: 0; bottom: 0;
    display: none; justify-content: flex-end; align-items: flex-start;
    padding: 12px 14px;
    background: rgba(15, 15, 20, 0.08);
    backdrop-filter: blur(1.5px);
    z-index: 50;
  }
  .overlay-backdrop.open {
    display: flex;
  }
  .floating-sheet {
    position: relative;
    z-index: 2;
  }

  @keyframes glassFloatIn {
    0%   { opacity: 0; transform: translateY(-12px) scale(0.95); }
    100% { opacity: 1; transform: translateY(0) scale(1); }
  }

  .floating-sheet {
    width: 430px; max-width: calc(100vw - 28px); max-height: calc(100vh - 136px);
    background: var(--glass);
    backdrop-filter: blur(32px) saturate(190%);
    border: 1px solid var(--border);
    border-radius: 18px;
    padding: 14px;
    overflow-y: auto;
    box-shadow: 0 22px 54px rgba(0, 0, 0, 0.26);
    transform-origin: top right;
    animation: glassFloatIn 0.32s var(--ease-spring) forwards;
  }
  .floating-sheet.wide { width: 520px; }

  .card {
    background: var(--surface); border-radius: 14px; padding: 12px 13px; margin-bottom: 10px;
    border: 1px solid var(--border);
    transition: background 0.24s var(--ease-fluid);
  }
  .card:last-child { margin-bottom: 0; }
  .card-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; gap: 6px; flex-wrap: wrap; }
  .card-title { font-size: 13px; font-weight: 700; }
  .grid-toggles { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 6px; }
  .toggle-tile {
    display: flex; justify-content: space-between; align-items: center; gap: 4px;
    padding: 7px 9px; border-radius: 10px; background: var(--input-bg); font-size: 11.5px; font-weight: 600;
    min-width: 0;
  }
  .toggle-tile span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pill-btn {
    border: none; border-radius: 9px; padding: 5px 10px; font-size: 11.5px; font-weight: 600; cursor: pointer;
    transition: background 0.2s var(--ease-fluid), transform 0.16s var(--ease-spring);
  }
  .pill-btn:hover { filter: brightness(0.95); }
  .pill-btn:active { transform: scale(0.93); }
  .pill-blue { background: var(--blue); color: #fff; }
  .pill-soft { background: var(--blue-soft); color: var(--blue); }
  .pill-green { background: var(--green-soft); color: #24B44B; }
  .pill-orange { background: rgba(255,159,10,0.16); color: var(--orange); }
  .pill-red { background: rgba(255,59,48,0.16); color: var(--red); }

  .input-row { display: flex; gap: 6px; margin-bottom: 8px; flex-wrap: wrap; }
  .text-box {
    flex: 1; min-width: 110px; padding: 7px 10px; border-radius: 10px; border: 1px solid var(--border);
    background: var(--input-bg); color: var(--text); font-size: 12.5px; outline: none; user-select: text;
  }
  .text-box:focus { border-color: var(--blue); background: var(--surface-solid); }
  .select-box {
    padding: 7px 9px; border-radius: 10px; border: 1px solid var(--border);
    font-size: 12px; background: var(--input-bg); color: var(--text);
  }

  .speed-graph-box {
    background: var(--input-bg);
    border-radius: 11px;
    padding: 8px 10px 6px 10px;
    margin-bottom: 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .speed-graph-hdr {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 11px;
    font-weight: 700;
  }
  .prog-track { width: 100%; height: 6px; background: var(--input-bg); border-radius: 4px; overflow: hidden; margin: 6px 0; }
  .prog-fill { height: 100%; background: var(--blue); transition: width 0.3s var(--ease-fluid); }
  .prog-fill.done { background: var(--green); }
  .meta { font-size: 11px; color: var(--muted); }
  .list-row {
    display: flex; justify-content: space-between; align-items: center; padding: 7px 0;
    border-bottom: 1px solid var(--border); gap: 8px;
  }
  .list-row:last-child { border-bottom: none; }

  /* F11 Fullscreen hides Tab Bar + Navigation/Bookmarks Shell */
  body.fullscreen .chrome-shell { display: none !important; }
  .fs-exit-pill {
    position: fixed; top: 8px; right: 14px; z-index: 95;
    display: none; align-items: center; gap: 6px;
    background: rgba(24, 24, 28, 0.86); color: #fff;
    backdrop-filter: blur(14px); border: 1px solid rgba(255,255,255,0.2);
    border-radius: 10px; padding: 5px 11px; font-size: 11.5px; font-weight: 600;
    cursor: pointer; opacity: 0.35; transition: opacity 0.2s ease;
  }
  .fs-exit-pill:hover { opacity: 1; }
  body.fullscreen .fs-exit-pill { display: inline-flex; }

  /* Pinned Compact Icon-Only Tabs */
  .tab.pinned {
    flex: 0 0 38px !important;
    min-width: 38px !important;
    max-width: 38px !important;
    justify-content: center;
    padding: 4px 6px;
  }
  .tab.pinned .tab-title, .tab.pinned .tab-close { display: none; }
  .tab.sleeping { opacity: 0.62; }
  .tab.split-peer { box-shadow: inset 0 0 0 1.5px var(--blue); }

  /* Ctrl+K Command Palette */
  .palette-backdrop {
    position: fixed; inset: 0; z-index: 98;
    display: none; justify-content: center; align-items: flex-start;
    padding-top: 86px;
    background: rgba(12, 12, 16, 0.16);
    backdrop-filter: blur(2px);
  }
  .palette-backdrop.open { display: flex; }
  .palette-box {
    width: 520px; max-width: calc(100vw - 28px);
    background: var(--surface-solid); border: 1px solid var(--border);
    border-radius: 16px; padding: 10px;
    box-shadow: 0 22px 54px rgba(0, 0, 0, 0.28);
    display: flex; flex-direction: column; gap: 8px;
  }
  .palette-input {
    width: 100%; border: 1.5px solid var(--blue); border-radius: 11px;
    background: var(--input-bg); color: var(--text);
    padding: 9px 12px; font-size: 13.5px; outline: none; user-select: text;
  }
  .palette-list {
    max-height: 320px; overflow-y: auto; display: flex; flex-direction: column; gap: 3px;
  }
  .palette-item {
    display: flex; align-items: center; justify-content: space-between; gap: 8px;
    padding: 7px 10px; border-radius: 9px; border: none; background: transparent;
    color: var(--text); cursor: pointer; text-align: left; font-size: 12.5px;
  }
  .palette-item:hover, .palette-item.sel { background: var(--blue-soft); color: var(--blue); }
  .palette-kind {
    font-size: 10px; font-weight: 700; text-transform: uppercase;
    padding: 2px 6px; border-radius: 6px; background: var(--input-bg); color: var(--muted); flex-shrink: 0;
  }

  /* Larger Top-Left Navigation Hit Areas (34x34px) */
  .nav-icon-btn {
    width: 34px; height: 34px; padding: 0;
    display: inline-flex; align-items: center; justify-content: center;
    border: none; border-radius: 10px; background: var(--input-bg); color: var(--text);
    cursor: pointer; transition: background 0.2s var(--ease-fluid), transform 0.16s var(--ease-spring);
  }
  .nav-icon-btn:hover { filter: brightness(0.95); }
  .nav-icon-btn:active { transform: scale(0.93); }

  /* Full URL display without truncation */
  #url-input {
    text-overflow: clip !important;
    white-space: nowrap;
    overflow-x: auto;
  }

  .find-badge {
    font-size: 11.5px; font-weight: 700; color: var(--blue); background: var(--blue-soft);
    padding: 5px 9px; border-radius: 8px; white-space: nowrap;
  }
</style>
</head>
<body>

  <div class="chrome-shell">
    <!-- 1. Borderless Tab Strip (No redundant top title bar!) -->
    <div class="tab-bar" id="tab-bar">
      <div class="brand-emblem" onclick="send('NAV:shunno://newtab')" title="Shunno Home">
        <svg width="28" height="28" viewBox="0 0 28 28" fill="none">
          <defs>
            <radialGradient id="shunnoCompass" cx="35%" cy="30%" r="70%">
              <stop offset="0%" stop-color="#38BDF8"/>
              <stop offset="100%" stop-color="#0055D4"/>
            </radialGradient>
          </defs>
          <circle cx="14" cy="14" r="13" fill="url(#shunnoCompass)" stroke="#E2E8F0" stroke-width="1.5"/>
          <circle cx="14" cy="14" r="7.5" stroke="white" stroke-width="2.1" stroke-opacity="0.92"/>
          <polygon points="18.8,9.2 15.6,15.6 12.4,12.4" fill="#FF453A"/>
          <polygon points="9.2,18.8 15.6,15.6 12.4,12.4" fill="#F8FAFC"/>
          <circle cx="14" cy="14" r="1.3" fill="#1C1C1E"/>
        </svg>
      </div>
      <div id="tabs-container"></div>
      <button class="tab-add" onclick="send('NEW_TAB')" title="New Tab (Ctrl+T)">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
      </button>
      <button class="tab-add" style="color:var(--text)" onclick="send('NEW_INCOGNITO')" title="New Private Tab">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 11h18"/><path d="M5 11l2-6h10l2 6"/><circle cx="8.5" cy="16" r="2.5"/><circle cx="15.5" cy="16" r="2.5"/><line x1="11" y1="16" x2="13" y2="16"/></svg>
      </button>
      <button class="tab-add" id="btn-reopen-tab" style="display:none;color:var(--muted)" onclick="send('REOPEN_CLOSED_TAB')" title="Reopen Closed Tab (Ctrl+Shift+T)">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.3" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 14 4 9 9 4"/><path d="M20 20v-7a4 4 0 0 0-4-4H4"/></svg>
      </button>
      <div class="win-drag-spacer" onmousedown="send('WIN_DRAG')" ondblclick="send('WIN_MAXIMIZE')"></div>
      <div class="win-controls">
        <button class="win-btn" onclick="send('WIN_MINIMIZE')" title="Minimize">
          <svg width="11" height="11" viewBox="0 0 12 12"><line x1="2" y1="6" x2="10" y2="6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>
        </button>
        <button class="win-btn" onclick="send('WIN_MAXIMIZE')" title="Maximize / Restore">
          <svg width="11" height="11" viewBox="0 0 12 12"><rect x="2" y="2" width="8" height="8" rx="1.2" fill="none" stroke="currentColor" stroke-width="1.5"/></svg>
        </button>
        <button class="win-btn close" onclick="send('WIN_CLOSE')" title="Close">
          <svg width="11" height="11" viewBox="0 0 12 12"><line x1="2.5" y1="2.5" x2="9.5" y2="9.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/><line x1="9.5" y1="2.5" x2="2.5" y2="9.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>
        </button>
      </div>
    </div>

    <!-- 2. Firefox-Inspired Compact Centered Address + Search Bar -->
    <div class="nav-bar">
      <div class="nav-group">
        <button class="nav-icon-btn" onclick="send('BACK')" title="Back">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.3" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 18 9 12 15 6"/></svg>
        </button>
        <button class="nav-icon-btn" onclick="send('FORWARD')" title="Forward">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.3" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6"/></svg>
        </button>
        <button class="nav-icon-btn" onclick="send('RELOAD')" title="Reload (Ctrl+R)">
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.3" stroke-linecap="round" stroke-linejoin="round"><polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/></svg>
        </button>
        <button class="nav-icon-btn" onclick="send('NAV:shunno://newtab')" title="Home">
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><polyline points="9 22 9 12 15 12 15 22"/></svg>
        </button>
      </div>

      <div class="firefox-capsule">
        <button class="engine-logo-btn" id="engine-logo-btn" onclick="toggleEngineMenu(event)" title="Switch Search Engine"></button>
        <input id="url-input" placeholder="Search or enter address..."
               onfocus="this.dataset.justFocused='1'; this.select();"
               onmouseup="if(this.dataset.justFocused==='1'){ event.preventDefault(); this.dataset.justFocused='0'; }"
               onkeydown="if(event.key==='Enter'){ event.preventDefault(); navigateFromBar(); }" />
        <span class="zoom-pill" id="zoom-badge" style="display:none" onclick="send('ZOOM_RESET')" title="Click to reset zoom">100%</span>
      </div>

      <div class="nav-group">
        <button class="nav-btn green" id="btn-streams" style="display:none" onclick="toggleDrawer('streams')" title="Detected Media">🎯 0</button>
        <button class="nav-btn on" id="btn-downloads" onclick="toggleDrawer('downloads')" title="Downloads (Ctrl+J)">⚡ 0</button>
        <button class="nav-btn" id="btn-controls" onclick="toggleDrawer('controls')" title="Controls &amp; Settings">•••</button>
      </div>
    </div>

    <!-- 3. Always-Visible Bookmarks & Library Strip Under Address Bar -->
    <div class="bookmarks-strip" id="bookmarks-strip"></div>
  </div>
  <div class="engine-menu" id="engine-menu"></div>
  <button class="fs-exit-pill" onclick="send('TOGGLE_FULLSCREEN')">Exit Full Screen (F11)</button>
  <div class="palette-backdrop" id="cmd-palette" onclick="if(event.target===this) closePalette()">
    <div class="palette-box">
      <input class="palette-input" id="palette-input" placeholder="Search tabs, bookmarks, history, or actions... (Esc to close)"
             oninput="renderPaletteItems()" onkeydown="handlePaletteKey(event)" />
      <div class="palette-list" id="palette-list"></div>
    </div>
  </div>

  <!-- 4. Clean Home Page Canvas (Displayed when active tab is shunno://newtab) -->
  <div class="home-canvas" id="home-canvas">
    <div class="home-hero">
      <svg width="58" height="58" viewBox="0 0 28 28" fill="none">
        <circle cx="14" cy="14" r="13" fill="url(#shunnoCompass)" stroke="#CBD5E1" stroke-width="1.2"/>
        <circle cx="14" cy="14" r="7.5" stroke="white" stroke-width="2.1"/>
        <polygon points="18.8,9.2 15.6,15.6 12.4,12.4" fill="#FF453A"/>
        <polygon points="9.2,18.8 15.6,15.6 12.4,12.4" fill="#F8FAFC"/>
        <circle cx="14" cy="14" r="1.3" fill="#1C1C1E"/>
      </svg>
      <div class="home-search-box">
        <span style="font-size:16px;opacity:0.65;">🔎</span>
        <input class="home-search-input" id="home-search-input" placeholder="Search the web or enter website address..."
               onkeydown="if(event.key==='Enter'){ event.preventDefault(); if(this.value.trim()) send('NAV:' + this.value.trim()); }" />
      </div>
      <div class="shortcuts-grid" id="home-shortcuts-grid"></div>
    </div>
  </div>

  <!-- 5. Translucent Frosted-Glass Floating Overlay (Floats over live webpage!) -->
  <div class="overlay-backdrop" id="drawer" onclick="if(event.target===this) closeDrawer()">
    <div class="floating-sheet" id="floating-sheet">

      <!-- Panel A: Controls & Settings -->
      <div id="panel-controls" style="display:none">
        <div class="card">
          <div class="card-header">
            <span class="card-title">Controls</span>
            <div style="display:flex;gap:5px;align-items:center;">
              <span class="meta" id="version-badge" style="font-weight:700;">v1.5.0</span>
              <button class="pill-btn pill-soft" onclick="send('ZOOM_OUT')" title="Zoom Out (Ctrl+-)">A−</button>
              <button class="pill-btn pill-soft" onclick="send('ZOOM_RESET')" title="Reset Zoom (Ctrl+0)">100%</button>
              <button class="pill-btn pill-soft" onclick="send('ZOOM_IN')" title="Zoom In (Ctrl++)">A+</button>
              <button class="pill-btn pill-soft" onclick="send('TOGGLE_FULLSCREEN')" title="Full Screen (F11)">⛶</button>
              <button class="pill-btn pill-red" onclick="closeDrawer()">✕</button>
            </div>
          </div>

          <div style="display:flex;gap:6px;flex-wrap:wrap;margin-bottom:10px;">
            <button class="pill-btn pill-soft" onclick="toggleDrawer('find')">🔍 Find</button>
            <button class="pill-btn pill-soft" onclick="toggleDrawer('bookmarks')">⭐ Library</button>
            <button class="pill-btn pill-soft" id="btn-reader-toggle" onclick="send('READER_MODE');">📖 Reader</button>
          <button class="pill-btn pill-soft" id="btn-split-toggle" onclick="send('TOGGLE_SPLIT_VIEW');" title="View 2 tabs side-by-side">◫ Split</button>
          <button class="pill-btn pill-soft" onclick="closeDrawer(); send('CAPTURE_SCREENSHOT');" title="Save PNG screenshot of current page">📸 Capture</button>
          <button class="pill-btn pill-soft" onclick="send('SLEEP_INACTIVE_TABS');" title="Hibernate idle background tabs to save RAM">💤 Sleep Tabs</button>
          <button class="pill-btn pill-soft" onclick="closeDrawer(); openPalette();" title="Command Palette (Ctrl+K)">⌘ Palette</button>
            <button class="pill-btn pill-soft" onclick="send('TRIGGER_PIP'); closeDrawer();">📺 PiP</button>
            <button class="pill-btn pill-soft" id="menu-speed-btn" onclick="cycleSpeed()">⏩ 1.0×</button>
            <button class="pill-btn pill-green" onclick="send('SAVE_OFFLINE_HTML'); closeDrawer();">💾 Save</button>
          </div>

          <div class="grid-toggles" id="global-toggles-grid"></div>
        </div>

        <div class="card">
          <div class="card-header">
            <span class="card-title" id="site-domain-title">🌐 Site Controls</span>
            <button class="pill-btn pill-orange" id="btn-clear-site-data" onclick="send('CLEAR_SITE_DATA')" title="Clear cookies, localStorage, sessionStorage & cache for this site">🧹 Clear Site Data</button>
          </div>
          <div class="grid-toggles" id="site-toggles-grid"></div>
        </div>

        <div class="card">
          <div class="card-header">
            <span class="card-title">📜 UserScripts (Explicit Approval)</span>
            <button class="pill-btn pill-soft" onclick="send('OPEN_SCRIPTS_FOLDER')">📂 Folder</button>
          </div>
          <div id="userscripts-list"></div>
        </div>

        <div class="card">
          <div class="card-header">
            <span class="card-title">🔎 Search Engines</span>
          </div>
          <div class="input-row">
            <input class="text-box" id="new-se-name" placeholder="Name" style="max-width:110px;" />
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
          <div class="input-row" style="margin-bottom:0;align-items:center;">
            <input class="text-box" id="find-input" placeholder="Search text..."
                   oninput="send('FIND:' + this.value)" onkeydown="if(event.key==='Enter') send(event.shiftKey ? 'FIND_PREV' : 'FIND_NEXT')" />
            <span class="find-badge" id="find-count-badge">0 / 0</span>
            <button class="pill-btn pill-soft" onclick="send('FIND_PREV')">↑</button>
            <button class="pill-btn pill-blue" onclick="send('FIND_NEXT')">↓</button>
          </div>
        </div>
      </div>

      <!-- Panel C: Detected Media & Grabbed Page Links -->
      <div id="panel-streams" style="display:none">
        <div class="card">
          <div class="card-header">
            <span class="card-title">🎯 Detected Media &amp; Page Links</span>
            <div style="display:flex;gap:5px;">
              <button class="pill-btn pill-soft" onclick="send('GRAB_PAGE_LINKS')">↻ Rescan</button>
              <button class="pill-btn pill-red" onclick="closeDrawer()">✕</button>
            </div>
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
              <button class="pill-btn pill-blue" onclick="promptSaveBookmarkWithFolder()">+ Save</button>
              <button class="pill-btn pill-soft" onclick="send('EXPORT_TXT_BOOKMARKS')">Export</button>
              <button class="pill-btn pill-soft" onclick="send('IMPORT_TXT_BOOKMARKS')">Import</button>
              <button class="pill-btn pill-red" onclick="closeDrawer()">✕</button>
            </div>
          </div>
          <div class="folder-pills" id="bookmark-folders-bar"></div>
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

      <!-- Panel E: Downloads Downloads -->
      <div id="panel-downloads" style="display:none">
        <div class="card">
          <div class="card-header">
            <span class="card-title">⚡ Downloads</span>
            <div style="display:flex;gap:5px;align-items:center;flex-wrap:wrap;">
              <button class="pill-btn pill-soft" id="btn-ffmpeg-setup" onclick="send('INSTALL_FFMPEG')" title="Auto-download & extract ffmpeg.exe">🎬 FFmpeg</button>
              <button class="pill-btn pill-soft" onclick="triggerGrabPage()" title="Site Grabber: Scan all links & media on active page">🕸️ Grab</button>
              <button class="pill-btn pill-soft" onclick="send('CLEAR_COMPLETED_DL')" title="Clean completed, failed & missing files">🧹 Clean</button>
              <button class="pill-btn pill-soft" onclick="send('EXPORT_DL_CSV')" title="Export download history with SHA-256 checksums to CSV">📄 CSV</button>
              <button class="pill-btn pill-soft" onclick="send('OPEN_DOWNLOADS_FOLDER')" title="Open Downloads Folder">📂</button>
              <button class="pill-btn pill-red" onclick="closeDrawer()">✕</button>
            </div>
          </div>
          <div class="speed-graph-box">
            <div class="speed-graph-hdr">
              <span style="color:var(--blue);" id="dl-live-speed">⚡ 0 B/s</span>
              <span class="meta" id="dl-peak-speed">Peak: 0 B/s</span>
            </div>
            <svg id="dl-speed-svg" width="100%" height="36" viewBox="0 0 360 36" preserveAspectRatio="none" style="display:block;overflow:visible;">
              <defs>
                <linearGradient id="dlSpeedGrad" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stop-color="#0A84FF" stop-opacity="0.34"/>
                  <stop offset="100%" stop-color="#0A84FF" stop-opacity="0.02"/>
                </linearGradient>
              </defs>
              <polygon id="dl-speed-area" fill="url(#dlSpeedGrad)" points="0,36 360,36"/>
              <polyline id="dl-speed-line" fill="none" stroke="#0A84FF" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" points="0,35 360,35"/>
            </svg>
          </div>
          <div class="input-row">
            <input class="text-box" id="dl-url-input" placeholder="Paste link or track_[01-12].flac..." />
            <select class="select-box" id="dl-threads-select" title="Parallel Connections">
              <option value="1">1T</option>
              <option value="4">4T</option>
              <option value="8">8T</option>
              <option value="16" selected>16T</option>
              <option value="32">32T</option>
            </select>
            <select class="select-box" id="dl-delay-select" title="Schedule Start">
              <option value="0">Now</option>
              <option value="60">In 1m</option>
              <option value="300">In 5m</option>
              <option value="900">In 15m</option>
            </select>
            <button class="pill-btn pill-blue" onclick="startManualDownload()">⚡ Get</button>
          </div>
          <div class="input-row">
            <select class="select-box" id="max-conc-select" onchange="send('SET_MAX_CONCURRENT:' + this.value)" title="Sequential Queue Limit">
              <option value="1">Queue: 1 at a time</option>
              <option value="2">Queue: Max 2</option>
              <option value="3" selected>Queue: Max 3</option>
              <option value="4">Queue: Max 4</option>
            </select>
            <select class="select-box" id="speed-limit-select" onchange="send('SET_SPEED_LIMIT:' + this.value)" title="Bandwidth Limit">
              <option value="0">Speed: Unlimited</option>
              <option value="10485760">Speed: 10 MB/s</option>
              <option value="5242880">Speed: 5 MB/s</option>
              <option value="2097152">Speed: 2 MB/s</option>
              <option value="524288">Speed: 512 KB/s</option>
            </select>
            <select class="select-box" id="on-complete-select" onchange="send('SET_ON_COMPLETE:' + this.value)" title="Action when queue finishes">
              <option value="NONE">Done: Keep Open</option>
              <option value="OPEN_FOLDER">Done: Open Folder</option>
              <option value="EXIT_APP">Done: Exit Browser</option>
            </select>
          </div>
          <div class="input-row">
            <input class="text-box" id="mux-video-input" placeholder="DASH Video URL..." />
            <input class="text-box" id="mux-audio-input" placeholder="DASH Audio URL..." />
            <button class="pill-btn pill-green" onclick="startDashMux()">🎬 Mux</button>
          </div>
          <div class="input-row" style="margin-bottom:0;">
            <input class="text-box" id="custom-dl-dir-input" placeholder="Custom Downloads Folder (e.g., D:\Downloads)..." />
            <button class="pill-btn pill-soft" onclick="applyCustomDlDir()">Set</button>
            <button class="pill-btn pill-soft" onclick="send('SET_DOWNLOAD_DIR:'); document.getElementById('custom-dl-dir-input').value='';" title="Reset to default Downloads\ShunnoBrowser">Reset</button>
          </div>
        </div>

        <div class="folder-pills" id="dl-category-bar"></div>
        <div id="duplicate-banner" class="card" style="display:none;border-color:var(--orange);"></div>
        <div id="downloads-list"></div>
      </div>

    </div>
  </div>

<script>
  let activeDrawer = null;
  let currentSpeed = 1.0;
  const speeds = [1.0, 1.25, 1.5, 2.0, 3.0, 4.0, 0.75];
  let knownTabIds = new Set();
  let draggedTabIdx = null;
  let activeBookmarkFolder = 'All';
  let activeDlCategory = 'All';
  const speedHistory = new Array(36).fill(0);
  let peakSpeedBps = 0;

  function promptSaveBookmarkWithFolder() {
    const folder = prompt('Save bookmark to folder:', activeBookmarkFolder === 'All' ? 'General' : activeBookmarkFolder);
    if (folder === null) return;
    send('ADD_BOOKMARK:' + (folder.trim() || 'General'));
  }

  function send(msg) {
    if (window.ipc) window.ipc.postMessage(msg);
  }

  function navigateFromBar() {
    const input = document.getElementById('url-input');
    const val = input.value.trim();
    if (!val) return;
    input.blur();
    if (activeDrawer) closeDrawer();
    send('NAV:' + val);
  }

  function triggerGrabPage() {
    toggleDrawer('streams');
    send('GRAB_PAGE_LINKS');
  }

  function toggleDrawer(name) {
    if (activeDrawer === name) {
      closeDrawer();
      return;
    }
    const drawer = document.getElementById('drawer');
    const sheet = document.getElementById('floating-sheet');
    activeDrawer = name;

    if (name === 'downloads' || name === 'bookmarks') {
      sheet.classList.add('wide');
    } else {
      sheet.classList.remove('wide');
    }

    ['controls', 'find', 'streams', 'bookmarks', 'downloads'].forEach(p => {
      document.getElementById('panel-' + p).style.display = (p === name) ? 'block' : 'none';
    });

    sheet.style.animation = 'none';
    void sheet.offsetWidth;
    sheet.style.animation = '';
    drawer.classList.add('open');
    send('DRAWER_OPEN:' + name);
  }

  function closeDrawer() {
    activeDrawer = null;
    document.getElementById('drawer').classList.remove('open');
    send('DRAWER_CLOSE');
  }

  function cycleSpeed() {
    const idx = (speeds.indexOf(currentSpeed) + 1) % speeds.length;
    currentSpeed = speeds[idx];
    document.getElementById('menu-speed-btn').textContent = '⏩ ' + currentSpeed + '×';
    send('SET_PLAYBACK_SPEED:' + currentSpeed);
  }

  function applyCustomDlDir() {
    const inp = document.getElementById('custom-dl-dir-input');
    if (!inp) return;
    send('SET_DOWNLOAD_DIR:' + inp.value.trim());
  }

  function startManualDownload() {
    const input = document.getElementById('dl-url-input');
    const threads = document.getElementById('dl-threads-select').value;
    const delay = document.getElementById('dl-delay-select').value;
    if (!input.value.trim()) return;
    send('ENQUEUE_SCHED:' + threads + '|' + delay + '|' + input.value.trim());
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

  function promptAddShortcut() {
    const title = prompt('Shortcut Name (e.g. Gmail):');
    if (!title || !title.trim()) return;
    const url = prompt('Website URL (e.g. https://mail.google.com):');
    if (!url || !url.trim()) return;
    send('ADD_SHORTCUT:' + title.trim() + '|' + url.trim());
  }

  function fmtBytes(b) {
    if (!b || b <= 0) return '0 B';
    const k = b / 1024, m = k / 1024, g = m / 1024;
    if (g >= 1) return g.toFixed(2) + ' GB';
    if (m >= 1) return m.toFixed(1) + ' MB';
    if (k >= 1) return k.toFixed(0) + ' KB';
    return b + ' B';
  }

  // Flicker-free incremental tab reconciliation (never blinks or re-animates existing tabs!)
  function syncTabsStatic(tabs, activeIdx) {
    const tc = document.getElementById('tabs-container');
    const currentIds = new Set(tabs.map(t => String(t.id)));

    // Remove closed tabs
    Array.from(tc.children).forEach(el => {
      if (!currentIds.has(el.dataset.tabId)) {
        tc.removeChild(el);
        knownTabIds.delete(el.dataset.tabId);
      }
    });

    tabs.forEach((t, idx) => {
      const idStr = String(t.id);
      let el = tc.querySelector(`.tab[data-tab-id="${idStr}"]`);
      if (!el) {
        el = document.createElement('div');
        el.dataset.tabId = idStr;
        el.className = 'tab is-new';
        setTimeout(() => el.classList.remove('is-new'), 260);
        tc.appendChild(el);
        knownTabIds.add(idStr);
      }

      const wantClass = `tab ${idx === activeIdx ? 'active' : ''} ${t.incognito ? 'incognito' : ''} ${t.pinned ? 'pinned' : ''} ${t.sleeping ? 'sleeping' : ''} ${idx === window.__shunnoSplitTabIdx ? 'split-peer' : ''}`;
      if (el.className !== wantClass && !el.classList.contains('is-new')) {
        el.className = wantClass;
      } else if (el.classList.contains('is-new')) {
        el.classList.toggle('active', idx === activeIdx);
        el.classList.toggle('incognito', !!t.incognito);
      }

      el.draggable = true;
      el.ondragstart = (ev) => {
        draggedTabIdx = idx;
        if (ev.dataTransfer) {
          ev.dataTransfer.effectAllowed = 'move';
          ev.dataTransfer.setData('text/plain', String(idx));
        }
      };
      el.ondragover = (ev) => {
        ev.preventDefault();
        if (draggedTabIdx === null || draggedTabIdx === idx) return;
        const rect = el.getBoundingClientRect();
        const isRight = (ev.clientX - rect.left) > (rect.width / 2);
        el.classList.toggle('drag-over-right', isRight);
        el.classList.toggle('drag-over-left', !isRight);
      };
      el.ondragleave = () => {
        el.classList.remove('drag-over-left', 'drag-over-right');
      };
      el.ondrop = (ev) => {
        ev.preventDefault();
        el.classList.remove('drag-over-left', 'drag-over-right');
        if (draggedTabIdx !== null && draggedTabIdx !== idx) {
          send('REORDER_TAB:' + draggedTabIdx + '|' + idx);
        }
        draggedTabIdx = null;
      };
      el.ondragend = () => {
        draggedTabIdx = null;
        el.classList.remove('drag-over-left', 'drag-over-right');
      };
      el.onclick = () => send('SWITCH_TAB:' + idx);
      el.onauxclick = (ev) => { if (ev.button === 1) { ev.preventDefault(); ev.stopPropagation(); send('CLOSE_TAB:' + idx); } };
      el.oncontextmenu = (ev) => { ev.preventDefault(); send('TOGGLE_PIN_TAB:' + idx); };
      el.title = t.title || 'New Tab';

      const displayTitle = (t.sleeping ? '💤 ' : '') + (t.incognito ? '🕵 ' : '') + (t.title || 'New Tab');
      const hasAudio = !!(t.playing_audio || t.muted);
      const audioKey = hasAudio ? (t.muted ? 'M' : 'P') : 'N';

      if (el.dataset.renderedTitle !== displayTitle || el.dataset.audioKey !== audioKey || el.dataset.tabIdx !== String(idx)) {
        el.dataset.renderedTitle = displayTitle;
        el.dataset.audioKey = audioKey;
        el.dataset.tabIdx = String(idx);
        const isNewTab = t.url === 'shunno://newtab';
        const tabLogoHtml = (!hasAudio && (isNewTab || t.pinned))
          ? `<span style="display:inline-flex;align-items:center;flex-shrink:0;" title="${t.pinned ? 'Pinned (Right-click to unpin)' : ''}"><svg width="18" height="18" viewBox="0 0 28 28" fill="none"><circle cx="14" cy="14" r="13" fill="url(#shunnoCompass)"/><circle cx="14" cy="14" r="7.5" stroke="white" stroke-width="2.2"/><polygon points="18.8,9.2 15.6,15.6 12.4,12.4" fill="#FF453A"/><polygon points="9.2,18.8 15.6,15.6 12.4,12.4" fill="#F8FAFC"/></svg></span>`
          : '';
        const audioHtml = hasAudio
          ? `<span class="tab-audio" onclick="event.stopPropagation(); send('TOGGLE_MUTE_TAB:${idx}')" title="${t.muted ? 'Unmute' : 'Mute'}">${t.muted ? '🔇' : '🔊'}</span>`
          : '';
        el.innerHTML = `${tabLogoHtml}${audioHtml}<span class="tab-title">${escapeHtml(displayTitle)}</span><span class="tab-close" onclick="event.stopPropagation(); send('CLOSE_TAB:${idx}')">✕</span>`;
      }
    });
  }

  window.setOverlayBackdropImage = function(dataUrl) {
    const el = document.getElementById('drawer');
    if (dataUrl) {
      el.style.backgroundImage = 'url(' + dataUrl + ')';
    } else {
      el.style.backgroundImage = 'none';
    }
  };

  window.__shunnoLastState = null;
  window.__shunnoSplitTabIdx = -1;
  let paletteItems = [];
  let paletteSel = 0;

  window.openPalette = function() {
    const el = document.getElementById('cmd-palette');
    const inp = document.getElementById('palette-input');
    if (!el || !inp) return;
    el.classList.add('open');
    inp.value = '';
    paletteSel = 0;
    renderPaletteItems();
    send('PALETTE:1');
    setTimeout(() => inp.focus(), 20);
  };

  window.closePalette = function() {
    const el = document.getElementById('cmd-palette');
    if (el && el.classList.contains('open')) {
      el.classList.remove('open');
      send('PALETTE:0');
    }
  };

  function renderPaletteItems() {
    const inp = document.getElementById('palette-input');
    const listEl = document.getElementById('palette-list');
    if (!inp || !listEl) return;
    const q = inp.value.trim().toLowerCase();
    const st = window.__shunnoLastState;
    const items = [];

    const actions = [
      { kind: 'Action', title: 'Toggle Split-View (50/50 Dual Pane)', run: () => send('TOGGLE_SPLIT_VIEW') },
      { kind: 'Action', title: 'Capture Page Screenshot (PNG)', run: () => send('CAPTURE_SCREENSHOT') },
      { kind: 'Action', title: 'Duplicate Current Tab', run: () => send('DUPLICATE_TAB') },
      { kind: 'Action', title: 'Export Download History to CSV (with SHA-256)', run: () => send('EXPORT_DL_CSV') },
      { kind: 'Action', title: 'Sleep Inactive Background Tabs (Save RAM)', run: () => send('SLEEP_INACTIVE_TABS') },
      { kind: 'Action', title: 'Toggle Dark Mode', run: () => send('TOGGLE_GLOBAL:full_dark') },
      { kind: 'Action', title: 'Toggle Reader Mode', run: () => send('READER_MODE') },
      { kind: 'Action', title: 'Clear Current Site Cookies & Storage', run: () => send('CLEAR_SITE_DATA') },
      { kind: 'Action', title: 'Open Downloads Manager (Ctrl+J)', run: () => toggleDrawer('downloads') },
      { kind: 'Action', title: 'New Private Tab', run: () => send('NEW_INCOGNITO') },
      { kind: 'Action', title: 'Toggle Fullscreen (F11)', run: () => send('TOGGLE_FULLSCREEN') },
    ];
    actions.forEach(a => items.push(a));

    if (st) {
      (st.tabs || []).forEach((t, idx) => {
        items.push({
          kind: 'Tab',
          title: (t.title || 'New Tab') + (t.url && t.url !== 'shunno://newtab' ? (' — ' + t.url) : ''),
          run: () => send('SWITCH_TAB:' + idx),
        });
      });
      (st.bookmarks || []).slice(0, 25).forEach(b => {
        items.push({
          kind: 'Bookmark',
          title: b.title + ' — ' + b.url,
          run: () => send('NAV:' + b.url),
        });
      });
      (st.history || []).slice(0, 20).forEach(h => {
        items.push({
          kind: 'History',
          title: h.title + ' — ' + h.url,
          run: () => send('NAV:' + h.url),
        });
      });
    }

    paletteItems = q ? items.filter(it => it.title.toLowerCase().includes(q) || it.kind.toLowerCase().includes(q)) : items.slice(0, 18);
    if (paletteSel >= paletteItems.length) paletteSel = 0;

    listEl.innerHTML = paletteItems.length === 0
      ? '<div class="meta" style="padding:8px;">No matching commands, tabs, or bookmarks.</div>'
      : paletteItems.map((it, i) => `
          <button class="palette-item ${i === paletteSel ? 'sel' : ''}" onclick="execPaletteItem(${i})">
            <span style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap;flex:1;">${escapeHtml(it.title)}</span>
            <span class="palette-kind">${escapeHtml(it.kind)}</span>
          </button>
        `).join('');
  }

  function execPaletteItem(i) {
    const it = paletteItems[i];
    closePalette();
    if (it && it.run) it.run();
  }

  function handlePaletteKey(e) {
    if (e.key === 'Escape') {
      e.preventDefault();
      closePalette();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (paletteItems.length > 0) {
        paletteSel = (paletteSel + 1) % paletteItems.length;
        renderPaletteItems();
      }
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (paletteItems.length > 0) {
        paletteSel = paletteSel <= 0 ? paletteItems.length - 1 : paletteSel - 1;
        renderPaletteItems();
      }
    } else if (e.key === 'Enter') {
      e.preventDefault();
      execPaletteItem(paletteSel);
    }
  }

  window.renderChromeState = function(state) {
    window.__shunnoLastState = state;
    window.__shunnoSplitTabIdx = (typeof state.split_tab === 'number') ? state.split_tab : -1;
    const spBtn = document.getElementById('btn-split-toggle');
    if (spBtn) {
      const splitOn = window.__shunnoSplitTabIdx >= 0;
      spBtn.className = 'pill-btn ' + (splitOn ? 'pill-blue' : 'pill-soft');
      spBtn.textContent = splitOn ? '◫ Split: ON' : '◫ Split';
    }
    if (state.global.full_dark_mode) {
      document.body.classList.add('dark');
    } else {
      document.body.classList.remove('dark');
    }

    document.getElementById('version-badge').textContent = state.version || 'v2.6.0';
    if (state.is_fullscreen) {
      document.body.classList.add('fullscreen');
    } else {
      document.body.classList.remove('fullscreen');
    }

    const reopenBtn = document.getElementById('btn-reopen-tab');
    if (reopenBtn) {
      reopenBtn.style.display = state.has_closed_tabs ? 'inline-flex' : 'none';
    }

    const ffBtn = document.getElementById('btn-ffmpeg-setup');
    if (ffBtn) {
      if (state.ffmpeg_available) {
        ffBtn.textContent = '🎬 FFmpeg: Ready';
        ffBtn.className = 'pill-btn pill-green';
      } else if (state.ffmpeg_status) {
        ffBtn.textContent = '🎬 ' + state.ffmpeg_status;
      } else {
        ffBtn.textContent = '⬇ Install FFmpeg';
      }
    }
    const maxConcEl = document.getElementById('max-conc-select');
    if (maxConcEl) maxConcEl.value = String(state.global.max_concurrent_downloads || 3);
    const onCompEl = document.getElementById('on-complete-select');
    if (onCompEl) onCompEl.value = state.global.on_queue_complete || 'NONE';
    const dlDirInp = document.getElementById('custom-dl-dir-input');
    if (dlDirInp && document.activeElement !== dlDirInp) {
      dlDirInp.value = state.global.custom_downloads_dir || '';
    }

    const findBadge = document.getElementById('find-count-badge');
    if (findBadge) {
      findBadge.textContent = (state.find_current || 0) + ' / ' + (state.find_total || 0);
    }

    // Static, flicker-free tab updates
    syncTabsStatic(state.tabs, state.active_tab);

    // Zoom pill indicator
    const zoomBadge = document.getElementById('zoom-badge');
    const zPct = Math.round((state.active_zoom || 1.0) * 100);
    if (zPct !== 100) {
      zoomBadge.style.display = 'inline-block';
      zoomBadge.textContent = zPct + '%';
    } else {
      zoomBadge.style.display = 'none';
    }

    // Home page canvas visibility when on shunno://newtab
    const isHome = state.active_url === 'shunno://newtab';
    const homeCanvas = document.getElementById('home-canvas');
    if (isHome) {
      homeCanvas.classList.add('active');
      const grid = document.getElementById('home-shortcuts-grid');
      const shortcuts = state.global.quick_shortcuts || [];
      grid.innerHTML = shortcuts.map((sc, i) => `
        <div class="shortcut-tile" onclick="send('NAV:${encodeAttr(sc.url)}')">
          <button class="shortcut-del" onclick="event.stopPropagation(); send('DEL_SHORTCUT:${i}')" title="Remove">✕</button>
          <div class="shortcut-icon">${escapeHtml((sc.title || 'W').charAt(0).toUpperCase())}</div>
          <div class="shortcut-label">${escapeHtml(sc.title)}</div>
        </div>
      `).join('') + `
        <div class="shortcut-tile" onclick="promptAddShortcut()" style="border-style:dashed;">
          <div class="shortcut-icon" style="background:rgba(128,128,128,0.14);color:var(--text);">+</div>
          <div class="shortcut-label">Add Shortcut</div>
        </div>
      `;
    } else {
      homeCanvas.classList.remove('active');
    }

    // Bookmarks & Folder Grouping Bar under address bar
    const allFolders = ['All'].concat(Array.from(new Set(state.bookmarks.map(b => b.folder || 'General'))));
    if (!allFolders.includes(activeBookmarkFolder)) activeBookmarkFolder = 'All';

    const bmStrip = document.getElementById('bookmarks-strip');
    const visibleBarBms = state.bookmarks
      .map((b, i) => ({ b, i }))
      .filter(({ b }) => activeBookmarkFolder === 'All' || (b.folder || 'General') === activeBookmarkFolder);

    bmStrip.innerHTML = `
      <button class="bm-chip action" onclick="toggleDrawer('bookmarks')" title="Open Library">📚 Library</button>
      <button class="bm-chip action" onclick="promptSaveBookmarkWithFolder()" title="Bookmark Current Page" style="padding:3px 7px;">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.3" stroke-linecap="round" stroke-linejoin="round">
          <path d="M19 21l-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z"></path>
        </svg>
      </button>
    ` + allFolders.filter(f => f !== 'All').map(f => `
      <button class="bm-chip ${activeBookmarkFolder === f ? 'action' : ''}"
              onclick="activeBookmarkFolder = (activeBookmarkFolder === '${encodeAttr(f)}' ? 'All' : '${encodeAttr(f)}'); send('REFRESH_UI');"
              title="Filter folder: ${escapeHtml(f)}">📁 ${escapeHtml(f)}</button>
    `).join('') + visibleBarBms.slice(0, 12).map(({ b, i }) => `
      <button class="bm-chip" onclick="send('NAV:${encodeAttr(b.url)}')"
              oncontextmenu="event.preventDefault(); const nt=prompt('Rename bookmark (leave empty to remove):','${encodeAttr(b.title)}'); if(nt===null) return; if(nt.trim()===''){send('DEL_BOOKMARK:${i}');}else{send('RENAME_BOOKMARK:${i}|'+nt.trim());}"
              title="[${escapeHtml(b.folder || 'General')}] ${escapeHtml(b.url)} (Right-click to rename/remove)">⭐ ${escapeHtml(b.title)}</button>
    `).join('');

    const seBtn = document.getElementById('engine-logo-btn');
    const seMenu = document.getElementById('engine-menu');
    if (state.global.search_engines && state.global.search_engines.length > 0) {
      const activeSe = state.global.search_engines[state.global.active_search_engine] || state.global.search_engines[0];
      seBtn.innerHTML = getEngineLogoSvg(activeSe.name);
      seBtn.title = 'Search Engine: ' + activeSe.name;
      seMenu.innerHTML = state.global.search_engines.map((se, i) => `
        <button class="engine-opt ${i === state.global.active_search_engine ? 'active' : ''}"
                onclick="send('SET_SEARCH_ENGINE:${i}'); document.getElementById('engine-menu').classList.remove('open');">
          ${getEngineLogoSvg(se.name)}
          <span>${escapeHtml(se.name)}</span>
        </button>
      `).join('');
    }

    const urlBox = document.getElementById('url-input');
    if (document.activeElement !== urlBox) {
      urlBox.value = isHome ? '' : (state.active_url || '');
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

    const dupBanner = document.getElementById('duplicate-banner');
    if (state.duplicate_alert) {
      dupBanner.style.display = 'block';
      dupBanner.innerHTML = `
        <div style="display:flex;justify-content:space-between;align-items:center;gap:8px;flex-wrap:wrap;">
          <div>
            <div style="font-size:12.5px;font-weight:700;color:var(--orange);">⚠️ Duplicate File / URL Detected</div>
            <div class="meta">${escapeHtml(state.duplicate_alert.file_name)}</div>
          </div>
          <div style="display:flex;gap:5px;">
            <button class="pill-btn pill-soft" onclick="send('DISMISS_DUP');">Skip</button>
            <button class="pill-btn pill-blue" onclick="send('DUP_RENAME:${encodeAttr(state.duplicate_alert.url)}');">Rename</button>
            <button class="pill-btn pill-orange" onclick="send('DUP_OVERWRITE:${encodeAttr(state.duplicate_alert.url)}');">Overwrite</button>
          </div>
        </div>
      `;
    } else {
      dupBanner.style.display = 'none';
    }

    document.getElementById('streams-list').innerHTML = state.streams.length === 0
      ? '<div class="meta">No direct media or file links detected on this page yet.</div>'
      : state.streams.map(s => `
          <div class="list-row">
            <div style="overflow:hidden;">
              <div style="font-size:12px;font-weight:600;">${escapeHtml(s.name)} <span class="pill-btn pill-green" style="padding:1px 5px;font-size:10.5px;">${escapeHtml(s.size_label)}</span></div>
              <div class="meta" style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">${escapeHtml(s.url)}</div>
            </div>
            <button class="pill-btn pill-blue" onclick="send('ENQUEUE_DL:16|${encodeAttr(s.url)}'); toggleDrawer('downloads');">⚡</button>
          </div>
        `).join('');

    const rBtn = document.getElementById('btn-reader-toggle');
    if (rBtn) {
      rBtn.className = 'pill-btn ' + (state.active_reader ? 'pill-blue' : 'pill-soft');
      rBtn.textContent = state.active_reader ? '📖 Reader: ON' : '📖 Reader';
    }

    const globalItems = [
      ['full_dark', '🌙 Dark Mode', state.global.full_dark_mode],
      ['drop_basket', '🧺 Drop Basket', state.global.drop_basket_enabled],
      ['bg_audio', '🎧 Background Play', state.global.background_media_play],
      ['auto_pip', '📺 Auto-PiP on Blur', state.global.auto_pip_on_blur],
      ['auto_clean', '🧹 Auto-Clean Done', state.global.auto_clean_completed],
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

    document.getElementById('userscripts-list').innerHTML = (!state.userscripts || state.userscripts.length === 0)
      ? '<div class="meta">No .js scripts in %LOCALAPPDATA%\\ShunnoBrowser\\scripts\\</div>'
      : state.userscripts.map(us => `
          <div class="list-row">
            <div style="overflow:hidden;">
              <div style="font-size:12px;font-weight:600;">${escapeHtml(us.file_name)}</div>
              <div class="meta">@match ${escapeHtml(us.match_pattern)} • SHA: ${us.sha256.slice(0, 10)}...</div>
            </div>
            <button class="pill-btn ${us.approved ? 'pill-green' : 'pill-soft'}" onclick="send('TOGGLE_SCRIPT:${us.sha256}')">
              ${us.approved ? 'Approved' : 'Approve'}
            </button>
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

    const folderBar = document.getElementById('bookmark-folders-bar');
    folderBar.innerHTML = allFolders.map(f => `
      <button class="folder-pill ${activeBookmarkFolder === f ? 'active' : ''}"
              onclick="activeBookmarkFolder = '${encodeAttr(f)}'; send('REFRESH_UI');">${f === 'All' ? 'All Folders' : '📁 ' + escapeHtml(f)}</button>
    `).join('');

    const filteredBms = state.bookmarks
      .map((b, i) => ({ b, i }))
      .filter(({ b }) => activeBookmarkFolder === 'All' || (b.folder || 'General') === activeBookmarkFolder);

    document.getElementById('bookmarks-list').innerHTML = filteredBms.length === 0
      ? '<div class="meta">No bookmarks in this folder.</div>'
      : filteredBms.map(({ b, i }) => `
          <div class="list-row">
            <div style="overflow:hidden;">
              <div style="font-size:12.5px;font-weight:600;">${escapeHtml(b.title)} <span class="pill-btn pill-soft" style="padding:1px 6px;font-size:10px;">📁 ${escapeHtml(b.folder || 'General')}</span></div>
              <div class="meta" style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">${escapeHtml(b.url)}</div>
            </div>
            <div style="display:flex;gap:4px;flex-shrink:0;">
              <button class="pill-btn pill-soft" onclick="send('NAV:${encodeAttr(b.url)}'); closeDrawer();">Open</button>
              <button class="pill-btn pill-soft" onclick="const nf=prompt('Move to folder:','${encodeAttr(b.folder || 'General')}'); if(nf && nf.trim()) send('SET_BOOKMARK_FOLDER:${i}|'+nf.trim());" title="Move Folder">📁</button>
              <button class="pill-btn pill-soft" onclick="const nt=prompt('Rename bookmark:','${encodeAttr(b.title)}'); if(nt && nt.trim()) send('RENAME_BOOKMARK:${i}|'+nt.trim());" title="Rename">✎</button>
              <button class="pill-btn pill-red" onclick="send('DEL_BOOKMARK:${i}')" title="Remove">✕</button>
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

    // Update live download speed sparkline
    const totalSpeed = state.downloads
      .filter(d => d.status === 'DOWNLOADING')
      .reduce((acc, d) => acc + (d.speed_bps || 0), 0);
    if (totalSpeed > peakSpeedBps) peakSpeedBps = totalSpeed;
    speedHistory.push(totalSpeed);
    if (speedHistory.length > 36) speedHistory.shift();

    const liveSpdEl = document.getElementById('dl-live-speed');
    const peakSpdEl = document.getElementById('dl-peak-speed');
    if (liveSpdEl) liveSpdEl.textContent = '⚡ ' + fmtBytes(totalSpeed) + '/s';
    if (peakSpdEl) peakSpdEl.textContent = 'Peak: ' + fmtBytes(peakSpeedBps) + '/s';

    const polyLine = document.getElementById('dl-speed-line');
    const polyArea = document.getElementById('dl-speed-area');
    if (polyLine && polyArea) {
      const maxVal = Math.max(10240, ...speedHistory);
      const pts = speedHistory.map((v, i) => {
        const x = (i / (speedHistory.length - 1)) * 360;
        const y = 34 - ((v / maxVal) * 30);
        return x.toFixed(1) + ',' + y.toFixed(1);
      }).join(' ');
      polyLine.setAttribute('points', pts);
      polyArea.setAttribute('points', '0,36 ' + pts + ' 360,36');
    }

    const clearSiteBtn = document.getElementById('btn-clear-site-data');
    if (clearSiteBtn) {
      if (state.site_data_cleared) {
        clearSiteBtn.textContent = '✓ Cleared';
        clearSiteBtn.className = 'pill-btn pill-green';
      } else {
        clearSiteBtn.textContent = '🧹 Clear Site Data';
        clearSiteBtn.className = 'pill-btn pill-orange';
      }
    }

    const dlCats = ['All', 'Videos', 'Lossless Audio', 'Archives', 'Programs', 'Documents'];
    document.getElementById('dl-category-bar').innerHTML = dlCats.map(c => `
      <button class="folder-pill ${activeDlCategory === c ? 'active' : ''}"
              onclick="activeDlCategory = '${c}'; send('REFRESH_UI');">${c}</button>
    `).join('');

    const visibleDownloads = state.downloads.filter(d => activeDlCategory === 'All' || d.category === activeDlCategory);
    document.getElementById('downloads-list').innerHTML = visibleDownloads.length === 0
      ? '<div class="card"><div class="meta">No downloads yet. Hover over any video/file or click 🕸️ Grab to scan all downloadable files on the active tab.</div></div>'
      : visibleDownloads.map(d => {
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

  function getEngineLogoSvg(name) {
    const n = String(name || '').toLowerCase();
    if (n.includes('google')) {
      return '<svg width="15" height="15" viewBox="0 0 24 24"><path fill="#4285F4" d="M23.5 12.3c0-.8-.1-1.6-.2-2.3H12v4.5h6.5c-.3 1.5-1.1 2.8-2.4 3.6v3h3.9c2.3-2.1 3.5-5.2 3.5-8.8z"/><path fill="#34A853" d="M12 24c3.2 0 6-1.1 8-3l-3.9-3c-1.1.7-2.5 1.2-4.1 1.2-3.2 0-5.9-2.1-6.8-5H1.2v3.1C3.2 21.3 7.3 24 12 24z"/><path fill="#FBBC05" d="M5.2 14.2c-.2-.7-.4-1.4-.4-2.2s.2-1.5.4-2.2V6.7H1.2C.4 8.3 0 10.1 0 12s.4 3.7 1.2 5.3l4-3.1z"/><path fill="#EA4335" d="M12 4.8c1.8 0 3.3.6 4.6 1.8l3.4-3.4C18 1.2 15.2 0 12 0 7.3 0 3.2 2.7 1.2 6.7l4 3.1c.9-2.9 3.6-5 6.8-5z"/></svg>';
    }
    if (n.includes('duck')) {
      return '<svg width="15" height="15" viewBox="0 0 24 24"><circle cx="12" cy="12" r="11" fill="#DE5833"/><circle cx="12" cy="12" r="6" fill="#FFF"/><circle cx="13.5" cy="10.5" r="1.5" fill="#1C1C1E"/><path d="M15 13c2 0 3.5.8 3.5 1.8S17 16.5 15 16.5s-3-.7-3-1.7S13 13 15 13z" fill="#FBB03B"/></svg>';
    }
    if (n.includes('bing')) {
      return '<svg width="15" height="15" viewBox="0 0 24 24"><path fill="#008373" d="M5 3v16.5l4.2 2.5 9.8-5.6v-4.6l-6.8-2.4 2.1 5.1-4.3 2.5V5.4L5 3z"/></svg>';
    }
    if (n.includes('youtube')) {
      return '<svg width="15" height="15" viewBox="0 0 24 24"><rect x="1" y="4" width="22" height="16" rx="4" fill="#FF0000"/><polygon points="10,8.5 16,12 10,15.5" fill="#FFF"/></svg>';
    }
    if (n.includes('startpage')) {
      return '<svg width="15" height="15" viewBox="0 0 24 24"><circle cx="12" cy="12" r="10" fill="#6573FF"/><circle cx="11" cy="11" r="4" fill="none" stroke="#FFF" stroke-width="2.2"/><line x1="14" y1="14" x2="17.5" y2="17.5" stroke="#FFF" stroke-width="2.2" stroke-linecap="round"/></svg>';
    }
    const letter = escapeHtml((name || 'S').charAt(0).toUpperCase());
    return `<span style="display:inline-flex;align-items:center;justify-content:center;width:15px;height:15px;border-radius:4px;background:var(--blue);color:#fff;font-size:10px;font-weight:800;">${letter}</span>`;
  }

  function toggleEngineMenu(e) {
    e.stopPropagation();
    const menu = document.getElementById('engine-menu');
    const btn = document.getElementById('engine-logo-btn');
    const willOpen = !menu.classList.contains('open');
    if (willOpen && btn) {
      const r = btn.getBoundingClientRect();
      menu.style.left = Math.max(8, r.left) + 'px';
      menu.style.transform = 'none';
      menu.classList.add('open');
      send('ENGINE_MENU:1');
    } else {
      menu.classList.remove('open');
      send('ENGINE_MENU:0');
    }
  }
  document.addEventListener('click', () => {
    const menu = document.getElementById('engine-menu');
    if (menu && menu.classList.contains('open')) {
      menu.classList.remove('open');
      send('ENGINE_MENU:0');
    }
  });

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
    } else if (e.ctrlKey && e.key === 'Tab') {
      e.preventDefault();
      send(e.shiftKey ? 'PREV_TAB' : 'NEXT_TAB');
    } else if (e.ctrlKey && e.shiftKey && !e.altKey && e.key.toLowerCase() === 't') {
      e.preventDefault();
      send('REOPEN_CLOSED_TAB');
    } else if (e.ctrlKey && !e.shiftKey && !e.altKey) {
      const k = e.key.toLowerCase();
      if (k === 't') { e.preventDefault(); send('NEW_TAB'); }
      else if (k === 'k') { e.preventDefault(); openPalette(); }
      else if (k === 'w') { e.preventDefault(); send('CLOSE_ACTIVE_TAB'); }
      else if (k === 'l') { e.preventDefault(); const u = document.getElementById('url-input'); u.focus(); u.select(); }
      else if (k === 'f') { e.preventDefault(); toggleDrawer('find'); setTimeout(() => document.getElementById('find-input').focus(), 50); }
      else if (k === 'j') { e.preventDefault(); toggleDrawer('downloads'); }
      else if (k === 'r') { e.preventDefault(); send('RELOAD'); }
      else if (k === '=' || k === '+') { e.preventDefault(); send('ZOOM_IN'); }
      else if (k === '-') { e.preventDefault(); send('ZOOM_OUT'); }
      else if (k === '0') { e.preventDefault(); send('ZOOM_RESET'); }
    } else if (e.key === 'Escape' && activeDrawer) {
      closeDrawer();
    }
  });
</script>
</body>
</html>
"##;

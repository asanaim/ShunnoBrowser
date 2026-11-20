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
    /* Motion: one curve family, no bounce (Apple-style deceleration). */
    --ease-fluid: cubic-bezier(0.32, 0.72, 0, 1);
    --ease-spring: cubic-bezier(0.32, 0.72, 0, 1);
    --ease-in: cubic-bezier(0.4, 0, 1, 1);
    --dur-fast: 0.16s;
    --dur: 0.28s;
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

  body.maximized, html.maximized { border-radius: 0; }
  body.maximized .chrome-shell, body.maximized .tab-bar { border-top-left-radius: 0; border-top-right-radius: 0; }
  body.maximized .tab-bar { padding-top: 6px; height: 38px; }

  /* Theme switch is one cross-fade (View Transitions); per-element transitions are paused meanwhile. */
  body.theme-switching, body.theme-switching * { transition: none !important; }
  ::view-transition-old(root), ::view-transition-new(root) {
    animation-duration: 0.38s;
    animation-timing-function: cubic-bezier(0.32, 0.72, 0, 1);
  }

  @media (prefers-reduced-motion: reduce) {
    *, *::before, *::after { animation-duration: 0.01ms !important; transition-duration: 0.01ms !important; }
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

  /* 2. Unified Responsive Centered Address + Search Toolbar */
  .nav-bar {
    display: grid;
    grid-template-columns: minmax(max-content, 1fr) minmax(240px, clamp(320px, 48vw, 640px)) minmax(max-content, 1fr);
    align-items: center;
    gap: 12px;
    padding: 5px 14px;
    height: 46px;
  }
  .nav-group { display: flex; align-items: center; gap: 5px; flex-shrink: 0; }
  .nav-group.left { justify-self: start; }
  .nav-group.right { justify-self: end; }
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

  /* Responsive Centered Search + URL Capsule (proportional scaling, balanced margins) */
  .firefox-capsule {
    width: 100%; min-width: 0; height: 32px;
    display: flex; align-items: center; gap: 6px;
    background: var(--input-bg); border-radius: 16px; padding: 2px 10px 2px 4px;
    border: 1.5px solid transparent;
    transition: background 0.32s var(--ease-fluid), border-color 0.25s var(--ease-fluid), box-shadow 0.28s var(--ease-fluid);
  }
  .firefox-capsule:focus-within {
    background: var(--surface-solid);
    border-color: var(--blue);
    box-shadow: 0 0 0 3px rgba(10, 132, 255, 0.18);
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
    border: none; background: transparent; outline: none; flex: 1 1 auto; width: 100%; min-width: 140px;
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

  /* Floating Overlay Panel over Webpage (10% Transparency = 90% Opaque, Zero Page Splitting!) */
  /* Popover: anchored under the button that opened it, with a pointer, like macOS Safari. */
  .overlay-backdrop {
    position: fixed; inset: 0;
    display: none;
    background: transparent;
    z-index: 50;
  }
  .overlay-backdrop.open { display: block; }

  @keyframes popoverIn {
    from { opacity: 0; transform: translateY(-6px) scale(0.95); }
    to   { opacity: 1; transform: translateY(0) scale(1); }
  }
  @keyframes popoverOut {
    from { opacity: 1; transform: translateY(0) scale(1); }
    to   { opacity: 0; transform: translateY(-4px) scale(0.97); }
  }

  .floating-sheet {
    position: fixed; top: 84px; left: 12px;
    width: 430px; max-width: calc(100vw - 24px); max-height: calc(100vh - 100px);
    display: flex; flex-direction: column;
    background: rgba(255, 255, 255, 0.92);
    backdrop-filter: blur(22px) saturate(170%);
    border: 1px solid var(--border);
    border-radius: 16px;
    box-shadow: 0 18px 48px rgba(0, 0, 0, 0.20), 0 2px 8px rgba(0, 0, 0, 0.06);
    transform-origin: 90% 0;
    animation: popoverIn var(--dur) var(--ease-fluid) both;
  }
  .floating-sheet.closing { animation: popoverOut var(--dur-fast) var(--ease-in) both; pointer-events: none; }
  body.dark .floating-sheet { background: rgba(30, 30, 34, 0.92); }
  .floating-sheet.wide { width: 510px; }
  .sheet-scroll { overflow-y: auto; padding: 14px; border-radius: 16px; min-height: 0; flex: 1 1 auto; }
  .sheet-caret {
    position: absolute; top: -7px; left: var(--caret-left, 380px);
    width: 14px; height: 14px; transform: rotate(45deg);
    background: inherit; backdrop-filter: inherit;
    border-left: 1px solid var(--border); border-top: 1px solid var(--border);
    border-top-left-radius: 3px;
  }

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

  /* Clean Windows 11 Snap Layout Hover Flyout (Zero 1-9 numbers) */
  .snap-flyout {
    position: fixed; top: 36px; right: 10px; z-index: 96;
    display: none; grid-template-columns: repeat(3, 76px); gap: 10px;
    padding: 12px; border-radius: 14px;
    background: var(--surface-solid); border: 1px solid var(--border);
    box-shadow: 0 18px 46px rgba(0, 0, 0, 0.24);
  }
  .snap-flyout.open { display: grid; animation: popoverIn var(--dur) var(--ease-fluid) both; transform-origin: 90% 0; }
  .snap-grid {
    display: grid; gap: 3px; width: 76px; height: 48px;
  }
  .snap-zone {
    border-radius: 5px; background: var(--input-bg); border: 1px solid var(--border);
    cursor: pointer; transition: background 0.14s ease, border-color 0.14s ease, transform 0.12s ease;
  }
  .snap-zone:hover {
    background: var(--blue); border-color: var(--blue); transform: scale(1.03);
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
            <linearGradient id="shunnoCompass" x1="2" y1="2" x2="26" y2="26" gradientUnits="userSpaceOnUse">
              <stop offset="0%" stop-color="#38C6FF"/>
              <stop offset="52%" stop-color="#0A84FF"/>
              <stop offset="100%" stop-color="#0442D6"/>
            </linearGradient>
            <linearGradient id="shunnoArc" x1="15" y1="5" x2="23" y2="13" gradientUnits="userSpaceOnUse">
              <stop offset="0%" stop-color="#FF4E42"/>
              <stop offset="100%" stop-color="#FF942E"/>
            </linearGradient>
          </defs>
          <circle cx="14" cy="14" r="13.5" fill="url(#shunnoCompass)"/>
          <circle cx="14" cy="14" r="7.2" stroke="white" stroke-width="3.2"/>
          <path d="M 16.5 7.2 A 7.2 7.2 0 0 1 20.9 11.9" stroke="url(#shunnoArc)" stroke-width="3.2" stroke-linecap="round"/>
          <circle cx="14" cy="14" r="1.6" fill="white"/>
        </svg>
      </div>
      <div id="tabs-container"></div>
      <button class="tab-add" onclick="send('NEW_TAB')" title="New Tab (Ctrl+T)">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
      </button>
      <button class="tab-add" style="color:var(--text)" onclick="send('NEW_INCOGNITO')" title="New Private Tab">
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round"><path d="M2 11h20"/><path d="M5 11l2.8-6.5h8.4L19 11"/><circle cx="7.5" cy="17.5" r="3.3"/><circle cx="16.5" cy="17.5" r="3.3"/><path d="M10.8 17.3q1.2-1.1 2.4 0"/></svg>
      </button>
      <button class="tab-add" id="btn-reopen-tab" style="display:none;color:var(--muted)" onclick="send('REOPEN_CLOSED_TAB')" title="Reopen Closed Tab (Ctrl+Shift+T)">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.3" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 14 4 9 9 4"/><path d="M20 20v-7a4 4 0 0 0-4-4H4"/></svg>
      </button>
      <div class="win-drag-spacer" onmousedown="send('WIN_DRAG')" ondblclick="send('WIN_MAXIMIZE')"></div>
      <div class="win-controls">
        <button class="win-btn" onclick="send('WIN_MINIMIZE')" title="Minimize">
          <svg width="11" height="11" viewBox="0 0 12 12"><line x1="2" y1="6" x2="10" y2="6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>
        </button>
        <button class="win-btn" id="btn-max" onclick="closeSnapFlyout(); send('WIN_MAXIMIZE')" onmouseenter="startSnapHover()" onmouseleave="cancelSnapHover()" title="Maximize">
          <svg width="11" height="11" viewBox="0 0 12 12"><rect x="2" y="2" width="8" height="8" rx="1.2" fill="none" stroke="currentColor" stroke-width="1.5"/></svg>
        </button>
        <button class="win-btn close" onclick="send('WIN_CLOSE')" title="Close">
          <svg width="11" height="11" viewBox="0 0 12 12"><line x1="2.5" y1="2.5" x2="9.5" y2="9.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/><line x1="9.5" y1="2.5" x2="2.5" y2="9.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>
        </button>
      </div>
    </div>

    <!-- 2. Firefox-Inspired Compact Centered Address + Search Bar -->
    <div class="nav-bar">
      <div class="nav-group left">
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

      <div class="nav-group right">
        <button class="nav-btn on" id="btn-downloads" onclick="toggleDrawer('downloads')" title="Downloads (Ctrl+J)">⚡ 0</button>
        <button class="nav-btn" id="btn-controls" onclick="toggleDrawer('controls')" title="Controls &amp; Settings">•••</button>
      </div>
    </div>

    <!-- 3. Always-Visible Bookmarks & Library Strip Under Address Bar -->
    <div class="bookmarks-strip" id="bookmarks-strip"></div>
  </div>
  <div class="engine-menu" id="engine-menu"></div>
  <div class="snap-flyout" id="snap-flyout" onmouseenter="keepSnapOpen()" onmouseleave="scheduleSnapClose()">
    <!-- 1. 50 / 50 -->
    <div class="snap-grid" style="grid-template-columns:1fr 1fr;">
      <div class="snap-zone" onclick="applySnap(0,0,500,1000)" title="Left Half"></div>
      <div class="snap-zone" onclick="applySnap(500,0,500,1000)" title="Right Half"></div>
    </div>
    <!-- 2. 66 / 34 -->
    <div class="snap-grid" style="grid-template-columns:2fr 1fr;">
      <div class="snap-zone" onclick="applySnap(0,0,666,1000)" title="Left Two-Thirds"></div>
      <div class="snap-zone" onclick="applySnap(666,0,334,1000)" title="Right Third"></div>
    </div>
    <!-- 3. 33 / 33 / 34 -->
    <div class="snap-grid" style="grid-template-columns:1fr 1fr 1fr;">
      <div class="snap-zone" onclick="applySnap(0,0,333,1000)" title="Left Third"></div>
      <div class="snap-zone" onclick="applySnap(333,0,334,1000)" title="Center Third"></div>
      <div class="snap-zone" onclick="applySnap(667,0,333,1000)" title="Right Third"></div>
    </div>
    <!-- 4. 50 + Top/Bottom Right -->
    <div class="snap-grid" style="grid-template-columns:1fr 1fr;grid-template-rows:1fr 1fr;">
      <div class="snap-zone" style="grid-row:1/3;" onclick="applySnap(0,0,500,1000)" title="Left Half"></div>
      <div class="snap-zone" onclick="applySnap(500,0,500,500)" title="Top Right"></div>
      <div class="snap-zone" onclick="applySnap(500,500,500,500)" title="Bottom Right"></div>
    </div>
    <!-- 5. 4-Quadrant Grid -->
    <div class="snap-grid" style="grid-template-columns:1fr 1fr;grid-template-rows:1fr 1fr;">
      <div class="snap-zone" onclick="applySnap(0,0,500,500)" title="Top Left"></div>
      <div class="snap-zone" onclick="applySnap(500,0,500,500)" title="Top Right"></div>
      <div class="snap-zone" onclick="applySnap(0,500,500,500)" title="Bottom Left"></div>
      <div class="snap-zone" onclick="applySnap(500,500,500,500)" title="Bottom Right"></div>
    </div>
    <!-- 6. 25 / 50 / 25 -->
    <div class="snap-grid" style="grid-template-columns:1fr 2fr 1fr;">
      <div class="snap-zone" onclick="applySnap(0,0,250,1000)" title="Left Quarter"></div>
      <div class="snap-zone" onclick="applySnap(250,0,500,1000)" title="Center Half"></div>
      <div class="snap-zone" onclick="applySnap(750,0,250,1000)" title="Right Quarter"></div>
    </div>
  </div>
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
        <circle cx="14" cy="14" r="13.5" fill="url(#shunnoCompass)"/>
        <circle cx="14" cy="14" r="7.2" stroke="white" stroke-width="3.2"/>
        <path d="M 16.5 7.2 A 7.2 7.2 0 0 1 20.9 11.9" stroke="url(#shunnoArc)" stroke-width="3.2" stroke-linecap="round"/>
        <circle cx="14" cy="14" r="1.6" fill="white"/>
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
      <div class="sheet-caret"></div>
      <div class="sheet-scroll">

      <!-- Panel A: Controls & Settings -->
      <div id="panel-controls" style="display:none">
        <div class="card">
          <div class="card-header">
            <span class="card-title">Controls</span>
            <div style="display:flex;gap:5px;align-items:center;">
              <span class="meta" id="version-badge" style="font-weight:700;">v1.5.0</span>
              <button class="pill-btn pill-soft" id="btn-update" style="display:none;"></button>
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
            <button class="pill-btn pill-blue" id="menu-speed-btn" onclick="cycleSpeed()" title="Video &amp; Audio Playback Speed">⏩ 1.0×</button>
            <button class="pill-btn pill-soft" onclick="closeDrawer(); send('PRINT_TO_PDF');" title="Save current page as PDF">📄 PDF</button>
            <button class="pill-btn pill-green" onclick="send('SAVE_OFFLINE_HTML'); closeDrawer();">💾 Save</button>
          </div>

          <div class="grid-toggles" id="global-toggles-grid"></div>
        </div>

        <div class="card">
          <div class="card-header">
            <span class="card-title" id="site-domain-title">🌐 Site Controls</span>
            <button class="pill-btn pill-orange" id="btn-clear-site-data" onclick="send('CLEAR_SITE_DATA')" title="Clear cookies, localStorage, sessionStorage & cache for this site">🧹 Clear Site Data</button>
            <button class="pill-btn pill-soft" onclick="if (confirm('Delete every saved password and autofill entry?')) send('CLEAR_PASSWORDS')" title="Delete all passwords saved by Shunno">🔑 Clear Passwords</button>
            <button class="pill-btn pill-soft" id="btn-update-filters" onclick="send('UPDATE_FILTERS')" title="Download the latest EasyList and EasyPrivacy">🛡️ Update Block Lists</button>
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
            <span class="card-title">Downloads <span class="meta" id="dl-live-speed"></span></span>
            <div style="display:flex;gap:5px;align-items:center;">
              <button class="pill-btn pill-soft" onclick="send('CLEAR_FINISHED_DL')" title="Remove finished and failed items from this list. Files stay on disk.">Clear</button>
              <button class="pill-btn pill-soft" onclick="triggerGrabPage()" title="Find every downloadable file and video on this page">Find on page</button>
              <button class="pill-btn pill-soft" onclick="send('OPEN_DOWNLOADS_FOLDER')" title="Open downloads folder">📂</button>
              <button class="pill-btn pill-soft" id="btn-dl-settings" onclick="toggleDlSettings()" title="Download settings">⚙</button>
              <button class="pill-btn pill-red" onclick="closeDrawer()">✕</button>
            </div>
          </div>
          <div class="input-row" style="margin-bottom:0;">
            <input class="text-box" id="dl-url-input" placeholder="Paste a link to download" onkeydown="if (event.key === 'Enter') startManualDownload();" />
            <button class="pill-btn pill-blue" onclick="startManualDownload()">Download</button>
          </div>
          <div id="dl-settings" style="display:none;margin-top:10px;border-top:1px solid var(--border);padding-top:10px;">
            <div class="meta" style="margin-bottom:6px;">Batch links: <code>track_[01-12].flac</code> downloads 12 files.</div>
            <div class="input-row">
              <select class="select-box" id="dl-threads-select" title="Connections per file">
                <option value="1">1 connection</option>
                <option value="4">4 connections</option>
                <option value="8">8 connections</option>
                <option value="16" selected>16 connections</option>
                <option value="32">32 connections</option>
              </select>
              <select class="select-box" id="dl-delay-select" title="Start time">
                <option value="0">Start now</option>
                <option value="60">Start in 1 min</option>
                <option value="300">Start in 5 min</option>
                <option value="900">Start in 15 min</option>
              </select>
            </div>
            <div class="input-row">
              <select class="select-box" id="max-conc-select" onchange="send('SET_MAX_CONCURRENT:' + this.value)" title="How many files download at once">
                <option value="1">1 at a time</option>
                <option value="2">2 at a time</option>
                <option value="3" selected>3 at a time</option>
                <option value="4">4 at a time</option>
              </select>
              <select class="select-box" id="speed-limit-select" onchange="send('SET_SPEED_LIMIT:' + this.value)" title="Bandwidth limit">
                <option value="0">No speed limit</option>
                <option value="10485760">Limit 10 MB/s</option>
                <option value="5242880">Limit 5 MB/s</option>
                <option value="2097152">Limit 2 MB/s</option>
                <option value="524288">Limit 512 KB/s</option>
              </select>
              <select class="select-box" id="on-complete-select" onchange="send('SET_ON_COMPLETE:' + this.value)" title="When all downloads finish">
                <option value="NONE">When done: nothing</option>
                <option value="OPEN_FOLDER">When done: open folder</option>
                <option value="EXIT_APP">When done: close browser</option>
              </select>
            </div>
            <div class="input-row">
              <input class="text-box" id="custom-dl-dir-input" placeholder="Save to folder (e.g. D:\Downloads)" />
              <button class="pill-btn pill-soft" onclick="applyCustomDlDir()">Set</button>
              <button class="pill-btn pill-soft" onclick="send('SET_DOWNLOAD_DIR:'); document.getElementById('custom-dl-dir-input').value='';" title="Back to Downloads\ShunnoBrowser">Reset</button>
            </div>
            <div class="input-row">
              <button class="pill-btn pill-soft" id="btn-ffmpeg-setup" onclick="send('INSTALL_FFMPEG')" title="ffmpeg.exe, SHA-256 verified. Installs automatically when a video needs it.">🎬 FFmpeg</button>
              <button class="pill-btn pill-soft" id="btn-ytdlp-setup" onclick="send('INSTALL_YTDLP')" title="yt-dlp.exe, SHA-256 verified. Installs automatically for YouTube and streaming sites.">▶ yt-dlp</button>
              <button class="pill-btn pill-soft" onclick="send('EXPORT_DL_CSV')" title="Export the list with SHA-256 checksums">Export CSV</button>
            </div>
            <div class="input-row" style="margin-bottom:0;">
              <input class="text-box" id="mux-video-input" placeholder="Advanced: DASH video URL" />
              <input class="text-box" id="mux-audio-input" placeholder="DASH audio URL" />
              <button class="pill-btn pill-soft" onclick="startDashMux()">Merge</button>
            </div>
          </div>
        </div>

        <div class="folder-pills" id="dl-category-bar"></div>
        <div id="duplicate-banner" class="card" style="display:none;border-color:var(--orange);"></div>
        <div id="downloads-list"></div>
      </div>

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

  let snapHoverTimer = null;
  let snapCloseTimer = null;
  function startSnapHover() {
    keepSnapOpen();
    if (snapHoverTimer) clearTimeout(snapHoverTimer);
    snapHoverTimer = setTimeout(() => {
      const el = document.getElementById('snap-flyout');
      if (el && !el.classList.contains('open')) {
        el.classList.add('open');
        send('ENGINE_MENU:1');
      }
    }, 600);
  }
  function keepSnapOpen() {
    if (snapCloseTimer) { clearTimeout(snapCloseTimer); snapCloseTimer = null; }
  }
  function scheduleSnapClose() {
    if (snapHoverTimer) { clearTimeout(snapHoverTimer); snapHoverTimer = null; }
    snapCloseTimer = setTimeout(closeSnapFlyout, 180);
  }
  function closeSnapFlyout() {
    if (snapHoverTimer) { clearTimeout(snapHoverTimer); snapHoverTimer = null; }
    if (snapCloseTimer) { clearTimeout(snapCloseTimer); snapCloseTimer = null; }
    const el = document.getElementById('snap-flyout');
    if (el && el.classList.contains('open')) {
      el.classList.remove('open');
      send('ENGINE_MENU:0');
    }
  }
  function cancelSnapHover() {
    scheduleSnapClose();
  }
  function applySnap(x, y, w, h) {
    closeSnapFlyout();
    send(`SNAP_WIN:${x}|${y}|${w}|${h}`);
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

  function openDrawer(name) {
    if (activeDrawer === name) return;
    toggleDrawer(name);
  }

  let drawerCloseTimer = null;
  let drawerAnchor = null;

  function defaultAnchorFor(name) {
    return document.getElementById(name === 'downloads' ? 'btn-downloads' : 'btn-controls');
  }

  // Place the popover under its anchor; the caret points at the anchor's centre.
  function positionSheet() {
    const sheet = document.getElementById('floating-sheet');
    const anchor = (drawerAnchor && drawerAnchor.isConnected) ? drawerAnchor : defaultAnchorFor(activeDrawer);
    if (!sheet || !anchor) return;
    const r = anchor.getBoundingClientRect();
    const vw = window.innerWidth, vh = window.innerHeight;
    const w = Math.min(sheet.offsetWidth || 430, vw - 24);
    let left = Math.round(r.right - w + 4);
    left = Math.max(12, Math.min(left, vw - w - 12));
    const top = Math.round(r.bottom + 10);
    sheet.style.left = left + 'px';
    sheet.style.top = top + 'px';
    sheet.style.maxHeight = Math.max(160, vh - top - 14) + 'px';
    const caretLeft = Math.max(14, Math.min(w - 28, Math.round(r.left + r.width / 2 - left - 7)));
    sheet.style.setProperty('--caret-left', caretLeft + 'px');
    sheet.style.transformOrigin = (caretLeft + 7) + 'px 0';
  }
  window.addEventListener('resize', () => { if (activeDrawer) positionSheet(); });

  function toggleDrawer(name, anchorEl) {
    if (activeDrawer === name) {
      closeDrawer();
      return;
    }
    const drawer = document.getElementById('drawer');
    const sheet = document.getElementById('floating-sheet');
    if (drawerCloseTimer) { clearTimeout(drawerCloseTimer); drawerCloseTimer = null; }
    sheet.classList.remove('closing');
    const ev = window.event;
    const clicked = anchorEl || (ev && ev.currentTarget && ev.currentTarget.getBoundingClientRect ? ev.currentTarget : null);
    // Only anchor to toolbar elements; buttons inside the sheet keep the current anchor.
    if (clicked && !sheet.contains(clicked)) drawerAnchor = clicked;
    else if (!activeDrawer) drawerAnchor = defaultAnchorFor(name);
    activeDrawer = name;

    if (name === 'downloads' || name === 'bookmarks') {
      sheet.classList.add('wide');
    } else {
      sheet.classList.remove('wide');
    }

    ['controls', 'find', 'streams', 'bookmarks', 'downloads'].forEach(p => {
      document.getElementById('panel-' + p).style.display = (p === name) ? 'block' : 'none';
    });

    drawer.classList.add('open');
    positionSheet();
    sheet.style.animation = 'none';
    void sheet.offsetWidth;
    sheet.style.animation = '';
    send('DRAWER_OPEN:' + name);
  }

  function closeDrawer() {
    if (!activeDrawer) return;
    activeDrawer = null;
    const drawer = document.getElementById('drawer');
    const sheet = document.getElementById('floating-sheet');
    sheet.classList.add('closing');
    if (drawerCloseTimer) clearTimeout(drawerCloseTimer);
    // Let the fade-out finish before the native layer shrinks back over the page.
    drawerCloseTimer = setTimeout(() => {
      drawerCloseTimer = null;
      drawer.classList.remove('open');
      sheet.classList.remove('closing');
      send('DRAWER_CLOSE');
    }, 150);
  }

  function applyTheme(dark) {
    const flip = () => {
      document.body.classList.add('theme-switching');
      document.body.classList.toggle('dark', dark);
      requestAnimationFrame(() => requestAnimationFrame(() => document.body.classList.remove('theme-switching')));
    };
    const reduce = window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (document.startViewTransition && !reduce) document.startViewTransition(flip);
    else flip();
  }

  const ICON_MAX = '<svg width="11" height="11" viewBox="0 0 12 12"><rect x="2" y="2" width="8" height="8" rx="1.2" fill="none" stroke="currentColor" stroke-width="1.5"/></svg>';
  const ICON_RESTORE = '<svg width="11" height="11" viewBox="0 0 12 12"><rect x="1.8" y="3.6" width="6.6" height="6.6" rx="1.1" fill="none" stroke="currentColor" stroke-width="1.4"/><path d="M3.8 3.6V2.9a1.1 1.1 0 0 1 1.1-1.1h4.2a1.1 1.1 0 0 1 1.1 1.1v4.2a1.1 1.1 0 0 1-1.1 1.1h-0.7" fill="none" stroke="currentColor" stroke-width="1.4"/></svg>';
  let lastMaximized = null;
  function applyWindowState(maximized) {
    if (lastMaximized === maximized) return;
    lastMaximized = maximized;
    document.body.classList.toggle('maximized', maximized);
    document.documentElement.classList.toggle('maximized', maximized);
    const b = document.getElementById('btn-max');
    if (b) {
      b.innerHTML = maximized ? ICON_RESTORE : ICON_MAX;
      b.title = maximized ? 'Restore Down' : 'Maximize';
    }
    if (activeDrawer) requestAnimationFrame(positionSheet);
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

  function toggleDlSettings() {
    const box = document.getElementById('dl-settings');
    const btn = document.getElementById('btn-dl-settings');
    const open = box.style.display === 'none';
    box.style.display = open ? 'block' : 'none';
    btn.className = 'pill-btn ' + (open ? 'pill-blue' : 'pill-soft');
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

      const displayTitle = (t.sleeping ? '💤 ' : '') + (t.title || 'New Tab');
      const hasAudio = !!(t.playing_audio || t.muted);
      const audioKey = hasAudio ? (t.muted ? 'M' : 'P') : 'N';

      if (el.dataset.renderedTitle !== displayTitle || el.dataset.audioKey !== audioKey || el.dataset.tabIdx !== String(idx)) {
        el.dataset.renderedTitle = displayTitle;
        el.dataset.audioKey = audioKey;
        el.dataset.tabIdx = String(idx);
        const isNewTab = t.url === 'shunno://newtab';
        const anonSvg = `<span style="display:inline-flex;align-items:center;flex-shrink:0;color:#BF5AF2;" title="Private Tab"><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round"><path d="M2 11h20"/><path d="M5 11l2.8-6.5h8.4L19 11"/><circle cx="7.5" cy="17.5" r="3.3"/><circle cx="16.5" cy="17.5" r="3.3"/><path d="M10.8 17.3q1.2-1.1 2.4 0"/></svg></span>`;
        const tabLogoHtml = t.incognito
          ? anonSvg
          : ((!hasAudio && (isNewTab || t.pinned))
              ? `<span style="display:inline-flex;align-items:center;flex-shrink:0;" title="${t.pinned ? 'Pinned (Right-click to unpin)' : ''}"><svg width="18" height="18" viewBox="0 0 28 28" fill="none"><circle cx="14" cy="14" r="13.5" fill="url(#shunnoCompass)"/><circle cx="14" cy="14" r="7.2" stroke="white" stroke-width="3.2"/><path d="M 16.5 7.2 A 7.2 7.2 0 0 1 20.9 11.9" stroke="url(#shunnoArc)" stroke-width="3.2" stroke-linecap="round"/><circle cx="14" cy="14" r="1.6" fill="white"/></svg></span>`
              : '');
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
      { kind: 'Action', title: 'Save Page as PDF (PrintToPdf)', run: () => send('PRINT_TO_PDF') },
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
    const wantDark = !!state.global.full_dark_mode;
    if (document.body.classList.contains('dark') !== wantDark) applyTheme(wantDark);
    applyWindowState(!!state.is_maximized);

    document.getElementById('version-badge').textContent = state.version || '';
    const upBtn = document.getElementById('btn-update');
    if (upBtn) {
      const k = state.update_kind || '';
      upBtn.title = state.update_label || '';
      if (k === 'ready') {
        upBtn.style.display = ''; upBtn.className = 'pill-btn pill-green';
        upBtn.textContent = '⬆ Restart to update'; upBtn.onclick = () => send('RESTART_FOR_UPDATE');
      } else if (k === 'available') {
        upBtn.style.display = ''; upBtn.className = 'pill-btn pill-blue';
        upBtn.textContent = '⬆ Update available'; upBtn.onclick = () => send('OPEN_RELEASES');
      } else {
        upBtn.style.display = ''; upBtn.className = 'pill-btn pill-soft';
        upBtn.textContent = k === 'uptodate' ? '✓ Up to date' : (k === 'failed' ? '⟳ Check again' : '⟳ Check for updates');
        upBtn.onclick = () => send('CHECK_UPDATES');
      }
    }
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
    const ytBtn = document.getElementById('btn-ytdlp-setup');
    if (ytBtn) {
      const ys = state.ytdlp_status || '';
      if (ys === 'Ready' || ys === 'Installed!') {
        ytBtn.textContent = '▶ yt-dlp: Ready';
        ytBtn.className = 'pill-btn pill-green';
      } else {
        ytBtn.textContent = ys ? ('▶ ' + ys) : '▶ yt-dlp';
        ytBtn.className = 'pill-btn pill-soft';
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
      ['block_popups', '🚫 Block Pop-Ups', state.global.block_popups],
      ['https_only', '🔒 HTTPS Only', state.global.https_only],
      ['auto_update', '⬆️ Auto Update', state.global.auto_update],
      ['save_passwords', '🔑 Save Passwords', state.global.save_passwords],
      ['strict_tracking', '🛡️ Strict Tracking Block', state.global.strict_tracking]
    ];
    document.getElementById('global-toggles-grid').innerHTML = globalItems.map(([key, label, val]) => `
      <div class="toggle-tile">
        <span>${label}</span>
        <button class="pill-btn ${val ? 'pill-blue' : 'pill-soft'}" onclick="send('TOGGLE_GLOBAL:${key}')">${val ? 'ON' : 'OFF'}</button>
      </div>
    `).join('');

    const siteTitle = document.getElementById('site-domain-title');
    siteTitle.textContent = `🌐 ${state.active_host || 'Site Controls'} (${state.blocked_count} blocked)`;
    siteTitle.title = 'Blocking: ' + (state.adblock_status || 'built-in list');
    const flBtn = document.getElementById('btn-update-filters');
    if (flBtn) flBtn.title = 'Filter lists: ' + (state.adblock_status || 'built-in list');
    const siteItems = [
      ['adblock', '🛡️ Ad Blocker', state.site.adblock],
      ['sponsorblock', '🎬 YT Ad Clean', state.site.sponsorblock],
      ['js_enabled', '⚙️ JavaScript', state.site.js_enabled],
      ['images_enabled', '🖼️ Images', state.site.images_enabled],
      ['mobile_ua', '📱 Mobile Site', state.site.mobile_ua],
      ['auto_reject_cookies', '🍪 Reject Cookie Banners', state.site.auto_reject_cookies],
      ['fp_protect', '🫥 Fingerprint Shield', state.site.fp_protect]
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
    if (liveSpdEl) liveSpdEl.textContent = totalSpeed > 0 ? '· ' + fmtBytes(totalSpeed) + '/s' : '';
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
    const catBar = document.getElementById('dl-category-bar');
    catBar.style.display = state.downloads.length >= 6 ? '' : 'none';
    if (catBar.style.display === 'none') activeDlCategory = 'All';
    catBar.innerHTML = dlCats.map(c => `
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
                ? `<span title="SHA-256 ${escapeHtml(d.sha256 || '')}">✓ Saved</span>`
                : d.status === 'FAILED'
                  ? `⚠ ${escapeHtml(d.error_message || 'Error')}`
                  : `Paused at ${fmtBytes(d.downloaded_bytes)}`;

          return `
            <div class="card">
              <div class="card-header" style="margin-bottom:4px;">
                <div style="overflow:hidden;">
                  <div style="font-size:13px;font-weight:700;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">${escapeHtml(d.file_name)}</div>
                  <div class="meta" style="color:#0A84FF;">${escapeHtml(d.category)}${d.status === 'DOWNLOADING' && d.supports_ranges ? ' · ' + d.threads + ' connections' : ''} · ${escapeHtml(({DOWNLOADING:'Downloading',PAUSED:'Paused',FAILED:'Failed',COMPLETED:'Done',QUEUED:'Waiting',MUXING:'Merging'})[d.status] || d.status)}</div>
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
  // For values placed inside a single-quoted JS string inside an HTML attribute:
  // JS-escape first (backslash, quotes, control chars), then HTML-escape for the attribute.
  function encodeAttr(str) {
    const js = JSON.stringify(String(str)).slice(1, -1).replace(/'/g, "\\'");
    return escapeHtml(js);
  }

  document.addEventListener('keydown', (e) => {
    if (e.key === 'F11') {
      e.preventDefault();
      send('TOGGLE_FULLSCREEN');
      return;
    }
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

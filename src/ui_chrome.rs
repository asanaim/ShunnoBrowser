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
    font-family: -apple-system, "Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif;
    -webkit-font-smoothing: antialiased;
    user-select: none;
  }
  html, body {
    background: transparent;
    color: var(--text);
    overflow: hidden;
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  .chrome-shell, .floating-sheet, .card, .home-canvas, .home-search-box, .shortcut-tile, .engine-menu, .nav-icon-btn, .tab, .bookmarks-strip {
    transition: background-color 0.18s var(--ease-fluid), color 0.18s var(--ease-fluid), border-color 0.18s var(--ease-fluid), box-shadow 0.18s var(--ease-fluid);
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
    z-index: 20;
    transition: background 0.3s var(--ease-fluid);
  }

  /* 1. Borderless Tab Strip (No redundant "Shunno Browser" title bar!) */
  .tab-bar {
    display: flex; align-items: center; gap: 6px; padding: 5px 8px 4px 10px;
    background: var(--tab-bg); height: 36px; overflow: hidden;
    border-top-left-radius: 12px; border-top-right-radius: 12px;
  }
  /* Arc-style: with more than one Space, the whole tab bar takes a light tint of the Space colour,
     so the current Space is obvious at a glance. */
  html.spaces-tinted .tab-bar { background: color-mix(in srgb, var(--space-accent) 11%, var(--tab-bg)); }
  html.spaces-tinted.dark .tab-bar, html.spaces-tinted body.dark .tab-bar { background: color-mix(in srgb, var(--space-accent) 16%, var(--tab-bg)); }

  /* Chrome/Safari-inspired Compass-Ring Logo */
  .brand-emblem {
    width: 28px; height: 28px; border-radius: 7px;
    display: inline-flex; align-items: center; justify-content: center;
    flex-shrink: 0; cursor: pointer;
    box-shadow: 0 2px 6px rgba(10, 132, 255, 0.28);
    transition: transform 0.28s var(--ease-spring);
  }
  .brand-emblem:hover { transform: scale(1.06); }

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
  /* Tab groups: a capsule label in the group colour, member tabs tinted with a thin colour rule. */
  .tg-chip {
    display: inline-flex; align-items: center; gap: 5px; height: 24px; padding: 0 9px; border-radius: 12px; flex-shrink: 0;
    background: color-mix(in srgb, var(--tg) 20%, transparent); color: color-mix(in srgb, var(--tg) 70%, var(--text));
    font-size: 11.5px; font-weight: 700; cursor: pointer; user-select: none; max-width: 140px;
    transition: background .18s var(--ease-fluid), transform .18s var(--ease-fluid);
  }
  .tg-chip:hover { background: color-mix(in srgb, var(--tg) 30%, transparent); }
  .tg-chip:active { transform: scale(.96); }
  .tg-chip .nm { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tg-chip .dot { width: 9px; height: 9px; border-radius: 50%; background: var(--tg); flex-shrink: 0; }
  .tg-chip .ct { font-size: 10.5px; font-weight: 600; opacity: .8; }
  .tab.in-group { background: color-mix(in srgb, var(--tg) 12%, rgba(128, 128, 128, 0.08)); box-shadow: inset 0 -2px 0 color-mix(in srgb, var(--tg) 75%, transparent); }
  .tab.in-group.active { background: var(--tab-active-bg, #fff); box-shadow: 0 1px 6px rgba(0,0,0,.12), inset 0 -2px 0 var(--tg); }
  .tab.tg-hidden { display: none; }
  .ctx-row { display: flex; align-items: center; gap: 9px; width: 100%; padding: 7px 10px; border: none; background: transparent; border-radius: 8px;
    color: var(--text); font: inherit; font-size: 13px; text-align: left; cursor: pointer; }
  .ctx-row:hover { background: var(--blue); color: #fff; }
  .ctx-row .dot { width: 10px; height: 10px; border-radius: 50%; flex-shrink: 0; }
  .ctx-row.danger { color: var(--red); }
  .ctx-row.danger:hover { background: var(--red); color: #fff; }
  .ctx-sep { height: 1px; background: var(--border); margin: 4px 6px; }
  .ctx-label { font-size: 11px; color: var(--text-dim, #8E8E93); padding: 6px 10px 2px; font-weight: 600; }
  .tab:hover { background: rgba(128, 128, 128, 0.20); }
  .tab.active {
    background: var(--surface-solid); color: var(--text); font-weight: 600;
    box-shadow: 0 1px 6px rgba(0, 0, 0, 0.12);
  }
  .tab.incognito.active { background: #2C2C2E; color: #FFFFFF; }
  /* Spaces: the active Space's colour marks the current tab and the Space button. */
  .tab.active:not(.incognito) { box-shadow: 0 1px 6px rgba(0, 0, 0, 0.12), inset 0 -2px 0 var(--space-accent, transparent); }
  .space-pill {
    display: inline-flex; align-items: center; gap: 6px; height: 28px; max-width: 150px; padding: 0 10px 0 8px; margin: 0 4px 0 2px;
    border: none; border-radius: 9px; cursor: pointer; flex-shrink: 0;
    background: color-mix(in srgb, var(--space-accent, #0A84FF) 14%, transparent); color: var(--text);
    font-size: 12px; font-weight: 600; transition: background 0.18s var(--ease-fluid);
  }
  .space-pill:hover { background: color-mix(in srgb, var(--space-accent, #0A84FF) 24%, transparent); }
  .space-pill .ic { font-size: 13px; line-height: 1; }
  .space-pill .nm { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  /* Profiles: a round avatar in the toolbar, coloured like Safari's profile badge. */
  .profile-btn { width: 26px; height: 26px; border-radius: 50%; border: none; cursor: pointer; flex-shrink: 0; margin-right: 4px;
    background: var(--profile-accent, #0A84FF); color: #fff; font-size: 12px; font-weight: 700; display: none;
    align-items: center; justify-content: center; box-shadow: 0 0 0 2px var(--bg, transparent), 0 0 0 3px color-mix(in srgb, var(--profile-accent, #0A84FF) 35%, transparent);
    transition: transform .16s var(--ease-fluid); }
  .profile-btn.on { display: inline-flex; }
  .profile-btn:hover { transform: scale(1.06); }
  /* Extensions: Safari-style toolbar icons for extensions with a popup, plus a puzzle button. */
  .ext-bar { display: none; align-items: center; gap: 2px; }
  .ext-bar.on { display: inline-flex; }
  .ext-ic { width: 28px; height: 28px; border: none; border-radius: 8px; background: transparent; cursor: pointer; display: inline-flex;
    align-items: center; justify-content: center; padding: 0; transition: background .15s var(--ease-fluid); }
  .ext-ic:hover { background: rgba(118,118,128,.14); }
  .ext-ic img { width: 18px; height: 18px; border-radius: 4px; }
  .ph { width: 18px; height: 18px; border-radius: 5px; background: var(--blue); color: #fff; font-size: 10px; font-weight: 700;
    display: flex; align-items: center; justify-content: center; }
  .ext-row { display: flex; align-items: center; gap: 10px; padding: 8px 10px; border-radius: 10px; }
  .ext-row:hover { background: rgba(118,118,128,.08); }
  .ext-row .ic { width: 32px; height: 32px; border-radius: 8px; flex-shrink: 0; display: flex; align-items: center; justify-content: center; background: rgba(118,118,128,.12); }
  .ext-row .ic img { width: 24px; height: 24px; }
  .ext-row .tx { flex: 1; min-width: 0; }
  .ext-row .nm { font-size: 13px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ext-row .ds { font-size: 11px; color: var(--text-dim, #8E8E93); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ext-row .acts { display: flex; gap: 4px; margin-top: 3px; }
  .ext-row .acts button { border: none; background: rgba(118,118,128,.14); color: var(--text); font-size: 11px; border-radius: 6px; padding: 2px 8px; cursor: pointer; }
  .ext-row .acts button.rm { color: var(--red); }
  .ext-row.off .ic, .ext-row.off .tx { opacity: .5; }
  .ext-notice { font-size: 12px; padding: 8px 10px; border-radius: 9px; background: rgba(10,132,255,.10); margin-bottom: 8px; display: none; }
  .ext-notice.on { display: block; }
  .prof-links { display: none; align-items: center; justify-content: space-between; gap: 8px; margin-top: 6px; padding: 8px 10px;
    border-radius: 10px; background: rgba(118,118,128,.08); font-size: 12px; }
  .prof-links.on { display: flex; }
  .prof-links select { border: 1px solid var(--border); border-radius: 7px; background: var(--surface, #fff); color: var(--text);
    font: inherit; font-weight: 600; padding: 3px 6px; max-width: 160px; }
  .prof-row { display: flex; align-items: center; gap: 10px; padding: 8px 10px; border-radius: 9px; }
  .prof-row:hover { background: rgba(118,118,128,.10); }
  .prof-av { width: 28px; height: 28px; border-radius: 50%; color: #fff; font-weight: 700; font-size: 13px; display: flex; align-items: center; justify-content: center; flex-shrink: 0; }
  .prof-row .nm { flex: 1; font-size: 13px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .prof-row .sub { font-size: 11px; color: var(--text-dim, #8E8E93); font-weight: 400; }
  .space-row { display: flex; align-items: center; gap: 9px; padding: 7px 9px; border-radius: 9px; cursor: pointer; position: relative; }
  .space-row:hover { background: rgba(118,118,128,0.10); }
  .space-row.on { background: color-mix(in srgb, var(--row-accent) 16%, transparent); }
  .space-row .sw { width: 26px; height: 26px; border-radius: 8px; display: flex; align-items: center; justify-content: center; font-size: 14px; background: color-mix(in srgb, var(--row-accent) 22%, transparent); }
  .space-row .nm { flex: 1; font-size: 12.5px; font-weight: 500; }
  .space-row .ct { font-size: 11px; color: var(--muted); font-variant-numeric: tabular-nums; }
  .swatches { display: flex; gap: 6px; flex-wrap: wrap; }
  .swatch { width: 20px; height: 20px; border-radius: 50%; border: 2px solid transparent; cursor: pointer; padding: 0; }
  .swatch.on { border-color: var(--text); }
  .tab.drag-over-left { box-shadow: -3px 0 0 0 var(--blue); }
  .tab.drag-over-right { box-shadow: 3px 0 0 0 var(--blue); }
  .folder-pills { display: flex; gap: 5px; flex-wrap: wrap; margin-bottom: 8px; }
  #dl-category-bar { flex-wrap: nowrap; overflow-x: auto; scrollbar-width: none; }
  #dl-category-bar::-webkit-scrollbar { display: none; }
  #dl-category-bar .folder-pill { flex-shrink: 0; }
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
  .cap-glyph { font-family: "Segoe Fluent Icons", "Segoe MDL2 Assets"; font-size: 10px; font-weight: 400; line-height: 1; }
  /* Windows 11: the maximize button is drawn and hit-tested by the window itself (real Snap Layouts);
     this one only holds its place. */
  .win-btn.native-max { visibility: hidden; }
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
  /* Toolbar icon buttons (Bookmark, Library) and the compact Downloads button, one height. */
  .tb-icon { border: none; background: transparent; color: var(--text); width: 30px; height: 30px; border-radius: 9px; padding: 0;
    display: inline-flex; align-items: center; justify-content: center; cursor: pointer; flex-shrink: 0;
    transition: background 0.18s var(--ease-fluid), transform 0.18s var(--ease-spring), color 0.2s ease; }
  .tb-icon:hover { background: var(--input-bg); }
  .tb-icon:active { transform: scale(0.92); }
  .tb-icon.marked { color: var(--blue); }
  .nav-btn.dl { height: 30px; padding: 0 8px; gap: 3px; font-size: 12px; border-radius: 9px; }
  .nav-btn.dl .n { font-variant-numeric: tabular-nums; }
  .nav-btn.dl .n:empty { display: none; }
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
    width: 400px; max-width: calc(100vw - 24px); max-height: calc(100vh - 100px);
    display: flex; flex-direction: column;
    background: rgba(255, 255, 255, 0.92);
    backdrop-filter: blur(22px) saturate(170%);
    border: 1px solid var(--border);
    border-radius: 16px;
    box-shadow: 0 18px 48px rgba(0, 0, 0, 0.20), 0 2px 8px rgba(0, 0, 0, 0.06);
    transform-origin: 90% 0;
    animation: popoverIn var(--dur) var(--ease-fluid) both;
  }
  /* Context menus (tab, group) are narrow and open from the item's left edge, like macOS menus. */
  .floating-sheet[data-panel="tabmenu"], .floating-sheet[data-panel="groupmenu"] { width: 250px; padding: 6px; }
  #group-swatches { gap: 6px; flex-wrap: nowrap; }
  #group-swatches .swatch { width: 18px; height: 18px; flex-shrink: 0; }
  .floating-sheet.closing { animation: popoverOut var(--dur-fast) var(--ease-in) both; pointer-events: none; }
  body.dark .floating-sheet { background: rgba(30, 30, 34, 0.92); }
  /* All popovers share one width (Menu, Downloads, Library). */
  /* Settings rows: one column, hairline separators, iOS switches. */
  .sheet-scroll .grid-toggles { grid-template-columns: 1fr 1fr; column-gap: 14px; row-gap: 0; }
  .sheet-scroll .toggle-tile {
    background: transparent; border-radius: 0; padding: 5px 0; gap: 6px;
    font-weight: 500; font-size: 11.5px; border-bottom: 1px solid var(--border);
  }
  .sheet-scroll .toggle-tile:nth-last-child(-n+2):nth-child(odd),
  .sheet-scroll .toggle-tile:last-child { border-bottom: none; }
  .switch {
    position: relative; width: 30px; height: 18px; flex-shrink: 0; border: none; border-radius: 9px; padding: 0; cursor: pointer;
    background: rgba(120, 120, 128, 0.32); transition: background 0.22s var(--ease-fluid);
  }
  .switch > i {
    position: absolute; top: 2px; left: 2px; width: 14px; height: 14px; border-radius: 50%; background: #fff;
    box-shadow: 0 1px 3px rgba(0,0,0,0.25); transition: transform 0.22s var(--ease-fluid);
  }
  .switch.on { background: var(--blue); }

  /* Reader button in the address bar: shown only when the page is an article (Safari-style). */
  .reader-btn { border: none; background: transparent; color: var(--muted); width: 28px; height: 24px; border-radius: 7px;
    display: inline-flex; align-items: center; justify-content: center; cursor: pointer; flex-shrink: 0;
    animation: popoverIn var(--dur) var(--ease-fluid) both; transition: background .16s var(--ease-fluid), color .16s var(--ease-fluid); }
  .reader-btn:hover { background: rgba(118,118,128,.14); color: var(--text); }
  .reader-btn.on { background: var(--blue); color: #fff; }

  /* Password save prompt: inline in the toolbar, never blocks the page. */
  .pw-offer {
    display: flex; align-items: center; gap: 6px; height: 30px; padding: 0 4px 0 10px; margin-right: 4px;
    border-radius: 15px; background: var(--blue-soft); font-size: 12px; white-space: nowrap; max-width: 420px;
    animation: popoverIn var(--dur) var(--ease-fluid) both;
  }
  .pw-offer .t { overflow: hidden; text-overflow: ellipsis; }
  .pw-offer .t b { font-weight: 600; }
  .pw-offer .save { border: none; background: var(--blue); color: #fff; border-radius: 11px; padding: 3px 10px; font-size: 12px; font-weight: 600; cursor: pointer; }
  .perm-offer .deny { border: none; background: rgba(118,118,128,.16); color: var(--text); border-radius: 11px; padding: 3px 10px; font-size: 12px; font-weight: 600; cursor: pointer; }
  .perm-site { font-size: 12px; font-weight: 600; padding: 8px 10px 2px; display: flex; align-items: center; justify-content: space-between; }
  .perm-row { display: flex; align-items: center; gap: 8px; padding: 5px 10px; font-size: 12px; }
  .perm-row .lbl { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .perm-seg { display: inline-flex; background: rgba(118,118,128,.14); border-radius: 8px; padding: 2px; }
  .perm-seg button { border: none; background: transparent; color: var(--text-dim, inherit); font-size: 11px; padding: 2px 9px; border-radius: 6px; cursor: pointer; }
  .perm-seg button.on { background: var(--surface, #fff); color: var(--text); box-shadow: 0 1px 2px rgba(0,0,0,.12); font-weight: 600; }
  .pw-row { display: flex; align-items: center; gap: 9px; padding: 8px 10px; position: relative; }
  .pw-row + .pw-row::before { content: ""; position: absolute; top: 0; left: 44px; right: 0; border-top: 1px solid var(--border); }
  .pw-av { width: 26px; height: 26px; border-radius: 7px; flex-shrink: 0; display: flex; align-items: center; justify-content: center;
    font-size: 12px; font-weight: 700; color: #fff; background: linear-gradient(180deg, #48b2ff, #0062eb); }
  .pw-main { flex: 1; min-width: 0; }
  .pw-host { font-size: 12.5px; font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pw-user { font-size: 11px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pw-secret { font-family: ui-monospace, "Cascadia Mono", Consolas, monospace; font-size: 11.5px; color: var(--text); user-select: text; }
  .pw-edit { display: flex; flex-direction: column; gap: 5px; padding: 6px 10px 10px 45px; }

  /* Peek: dimmed backdrop + a floating sheet whose body is the native preview web view. */
  #peek-layer { position: fixed; inset: 0; z-index: 80; display: none; }
  #peek-layer.open { display: block; }
  #peek-backdrop { position: absolute; inset: 0; background: rgba(0, 0, 0, 0.28); animation: fadeIn var(--dur) var(--ease-fluid) both; }
  body.dark #peek-backdrop { background: rgba(0, 0, 0, 0.45); }
  #peek-frame {
    position: absolute; border-radius: 14px; background: var(--surface-solid);
    box-shadow: 0 24px 70px rgba(0,0,0,0.35), 0 0 0 0.5px rgba(0,0,0,0.12);
    animation: popoverIn var(--dur) var(--ease-fluid) both; transform-origin: 50% 0;
  }
  #peek-head { height: 46px; display: flex; align-items: center; gap: 8px; padding: 0 10px 0 14px; }
  #peek-url { flex: 1; min-width: 0; font-size: 12px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  #peek-url b { color: var(--text); font-weight: 600; }
  @keyframes fadeIn { from { opacity: 0; } to { opacity: 1; } }
  .switch.on > i { transform: translateX(12px); }
  .sheet-scroll .card {
    background: rgba(118, 118, 128, 0.07); border: none; border-radius: 12px; padding: 11px 12px; margin-bottom: 8px;
  }
  body.dark .sheet-scroll .card { background: rgba(255, 255, 255, 0.05); }
  .sheet-scroll .card-title { font-size: 13px; font-weight: 600; letter-spacing: -0.01em; }
  /* Popovers dismiss by clicking outside or Esc, like macOS; no close buttons inside. */
  .sheet-scroll button[onclick="closeDrawer()"] { display: none; }

  /* Segmented control */
  .segmented {
    display: flex; padding: 2px; gap: 2px; border-radius: 9px; background: var(--input-bg); margin: 0 0 8px;
  }
  .segmented button {
    flex: 1 1 auto; min-width: 0; border: none; background: transparent; color: var(--text);
    font-size: 11.5px; font-weight: 500; padding: 4px 6px; border-radius: 7px; cursor: pointer;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
    transition: background 0.2s var(--ease-fluid), box-shadow 0.2s var(--ease-fluid);
  }
  .segmented button .n { color: var(--muted); font-weight: 500; margin-left: 3px; font-variant-numeric: tabular-nums; }
  .segmented button.on { background: var(--surface-solid); box-shadow: 0 1px 3px rgba(0,0,0,0.12), 0 0 0 0.5px rgba(0,0,0,0.04); font-weight: 600; }
  body.dark .segmented button.on { background: #636366; }

  /* Download rows (Safari downloads list) */
  .dl-group { border-radius: 12px; background: rgba(118,118,128,0.07); overflow: hidden; }
  body.dark .dl-group { background: rgba(255,255,255,0.05); }
  .dl-row { display: flex; align-items: center; gap: 10px; padding: 9px 10px; position: relative; }
  .dl-row + .dl-row::before { content: ""; position: absolute; top: 0; left: 52px; right: 0; border-top: 1px solid var(--border); }
  .dl-icon {
    width: 32px; height: 38px; flex-shrink: 0; border-radius: 5px; position: relative;
    background: linear-gradient(180deg, var(--surface-solid), var(--input-bg)); border: 1px solid var(--border);
    display: flex; align-items: flex-end; justify-content: center; padding-bottom: 5px;
  }
  .dl-icon b { font-size: 7.5px; font-weight: 800; letter-spacing: 0.02em; color: #fff; padding: 1px 3px; border-radius: 3px; background: var(--muted); max-width: 30px; overflow: hidden; }
  .dl-icon.c-video b { background: #AF52DE; } .dl-icon.c-audio b { background: #FF2D55; }
  .dl-icon.c-lossless b { background: #FF9500; }
  .dl-icon.c-archive b { background: #A2845E; } .dl-icon.c-program b { background: #5856D6; }
  .dl-icon.c-doc b { background: #0A84FF; }
  .dl-main { flex: 1; min-width: 0; }
  .dl-name { font-size: 12.5px; font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dl-sub { font-size: 11px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-variant-numeric: tabular-nums; }
  .dl-sub.err { color: var(--red); }
  .dl-bar { height: 4px; border-radius: 2px; background: var(--input-bg); overflow: hidden; margin: 4px 0 3px; }
  .dl-bar > i { display: block; height: 100%; background: var(--blue); border-radius: 2px; transition: width 0.4s var(--ease-fluid); }
  .dl-bar.paused > i { background: var(--muted); }
  .dl-actions { display: flex; gap: 4px; flex-shrink: 0; }
  .icon-btn {
    width: 24px; height: 24px; border-radius: 50%; border: none; cursor: pointer; padding: 0;
    display: inline-flex; align-items: center; justify-content: center;
    background: var(--input-bg); color: var(--muted);
    transition: background 0.16s var(--ease-fluid), color 0.16s var(--ease-fluid), transform 0.12s var(--ease-fluid);
  }
  .icon-btn:hover { color: var(--text); background: rgba(118,118,128,0.2); }
  .icon-btn:active { transform: scale(0.9); }
  .icon-btn.primary { color: var(--blue); }
  .sheet-head { display: flex; align-items: center; justify-content: space-between; margin: 0 2px 8px; gap: 6px; }
  .sheet-head h3 { font-size: 15px; font-weight: 700; letter-spacing: -0.02em; }
  .text-btn { border: none; background: none; color: var(--blue); font-size: 12px; font-weight: 500; cursor: pointer; padding: 3px 4px; border-radius: 6px; }
  .text-btn:hover { background: var(--blue-soft); }
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
        <svg width="28" height="28" viewBox="0 0 28 28" fill="none"><defs><linearGradient id="shunnoTile" x1="0" y1="1.5" x2="0" y2="26.5" gradientUnits="userSpaceOnUse"><stop offset="0%" stop-color="#48B2FF"/><stop offset="100%" stop-color="#0062EB"/></linearGradient></defs><rect x="0.5" y="0.5" width="27" height="27" rx="7.5" fill="url(#shunnoTile)"/><path d="M21.50 12.27A7.70 7.70 0 1 1 15.73 6.50" stroke="white" stroke-width="3.30" stroke-linecap="round" fill="none"/><circle cx="19.44" cy="8.56" r="1.98" fill="#FFC43D"/></svg>
      </div>
      <button class="space-pill" id="space-pill" onclick="toggleDrawer('spaces', this)" title="Spaces"></button>
      <div id="tabs-container"></div>
      <button class="tab-add" onclick="openNewTab()" title="New Tab (Ctrl+T)">
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
        <!-- Windows' own caption glyphs (Segoe Fluent Icons / MDL2): the same marks as every other app. -->
        <button class="win-btn" onclick="send('WIN_MINIMIZE')" title="Minimize"><span class="cap-glyph">&#xE921;</span></button>
        <button class="win-btn" id="btn-max" onclick="closeSnapFlyout(); send('WIN_MAXIMIZE')" onmouseenter="startSnapHover()" onmouseleave="cancelSnapHover()" title="Maximize"><span class="cap-glyph">&#xE922;</span></button>
        <button class="win-btn close" onclick="send('WIN_CLOSE')" title="Close"><span class="cap-glyph">&#xE8BB;</span></button>
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
        <button class="reader-btn" id="reader-btn" style="display:none" onclick="send('READER_MODE')" title="Reader (F9)">
          <svg width="16" height="14" viewBox="0 0 20 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><path d="M10 3.2C8.2 1.9 5.6 1.4 2 1.5v11.6c3.6-.1 6.2.4 8 1.7 1.8-1.3 4.4-1.8 8-1.7V1.5c-3.6-.1-6.2.4-8 1.7z"/><path d="M10 3.2v11.6"/></svg>
        </button>
      </div>

      <div class="nav-group right">
        <div id="perm-offer" class="pw-offer perm-offer" style="display:none"></div>
        <div class="ext-bar" id="ext-bar"></div>
        <button class="profile-btn" id="profile-btn" onclick="toggleDrawer('profiles', this)" title="Profiles"></button>
        <div id="pw-offer" class="pw-offer" style="display:none"></div>
        <button class="tb-icon" id="btn-bookmark" onclick="promptSaveBookmarkWithFolder()" title="Bookmark this page">
          <svg width="15" height="16" viewBox="0 0 15 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"><path d="M3 1.75h9a.75.75 0 0 1 .75.75v12.1l-5.25-3.3-5.25 3.3V2.5A.75.75 0 0 1 3 1.75z"/></svg>
        </button>
        <button class="tb-icon" id="btn-library" onclick="toggleDrawer('bookmarks', this)" title="Library: bookmarks and history">
          <svg width="18" height="16" viewBox="0 0 18 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"><rect x="1.25" y="2.25" width="3.5" height="12" rx=".8"/><rect x="5.75" y="1.25" width="3.5" height="13" rx=".8"/><path d="M10.6 3.4l2.9-.9a.8.8 0 0 1 1 .5l2.9 9.9a.8.8 0 0 1-.5 1l-2.9.9a.8.8 0 0 1-1-.5L10 4.4a.8.8 0 0 1 .6-1z"/></svg>
        </button>
        <button class="nav-btn on dl" id="btn-downloads" onclick="toggleDrawer('downloads')" title="Downloads (Ctrl+J)">
          <svg width="12" height="13" viewBox="0 0 12 13" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M6 1.5v7.2M2.8 5.6L6 8.8l3.2-3.2M1.5 11.6h9"/></svg><span class="n" id="dl-count"></span>
        </button>
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
      <svg width="58" height="58" viewBox="0 0 28 28" fill="none"><rect x="0.5" y="0.5" width="27" height="27" rx="7.5" fill="url(#shunnoTile)"/><path d="M21.50 12.27A7.70 7.70 0 1 1 15.73 6.50" stroke="white" stroke-width="3.30" stroke-linecap="round" fill="none"/><circle cx="19.44" cy="8.56" r="1.98" fill="#FFC43D"/></svg>
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
            <button class="pill-btn pill-soft" onclick="closeDrawer(); send('LITTLE_OPEN');" title="Little Shunno: a small quick window (Ctrl+Alt+N from anywhere)">🪟 Little</button>
            <button class="pill-btn pill-soft" onclick="send('SET_DEFAULT_BROWSER')" title="Make Shunno the browser for links from other apps">⭐ Set as Default</button>
          </div>

          <div style="display:flex;align-items:center;gap:10px;margin:2px 0 6px;">
            <span style="font-size:11.5px;font-weight:500;white-space:nowrap;">🌓 Appearance</span>
            <div class="segmented" id="theme-seg" style="flex:1;margin:0;">
              <button data-mode="system" onclick="setThemeMode('system')">System</button>
              <button data-mode="light" onclick="setThemeMode('light')">Light</button>
              <button data-mode="dark" onclick="setThemeMode('dark')">Dark</button>
            </div>
          </div>
          <div class="grid-toggles" id="global-toggles-grid"></div>
          <div class="meta" id="menu-note" style="display:none;margin-top:4px;"></div>
          <div class="meta" id="force-dark-note" style="display:none;margin-top:2px;">Darken All Sites applies after you restart Shunno.</div>
          <div class="toggle-tile" style="border-top:1px solid var(--border);border-bottom:none;margin-top:0;" title="Unpinned tabs you have not looked at for this long close by themselves (Arc-style). Reopen with Ctrl+Shift+T or from History. Pinned, playing and private tabs are kept.">
            <span>🗄️ Close unused tabs after</span>
            <select class="select-box" id="auto-archive-select" style="padding:3px 6px;font-size:11.5px;" onchange="send('SET_AUTO_ARCHIVE:' + this.value)">
              <option value="0">Never</option>
              <option value="12">12 hours</option>
              <option value="24">1 day</option>
              <option value="168">7 days</option>
            </select>
          </div>
        </div>

        <div class="card">
          <div class="card-header">
            <span class="card-title" id="site-domain-title">🌐 Site Controls</span>
            <button class="pill-btn pill-orange" id="btn-clear-site-data" onclick="send('CLEAR_SITE_DATA')" title="Clear cookies, localStorage, sessionStorage & cache for this site">🧹 Clear Site Data</button>
            <button class="pill-btn pill-soft" onclick="toggleDrawer('passwords', document.getElementById('btn-controls'))" title="Saved passwords">🔑 Passwords</button>
            <button class="pill-btn pill-soft" onclick="toggleDrawer('permissions', document.getElementById('btn-controls'))" title="Camera, microphone, location and more, per site">🛡️ Permissions</button>
            <button class="pill-btn pill-soft" onclick="toggleDrawer('profiles', document.getElementById('btn-controls'))" title="Separate cookies, logins and history">👤 Profiles</button>
            <button class="pill-btn pill-soft" onclick="toggleDrawer('extensions', document.getElementById('btn-controls'))" title="Add and manage browser extensions">🧩 Extensions</button>
            <button class="pill-btn pill-soft" onclick="send('NEW_WINDOW'); closeDrawer()" title="New Window (Ctrl+N)">🪟 New Window</button>
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
        <div class="card" id="import-card" style="display:none">
          <div class="card-header">
            <span class="card-title">Import from another browser</span>
            <button class="pill-btn pill-red" onclick="document.getElementById('import-card').style.display='none'">✕</button>
          </div>
          <div style="display:flex;gap:14px;margin:2px 0 8px;font-size:12px;">
            <label><input type="checkbox" id="imp-b" checked> Bookmarks</label>
            <label><input type="checkbox" id="imp-h" checked> History</label>
          </div>
          <div id="import-sources"></div>
          <div class="list-row">
            <div class="meta">Shunno backup file (shunno_bookmarks.txt in Downloads)</div>
            <button class="pill-btn pill-soft" onclick="send('IMPORT_TXT_BOOKMARKS')">Import</button>
          </div>
          <div class="meta" id="import-status" style="margin-top:6px;"></div>
          <div class="meta" style="margin-top:6px;">Passwords: export them as CSV from the other browser's password settings; then use Import… in the Passwords panel.</div>
        </div>
        <div class="card">
          <div class="card-header">
            <span class="card-title">⭐ Bookmarks</span>
            <div style="display:flex;gap:5px;">
              <button class="pill-btn pill-blue" onclick="promptSaveBookmarkWithFolder()">+ Save</button>
              <button class="pill-btn pill-soft" onclick="openImport()">Import…</button>
              <button class="pill-btn pill-soft" onclick="send('EXPORT_TXT_BOOKMARKS')" title="Save shunno_bookmarks.txt to your downloads folder">Export</button>
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
      <div id="panel-spaces" style="display:none">
        <div class="sheet-head">
          <h3>Spaces</h3>
          <button class="text-btn" onclick="spaceForm(null)">New Space</button>
        </div>
        <div id="spaces-list"></div>
        <div class="card" id="space-form" style="display:none;margin-top:8px;">
          <div class="card-title" id="space-form-title" style="margin-bottom:8px;">New Space</div>
          <div style="display:flex;gap:6px;margin-bottom:8px;">
            <input class="text-box" id="space-icon" maxlength="2" style="width:44px;text-align:center;" placeholder="✦" />
            <input class="text-box" id="space-name" maxlength="24" style="flex:1;" placeholder="Name (e.g. Work, Study)" />
          </div>
          <div class="swatches" id="space-swatches"></div>
          <div style="display:flex;gap:6px;justify-content:space-between;margin-top:10px;">
            <button class="text-btn" id="space-delete" style="color:var(--red);" onclick="spaceDelete()">Delete Space</button>
            <div style="display:flex;gap:6px;">
              <button class="text-btn" onclick="document.getElementById('space-form').style.display='none'">Cancel</button>
              <button class="pill-btn pill-blue" onclick="spaceSave()">Save</button>
            </div>
          </div>
        </div>
        <div class="meta" id="space-move" style="margin-top:10px;"></div>
        <div class="meta" style="margin-top:8px;line-height:1.45;">Each Space keeps its own tabs. New tabs open in the current Space. Spaces share cookies and logins.</div>
      </div>

      <div id="panel-passwords" style="display:none">
        <div class="sheet-head">
          <h3>Passwords</h3>
          <div style="display:flex;gap:4px;align-items:center;">
            <button class="text-btn" onclick="send('PW_IMPORT')" title="Import a CSV exported from Chrome, Edge, Brave, Firefox, Opera, Vivaldi or Bitwarden">Import…</button>
          </div>
        </div>
        <input class="text-box" id="pw-search" placeholder="Search sites and usernames" style="width:100%;margin-bottom:8px;" oninput="renderPasswords(lastState)" />
        <div id="pw-list"></div>
        <div class="meta" id="pw-status" style="margin-top:8px;"></div>
        <button class="pill-btn pill-red" id="pw-del-csv" style="display:none;margin-top:6px;" onclick="send('PW_DELETE_CSV')">Delete the imported CSV file</button>
        <div class="meta" style="margin-top:8px;line-height:1.45;">
          Stored only on this PC, encrypted with your Windows account. Showing or copying a password asks for Windows Hello.
          <a href="#" onclick="event.preventDefault(); if (confirm('Delete passwords kept by the old built-in saver (before v2.17)? Your Shunno passwords are not affected.')) send('CLEAR_PASSWORDS');" style="color:var(--blue);">Clear old built-in passwords</a>
        </div>
      </div>

      <div id="panel-extensions" style="display:none">
        <div class="sheet-head">
          <h3>Extensions</h3>
          <div style="display:flex;gap:6px;">
            <button class="text-btn" onclick="send('EXT_ADD_FILE')" title="A .crx or .zip extension file">Add File…</button>
            <button class="text-btn" onclick="send('EXT_ADD_FOLDER')" title="An unpacked extension folder with manifest.json">Add Folder…</button>
          </div>
        </div>
        <div class="ext-notice" id="ext-notice"></div>
        <div id="ext-list"></div>
        <div class="meta" style="margin-top:8px;line-height:1.45;">
          Add a Chrome or Edge extension as a <b>.crx</b> or <b>.zip</b> file or an unpacked folder; Shunno keeps its own copy.
          The Chrome Web Store cannot install into Shunno directly. Extensions run in normal tabs, not private ones, and some that depend on Chrome only features may not work.
        </div>
      </div>

      <div id="panel-tabmenu" style="display:none"><div id="tabmenu-body"></div></div>

      <div id="panel-groupmenu" style="display:none">
        <input class="text-box" id="group-name" maxlength="30" style="width:100%;margin-bottom:8px;" placeholder="Name this group"
          oninput="groupSaveSoon()" onkeydown="if (event.key === 'Enter') { groupSaveNow(); closeDrawer(); }" />
        <div class="swatches" id="group-swatches" style="margin-bottom:6px;"></div>
        <div id="groupmenu-body"></div>
      </div>

      <div id="panel-profiles" style="display:none">
        <div class="sheet-head">
          <h3>Profiles</h3>
          <button class="text-btn" onclick="profileForm(null)">New Profile</button>
        </div>
        <div id="profiles-list"></div>
        <div class="prof-links" id="prof-links">
          <span>Open links from other apps in</span>
          <select id="prof-links-sel" onchange="send('PROFILE_LINKS:' + this.value)"></select>
        </div>
        <div class="card" id="profile-form" style="display:none;margin-top:8px;">
          <div class="card-title" id="profile-form-title" style="margin-bottom:8px;">New Profile</div>
          <input class="text-box" id="profile-name" maxlength="40" style="width:100%;margin-bottom:8px;" placeholder="Name (e.g. Work, School)"
            onkeydown="if (event.key === 'Enter') profileSave()" />
          <div class="swatches" id="profile-swatches"></div>
          <div style="display:flex;gap:6px;justify-content:space-between;margin-top:10px;">
            <button class="text-btn" id="profile-delete" style="color:var(--red);" onclick="profileDelete()">Delete Profile</button>
            <div style="display:flex;gap:6px;">
              <button class="text-btn" onclick="document.getElementById('profile-form').style.display='none'">Cancel</button>
              <button class="pill-btn pill-blue" id="profile-save" onclick="profileSave()">Create</button>
            </div>
          </div>
        </div>
        <div class="meta" style="margin-top:8px;line-height:1.45;">
          Each profile opens in its own window with separate cookies, sign-ins, history, bookmarks, passwords, permissions and settings.
          Spaces, by contrast, share one profile. Ctrl+Alt+N opens Little Shunno in the profile you used last.
        </div>
      </div>

      <div id="panel-permissions" style="display:none">
        <div class="sheet-head"><h3>Site Permissions</h3></div>
        <div id="perm-list"></div>
        <div class="meta" style="margin-top:8px;line-height:1.45;">
          Sites ask in the toolbar the first time. Your answer is kept per site. Private tabs ask every time and remember nothing.
          Peek asks here too; Little Shunno asks in its own bar.
        </div>
      </div>

      <div id="panel-downloads" style="display:none">
        <div class="sheet-head">
          <h3>Downloads <span class="meta" id="dl-live-speed" style="font-weight:500;letter-spacing:0;"></span></h3>
          <div style="display:flex;gap:4px;align-items:center;">
            <button class="text-btn" onclick="send('CLEAR_FINISHED_DL')" title="Remove finished and failed items from this list. Files stay on disk.">Clear</button>
            <button class="icon-btn" onclick="triggerGrabPage()" title="Find downloadable files and videos on this page"><svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><circle cx="7" cy="7" r="4.6"/><path d="M10.5 10.5L14 14"/></svg></button>
            <button class="icon-btn" onclick="send('OPEN_DOWNLOADS_FOLDER')" title="Open downloads folder"><svg width="13" height="13" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><path d="M1.8 4.2a1.2 1.2 0 0 1 1.2-1.2h3l1.5 1.6h5.5a1.2 1.2 0 0 1 1.2 1.2v6.2a1.2 1.2 0 0 1-1.2 1.2H3a1.2 1.2 0 0 1-1.2-1.2z"/></svg></button>
            <button class="icon-btn" id="btn-dl-settings" onclick="toggleDlSettings()" title="Download settings"><svg width="13" height="13" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"><path d="M2 4.5h7M12 4.5h2M2 11.5h2M7 11.5h7"/><circle cx="10.5" cy="4.5" r="1.6"/><circle cx="5.5" cy="11.5" r="1.6"/></svg></button>
          </div>
        </div>
        <div class="card">
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

        <div class="segmented" id="dl-status-bar"></div>
        <div class="folder-pills" id="dl-category-bar"></div>
        <div id="duplicate-banner" class="card" style="display:none;border-color:var(--orange);"></div>
        <div id="downloads-list"></div>
      </div>

      </div>
    </div>
  </div>

  <div id="peek-layer">
    <div id="peek-backdrop" onclick="send('PEEK_CLOSE')" title="Close preview (Esc)"></div>
    <div id="peek-frame">
      <div id="peek-head">
        <div id="peek-url"></div>
        <button class="text-btn" onclick="send('PEEK_OPEN')" title="Keep this page as a tab">Open in New Tab</button>
        <button class="icon-btn" onclick="send('PEEK_CLOSE')" title="Close (Esc)"><svg width="9" height="9" viewBox="0 0 10 10" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><path d="M2 2l6 6M8 2l-6 6"/></svg></button>
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
  let activeDlStatus = 'all';
  const DL_STATUS_GROUPS = [
    ['all', 'All', () => true],
    ['active', 'Downloading', d => d.status === 'DOWNLOADING' || d.status === 'QUEUED' || d.status === 'MUXING'],
    ['done', 'Downloaded', d => d.status === 'COMPLETED'],
    ['paused', 'Paused', d => d.status === 'PAUSED'],
    ['failed', 'Failed', d => d.status === 'FAILED'],
  ];
  function setDlStatus(k) { activeDlStatus = k; renderDownloadsList(lastState); }
  function fmtEta(sec) {
    if (!isFinite(sec) || sec <= 0) return '';
    if (sec < 60) return Math.ceil(sec) + ' s left';
    if (sec < 3600) return Math.ceil(sec / 60) + ' min left';
    const h = Math.floor(sec / 3600), m = Math.round((sec % 3600) / 60);
    return h + ' h ' + m + ' min left';
  }
  function dlIconClass(cat) {
    const c = String(cat || '').toLowerCase();
    if (c.includes('video')) return 'c-video';
    if (c.includes('lossless')) return 'c-lossless';
    if (c.includes('audio')) return 'c-audio';
    if (c.includes('archive')) return 'c-archive';
    if (c.includes('program')) return 'c-program';
    if (c.includes('doc')) return 'c-doc';
    return '';
  }
  const SVG_PAUSE = '<svg width="10" height="10" viewBox="0 0 10 10"><rect x="2" y="1.5" width="2" height="7" rx="0.6" fill="currentColor"/><rect x="6" y="1.5" width="2" height="7" rx="0.6" fill="currentColor"/></svg>';
  const SVG_RESUME = '<svg width="11" height="11" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M10 6a4 4 0 1 1-1.2-2.85"/><path d="M9.2 1.4v2.2H7"/></svg>';
  const SVG_REVEAL = '<svg width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round"><circle cx="7" cy="7" r="4.6"/><path d="M10.5 10.5L14 14"/></svg>';
  const SVG_LINK = '<svg width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><path d="M6.5 9.5l3-3"/><path d="M7.3 4.3l1-1a2.8 2.8 0 0 1 4 4l-1 1"/><path d="M8.7 11.7l-1 1a2.8 2.8 0 0 1-4-4l1-1"/></svg>';
  const SVG_X = '<svg width="9" height="9" viewBox="0 0 10 10" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><path d="M2 2l6 6M8 2l-6 6"/></svg>';
  const speedHistory = new Array(36).fill(0);
  let peakSpeedBps = 0;

  function promptSaveBookmarkWithFolder() {
    const folder = prompt('Save bookmark to folder:', activeBookmarkFolder === 'All' ? 'General' : activeBookmarkFolder);
    if (folder === null) return;
    send('ADD_BOOKMARK:' + (folder.trim() || 'General'));
  }

  let snapHoverTimer = null;
  let snapCloseTimer = null;
  // Windows 11: hovering Maximize opens the system's own Snap Layouts flyout (same as every other app).
  // Windows 10 (no Snap Layouts) and background windows use Shunno's flyout.
  let snapNative = false;
  function startSnapHover() {
    if (snapNative) return; // Windows shows Snap Layouts itself over the native button.
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
  // Tell the window where the maximize button is and how to paint it (physical px, opaque colours).
  function parseRgba(c) {
    c = c || '';
    // color-mix() resolves to "color(srgb r g b / a)" with 0..1 channels.
    let m = c.match(/color\(srgb\s+([^)]+)\)/);
    if (m) {
      const [rgb, a] = m[1].split('/');
      const p = rgb.trim().split(/\s+/).map(v => parseFloat(v) * 255);
      return [p[0], p[1], p[2], a !== undefined ? parseFloat(a) : 1];
    }
    m = c.match(/rgba?\(([^)]+)\)/);
    if (!m) return null;
    const p = m[1].split(',').map(v => parseFloat(v));
    return [p[0], p[1], p[2], p.length > 3 ? p[3] : 1];
  }
  function over(top, base) { const a = top[3]; return [0, 1, 2].map(i => top[i] * a + base[i] * (1 - a)); }
  function hex(c) { return '#' + c.map(v => Math.max(0, Math.min(255, Math.round(v))).toString(16).padStart(2, '0')).join(''); }
  let lastMaxBtn = '';
  function reportMaxButton() {
    if (!snapNative) return;
    const b = document.getElementById('btn-max');
    if (!b) return;
    const r = b.getBoundingClientRect();
    const d = window.devicePixelRatio || 1;
    // Composite the backgrounds behind the button over the window's own frame colour.
    const chain = [];
    for (let el = b.parentElement; el; el = el.parentElement) chain.push(el);
    let col = document.body.classList.contains('dark') ? [0x1C, 0x1C, 0x1E] : [255, 255, 255];
    for (const el of chain.reverse()) {
      const c = parseRgba(getComputedStyle(el).backgroundColor);
      if (c && c[3] > 0) col = over(c, col);
    }
    const fg = parseRgba(getComputedStyle(b).color) || [0, 0, 0, 1];
    const hov = over([128, 128, 128, 0.18], col);
    const msg = `MAXBTN:${r.left * d}|${r.top * d}|${r.width * d}|${r.height * d}|${hex(col)}|${hex(over(fg, col))}|${hex(hov)}|${7 * d}|${10 * d}`;
    if (msg !== lastMaxBtn) { lastMaxBtn = msg; send(msg); }
  }
  window.addEventListener('resize', () => requestAnimationFrame(reportMaxButton));
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
    return document.getElementById(name === 'downloads' ? 'btn-downloads' : name === 'bookmarks' ? 'btn-library' : 'btn-controls');
  }

  // Place the popover under its anchor; the caret points at the anchor's centre.
  function positionSheet() {
    const sheet = document.getElementById('floating-sheet');
    const anchor = (drawerAnchor && drawerAnchor.isConnected) ? drawerAnchor : defaultAnchorFor(activeDrawer);
    if (!sheet || !anchor) return;
    const r = anchor.getBoundingClientRect();
    const vw = window.innerWidth, vh = window.innerHeight;
    const w = Math.min(sheet.offsetWidth || 430, vw - 24);
    const isMenu = activeDrawer === 'tabmenu' || activeDrawer === 'groupmenu';
    let left = isMenu ? Math.round(r.left - 4) : Math.round(r.right - w + 4);
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

    sheet.dataset.panel = name;

    ['controls', 'find', 'streams', 'bookmarks', 'downloads', 'passwords', 'spaces', 'permissions', 'profiles', 'tabmenu', 'groupmenu', 'extensions'].forEach(p => {
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

  // Instant feedback: highlight the choice now; the native side applies it and pushes state.
  function setThemeMode(m) {
    document.querySelectorAll('#theme-seg button').forEach(b => b.classList.toggle('on', b.dataset.mode === m));
    send('SET_THEME:' + m);
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

  const ICON_MAX = '<span class="cap-glyph">&#xE922;</span>';
  const ICON_RESTORE = '<span class="cap-glyph">&#xE923;</span>';
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
    // Spaces: show only the active Space's tabs (indices stay those of the full list).
    const sp = window.__shunnoLastState && window.__shunnoLastState.spaces ? window.__shunnoLastState.spaces.active : null;
    const inSpace = (t) => sp === null || t.space === undefined || t.space === sp;
    const currentIds = new Set(tabs.filter(inSpace).map(t => String(t.id)));

    // Remove closed tabs
    Array.from(tc.children).forEach(el => {
      if (el.dataset.groupId) return; // group labels are handled after the tabs
      if (!currentIds.has(el.dataset.tabId)) {
        tc.removeChild(el);
        knownTabIds.delete(el.dataset.tabId);
      }
    });

    tabs.forEach((t, idx) => {
      if (!inSpace(t)) return;
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
      el.ondragend = (ev) => {
        const from = draggedTabIdx;
        draggedTabIdx = null;
        el.classList.remove('drag-over-left', 'drag-over-right');
        // Dropped well away from the tab bar, where nothing took it: tear the tab off into a new window.
        const bar = document.getElementById('tab-bar').getBoundingClientRect();
        const away = ev.clientY > bar.bottom + 40 || ev.clientY < -20 || ev.clientX < -20 || ev.clientX > window.innerWidth + 20;
        const st = window.__shunnoLastState;
        if (from !== null && ev.dataTransfer && ev.dataTransfer.dropEffect === 'none' && away && st && st.tabs.length > 1) {
          const dpr = window.devicePixelRatio || 1;
          send('MOVE_TAB_NEW_WINDOW:' + from + '|' + Math.round(ev.screenX * dpr) + '|' + Math.round(ev.screenY * dpr));
        }
      };
      el.onclick = () => send('SWITCH_TAB:' + idx);
      el.onauxclick = (ev) => { if (ev.button === 1) { ev.preventDefault(); ev.stopPropagation(); send('CLOSE_TAB:' + idx); } };
      el.oncontextmenu = (ev) => { ev.preventDefault(); openTabMenu(idx, el); };
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
              ? `<span style="display:inline-flex;align-items:center;flex-shrink:0;" title="${t.pinned ? 'Pinned (Right-click to unpin)' : ''}"><svg width="18" height="18" viewBox="0 0 28 28" fill="none"><rect x="0.5" y="0.5" width="27" height="27" rx="7.5" fill="url(#shunnoTile)"/><path d="M21.50 12.27A7.70 7.70 0 1 1 15.73 6.50" stroke="white" stroke-width="3.30" stroke-linecap="round" fill="none"/><circle cx="19.44" cy="8.56" r="1.98" fill="#FFC43D"/></svg></span>`
              : '');
        const audioHtml = hasAudio
          ? `<span class="tab-audio" onclick="event.stopPropagation(); send('TOGGLE_MUTE_TAB:${idx}')" title="${t.muted ? 'Unmute' : 'Mute'}">${t.muted ? '🔇' : '🔊'}</span>`
          : '';
        el.innerHTML = `${tabLogoHtml}${audioHtml}<span class="tab-title">${escapeHtml(displayTitle)}</span><span class="tab-close" onclick="event.stopPropagation(); send('CLOSE_TAB:${idx}')">✕</span>`;
      }
    });
    syncTabGroups(tabs, inSpace);
  }

  // Tab groups: a label before each group's first tab; member tabs tinted; collapsed groups hide their tabs.
  function syncTabGroups(tabs, inSpace) {
    const tc = document.getElementById('tabs-container');
    const st = window.__shunnoLastState || {};
    const groups = new Map((st.tab_groups || []).map(g => [g.id, g]));
    const want = [];
    const counts = {};
    tabs.forEach(t => { if (inSpace(t) && t.group != null) counts[t.group] = (counts[t.group] || 0) + 1; });
    let prev = null;
    tabs.forEach((t, idx) => {
      if (!inSpace(t)) return;
      const el = tc.querySelector(`.tab[data-tab-id="${t.id}"]`);
      if (!el) return;
      const g = t.group != null ? groups.get(t.group) : null;
      if (g && prev !== g.id && !want.some(n => n.dataset && n.dataset.groupId === String(g.id))) {
        let chip = tc.querySelector(`.tg-chip[data-group-id="${g.id}"]`);
        if (!chip) {
          chip = document.createElement('div');
          chip.className = 'tg-chip';
          chip.dataset.groupId = String(g.id);
        }
        const html = (g.name ? `<span class="nm">${escapeHtml(g.name)}</span>` : '<span class="dot"></span>') + (g.collapsed ? `<span class="ct">${counts[g.id] || 0}</span>` : '');
        if (chip.dataset.h !== html) { chip.dataset.h = html; chip.innerHTML = html; }
        chip.style.setProperty('--tg', g.color);
        chip.title = (g.name || 'Unnamed group') + (g.collapsed ? ' (collapsed, click to expand)' : ' (click to collapse, right-click for options)');
        chip.onclick = () => send('GROUP_TOGGLE:' + g.id);
        chip.oncontextmenu = (ev) => { ev.preventDefault(); openGroupMenu(g.id, chip); };
        want.push(chip);
      }
      prev = g ? g.id : null;
      el.classList.toggle('in-group', !!g);
      el.classList.toggle('tg-hidden', !!(g && g.collapsed));
      if (g) el.style.setProperty('--tg', g.color); else el.style.removeProperty('--tg');
      want.push(el);
    });
    // Drop labels of groups that are gone, then put nodes in order without touching ones already in place.
    Array.from(tc.querySelectorAll('.tg-chip')).forEach(c => { if (!want.includes(c)) c.remove(); });
    want.forEach((node, k) => { if (tc.children[k] !== node) tc.insertBefore(node, tc.children[k] || null); });
    // A group just made from the tab menu: ask for its name.
    if (st.name_group != null && groups.has(st.name_group)) {
      const gid = st.name_group;
      st.name_group = null;
      setTimeout(() => { const c = tc.querySelector(`.tg-chip[data-group-id="${gid}"]`); if (c) openGroupMenu(gid, c); }, 60);
    }
  }

  // ---------------------------------------------------------------- Tab and group menus (right-click)
  let ctxTab = null, ctxGroup = null, groupColor = null, groupSaveTimer = null;
  function ctxRow(label, cmd, extra) {
    return `<button class="ctx-row ${extra && extra.danger ? 'danger' : ''}" onclick="${cmd}; closeDrawer()">${extra && extra.dot ? `<span class="dot" style="background:${extra.dot}"></span>` : ''}${escapeHtml(label)}</button>`;
  }
  function openTabMenu(idx, anchor) {
    const st = lastState; const t = st && st.tabs[idx];
    if (!t) return;
    ctxTab = idx;
    const groups = (st.tab_groups || []).filter(g => st.tabs.some(x => x.group === g.id && x.space === t.space) && g.id !== t.group);
    let h = '';
    h += ctxRow(t.pinned ? 'Unpin Tab' : 'Pin Tab', `send('TOGGLE_PIN_TAB:${idx}')`);
    if (!t.pinned && !t.incognito) {
      h += '<div class="ctx-sep"></div>';
      h += ctxRow('Add Tab to New Group', `send('TAB_GROUP_NEW:${idx}')`);
      if (groups.length) {
        h += '<div class="ctx-label">Move to Group</div>';
        h += groups.map(g => ctxRow(g.name || 'Unnamed group', `send('TAB_GROUP_ADD:${idx}|${g.id}')`, { dot: g.color })).join('');
      }
      if (t.group != null) h += ctxRow('Remove from Group', `send('TAB_UNGROUP:${idx}')`);
    }
    h += '<div class="ctx-sep"></div>';
    if (st.tabs.length > 1) h += ctxRow('Move Tab to New Window', `send('MOVE_TAB_NEW_WINDOW:${idx}')`);
    (st.other_windows || []).forEach((w, n) => {
      h += ctxRow('Move to Window: ' + (w.title || 'Window ' + (n + 2)), `send('MOVE_TAB_TO_WINDOW:${idx}|${w.key}')`);
    });
    h += '<div class="ctx-sep"></div>';
    h += ctxRow(t.muted ? 'Unmute Tab' : 'Mute Tab', `send('TOGGLE_MUTE_TAB:${idx}')`);
    h += ctxRow('Close Tab', `send('CLOSE_TAB:${idx}')`, { danger: true });
    document.getElementById('tabmenu-body').innerHTML = h;
    if (activeDrawer === 'tabmenu') closeDrawer();
    toggleDrawer('tabmenu', anchor);
  }
  function openGroupMenu(gid, anchor) {
    const g = (lastState.tab_groups || []).find(x => x.id === gid);
    if (!g) return;
    ctxGroup = gid; groupColor = g.color;
    document.getElementById('group-name').value = g.name;
    renderGroupSwatches();
    let h = '<div class="ctx-sep"></div>';
    h += ctxRow(g.collapsed ? 'Expand Group' : 'Collapse Group', `send('GROUP_TOGGLE:${gid}')`);
    h += ctxRow('Ungroup', `send('GROUP_UNGROUP:${gid}')`);
    h += ctxRow('Close Group', `send('GROUP_CLOSE:${gid}')`, { danger: true });
    document.getElementById('groupmenu-body').innerHTML = h;
    if (activeDrawer === 'groupmenu') closeDrawer();
    toggleDrawer('groupmenu', anchor);
    setTimeout(() => { const i = document.getElementById('group-name'); i.focus(); i.select(); }, 40);
  }
  function renderGroupSwatches() {
    document.getElementById('group-swatches').innerHTML = GROUP_COLORS.map(c =>
      `<button class="swatch ${c === groupColor ? 'on' : ''}" style="background:${c}" onclick="groupColor='${c}'; renderGroupSwatches(); groupSaveNow()" title="${c}"></button>`).join('');
  }
  const GROUP_COLORS = ['#0A84FF', '#30D158', '#FF9F0A', '#FF375F', '#BF5AF2', '#64D2FF', '#FFD60A', '#8E8E93'];
  function groupSaveNow() {
    clearTimeout(groupSaveTimer);
    if (ctxGroup === null) return;
    send('GROUP_EDIT:' + ctxGroup + '|' + JSON.stringify({ name: document.getElementById('group-name').value, color: groupColor }));
  }
  function groupSaveSoon() { clearTimeout(groupSaveTimer); groupSaveTimer = setTimeout(groupSaveNow, 250); }

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
      { kind: 'Action', title: 'New Window (Ctrl+N)', run: () => send('NEW_WINDOW') },
      { kind: 'Action', title: 'Extensions', run: () => toggleDrawer('extensions') },
      { kind: 'Action', title: 'Move Current Tab to New Window', run: () => send('MOVE_TAB_NEW_WINDOW:' + (st ? st.active_tab : 0)) },
      { kind: 'Action', title: 'Add Current Tab to New Group', run: () => send('TAB_GROUP_NEW:' + (st ? st.active_tab : 0)) },
      { kind: 'Action', title: 'Site Permissions (camera, microphone, location)', run: () => toggleDrawer('permissions') },
      { kind: 'Action', title: 'Profiles: switch or open another profile', run: () => toggleDrawer('profiles') },
      { kind: 'Action', title: 'New Profile', run: () => { toggleDrawer('profiles'); setTimeout(() => profileForm(null), 80); } },
      { kind: 'Action', title: 'Export Download History to CSV (with SHA-256)', run: () => send('EXPORT_DL_CSV') },
      { kind: 'Action', title: 'Sleep Inactive Background Tabs (Save RAM)', run: () => send('SLEEP_INACTIVE_TABS') },
      { kind: 'Action', title: 'Toggle Dark Mode', run: () => send('TOGGLE_GLOBAL:full_dark') },
      { kind: 'Action', title: 'Toggle Reader Mode', run: () => send('READER_MODE') },
      { kind: 'Action', title: 'Clear Current Site Cookies & Storage', run: () => send('CLEAR_SITE_DATA') },
      { kind: 'Action', title: 'Open Downloads Manager (Ctrl+J)', run: () => toggleDrawer('downloads') },
      { kind: 'Action', title: 'New Private Tab', run: () => send('NEW_INCOGNITO') },
      { kind: 'Action', title: 'Little Shunno Window (Ctrl+Alt+N)', run: () => send('LITTLE_OPEN') },
      { kind: 'Action', title: 'New Space', run: () => { toggleDrawer('spaces', document.getElementById('space-pill')); setTimeout(() => spaceForm(null), 80); } },
      { kind: 'Action', title: 'Toggle Fullscreen (F11)', run: () => send('TOGGLE_FULLSCREEN') },
    ];
    actions.forEach(a => items.push(a));
    if (st && st.spaces) {
      st.spaces.list.forEach(spc => {
        if (spc.id !== st.spaces.active) items.push({ kind: 'Space', title: 'Switch to Space: ' + spc.icon + ' ' + spc.name, run: () => send('SPACE_SWITCH:' + spc.id) });
      });
      st.spaces.list.forEach(spc => {
        if (spc.id !== st.spaces.active) items.push({ kind: 'Space', title: 'Move Tab to Space: ' + spc.icon + ' ' + spc.name, run: () => send('TAB_TO_SPACE:' + st.active_tab + '|' + spc.id) });
      });
    }

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
          run: () => send('NAV_NEW_TAB:' + b.url),
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

  function openImport() {
    document.getElementById('import-card').style.display = '';
    send('IMPORT_SCAN');
  }

  function renderImport(state) {
    const list = document.getElementById('import-sources');
    if (!list) return;
    const src = state.import_sources || [];
    list.innerHTML = src.map((s, i) => `
      <div class="list-row">
        <div style="overflow:hidden;">
          <div style="font-size:12.5px;font-weight:600;">${escapeHtml(s.browser)}</div>
          <div class="meta">${escapeHtml(s.profile)}${s.has_bookmarks ? '' : ' · no bookmarks'}${s.has_history ? '' : ' · no history'}</div>
        </div>
        <button class="pill-btn pill-blue" onclick="runImport(${i})">Import</button>
      </div>`).join('');
    document.getElementById('import-status').textContent = state.import_status || '';
  }

  function runImport(i) {
    const what = (document.getElementById('imp-b').checked ? 'b' : '') + (document.getElementById('imp-h').checked ? 'h' : '');
    if (!what) return;
    send('IMPORT_BROWSER:' + i + ':' + what);
  }

  // Report the toolbar's real height so the native layer places web pages flush beneath it.
  (function reportChromeHeight() {
    let last = 0;
    const measure = () => {
      const shell = document.querySelector('.chrome-shell');
      if (!shell) return;
      const h = Math.ceil(shell.getBoundingClientRect().bottom);
      if (h > 0 && h !== last) { last = h; send('CHROME_H:' + h); }
    };
    const start = () => {
      const shell = document.querySelector('.chrome-shell');
      if (!shell) return;
      if (window.ResizeObserver) new ResizeObserver(measure).observe(shell);
      measure();
    };
    if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', start); else start();
  })();

  let peekShownUrl = null;
  function renderPeek(state) {
    const layer = document.getElementById('peek-layer');
    const pk = state.peek;
    if (!pk) { layer.classList.remove('open'); peekShownUrl = null; return; }
    const [x, y, w, h] = pk.rect;
    const frame = document.getElementById('peek-frame');
    frame.style.left = x + 'px';
    frame.style.top = (y - 46) + 'px';
    frame.style.width = w + 'px';
    frame.style.height = (h + 46 + 6) + 'px';
    let host = '', rest = pk.url || '';
    try { const u = new URL(pk.url); host = u.host; rest = u.pathname + u.search; } catch (e) {}
    document.getElementById('peek-url').innerHTML = `<b>${escapeHtml(host)}</b>${escapeHtml(rest === '/' ? '' : rest)}`;
    if (!layer.classList.contains('open')) layer.classList.add('open');
    peekShownUrl = pk.url;
  }
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && document.getElementById('peek-layer').classList.contains('open')) send('PEEK_CLOSE');
  });

  let lastState = null;
  let pwEditing = null;
  const pwRevealed = new Map();
  window.__pwReveal = function (id, pw) {
    pwRevealed.set(id, pw);
    setTimeout(() => { pwRevealed.delete(id); if (lastState) renderPasswords(lastState); }, 30000);
    if (lastState) renderPasswords(lastState);
  };
  function pwHost(o) { try { return new URL(o).host.replace(/^www\./, ''); } catch (e) { return o; } }
  function renderPwOffer(state) {
    const el = document.getElementById('pw-offer');
    const o = state.pw_offer;
    if (!o) { el.style.display = 'none'; el.dataset.k = ''; return; }
    const k = o.origin + '|' + o.username + '|' + o.kind;
    if (el.dataset.k === k) return;
    el.dataset.k = k;
    const verb = o.kind === 'Update' ? 'Update password' : 'Save password';
    el.title = o.username ? o.username + ' on ' + pwHost(o.origin) : pwHost(o.origin);
    el.innerHTML = `<span class="t">🔑 ${verb} for <b>${escapeHtml(pwHost(o.origin))}</b>?</span>
      <button class="save" onclick="send('PW_SAVE')">${o.kind === 'Update' ? 'Update' : 'Save'}</button>
      ${o.kind === 'Update' ? '' : `<button class="text-btn" onclick="send('PW_NEVER')" title="Never offer for this site">Never</button>`}
      <button class="icon-btn" onclick="send('PW_DISMISS')" title="Not now">${SVG_X}</button>`;
    el.style.display = 'flex';
  }
  function renderPermOffer(state) {
    const el = document.getElementById('perm-offer');
    const o = state.perm_prompt;
    if (!o) { el.style.display = 'none'; el.dataset.k = ''; return; }
    const k = String(o.id);
    if (el.dataset.k === k) return;
    el.dataset.k = k;
    el.title = o.origin + (o.private ? ' (private tab: not remembered)' : '');
    el.innerHTML = `<span class="t">${escapeHtml(o.icon)} <b>${escapeHtml(o.host)}</b> wants to ${escapeHtml(o.label)}</span>
      <button class="deny" onclick="send('PERM_DECIDE:${o.id}|block')">Block</button>
      <button class="save" onclick="send('PERM_DECIDE:${o.id}|allow')">Allow</button>`;
    el.style.display = 'flex';
  }
  function renderPermissions(state) {
    const list = document.getElementById('perm-list');
    if (!list) return;
    const items = state.permissions || [];
    const sig = JSON.stringify(items);
    if (list.dataset.sig === sig) return;
    list.dataset.sig = sig;
    if (!items.length) {
      list.innerHTML = '<div class="meta" style="text-align:center;padding:20px 8px;">No site has asked yet.</div>';
      return;
    }
    const bySite = new Map();
    items.forEach(e => { if (!bySite.has(e.origin)) bySite.set(e.origin, []); bySite.get(e.origin).push(e); });
    list.innerHTML = '<div class="dl-group">' + Array.from(bySite.entries()).map(([origin, rows]) => {
      const o = escapeHtml(origin); // read back through dataset, so plain HTML escaping
      return `<div class="perm-site" title="${o}"><span>${escapeHtml(pwHost(origin))}</span>
          <button class="text-btn" onclick="send('PERM_RESET_SITE:' + this.dataset.o)" data-o="${o}" title="Forget every decision for this site">Reset</button></div>` +
        rows.map(e => `<div class="perm-row">
          <span>${escapeHtml(e.icon)}</span><span class="lbl">${escapeHtml(e.label.charAt(0).toUpperCase() + e.label.slice(1))}</span>
          <span class="perm-seg" data-o="${o}" data-k="${escapeHtml(e.kind)}">
            <button class="${e.decision === 'allow' ? 'on' : ''}" onclick="permSet(this.parentNode,'allow')">Allow</button>
            <button class="${e.decision === 'block' ? 'on' : ''}" onclick="permSet(this.parentNode,'block')">Block</button>
          </span>
          <button class="icon-btn" onclick="permSet(this.previousElementSibling,'reset')" title="Ask next time">${SVG_X}</button>
        </div>`).join('');
    }).join('') + '</div>';
  }
  function permSet(seg, d) { send('PERM_SET:' + seg.dataset.o + '|' + seg.dataset.k + '|' + d); }
  function renderPasswords(state) {
    const list = document.getElementById('pw-list');
    if (!list) return;
    document.getElementById('pw-status').textContent = state.pw_status || '';
    document.getElementById('pw-del-csv').style.display = state.pw_csv_pending ? '' : 'none';
    const q = (document.getElementById('pw-search').value || '').trim().toLowerCase();
    const items = (state.passwords || []).filter(c => !q || c.origin.toLowerCase().includes(q) || c.username.toLowerCase().includes(q));
    // Redraw only when something visible changed (state pushes are frequent; an edit box must keep its text).
    const sig = JSON.stringify([items, q, pwEditing, Array.from(pwRevealed.entries())]);
    if (list.dataset.sig === sig) return;
    list.dataset.sig = sig;
    if (!items.length) {
      list.innerHTML = `<div class="meta" style="text-align:center;padding:20px 8px;">${(state.passwords || []).length ? 'No matches.' : 'No saved passwords yet.<br>Sign in to a site and choose Save, or import a CSV.'}</div>`;
      return;
    }
    const EYE = '<svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6"><path d="M1.5 8s2.4-4.5 6.5-4.5S14.5 8 14.5 8s-2.4 4.5-6.5 4.5S1.5 8 1.5 8z"/><circle cx="8" cy="8" r="2"/></svg>';
    const COPY = '<svg width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6"><rect x="5" y="5" width="9" height="9" rx="2"/><path d="M11 5V3.5A1.5 1.5 0 0 0 9.5 2h-6A1.5 1.5 0 0 0 2 3.5v6A1.5 1.5 0 0 0 3.5 11H5"/></svg>';
    const EDIT = '<svg width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><path d="M10.5 2.5l3 3L6 13H3v-3z"/></svg>';
    list.innerHTML = '<div class="dl-group">' + items.slice(0, 400).map(c => {
      const host = pwHost(c.origin);
      const shown = pwRevealed.get(c.id);
      const editing = pwEditing === c.id;
      return `<div class="pw-row-wrap">
        <div class="pw-row" title="${escapeHtml(c.origin)}">
          <div class="pw-av">${escapeHtml(host.charAt(0).toUpperCase())}</div>
          <div class="pw-main">
            <div class="pw-host">${escapeHtml(host)}</div>
            <div class="pw-user">${escapeHtml(c.username || '(no username)')}${shown !== undefined ? ` \u00b7 <span class="pw-secret">${escapeHtml(shown)}</span>` : ''}</div>
          </div>
          <div class="dl-actions">
            <button class="icon-btn" onclick="send('PW_COPY_USER:${c.id}')" title="Copy username">@</button>
            <button class="icon-btn" onclick="send('PW_COPY:${c.id}')" title="Copy password">${COPY}</button>
            <button class="icon-btn ${shown !== undefined ? 'primary' : ''}" onclick="${shown !== undefined ? `pwRevealed.delete(${c.id}); renderPasswords(lastState)` : `send('PW_REVEAL:${c.id}')`}" title="${shown !== undefined ? 'Hide' : 'Show password'}">${EYE}</button>
            <button class="icon-btn" onclick="pwEditing = ${editing ? 'null' : c.id}; renderPasswords(lastState)" title="Edit">${EDIT}</button>
            <button class="icon-btn" onclick="if (confirm('Delete the saved password for ${encodeAttr(c.username || host)} on ${encodeAttr(host)}?')) send('PW_DELETE:${c.id}')" title="Delete">${SVG_X}</button>
          </div>
        </div>
        ${editing ? `<div class="pw-edit">
          <input class="text-box" id="pw-e-u-${c.id}" value="${escapeHtml(c.username)}" placeholder="Username" />
          <input class="text-box" id="pw-e-p-${c.id}" type="password" placeholder="New password" />
          <div style="display:flex;gap:6px;justify-content:flex-end;">
            <button class="text-btn" onclick="pwEditing = null; renderPasswords(lastState)">Cancel</button>
            <button class="pill-btn pill-blue" onclick="pwSaveEdit(${c.id})">Save</button>
          </div>
        </div>` : ''}
      </div>`;
    }).join('') + '</div>';
  }
  function pwSaveEdit(id) {
    const u = document.getElementById('pw-e-u-' + id).value;
    const p = document.getElementById('pw-e-p-' + id).value;
    if (!p) { document.getElementById('pw-status').textContent = 'Enter the new password.'; return; }
    pwEditing = null;
    pwRevealed.delete(id);
    send('PW_EDIT:' + id + '|' + JSON.stringify({ u, p }));
  }

  // New Tab: draw the start page first, then ask for the tab, so the window never shows an empty frame
  // while the toolbar grows to full height.
  let pendingHomeUntil = 0;
  function openNewTab() {
    pendingHomeUntil = Date.now() + 700;
    const hc = document.getElementById('home-canvas');
    if (hc) hc.classList.add('active');
    document.getElementById('bookmarks-strip').style.display = '';
    window.__focusHomeSearch();
    send('NEW_TAB');
  }
  // ---------------------------------------------------------------- Extensions
  const PUZZLE_SVG = '<svg width="15" height="15" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"><path d="M3 5h2.5a1.5 1.5 0 1 1 3 0H11v2.5a1.5 1.5 0 1 1 0 3V13H8.5a1.5 1.5 0 1 0-3 0H3v-2.5a1.5 1.5 0 1 0 0-3z"/></svg>';
  function extIcon(e, size) {
    return e.icon ? `<img src="${e.icon}" alt="">` : `<span class="ph" style="${size ? 'width:' + size + 'px;height:' + size + 'px' : ''}">${escapeHtml(profileInitial(e.name))}</span>`;
  }
  function renderExtensions(state) {
    const list = state.extensions || [];
    // Toolbar: enabled extensions with a popup (up to 4), then the puzzle button for the panel.
    const bar = document.getElementById('ext-bar');
    const shown = list.filter(e => e.enabled && e.popup).slice(0, 4);
    const barHtml = list.length ? shown.map(e =>
      `<button class="ext-ic" title="${escapeHtml(e.name)}" onclick="send('EXT_OPEN:${encodeAttr(e.id)}|popup')">${extIcon(e)}</button>`).join('') +
      `<button class="ext-ic" id="ext-puzzle" title="Extensions" onclick="toggleDrawer('extensions', this)">${PUZZLE_SVG}</button>` : '';
    if (bar.dataset.h !== barHtml) { bar.dataset.h = barHtml; bar.innerHTML = barHtml; }
    bar.classList.toggle('on', list.length > 0);
    // Panel.
    if (state.ext_notice) {
      const n = document.getElementById('ext-notice');
      n.textContent = state.ext_notice;
      n.classList.add('on');
      clearTimeout(renderExtensions.t);
      renderExtensions.t = setTimeout(() => n.classList.remove('on'), 8000);
    }
    const el = document.getElementById('ext-list');
    const html = list.length ? list.map(e => `<div class="ext-row ${e.enabled ? '' : 'off'}">
        <div class="ic">${extIcon(e, 24)}</div>
        <div class="tx">
          <div class="nm" title="${escapeHtml(e.name)}">${escapeHtml(e.name)}${e.version ? ` <span class="ds" style="display:inline">${escapeHtml(e.version)}</span>` : ''}</div>
          ${e.description ? `<div class="ds" title="${escapeHtml(e.description)}">${escapeHtml(e.description)}</div>` : ''}
          <div class="acts">
            ${e.popup && e.enabled ? `<button onclick="send('EXT_OPEN:${encodeAttr(e.id)}|popup')">Open</button>` : ''}
            ${e.options && e.enabled ? `<button onclick="send('EXT_OPEN:${encodeAttr(e.id)}|options')">Settings</button>` : ''}
            <button class="rm" onclick="extRemove('${encodeAttr(e.id)}')">Remove</button>
          </div>
        </div>
        <button class="switch ${e.enabled ? 'on' : ''}" role="switch" aria-checked="${e.enabled}" aria-label="Enable ${escapeHtml(e.name)}"
          onclick="this.classList.toggle('on'); send('EXT_SET:${encodeAttr(e.id)}|' + (this.classList.contains('on') ? 'enable' : 'disable'))"><i></i></button>
      </div>`).join('') : '<div class="meta" style="text-align:center;padding:18px 8px;">No extensions yet.</div>';
    if (el.dataset.h !== html) { el.dataset.h = html; el.innerHTML = html; }
  }
  function extRemove(id) {
    const e = (lastState.extensions || []).find(x => x.id === id);
    if (e && confirm(`Remove "${e.name}"? Its settings and data are deleted.`)) send('EXT_SET:' + id + '|remove');
  }

  // ---------------------------------------------------------------- Profiles
  const PROFILE_COLORS = ['#0A84FF', '#30D158', '#FF9F0A', '#FF375F', '#BF5AF2', '#64D2FF', '#8E8E93'];
  let profileEditing = null, profileColor = PROFILE_COLORS[0];
  function profileInitial(n) { const c = Array.from((n || '?').trim())[0] || '?'; return c.toUpperCase(); }
  function renderProfiles(state) {
    const list = state.profiles || [];
    const me = list.find(p => p.id === state.profile_id) || list[0];
    const btn = document.getElementById('profile-btn');
    if (me) {
      document.documentElement.style.setProperty('--profile-accent', me.color);
      const t = profileInitial(me.name);
      if (btn.textContent !== t) btn.textContent = t;
      btn.title = 'Profile: ' + me.name;
    }
    // Shown once there is more than one profile (one profile needs no switcher).
    btn.classList.toggle('on', list.length > 1);
    const el = document.getElementById('profiles-list');
    const html = list.map(p => {
      const mine = me && p.id === me.id;
      const sub = mine ? 'This window' : (p.running ? 'Open' : '');
      return `<div class="prof-row">
        <div class="prof-av" style="background:${p.color}">${escapeHtml(profileInitial(p.name))}</div>
        <div class="nm">${escapeHtml(p.name)}${sub ? `<div class="sub">${sub}</div>` : ''}</div>
        ${mine ? '' : `<button class="pill-btn pill-soft" onclick="send('PROFILE_OPEN:${p.id}')">${p.running ? 'Show' : 'Open'}</button>`}
        <button class="icon-btn" title="Edit" onclick="profileForm('${p.id}')"><svg width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><path d="M10.5 2.5l3 3L6 13H3v-3z"/></svg></button>
      </div>`;
    }).join('');
    if (el.dataset.h !== html) { el.dataset.h = html; el.innerHTML = html; }
    // Link routing matters only with two or more profiles.
    document.getElementById('prof-links').classList.toggle('on', list.length > 1);
    const sel = document.getElementById('prof-links-sel');
    const opts = '<option value="last">Last used window</option>' + list.map(p => `<option value="${p.id}">${escapeHtml(p.name)}</option>`).join('');
    if (sel.dataset.h !== opts) { sel.dataset.h = opts; sel.innerHTML = opts; }
    const want = list.some(p => p.id === state.links_profile) ? state.links_profile : 'last';
    if (document.activeElement !== sel) sel.value = want;
  }
  function profileForm(id) {
    const list = (lastState && lastState.profiles) || [];
    const p = id !== null ? list.find(x => x.id === id) : null;
    profileEditing = p ? p.id : null;
    profileColor = p ? p.color : PROFILE_COLORS[list.length % PROFILE_COLORS.length];
    document.getElementById('profile-form-title').textContent = p ? 'Edit Profile' : 'New Profile';
    document.getElementById('profile-save').textContent = p ? 'Save' : 'Create';
    document.getElementById('profile-name').value = p ? p.name : '';
    // The default profile, this window's profile and open profiles cannot be deleted.
    const canDelete = p && p.id !== 'default' && p.id !== lastState.profile_id && !p.running;
    document.getElementById('profile-delete').style.display = canDelete ? '' : 'none';
    renderProfileSwatches();
    document.getElementById('profile-form').style.display = '';
    document.getElementById('profile-name').focus();
  }
  function renderProfileSwatches() {
    document.getElementById('profile-swatches').innerHTML = PROFILE_COLORS.map(c =>
      `<button class="swatch ${c === profileColor ? 'on' : ''}" style="background:${c}" onclick="profileColor='${c}'; renderProfileSwatches()" title="${c}"></button>`).join('');
  }
  function profileSave() {
    const name = document.getElementById('profile-name').value.trim();
    if (!name) { document.getElementById('profile-name').focus(); return; }
    const payload = JSON.stringify({ name, color: profileColor });
    send(profileEditing === null ? 'PROFILE_NEW:' + payload : 'PROFILE_EDIT:' + profileEditing + '|' + payload);
    document.getElementById('profile-form').style.display = 'none';
  }
  function profileDelete() {
    if (profileEditing === null) return;
    const p = lastState.profiles.find(x => x.id === profileEditing);
    if (p && confirm(`Delete the profile "${p.name}"? Its cookies, sign-ins, history, bookmarks and saved passwords are erased from this PC. This cannot be undone.`)) {
      send('PROFILE_DELETE:' + profileEditing);
      document.getElementById('profile-form').style.display = 'none';
    }
  }

  // ---------------------------------------------------------------- Spaces
  const SPACE_COLORS = ['#0A84FF', '#30B0C7', '#34C759', '#FF9F0A', '#FF375F', '#BF5AF2', '#5E5CE6', '#8E8E93'];
  let spaceEditing = null, spaceColor = SPACE_COLORS[0];
  function renderSpaces(state) {
    const sv = state.spaces;
    if (!sv) return;
    const cur = sv.list.find(x => x.id === sv.active) || sv.list[0];
    document.documentElement.style.setProperty('--space-accent', cur.color);
    document.documentElement.classList.toggle('spaces-tinted', sv.list.length > 1);
    const pill = document.getElementById('space-pill');
    const pillHtml = `<span class="ic">${escapeHtml(cur.icon)}</span><span class="nm">${escapeHtml(cur.name)}</span>`;
    if (pill.innerHTML !== pillHtml) pill.innerHTML = pillHtml;
    const counts = {};
    (state.tabs || []).forEach(t => { counts[t.space] = (counts[t.space] || 0) + 1; });
    const list = document.getElementById('spaces-list');
    const html = sv.list.map(x => `
      <div class="space-row ${x.id === sv.active ? 'on' : ''}" style="--row-accent:${x.color}" onclick="send('SPACE_SWITCH:${x.id}')">
        <div class="sw">${escapeHtml(x.icon)}</div>
        <div class="nm">${escapeHtml(x.name)}</div>
        <div class="ct">${counts[x.id] || 0}</div>
        <button class="icon-btn" title="Edit" onclick="event.stopPropagation(); spaceForm(${x.id})"><svg width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><path d="M10.5 2.5l3 3L6 13H3v-3z"/></svg></button>
      </div>`).join('');
    if (list.dataset.h !== html) { list.dataset.h = html; list.innerHTML = html; }
    const others = sv.list.filter(x => x.id !== sv.active);
    const mv = document.getElementById('space-move');
    const mvHtml = others.length ? 'Move this tab to: ' + others.map(x => `<a href="#" style="color:${x.color};font-weight:600;" onclick="event.preventDefault(); send('TAB_TO_SPACE:${state.active_tab}|${x.id}')">${escapeHtml(x.icon + ' ' + x.name)}</a>`).join(' · ') : '';
    if (mv.dataset.h !== mvHtml) { mv.dataset.h = mvHtml; mv.innerHTML = mvHtml; }
  }
  function spaceForm(id) {
    const sv = lastState && lastState.spaces;
    const x = id !== null && sv ? sv.list.find(s => s.id === id) : null;
    spaceEditing = x ? x.id : null;
    spaceColor = x ? x.color : SPACE_COLORS[(sv ? sv.list.length : 0) % SPACE_COLORS.length];
    document.getElementById('space-form-title').textContent = x ? 'Edit Space' : 'New Space';
    document.getElementById('space-name').value = x ? x.name : '';
    document.getElementById('space-icon').value = x ? x.icon : '';
    document.getElementById('space-delete').style.display = x && sv.list.length > 1 ? '' : 'none';
    renderSwatches();
    document.getElementById('space-form').style.display = '';
    document.getElementById('space-name').focus();
  }
  function renderSwatches() {
    document.getElementById('space-swatches').innerHTML = SPACE_COLORS.map(c =>
      `<button class="swatch ${c === spaceColor ? 'on' : ''}" style="background:${c}" onclick="spaceColor='${c}'; renderSwatches()" title="${c}"></button>`).join('');
  }
  function spaceSave() {
    const payload = JSON.stringify({ name: document.getElementById('space-name').value, icon: document.getElementById('space-icon').value, color: spaceColor });
    send(spaceEditing === null ? 'SPACE_NEW:' + payload : 'SPACE_EDIT:' + spaceEditing + '|' + payload);
    document.getElementById('space-form').style.display = 'none';
  }
  function spaceDelete() {
    if (spaceEditing === null) return;
    const x = lastState.spaces.list.find(s => s.id === spaceEditing);
    if (x && confirm(`Delete the Space "${x.name}"? Its tabs close (you can reopen them with Ctrl+Shift+T).`)) {
      send('SPACE_DELETE:' + spaceEditing);
      document.getElementById('space-form').style.display = 'none';
    }
  }

  // One place for browser shortcuts: the toolbar's keydown and pages (via the native hook) both land here.
  function runShortcut(id) {
    switch (id) {
      case 'new-tab': openNewTab(); break;
      case 'new-window': send('NEW_WINDOW'); break;
      case 'reopen-tab': send('REOPEN_CLOSED_TAB'); break;
      case 'close-tab': send('CLOSE_ACTIVE_TAB'); break;
      case 'next-tab': send('NEXT_TAB'); break;
      case 'prev-tab': send('PREV_TAB'); break;
      case 'focus-url': { const u = document.getElementById('url-input'); u.focus(); u.select(); break; }
      case 'palette': openPalette(); break;
      case 'find': toggleDrawer('find', document.getElementById('btn-controls')); setTimeout(() => document.getElementById('find-input').focus(), 50); break;
      case 'downloads': toggleDrawer('downloads', document.getElementById('btn-downloads')); break;
      case 'reader': send('READER_MODE'); break;
    }
  }
  window.__shunnoShortcut = runShortcut;
  window.__focusHomeSearch = function () {
    const i = document.getElementById('home-search-input');
    if (i) { i.focus(); i.select(); }
  };

  // Called by the native side just before it opens a New Tab it initiated (e.g. Ctrl+T inside a page).
  window.__showHomeNow = function () {
    pendingHomeUntil = Date.now() + 700;
    const hc = document.getElementById('home-canvas');
    if (hc) hc.classList.add('active');
  };

  // Downloads list: also called directly by the filters, so they respond without a native round trip.
  function renderDownloadsList(state) {
    if (!state) return;
    const statusBar = document.getElementById('dl-status-bar');
    statusBar.innerHTML = DL_STATUS_GROUPS.map(([k, label, f]) => {
      const n = state.downloads.filter(f).length;
      return `<button class="${activeDlStatus === k ? 'on' : ''}" onclick="setDlStatus('${k}')">${label}${k !== 'all' && n ? `<span class="n">${n}</span>` : ''}</button>`;
    }).join('');

    const dlCats = ['All', 'Videos', 'Audio', 'Lossless Audio', 'Archives', 'Programs', 'Documents'];
    const catBar = document.getElementById('dl-category-bar');
    catBar.style.display = state.downloads.length >= 6 ? '' : 'none';
    if (catBar.style.display === 'none') activeDlCategory = 'All';
    catBar.innerHTML = dlCats.map(c => `
      <button class="folder-pill ${activeDlCategory === c ? 'active' : ''}"
              onclick="activeDlCategory = '${c}'; renderDownloadsList(lastState);">${c}</button>
    `).join('');

    const statusFilter = (DL_STATUS_GROUPS.find(g => g[0] === activeDlStatus) || DL_STATUS_GROUPS[0])[2];
    const visibleDownloads = state.downloads.filter(d => statusFilter(d) && (activeDlCategory === 'All' || d.category === activeDlCategory));
    const listEl = document.getElementById('downloads-list');
    if (visibleDownloads.length === 0) {
      listEl.innerHTML = state.downloads.length === 0
        ? '<div class="meta" style="text-align:center;padding:22px 8px;">No downloads.<br>Hover a video or file to download it, or search this page for downloadable files.</div>'
        : '<div class="meta" style="text-align:center;padding:18px 8px;">Nothing here.</div>';
    } else {
      listEl.innerHTML = '<div class="dl-group">' + visibleDownloads.map(d => {
        const known = d.total_bytes > 0;
        const pct = d.status === 'COMPLETED' ? 100 : (known ? Math.min(100, d.downloaded_bytes * 100 / d.total_bytes) : 0);
        const ofTotal = known ? `${fmtBytes(d.downloaded_bytes)} of ${fmtBytes(d.total_bytes)}` : fmtBytes(d.downloaded_bytes);
        const ext = ((String(d.file_name).match(/\.([A-Za-z0-9]{1,5})$/) || [])[1] || 'FILE').toUpperCase();
        let sub = '', subCls = '', bar = '';
        switch (d.status) {
          case 'DOWNLOADING': {
            const eta = known && d.speed_bps > 0 ? fmtEta((d.total_bytes - d.downloaded_bytes) / d.speed_bps) : '';
            sub = `${ofTotal} \u2014 ${fmtBytes(d.speed_bps)}/s${eta ? ' \u00b7 ' + eta : ''}`;
            bar = `<div class="dl-bar"><i style="width:${known ? pct : 8}%"></i></div>`;
            break;
          }
          case 'QUEUED': sub = 'Waiting\u2026'; bar = '<div class="dl-bar paused"><i style="width:0%"></i></div>'; break;
          case 'MUXING': sub = /audio\]/i.test(d.segment_map || '') ? 'Converting audio\u2026' : 'Merging audio and video\u2026'; bar = '<div class="dl-bar"><i style="width:100%"></i></div>'; break;
          case 'PAUSED': sub = `Paused \u2014 ${ofTotal}`; bar = `<div class="dl-bar paused"><i style="width:${pct}%"></i></div>`; break;
          case 'FAILED': sub = d.error_message || 'Failed'; subCls = 'err'; break;
          case 'COMPLETED': sub = `${fmtBytes(d.total_bytes > 0 ? d.total_bytes : d.downloaded_bytes)} \u00b7 ${(d.url.match(/^https?:\/\/([^/]+)/) || [])[1] || ''}`; break;
          default: sub = d.status;
        }
        const tip = `${d.category}${d.supports_ranges ? ' \u00b7 ' + d.threads + ' connections' : ''}${d.segment_map ? ' ' + d.segment_map : ''}${d.sha256 ? '\nSHA-256 ' + d.sha256 : ''}`;
        const actions =
          d.status === 'DOWNLOADING' || d.status === 'QUEUED'
            ? `<button class="icon-btn" onclick="send('DL_PAUSE:${d.id}')" title="Pause">${SVG_PAUSE}</button>`
          : d.status === 'PAUSED' || d.status === 'FAILED'
            ? `<button class="icon-btn primary" onclick="send('DL_RESUME:${d.id}')" title="${d.status === 'FAILED' ? 'Retry' : 'Resume'}">${SVG_RESUME}</button>
               <button class="icon-btn" onclick="const u=prompt('Paste a fresh link for this file:','${encodeAttr(d.url)}'); if(u) send('DL_FIX:${d.id}|'+u);" title="Use a new link">${SVG_LINK}</button>`
          : d.status === 'COMPLETED'
            ? `<button class="icon-btn primary" onclick="send('DL_SHOW:${encodeAttr(d.saved_path || '')}')" title="Show in folder">${SVG_REVEAL}</button>`
            : '';
        return `
          <div class="dl-row" title="${escapeHtml(tip)}">
            <div class="dl-icon ${dlIconClass(d.category)}"><b>${escapeHtml(ext)}</b></div>
            <div class="dl-main">
              <div class="dl-name">${escapeHtml(d.file_name)}</div>
              ${bar}
              <div class="dl-sub ${subCls}">${escapeHtml(sub)}</div>
            </div>
            <div class="dl-actions">${actions}<button class="icon-btn" onclick="send('DL_DEL:${d.id}')" title="Remove from list">${SVG_X}</button></div>
          </div>`;
      }).join('') + '</div>';
    }
  }

  window.renderChromeState = function(state) {
    lastState = state;
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
    if (snapNative !== !!state.snap_native) {
      snapNative = !!state.snap_native;
      const mb = document.getElementById('btn-max');
      if (mb) mb.classList.toggle('native-max', snapNative);
    }
    // Position and colours can change with layout, maximize and theme (after its cross-fade).
    requestAnimationFrame(reportMaxButton);
    setTimeout(reportMaxButton, 450);

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
    const archEl = document.getElementById('auto-archive-select');
    if (archEl && document.activeElement !== archEl) archEl.value = String(state.global.auto_archive_hours || 0);
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
    // Until the native side confirms, keep showing the New Tab page we opened optimistically.
    const isHome = state.active_url === 'shunno://newtab' || Date.now() < pendingHomeUntil;
    // Favorites bar only on the New Tab page (Safari); web pages get the full height.
    document.getElementById('bookmarks-strip').style.display = isHome ? '' : 'none';
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
    ` + allFolders.filter(f => f !== 'All').map(f => `
      <button class="bm-chip ${activeBookmarkFolder === f ? 'action' : ''}"
              onclick="activeBookmarkFolder = (activeBookmarkFolder === '${encodeAttr(f)}' ? 'All' : '${encodeAttr(f)}'); send('REFRESH_UI');"
              title="Filter folder: ${escapeHtml(f)}">📁 ${escapeHtml(f)}</button>
    `).join('') + visibleBarBms.slice(0, 12).map(({ b, i }) => `
      <button class="bm-chip" onclick="send('NAV_NEW_TAB:${encodeAttr(b.url)}')"
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



    // Compact Downloads button: the count shows only while there is something in the list.
    const dlCount = document.getElementById('dl-count');
    const dlN = state.downloads.length ? String(state.downloads.length) : '';
    if (dlCount.textContent !== dlN) dlCount.textContent = dlN;
    document.getElementById('btn-downloads').title = 'Downloads (Ctrl+J)' + (dlN ? ': ' + dlN : '');
    // Bookmark button is filled when this page is already bookmarked.
    const bmBtn = document.getElementById('btn-bookmark');
    const marked = !!state.active_url && (state.bookmarks || []).some(b => b.url === state.active_url);
    bmBtn.classList.toggle('marked', marked);
    bmBtn.querySelector('path').setAttribute('fill', marked ? 'currentColor' : 'none');
    bmBtn.title = marked ? 'Bookmarked (add again to another folder)' : 'Bookmark this page';
    bmBtn.style.display = isHome ? 'none' : '';
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
      ['force_dark_pages', '🌑 Darken All Sites', state.global.force_dark_pages],
      ['download_takeover', '⚡ Accelerate Downloads', state.global.download_takeover],
      ['little_for_links', '🪟 Links in Little Window', state.global.little_for_links],
      ['drop_basket', '🧺 Drop Basket', state.global.drop_basket_enabled],
      ['bg_audio', '🎧 Background Play', state.global.background_media_play],
      ['auto_pip', '📺 Auto-PiP on Blur', state.global.auto_pip_on_blur],
      ['auto_clean', '🧹 Tidy on Start', state.global.auto_clean_completed],
      ['strip_tracking', '✂️ Strip Trackers', state.global.strip_tracking_params],
      ['block_popups', '🚫 Block Pop-Ups', state.global.block_popups],
      ['https_only', '🔒 HTTPS Only', state.global.https_only],
      ['auto_update', '⬆️ Auto Update', state.global.auto_update],
      ['save_passwords', '🔑 Save Passwords', state.global.save_passwords],
      ['strict_tracking', '🛡️ Strict Tracking', state.global.strict_tracking],
      ['peek_links', '👁️ Shift-Click Peek', state.global.peek_links]
    ];
    const mode = state.global.theme_mode || 'system';
    document.querySelectorAll('#theme-seg button').forEach(b => b.classList.toggle('on', b.dataset.mode === mode));
    const mn = document.getElementById('menu-note');
    mn.textContent = state.menu_note || '';
    mn.style.display = state.menu_note ? '' : 'none';
    document.getElementById('force-dark-note').style.display = (!!state.global.force_dark_pages !== !!state.force_dark_active) ? '' : 'none';
    document.getElementById('global-toggles-grid').innerHTML = globalItems.map(([key, label, val]) => `
      <div class="toggle-tile">
        <span>${label}</span>
        <button class="switch ${val ? 'on' : ''}" role="switch" aria-checked="${val ? 'true' : 'false'}" aria-label="${label}" onclick="this.classList.toggle('on'); send('TOGGLE_GLOBAL:${key}')"><i></i></button>
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
      ['auto_reject_cookies', '🍪 Reject Cookies', state.site.auto_reject_cookies],
      ['fp_protect', '🫥 Fingerprint Shield', state.site.fp_protect]
    ];
    document.getElementById('site-toggles-grid').innerHTML = siteItems.map(([key, label, val]) => `
      <div class="toggle-tile">
        <span>${label}</span>
        <button class="switch ${val ? 'on' : ''}" role="switch" aria-checked="${val ? 'true' : 'false'}" aria-label="${label}" onclick="this.classList.toggle('on'); send('TOGGLE_SITE:${key}')"><i></i></button>
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

    renderImport(state);
    renderSpaces(state);
    renderPeek(state);
    const rb = document.getElementById('reader-btn');
    const showReader = !!(state.active_reader_available || state.active_reader);
    rb.style.display = showReader ? '' : 'none';
    rb.classList.toggle('on', !!state.active_reader);
    rb.title = state.active_reader ? 'Close Reader (F9 or Esc)' : 'Reader (F9)';
    renderPwOffer(state);
    renderPermOffer(state);
    renderProfiles(state);
    renderExtensions(state);
    renderPermissions(state);
    renderPasswords(state);

    const folderBar = document.getElementById('bookmark-folders-bar');
    folderBar.innerHTML = allFolders.map(f => `
      <button class="folder-pill ${activeBookmarkFolder === f ? 'active' : ''}"
              onclick="activeBookmarkFolder = '${encodeAttr(f)}'; send('REFRESH_UI');">${f === 'All' ? 'All Folders' : '📁 ' + escapeHtml(f)}</button>
    `).join('');

    const filteredBms = state.bookmarks
      .map((b, i) => ({ b, i }))
      .filter(({ b }) => activeBookmarkFolder === 'All' || (b.folder || 'General') === activeBookmarkFolder);

    const BM_RENDER_CAP = 300;
    document.getElementById('bookmarks-list').innerHTML = filteredBms.length === 0
      ? '<div class="meta">No bookmarks in this folder.</div>'
      : (filteredBms.length > BM_RENDER_CAP
          ? `<div class="meta" style="margin-bottom:6px;">Showing ${BM_RENDER_CAP} of ${filteredBms.length}. Pick a folder to narrow the list.</div>`
          : '') + filteredBms.slice(0, BM_RENDER_CAP).map(({ b, i }) => `
          <div class="list-row">
            <div style="overflow:hidden;">
              <div style="font-size:12.5px;font-weight:600;">${escapeHtml(b.title)} <span class="pill-btn pill-soft" style="padding:1px 6px;font-size:10px;">📁 ${escapeHtml(b.folder || 'General')}</span></div>
              <div class="meta" style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">${escapeHtml(b.url)}</div>
            </div>
            <div style="display:flex;gap:4px;flex-shrink:0;">
              <button class="pill-btn pill-soft" onclick="send('NAV_NEW_TAB:${encodeAttr(b.url)}'); closeDrawer();" title="Open in new tab">Open</button>
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

    renderDownloadsList(state);
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
    if (e.key === 'F9') {
      e.preventDefault();
      runShortcut('reader');
    } else if (e.ctrlKey && e.key === 'Tab') {
      e.preventDefault();
      if (!e.repeat) runShortcut(e.shiftKey ? 'prev-tab' : 'next-tab');
    } else if (e.ctrlKey && e.shiftKey && !e.altKey && e.key.toLowerCase() === 't') {
      e.preventDefault();
      if (!e.repeat) runShortcut('reopen-tab');
    } else if (e.ctrlKey && !e.shiftKey && !e.altKey && ({ t: 'new-tab', n: 'new-window', k: 'palette', w: 'close-tab', l: 'focus-url', f: 'find', j: 'downloads' })[e.key.toLowerCase()]) {
      e.preventDefault();
      if (!e.repeat) runShortcut(({ t: 'new-tab', n: 'new-window', k: 'palette', w: 'close-tab', l: 'focus-url', f: 'find', j: 'downloads' })[e.key.toLowerCase()]);
    } else if (e.ctrlKey && !e.shiftKey && !e.altKey) {
      const k = e.key.toLowerCase();
      if (k === 'r') { e.preventDefault(); send('RELOAD'); }
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

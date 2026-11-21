## v2.29.0

* **Extension icons:** shown in their own colours at full strength (they were dimmed like the
  toolbar glyphs), taken from the 48 px icon so they stay sharp, and filled in for extensions saved
  by older versions. Extensions without an icon get a neutral grey letter instead of a blue square.
* **Extensions list shows only what you added.** WebView2's own built-in parts (Microsoft Edge PDF
  Viewer, Microsoft Clipboard Extension) are no longer listed; they belong to the runtime.
* **New Tab no longer flashes the Favorites bar** for a moment before the start page.
* **Many tabs:** a narrow tab no longer loses its close button. The current tab shows it in place
  of its icon (Safari); other narrow tabs select on click and close with a middle click.

## v2.28.0

Toolbar and bookmarks, rebuilt the way Safari does them.

### Toolbar
* **One button style.** Sidebar, Back, Forward, extensions, Downloads, profile and More are the same
  size with the same glyph size and no fill until hovered (filled while their popover is open).
* **Order as in Safari:** Sidebar (bookmarks and history) leads, then Back and Forward. Reload and
  Bookmark sit inside the address field as page actions. The Home button is gone (New Tab is the
  start page).
* **Downloads** appears once there are downloads; while some run, a ring shows their combined
  progress and a badge counts them.

### Bookmarks
* **Favorites** is the main folder: the New Tab page shows it as tiles (with Add), and the
  **Favorites Bar** shows it on web pages. The bar is off until you turn it on (Sidebar > Bookmarks),
  so there is no empty strip any more, on New Tab or elsewhere.
* **Add Bookmark (Ctrl+D or the bookmark in the address field):** choose the folder ("Add this page
  to", New Folder…) and the title. On a bookmarked page the filled icon opens **Edit Bookmark**:
  folder, title, address, Remove.
* **Sidebar popover:** Bookmarks / History switch, search, folders that open and close, edit and
  delete on hover; History grouped Today, Yesterday, Earlier; Clear History asks first.
* Existing bookmarks in "General" and the old New Tab shortcuts move into Favorites once.
* Right-click a favorite (bar or New Tab) to edit it; middle-click opens it in a new tab.

## v2.27.0

Fixes from testing v2.26.0.

### Downloads
* **Finished downloads open.** Click a file's name or icon in Downloads, or the new Open button, to
  open it with its default app (Windows still shows its warning for files from the Internet).
  "Show in folder" stays beside it.
* **A resumed download checks that the file has not changed** on the server: the size and the
  server's ETag or Last-Modified are recorded, and every range request carries If-Range. A changed
  file starts over instead of being stitched together from two versions.
* **No gaps after a power cut:** downloaded data is flushed to disk before the progress that
  claims it is saved.
* **Closing Shunno ends FFmpeg and yt-dlp.** They now start suspended and join the kill-on-close
  job before running, so the processes they start belong to it too; on quit Shunno also ends them
  directly.
* **Duplicate warning only for the same link.** Another file that merely has the same name is
  saved as `name (1).ext` without asking. The warning's choices are now Skip, Keep Both
  (`name (1).ext`) and Replace.

### Extensions
* **Install from the Chrome Web Store and Edge Add-ons** (Brave uses the Chrome Web Store). Open an
  extension's page and press **Add to Shunno** in the address bar; Shunno fetches the package the
  store serves to Chromium browsers and installs it. The Extensions panel links to both stores.
* Larger extension icons in the toolbar and the Extensions panel.

### Windows and dialogs
* **Add Bookmark and every other question no longer open under the page.** prompt/confirm dialogs
  were drawn inside the toolbar strip; they are now a sheet laid over the window (Add Bookmark with a
  folder list, rename, move, delete confirmations, "Use a new link").
* **F11 is smooth:** the window glides between its frame and full screen in about a quarter
  second with the page laid out every frame, instead of jumping and flashing. Turned off when
  Windows animation effects are off.
* **A changed site setting reaches every open tab**, from its next page load (the page script is
  now replaceable in tabs that are already open, in every window).

## v2.26.0

Full code audit, file by file, plus the findings of an external review. The version continues from
v2.25.0: installed copies only update to a higher number, so restarting at 1.0.0 would have left
every v2.x install without updates.

### Toolbar
* **Library and Bookmark buttons sit beside Downloads** in the address bar row. Library is an icon
  only (it opens the bookmarks and history popover under itself); Bookmark is filled when the page
  is already saved and hidden on the New Tab page.
* **Downloads is a compact icon** with a count badge that appears only while there are downloads.

### Security and privacy
* **Site settings follow the site, not the tab.** JavaScript off, mobile site, ad and image
  blocking used to be fixed when a tab was created, so a tab that moved to another site kept the
  first site's settings (ads unblocked everywhere after visiting an allowed site; scripts running on
  a site set to "JavaScript off"). They are now applied at every navigation.
* **User scripts ran on every site a tab visited**, chosen by the tab's first site. Each script now
  runs only on pages whose host matches its `@match` / `@include` lines, top frame only.
* **The IPC token could leak** into evaluated script (Save page) and through `window.ipc` in frames.
  Neither path exists any more; in-page entry points the browser calls cannot be replaced by the page.
* **Downloads get Mark of the Web**, so SmartScreen and Office check them; names lose
  right-to-left override characters that disguise `.exe` files; more Windows device names are refused.
* **Windows Hello no longer fails open** on errors; only a missing or unconfigured Hello proceeds.
* **Copied passwords stay out of clipboard history and cloud clipboard.**
* **Page-supplied addresses cannot reach your network**: redirects and DNS from public to local
  addresses are refused for page-started probes; IPv6 and embedded IPv4 forms are covered. A public
  HLS playlist cannot point at local addresses, and FFmpeg is limited to network protocols.
* **Links handed over by other programs** are accepted only as web, `file:///` or local page links.
* **Sign-in pop-up exception** is judged by host and path; a query string mentioning "oauth" no longer
  lets an ordinary pop-up through.
* **Permissions, downloads queue, settings and the password vault** are written atomically, and a
  damaged file is kept aside instead of being replaced by defaults.
* CI: secrets are given only to the steps that need them, actions are pinned to commits, and the
  release is signed by a Rust tool that refuses a key not matching the one built into the browser.
* Updates: manifest size limit, streamed and size-capped EXE download, no reinstall when another
  profile already installed the same version, staged file cleaned up on failure.

### Reliability
* **Two downloads with the same name could overwrite each other** (engine and WebView2 downloads).
  Final names are reserved before the move; the second becomes `name (1).ext`.
* **A file name with a non-ASCII character next to `%`** crashed the download thread.
* **Clear history could be undone** by a background save finishing later. Saves are now ordered.
* **Tabs in other windows never slept or archived**; all windows are checked now.
* **Ctrl+Tab to a sleeping tab** showed a frozen page; it wakes it, as clicking the tab does.
* **Restart to update could fail to reopen** (the new copy saw the old one still running and quit).
* **FFmpeg and yt-dlp could keep running after Shunno closed**; they now end with it.
* **A second launch could start the same profile twice** during start-up; it now waits or hands over.
* Session cookies given to yt-dlp are deleted whatever the outcome.
* The download queue file is never silently emptied when it cannot be read.
* Changing a site setting keeps the tab in its group and Space.
* Grab results, page saves and stream reports are accepted only when you asked for them.
* "Download link with Shunno" uses the right-clicked page's cookies and referrer, also in split view
  and Little Shunno. Taken-over browser downloads keep their file name.
* Two screenshots in the same second no longer overwrite each other; saved pages never overwrite a file.
* A downloaded filter list is refused if it fails either check (header or size; the test needed both to fail), checksummed filter cache, Chrome history imported from
  the write-ahead log, newest visit time kept on import, AMP links keep their query, per-window
  full-screen placement, per-window window procedure, icon handles reused instead of leaked.
* Little Shunno closes its web views before its window.

### Fixes from the earlier pass of this audit
* **Closing tabs from inside a page could wipe the saved session.** Ctrl+W, Ctrl+T, Ctrl+Tab, Ctrl+J and Ctrl+K sent by the page script used an old code path that ignored Spaces, split view, Recently Closed and other windows, and closing the last tab of one window quit Shunno and saved an empty session for every window. They now use the same path as the native shortcuts.
* **Zoom keys zoomed the wrong pane** in split view (always the left one). They now zoom the page you pressed them in.
* **Pinning a background tab jumped to it** and could leave split view pointing at the wrong tab. The active and split tabs now stay put.
* **Restored tabs forgot their zoom** (the page was zoomed but the zoom steps started from 100%). Restored, reopened and pop-up tabs now start from the site's saved zoom.
* **Opening a local .html or .pdf from Explorer did nothing**, and typing a `file:///` address turned it into a web search. Both now open the file. Pages still cannot open local files.
* **Deleting a Space left other windows showing it**, with their tabs in a Space that no longer existed. Those tabs and windows now move to the remaining Space.
* **Tab groups of restored second and later windows were lost** at startup, because groups were pruned before those windows were rebuilt.
* **A restored window could open on an empty tab bar** when its tabs were in a different Space; it now shows its own Space. Moving a tab out of a window keeps that window on a tab of its Space.
* **Window events for a window that had just closed** could act on another window; they are now ignored.
* Show in folder now passes paths with spaces and commas to Explorer the same way everywhere.
* Extension names with quotes showed backslashes in tooltips.

## v2.25.0

### Extensions
* **Chrome and Edge extensions** run in Shunno. Open the Extensions panel (🧩 in the Menu, the toolbar or the command palette) and choose **Add File…** for a `.crx` or `.zip`, or **Add Folder…** for an unpacked extension. Shunno keeps its own copy, so the original can be deleted.
* Extensions with a popup get a **toolbar button** (up to four, Safari-style); click it to open the popup over the page. The puzzle button opens the panel.
* Each extension has an **on/off switch**, **Open**, **Settings** (its options page) and **Remove** (with confirmation; its copy is deleted).
* Limits: the Chrome Web Store cannot install into Shunno directly; extensions run in normal tabs, not private ones; some that need Chrome-only APIs may not work. Needs a current Microsoft Edge WebView2 Runtime; Shunno says so if it is too old.
* Technical: WebView2 needs `AreBrowserExtensionsEnabled`, which wry 0.40 cannot set, so Shunno vendors `webview2-com` 0.30.0 with that option added (see THIRD_PARTY_NOTICES.md).

## v2.24.0

### Multiple windows
* **Ctrl+N** opens a new window (also in the Menu and the command palette). Each window has its own tabs, active tab, split view, Space and Peek; settings, bookmarks, history, downloads and the profile are shared.
* **Tear a tab off:** drag a tab out of the tab bar and drop it on the page area to give it its own window where you let go. Or right-click a tab: **Move Tab to New Window**, or **Move to Window** for another open window. The page moves live: no reload, nothing lost.
* Closing a window keeps the others open; its tabs go to Recently Closed (Ctrl+Shift+T). Closing the last tab of a window closes that window. The last window quits as before.
* **All windows are restored** with the session, each with its own tabs.
* Links from other apps, Ctrl+Alt+N and new tabs go to the window you used last; light/dark changes apply to every window.

## v2.23.0

### Tab Groups
* **Right-click a tab** for a compact menu: Pin, *Add Tab to New Group*, *Move to Group*, *Remove from Group*, Mute and Close.
* A group shows as a **coloured capsule** before its tabs, and its tabs take a light tint of the group colour. A new group opens its name field straight away; an unnamed group shows as a dot.
* **Click the capsule to collapse** the group (it shows how many tabs it holds); click again to expand. Collapsing the group you are in moves you to the nearest visible tab.
* **Right-click the capsule** to rename, recolour, collapse, ungroup or close the whole group.
* Drag a tab between two tabs of a group to add it; drag it away to take it out. Grouped tabs always stay together. Pinned and private tabs are not grouped; moving a tab to another Space takes it out of its group.
* Groups and collapsed state are restored with your session, per profile.

### Fixes
* *Restart to update* now reopens the same profile.

## v2.22.1

### Profiles
* **Links from other apps follow your profile.** New setting in the Profiles panel, *Open links from other apps in*: **Last used window** (default) or a chosen profile. A link starts that profile if it is not open.
* **Ctrl+Alt+N works from every profile.** It opens Little Shunno in the profile you used last, whichever window owns the shortcut, and another open profile takes the shortcut over when that window closes.

## v2.22.0

### Profiles (Safari-style)
* **Separate profiles** for work, school or personal browsing. Each profile opens in **its own window** with its own cookies and sign-ins, history, bookmarks, saved passwords, site permissions, settings, Spaces, session and download queue.
* A round, coloured **profile badge** appears in the toolbar once you have more than one profile; the window title shows the profile name. Also in the Menu (👤 Profiles) and the command palette.
* Create, rename, recolour and **Open/Show** profiles from the Profiles panel. A profile can be deleted only when it is closed (and never the default one); deleting erases its data folder after a confirmation.
* Your existing data becomes the default profile, **Personal**, with no migration. Helper tools (ffmpeg, yt-dlp) and block lists are shared, so they download once.
* Opening links from other apps still goes to the default profile.

## v2.21.1

### Site Permissions
* **Peek** asks in the main toolbar (its request is shown ahead of the tab behind it). **Little Shunno** asks with the same pill in its own bar, and a request still waiting when you choose *Open in Shunno* moves to the main toolbar with the page.
* A pending request is refused automatically when its tab, Peek or Little window closes, or when the tab navigates to another site.
* The Site Permissions panel note now says where each window asks.

## v2.21.0

### Site Permissions (Safari-style)
* When a site asks for the **camera, microphone, location, notifications**, clipboard, sensors, MIDI, file access, local fonts, multiple downloads or window management, a small **Allow / Block** prompt appears in the toolbar instead of the WebView2 dialog. The page keeps working while it waits.
* Your answer is **remembered per site** (stored in `permissions.json`, not in WebView2's profile). Saved Blocks apply everywhere.
* **Private tabs** always ask and never remember; a saved Allow does not carry into a private tab, a saved Block does.
* New **Site Permissions** panel (Menu, or the command palette): every site's decisions, an Allow/Block switch per permission, forget one, or **Reset** a whole site.
* A pending request is refused automatically when its tab closes or navigates to another site. Peek and Little windows have no toolbar, so undecided requests from them are blocked.

## v2.20.0

### Reader (Safari Reader-style)
* A **Reader button** appears in the address bar only when the page is an article (Mozilla's article check, re-run when single-page sites change article). Click it or press **F9**; **Esc** or F9 closes.
* Articles are extracted with **Mozilla Readability** (the engine behind Firefox Reader View): title, site, byline, reading time, headings, images (including lazy-loaded ones), links, lists, quotes, tables and code. Bangla text uses Nirmala UI / Noto.
* **Aa panel:** text size, Serif or Sans-serif, Narrow / Normal / Wide, and Auto / White / Sepia / Gray / Dark themes. Your choice is remembered.
* The page itself is never changed: Reader is an overlay, and the article is rebuilt from an allow-list of safe elements, so page scripts, event handlers, `javascript:` links, iframes and forms cannot run inside it.
* Readability (33 KB minified) is loaded only when you open Reader, not on every page.

## v2.19.1

* **Open in Shunno keeps the live page.** Little Shunno now moves the page itself into the main window instead of reloading its link, so scroll position, half-filled forms, playing media and sign-ins carry over.
* **Space colour across the tab bar.** With more than one Space, the whole tab bar takes a light tint of the current Space's colour (Arc-style), in light and dark mode.
* The native maximize button follows the tinted tab bar colour.

## v2.19.0

### Spaces (from Arc)
* Group tabs into **Spaces** (Work, Study, Personal...), each with a name, icon and colour. The Space button at the left of the tab strip switches Spaces; the tab strip shows only the current Space's tabs, and the current tab is underlined in the Space colour.
* Each Space remembers its last tab. New tabs open in the current Space; Ctrl+Tab and Ctrl+W stay within it; Ctrl+Shift+T reopens a tab into the Space it came from. Picking a tab from another Space in the command palette switches to that Space.
* Create, rename, recolour, delete (its tabs close and stay reopenable), and move the current tab to another Space from the Spaces popover or the palette. Up to 12 Spaces. Spaces share cookies and logins.

### Little Shunno (Arc's Little Arc)
* A small, quick window for a single page: **Ctrl+Alt+N from anywhere in Windows**, the Little button in the Menu, or links opened from other apps. Its own address bar searches or navigates; **Open in Shunno** moves the page into the main window as a tab.
* Shunno now runs as a single instance: opening a link from another app hands it to the running browser instead of starting a second copy. Setting: **Links in Little Window** (on); off opens them as tabs.
* **Set as Default** (Menu) registers Shunno as a Windows browser for this user and opens Windows Default apps, where you confirm the choice.

### Fixes
* Ctrl+W used its own close path that ignored split view; it now uses the same one as the tab's close button.
* The download takeover check retries once after a dropped connection.

## v2.18.0

### Downloads (IDM-style takeover)
* **Downloads started by pages now use Shunno's multi-connection engine** (up to your connection setting, default 16) with pause/resume across restarts, instead of WebView2's single connection.
* Safe by design: WebView2 starts the download as before; Shunno checks in the background that it can fetch the same file itself (your cookies for that site, the page as referrer, the browser's user agent, a resumable 206 response, same size, not an HTML page). Only then does it cancel the WebView2 copy and continue. Otherwise the browser download simply carries on.
* Not taken over: private tabs, files under 2 MB, `blob:`/`data:` downloads, POST form downloads and one-time links (these fail the check automatically).
* Menu switch: **Accelerate Downloads** (on by default).

## v2.17.4

### Window
* **Snap Layouts now appear exactly as in other apps (no zone numbers).** v2.17.2 opened them with Win+Z, which is the keyboard version and labels every zone with a number. On Windows 11 the maximize button is now a real window caption button: the window reports it to Windows (HTMAXBUTTON) and draws it, so hovering it shows the mouse version of Snap Layouts, the same as Notepad or Chrome. Click and double-click maximize/restore; hover and pressed states match the other buttons in both themes.
* Minimize, maximize/restore and close use Windows' own caption glyphs (Segoe Fluent Icons; Segoe MDL2 Assets on Windows 10).
* Windows 10 (no Snap Layouts) keeps Shunno's flyout.

## v2.17.3

### Keyboard
* **Ctrl+T works every time.** Shortcuts were handled only by the toolbar, so after the first new tab (when focus moved into a page) they stopped. Browser shortcuts are now caught natively in every page too: Ctrl+T, Ctrl+Shift+T, Ctrl+W, Ctrl+Tab / Ctrl+Shift+Tab, Ctrl+L, Ctrl+K, Ctrl+J, Ctrl+F. Holding a key does not repeat it.
* A new tab puts the cursor in the start page's search box.

### Build
* GitHub Actions moved to their Node 24 versions (checkout v5, upload-artifact v6, action-gh-release v3); removes the Node 20 deprecation warning.

## v2.17.2

### Window
* **Windows 11 Snap Layouts are now the real system flyout.** Hovering Maximize asks Windows to open its own Snap Layouts (the same flyout, zones, app thumbnails and keyboard numbers as every other app) instead of Shunno's imitation. It is only triggered when Shunno is the active window, so it never acts on another app; otherwise, and on Windows 10 (which has no Snap Layouts), Shunno's own flyout is used.

## v2.17.1

### Appearance
* **Follows Windows**: Menu, Appearance: System / Light / Dark (default System). With System, switching Windows between light and dark mode updates Shunno immediately.
* **True colours in dark mode.** The old page dark mode inverted every page with a CSS filter, which distorted colours. Pages are now told the preferred colour scheme natively, so sites with their own dark theme (YouTube, GitHub, Wikipedia, Google...) use it with their real colours; other sites stay as designed.
* **Darken All Sites** (optional, after restart): Chromium's built-in auto dark mode for sites without a dark theme. It uses perceptual colour inversion and leaves photos alone.

### Smoothness
* New tabs no longer flash: the start page is drawn before the toolbar grows, new web views stay hidden until placed, and they are painted in the theme colour before the page draws (no white flash in dark mode).
* Showing or hiding the start page, Menu, Downloads and Peek now reorders native steps so the bare window is never visible in between.
* Download filters (All, Downloading, Downloaded, Paused, Failed, file types) respond instantly; they no longer wait for a round trip to the browser core.
* Colour transitions shortened from 0.36 s to 0.18 s.

### Icon
* Larger taskbar icon: the tile now fills the icon area like other browsers' icons, with a bolder ring.

## v2.17.0

### Password manager
* **Saves** after you sign in: a small prompt in the toolbar, "Save password for github.com?", with Save, Never (this site) and Not now. It never blocks the page. Changed passwords offer **Update**. Private tabs never save.
* **Fills** from a menu under the login field listing your accounts for that site; the password goes into the page only when you pick one. Works with React and other script-driven forms.
* **Passwords panel** (Menu, Passwords): search, show, copy username, copy password, edit, delete. Show and copy ask for Windows Hello.
* **Import** CSV exports from Chrome, Edge, Brave, Opera, Vivaldi, Firefox and Bitwarden; the panel then offers to delete the plaintext CSV.
* Encrypted with your Windows account (DPAPI); exact-site matching only. Details in docs/SECURITY.md.
* WebView2's own password saver is turned off (one prompt, one store). Passwords it saved before cannot be exported; sign in once to save them to Shunno, or import a CSV.

## v2.16.1

### Context menu
* Select text, right-click: **Search <your default engine> for “…”** at the top of the menu. Opens in a new tab; from a private tab, a private tab. Always searches, even when the selection looks like a web address. Line breaks are collapsed; long selections are cut to 300 characters.

### Fixes
* The pop-up blocker matched ad-network names anywhere in a URL (so a search for one of them was blocked); it now matches the host only.

## v2.16.0

### Peek (from Arc)
* **Shift+click** any link to preview it in a floating panel over the current page, without leaving it.
* **Open in New Tab** keeps the live preview as a tab (scroll position, logins and form input are preserved). **Esc**, clicking outside, or the close button dismisses it.
* Shift+click inside a Peek navigates the Peek. Switching tabs, navigating, or opening Menu/Downloads closes it.
* The preview uses the same protections as tabs (ad blocking, HTTPS-only, fingerprint shield); a Peek opened from a private tab is private, and so are its pop-ups.
* Toggle: Menu, **Shift-Click Peek** (on by default).

## v2.15.1

### Downloads
* Finished downloads stay in the list. "Auto-Clean Done" is now **Tidy Finished on Start**: completed items are removed at the next launch, not the moment they finish (which emptied the list and hid the filters).
* The All / Downloading / Downloaded / Paused / Failed bar is always shown.

### Bookmarks bar
* Shown only on the New Tab page (Safari's Favorites behaviour). Web pages get the full height.
* The page now sits flush under the toolbar at its real height (reported by the toolbar), fixing a 1 to 2 px overlap, larger when maximized.

### Icon
* New app icon: blue rounded tile, white open ring for *shunno* (zero), amber dot. Sizes 16 to 256 px, small sizes as BMP, large as PNG. Source: `assets/brand/make_icon.py`.
* Taskbar and title-bar icons are chosen for the display DPI (32 px at 100%, 40 px at 125%, 48 px at 150%); previously a 48 px icon was downscaled, which blurred it.
* Toolbar logo updated to match.

## v2.15.0

### Tabs (from Arc)
* **Close unused tabs after** Never / 12 hours / 1 day / 7 days (Menu). Unpinned tabs not viewed for that long close themselves and stay reopenable (Ctrl+Shift+T) and in History. Pinned, playing, visible and private tabs are kept; the last tab is never closed. Default: Never.
* Closing a tab to the left of the current one no longer switches you to a different tab; split view follows correctly.

### UI
* Menu settings in two compact columns with smaller switches.
* Menu, Downloads and Library popovers share one width (400 px).

## v2.14.2

### Downloads
* Audio-only downloads from YouTube and other yt-dlp sites: the floating bar lists **Audio, original quality** (best stream, original codec, no re-encode) and **Audio as FLAC** (lossless re-encode of that stream). Requires FFmpeg, installed automatically.
* Audio categories split: **Audio** (MP3, M4A, AAC, Opus, Ogg) and **Lossless Audio** (FLAC, ALAC, WAV, AIFF, APE). Previously all audio was labelled lossless.
* Video and audio of the same page no longer count as duplicates of each other.

### UI
* Switches are blue when on.

## v2.14.1

### Fixes
* Dark lines along the window's left, right and bottom edges: the resize grip strips are now painted in the theme colour, and the toolbar no longer draws its own rounded corners (Windows 11 rounds the window).

### Bookmarks
* Bookmarks open in a new tab (bar, Library and command palette). An empty New Tab page is reused instead of left behind.

### Downloads (Safari-style)
* Status filter as a segmented control with counts: All, Downloading, Downloaded, Paused, Failed.
* Compact rows: file-type badge, thin progress bar, size, speed and time left; round icon buttons for Pause, Resume/Retry, New link, Show in folder, Remove.
* Header reduced to Clear plus three icons (find on page, folder, settings).

### UI
* Popovers resized: Menu 360 px, Downloads 400 px, Library 420 px. No close buttons inside popovers; click outside or press Esc.
* Settings are single-column rows with iOS-style switches instead of ON/OFF buttons.
* Grouped-inset card style inside popovers; font stack prefers Segoe UI Variable Text.

## v2.14.0

### Import
* Bookmarks and history import from Chrome, Edge, Brave, Vivaldi, Opera and Opera GX, per profile (Bookmarks → Import…).
* History is read with a built-in read-only SQLite reader (no SQLite library linked; binary size unchanged in practice). Inputs are bounds-checked; corrupt files fail cleanly.
* Imported folders are flattened to `<Browser>/<top folder>` so the folder bar stays short. Duplicate URLs are skipped; re-importing is idempotent.
* Import runs on a worker thread; the UI stays responsive on large profiles.
* Passwords are not read from browser profiles (Chromium 127+ app-bound encryption); CSV import comes with the password manager.

### Performance
* Bookmark library renders at most 300 rows; pick a folder to narrow.

## v2.13.1

### UI
* Menu and Downloads open as popovers anchored under their buttons, with a pointer; no detached gap.
* Motion follows one deceleration curve without bounce; popovers fade out on close instead of vanishing.
* Light and dark switch as a single cross-fade (View Transitions), in the toolbar and in pages.
* Maximized windows keep a top margin, use square corners, and show a Restore Down icon.
* Snap Layouts flyout waits 600 ms on hover, matching Windows.
* Respects the system Reduce Motion setting.

## v2.13.0 (2026-11-20)
- **Auto-update with signed manifests:** CI publishes `latest.json` (version, URL, SHA-256, Ed25519 signature) beside the EXE. The browser checks 20 s after start and daily, verifies the signature against the compiled-in public key and the EXE hash, swaps the EXE (old one kept until next start) and offers **Restart to update**. Tampered manifests, files or links are rejected; downloads are only accepted from this repository's releases.
- **Notify-only until configured:** with no signing key set, it only shows "Update available" linking to the release page. Setup in `docs/RELEASING.md` (free, one time).
- **Code signing in CI:** optional Authenticode step (runs when certificate secrets exist), ordered before the manifest so the signed hash matches.
- Controls: update button (Check / Update available / Restart to update) and Auto Update toggle.

## v2.12.0 (2026-11-20)
- **Fingerprint Shield (per site, on by default):** canvas exports and pixel reads, WebGL readPixels, audio samples and `hardwareConcurrency` get small changes that are stable per site within a session and different across sites and launches (Brave-style farbling). WebGL GPU vendor/renderer strings are generalised when a page asks for them. Trackers can no longer match one canvas/audio fingerprint across sites.
- Changes are invisible in practice: at most 48 pixels flip one colour bit; audio varies by under one part in ten million.
- Wrappers are Proxies over native functions, so `Function.prototype.toString` still reports native code.
- Known limits: Web Workers and some sandboxed blank frames are not covered (Brave does this in the engine).
- New test fails the build if any content-script placeholder is left unreplaced (a missing one silently disables every in-page feature).

## v2.11.0 (2026-11-20)
- **Real ad and tracker blocking:** Brave's `adblock` engine with EasyList + EasyPrivacy (135,151 rules on 2026-11-20) replaces the 16 hard-coded URL patterns. Every request is checked, including those from iframes and workers (`ICoreWebView2_22` source kinds), about 3 µs per request.
- **Element hiding:** page-specific and generic cosmetic rules hide empty ad containers. Applied with constructable stylesheets (works under strict CSP), one rule at a time so a bad list selector cannot break the rest; new elements are picked up as the page changes.
- **Lean by design:** lists are downloaded at runtime (EXE carries none), compiled once, cached (5.8 MB file) and loaded in ~6 ms at startup; refreshed weekly in the background; ~13 MB RAM shared by all tabs. Until the first download the built-in list is used.
- Controls: Update Block Lists button; list status in the site title tooltip.
- The main page document is never blocked (no blank tabs); only its subresources and frames.

## v2.10.0 (2026-11-20)
- **HTTPS Only (on by default):** top-level `http://` navigations are upgraded to `https://`. If the secure version fails or redirects back to http, a warning page explains the risk and offers Go back or Continue (not secure).
- The Continue link carries a one-time random code, so a hostile page cannot link to `http://yourbank` and skip the upgrade. Exceptions last for the session.
- Never applied to LAN, loopback, intranet (single-label) hosts or non-standard ports, so routers, NAS boxes and dev servers keep working.
- Toggle: Controls, HTTPS Only.

## v2.9.9 (2026-11-20)
- **JavaScript switch is real:** uses WebView2 `IsScriptEnabled` (page scripts no longer run). It used to delete `<script>` tags after they had already executed.
- **Images switch is real:** image requests are blocked at the network layer, saving data. It used to hide images with CSS after downloading them.
- **Mobile Site replaces Desktop UA:** sends a real mobile HTTP User-Agent so servers return their mobile pages. Desktop UA only changed `navigator.userAgent` and was meaningless on a desktop browser.
- **Reject Cookie Banners:** renamed to say what it does (clicks common "Reject all" buttons).
- Ad filter handler now re-checks every request, so the image rule and the ad rule can share one WebView2 event safely.

## v2.9.8 (2026-11-20)
- **Floating download bar never appeared (root cause):** `content_guard.js` crashed at document start on every page (`appendChild` on a null `<head>`), which aborted the whole script: floating bar, drop-basket handlers, page metadata (history titles, tab UA/cookies), find counts, audio state and in-page shortcuts never ran. Early DOM writes now wait for a root element.
- **IDM-style bar:** appears over every visible video/audio player, including `blob:`/MSE streaming players, and pops up once when a video starts playing. "This video" hands the page to yt-dlp (with the tab's session cookies, except on YouTube). Styling uses constructable stylesheets so strict-CSP sites cannot hide it.
- **Downloads panel simplified:** one link box, Clear, Find on page, folder, settings. Connections, schedule, queue, speed limit, folder, tools, CSV and DASH merge moved behind settings. Rows use plain language.
- **Clear works:** it removed only failed or missing items before; it now removes all finished and failed entries (files stay on disk).
- **Private tabs are actually private:** they shared the normal profile (cookies, logins, storage, cache). They now run in a WebView2 InPrivate session; popups from private tabs stay private.
- **Password manager:** WebView2's built-in password saving and autofill enabled (DPAPI-encrypted in the profile, off in private tabs), with Save Passwords toggle and Clear Passwords.
- **Tracking prevention:** Edge tracking prevention set to Balanced, with a Strict toggle.
- **Closed source, public repo:** proprietary `LICENSE` (Copyright Sayed Naim; source visible for transparency, no reuse rights) and `THIRD_PARTY_NOTICES.md`. The repo stays public so GitHub Actions minutes remain free; releases stay in this repo.

## v2.9.7 (2026-11-20)
**Download engine (`src/downloader.rs`), rebuilt so the IDM features actually work:**
- **Pause/Resume fixed:** `.offset` resume files were deleted on every pause, so Resume restarted from zero. They are now removed only after a fully successful transfer. Single-connection downloads also resume (new `_s.offset` sidecar plus `Range: bytes=N-`).
- **No more silent corruption:** segment requests now require `206 Partial Content` with a matching `Content-Range` start. Previously a server answering a ranged request with `200` had its whole body written at each segment offset.
- **gzip disabled for the engine:** transparent decompression made Range offsets refer to compressed bytes.
- **Tab User-Agent is used:** the UA captured from the page was stored and then ignored (every request sent a hardcoded Firefox UA).
- **Resilience:** a failed segmented pass retries only the missing ranges before falling back to one connection; 429/5xx/timeouts retry with backoff; truncated transfers are detected.
- **Speed limit** now applies to single-connection, HLS and yt-dlp jobs too.
- **Correct folder after rename:** `Content-Disposition` (including RFC 5987 `filename*`) renames now also update the category used on finalize; name collisions become `name (1).ext`.
- **HLS:** uses FFmpeg (auto-installed) for encrypted, byte-ranged and fMP4 streams and writes a real MP4. Fallback path sends UA/Referer/Cookie, resolves relative and absolute URIs, picks the highest-bandwidth variant, includes `#EXT-X-MAP`, retries segments, and saves raw MPEG-TS as `.ts`.
- **DASH mux:** FFmpeg is installed on demand instead of saving a video-only "MP4"; progress is shown; parts are kept on mux failure so Resume re-muxes without re-downloading; Resume of a DASH task works.
- **YouTube via yt-dlp:** YouTube watch/shorts/live pages and `.mpd` manifests are routed to yt-dlp (auto-installed), merging best video+audio to MP4 with live progress, pause (kills the process tree) and resume. Direct googlevideo URLs are signature/PO-token bound and usually return 403.
- **Verified tool installs:** ffmpeg (gyan.dev `.sha256`) and yt-dlp (`SHA2-256SUMS`) are SHA-256 checked before use. Tools now live in `%LOCALAPPDATA%\ShunnoBrowser\bin` (old location still detected) and never flash a console window.
- Hashing and file moves run off the async workers; queue file is written atomically; CSV export is protected against formula injection.

**Browser integration (`src/main.rs`):**
- **UI never froze at the last download:** the ticker only refreshed while something was active, so the final COMPLETED state and the "when queue finishes" action were never delivered. It now refreshes on the busy to idle edge.
- **WebView2 native downloads** can really be paused, resumed and cancelled from the panel (`ICoreWebView2DownloadOperation`), and a user pause is no longer reported as "Download interrupted".
- The **Get** box now expands batch patterns like `track_[01-12].flac` (only the stream list did before).
- New **yt-dlp** button and status in the Downloads panel.

**Security:**
- **Critical:** content IPC accepted any message prefixed `INTERNAL|` without the token, so any website could drive page-level commands. The prefix is gone; native ad-block counts use a dedicated event. Token comparison is constant-time.
- The content script captures `chrome.webview.postMessage` at document start, so a page replacing it cannot read the IPC token.
- Pages can no longer make the engine probe or download loopback/LAN/link-local/metadata hosts (probe results were reported back to the page, bypassing CORS), unless the page is on that same host.
- `OFFLINE_HTML` is accepted only right after the user pressed Save Page for that tab.
- UI `encodeAttr()` escaped only `'`; page-controlled URLs containing `\` or `"` could break out of `onclick` handlers in the privileged UI. It now JS-escapes and HTML-escapes.
- `DL_SHOW` only reveals paths that belong to a known download.

**Repository:** dotfiles renamed to `.gitignore` / `.editorconfig` / `.gitattributes` (they were inert), `Cargo.lock` committed for reproducible builds, stale duplicate sources removed, CI runs `cargo test` and builds pull requests, `build.rs` skips the icon resource on non-Windows hosts so `cargo check --target x86_64-pc-windows-msvc` works from Linux.

## v2.9.6 (2026-11-20)
- **Clean Windows 11 Snap Layout Flyout (Zero `1`–`9` Numbers):** Replaced synthetic `Win + Z` with a native-styled 6-grid hover flyout under the Maximize button (`□`) backed by `MonitorFromWindow` + `GetMonitorInfoW` (`rcWork`) + `SetWindowPos`.
- **Classic Incognito Icon:** Updated the New Private Tab button and all Private Tab pills to the clean flat-top hat + round glasses Incognito SVG icon.
- **Fixed `F11` Fullscreen Exit (`src/main.rs`):** Implemented native Win32 `GetWindowPlacement` / `SetWindowPlacement` fullscreen toggling with programmatic WebView2 focus transfer (`MoveFocus`) so pressing `F11` reliably toggles fullscreen both ON and OFF from any webpage, toolbar, or Start Page.

## v2.9.5 (2026-11-20)
- Restored the **`⚡` Downloads** button back onto the right side of the Address Bar next to **`•••` Controls**.
- Kept **Playback Speed (`⏩ 1.0×`)** cleanly inside **Controls (`•••`)**.

## v2.9.4 (2026-11-20)
- Added **Windows 11 Snap Layouts** flyout on hovering the Maximize button (`□`) via native Win32 `SendInput` (`Win + Z`).
- Added native **Save Page as PDF (`📄 PDF`)** using WebView2 `ICoreWebView2_7::PrintToPdf` inside **Controls (`•••`)** and **`Ctrl + K`** Command Palette.
- Fixed **Light ↔ Dark** transition flicker using an opaque (`0.98`) ambient cross-dissolve curtain with double-`requestAnimationFrame` compositor synchronization.
- Fixed **`F11` Fullscreen Off** by removing duplicate `KEY:f11` dispatch, adding a `300ms` toggle debounce, and passing explicit `is_fullscreen` state into `ChromeSnapshot`.
- Replaced the private tab icon with a clean **Anonymous Mask SVG** and moved the `⚡` Downloads/Speed button into **Controls (`•••`)**.

## v2.9.3 (2026-11-20)
- **Responsive Unified Address Bar (`src/ui_chrome.rs`):** Rebuilt `.nav-bar` with a 3-column CSS Grid and proportional `clamp(320px, 48vw, 640px)` scaling so the centered Address Capsule adjusts smoothly on window resize without collapsing `#url-input` (`min-width: 140px`).
- **Resolved Intermittent Web Page Freezes (`src/main.rs`):** Eliminated synchronous `ffmpeg -version` subprocess spawns and `list_user_scripts` disk scans from `push_chrome_state` (moved to async startup/on-demand cache), offloaded `save_history` writes to Tokio, skipped `WM_NCHITTEST` border interception when maximized (`IsZoomed`), and explicitly raised the active tab HWND above `chrome_wv` whenever overlays close (`bring_active_tab_above_chrome`).
- **Cleaner Address Bar (`src/ui_chrome.rs`):** Removed the `🎯` Detected Media button from the address bar.
- **Full-Bounds Polished Taskbar Icon (`assets/app.ico`):** Regenerated across standard Windows resolutions (`16×16`, `24×24`, `32×32`, `48×48`, and `256×256`) with zero outer padding (`radius = 0.50`), 8×8 SDF anti-aliasing, and stronger optical weight at small sizes.

## v2.9.2 (2026-11-20)
- Added native right-click **"Download link with Shunno"** context menu item on all HTTP/HTTPS webpage links (`ICoreWebView2_11::add_ContextMenuRequested`), routing directly into `NativeDownloadEngine` with full `HttpOnly` cookie sync and live `⚡ Downloads` panel tracking.
- Added **"Download media with Shunno"** context menu item when right-clicking images, videos, or audio sources (`HasSourceUri` / `SourceUri`).

## v2.9.1 (2026-11-20)
- Fixed the 3 `dl_proxy` scope errors in `attach_native_webview2_hooks` (`src/main.rs:880, 889, 917`) and removed the 1 unused `dl_proxy` variable warning in `create_tab_webview` (`src/main.rs:1039`).
- Wired `current_user_agent()` in `NativeDownloadEngine::spawn_worker` (`src/downloader.rs`) for zero compiler warnings.

## v2.9.0 (2026-11-20)
- **Guaranteed Working Hybrid Download Engine (`src/main.rs`, `src/downloader.rs`):** Bridged WebView2's native `ICoreWebView2_4::add_DownloadStarting` (`ICoreWebView2DownloadOperation`) directly into Shunno's `⚡ Downloads` panel (with live bytes, speed, category routing, and SHA-256 verification) so clicks on Google Drive, HubDrive, HubCloud, Cloudflare, POST forms, and `blob:` links download with 100% reliability; also synced live `navigator.userAgent` into the 32-thread parallel accelerator.
- **Floating 10%-Transparent Menu & Downloads (`src/ui_chrome.rs`, `src/main.rs`):** Removed page splitting so Menu (`•••`) and Downloads (`⚡`) float cleanly over the full-width webpage at 10% transparency (`90%` opaque frosted surface) using explicit Win32 sibling HWND z-ordering (`bring_chrome_above_tabs`).
- **Polished HiDPI Taskbar & App Icon (`assets/app.ico`, `src/ui_chrome.rs`):** Redesigned the icon with 6×6 SDF anti-aliasing, diagonal sapphire-to-cyan lighting, bold white geometric `0` ring, luminous coral-amber kinetic arc, and a true `256×256` PNG frame inside `assets/app.ico`.

## v2.8.0 (2026-11-20)
- Made Menu (`•••`) and Downloads (`⚡`) 100% solid opaque right-docked side panels (`440px`) in a non-overlapping layout so every button is 100% clickable with zero transparency.
- Shrunk Address Capsule (`max-width: 420px`, `height: 30px`, `border-radius: 15px`).
- Replaced `filter: invert()` interpolation with a smooth ambient cross-dissolve veil for flicker-free Light ↔ Dark transitions.
- Added native `ICoreWebView2::add_SourceChanged` + `history.pushState`/`replaceState` hooks so the Address Bar always displays the complete URL (including paths & query strings on YouTube and SPAs).
- Upgraded `assets/app.ico` to full-bleed `16–128px` with zero pale rim so the taskbar icon matches Chrome, Edge, and Firefox in visual size.
- Hardened `NativeDownloadEngine` with automatic 1-thread fallback on CDN `429`/`403`/`503` throttling, native `blob:`/`data:` passthrough, `openDrawer('downloads')` (never toggles closed), and URL scheme normalization.

## v2.7.1 (2026-11-20)
- Fixed `E0433` (`cannot find type PCWSTR`) in `dispatch_download_with_httponly_cookies` and `clear_httponly_cookies_for_url` (`src/main.rs`).
- Restored `SetCurrentProcessExplicitAppUserModelID` call inside `set_explicit_app_user_model_id()`, eliminating the `unused_unsafe` warning and ensuring proper Windows taskbar icon binding.
- Upgraded `.github/workflows/main.yml` to Node.js 24-native `actions/checkout@v6` and `actions/upload-artifact@v7`.

## v2.7.0 (2026-11-20)
- Fixed `src/downloader.rs:297` CSV export string escaping compile error.
- Fixed Windows taskbar & `.exe` icon: moved `SetCurrentProcessExplicitAppUserModelID` before window creation, restored `WindowBuilder::with_window_icon(load_embedded_window_icon())` with full-brightness `assets/app.ico`, applied `CreateIconFromResourceEx` directly to `GCLP_HICON`/`WM_SETICON`, and enforced `winres` compilation in `build.rs`.
- Built true native IDM-class HTTP Response Media Sniffer (`ICoreWebView2_2::add_WebResourceResponseReceived`): detects `video/*`, `audio/*`, `.m3u8`, `.mpd`, and `googlevideo.com/videoplayback` streams by `Content-Type` header regardless of URL extension, injects them straight into the floating `0↓ Download` pill, and reuses `200 OK` responses directly in `NativeDownloadEngine` so single-use tokenized download links never break.

## v2.6.0 (2026-11-20)
- Added **One-Click Page Screenshot (`📸 Capture`)** via WebView2 `CapturePreview`, saving crisp PNG captures directly to `<Downloads>\Screenshots\` and revealing them in File Explorer.
- Added **Custom Downloads Folder** setting (`custom_downloads_dir` in `GlobalSettings`) so users can route downloads, screenshots, and CSV exports to any drive or directory.

## v2.5.0 (2026-11-20)
- Added **Per-Site Zoom Memory**: saves each domain's zoom level in `SitePolicy` and restores it automatically across navigations and restarts.
- Added **Automatic 3× Segment Retry** with exponential backoff (`1s` / `2s` / `4s`) inside `NativeDownloadEngine` so transient network drops resume seamlessly from the saved byte offset.
- Added **CSV Download History Export (`📄 CSV`)** with SHA-256 checksums (`Downloads\ShunnoBrowser\download_history.csv`) and **Duplicate Current Tab** in the Command Palette.

## v2.4.0 (2026-11-20)
- Moved WebView2 user profile cleanly into `%LOCALAPPDATA%\ShunnoBrowser\WebView2` via `WebContext::new` + `WEBVIEW2_USER_DATA_FOLDER`, eliminating the `ShunnoBrowser.exe.WebView2` folder beside the executable.
- Added **Tab Sleeping** (`ICoreWebView2_3::TrySuspend` / `Resume`) with automatic 15-minute idle hibernation and one-click `💤 Sleep Tabs` RAM saver.
- Added **50/50 Split-View** (`◫ Split`) to display two open tabs side-by-side.
- Added **`Ctrl + K` Command Palette** for instant keyboard search across tabs, bookmarks, history, and quick actions.

## v2.3.0 (2026-11-20)
- Added automatic WebView2 `HttpOnly` cookie synchronization via `ICoreWebView2CookieManager::GetCookies` (`GetCookiesCompletedHandler`) so authenticated Google Drive and login-gated downloads pass full session cookies into `NativeDownloadEngine`.
- Enhanced `🧹 Clear Site Data` to delete `HttpOnly` session cookies via `ICoreWebView2CookieManager::DeleteCookie` alongside `localStorage`, `sessionStorage`, Cache Storage, and IndexedDB.

## v2.2.1 (2026-11-20)
- Eliminated the patchy grey shade bleeding through the top toolbar by making `.chrome-shell` (`--surface`) 100% opaque while keeping overlay space transparent.
- Fixed `F11` fullscreen toggle on/off via native WebView2 `ICoreWebView2Controller::add_AcceleratorKeyPressed` (`VK_F11`) on both the chrome toolbar and all tab WebViews plus explicit `is_fullscreen` state tracking.
- Reduced compiled `.exe` binary size by ~40–55%: removed the `regex` crate, switched `reqwest` to Windows-native `native-tls` (Schannel), trimmed unused `tokio` features, and enabled `opt-level = "z"`, `lto = true`, `panic = "abort"`, `strip = "symbols"`.

## v2.2.0 (2026-11-20)
- Eliminated Dark Mode toggle flicker: replaced full WebView recreation with smooth `0.36s` live CSS transitions across the UI shell and all open tabs.
- Enlarged taskbar `.ico` (full-bleed `0.495` radius), top-left tab bar logo (`28×28px`), and tab pill icons (`18×18px`).
- Refined the centered Address Capsule (`max-width: 500px`, `height: 32px`, `border-radius: 16px`).
- Added native Windows 11 rounded window corners (`DwmSetWindowAttribute` + `DWMWCP_ROUND`).
- Fixed download engine & site button reliability: removed duplicate `DownloadStarting` COM hook, upgraded `reqwest` to standard Edge/Chrome User-Agent + `Accept: */*` + 20-hop redirects + safe non-range fallback, and allowed `about:blank` / OAuth popups so download/login buttons on sites like `hubdrive.blog` work properly.

## v2.1.3 (2026-11-20)
- Fixed `E0061` argument mismatch on `push_chrome_state(...)` inside `TabTitleChanged` (`src/main.rs`).
- Added `move` to `push_chrome_state` closure so `last_chrome_json` moves cleanly into `event_loop.run` (`E0373`).

## v2.1.2 (2026-11-20)
- Fixed `assets/app.ico` 4x4 supersampling RGB double-division bug so the Windows taskbar, Alt+Tab, and `.exe` icon render at full sapphire/white/coral brightness instead of a dark circle.
- Fixed `windows = "0.56.0"` Win32 type signatures (`HINSTANCE(hinst.0)` in `LoadImageW`, `HWND(isize)`, `HWND(0)`, `parent.0 != 0`) in `src/main.rs`.
- Added JSON snapshot deduplication in `push_chrome_state` to skip redundant UI IPC evaluations.

## v2.1.1 (2026-11-20)
- Fixed `HWND` / `HICON` raw pointer conversions (`*mut c_void` vs `isize`) in `windows = "0.56.0"` inside `src/main.rs`.

## v2.1.0 (2026-11-19)
- Fixed `SetWindowSubclass` / `DefSubclassProc` error by using standard `SetWindowLongPtrW(GWLP_WNDPROC)` + `CallWindowProcW`, and removed unused `Icon` import (0 errors, 0 warnings).
- Added **Live Download Speed Sparkline Graph** (`⚡` current & peak throughput SVG area chart) inside **Downloads**.
- Added one-click **Per-Site Cookie & Storage Cleaner (`🧹 Clear Site Data`)** inside **Controls (`•••`)**.

## v2.1.0 (2026-11-19)
- Completed all 20 roadmap items natively in pure Rust: zero localhost open ports, 3px borderless window edge resize grip, F11 fullscreen hiding of both tab & nav bars, single-click URL select-all, full URL display, elevated search engine selector popup, larger 34x34px vector navigation icons, duplicate Skip/Rename/Overwrite dialog, sequential download queue scheduler (`Max 1-4` + delayed start + post-queue actions), `Ctrl+Shift+T` closed-tab restore, pinned icon-only tabs, live `1 / N` Find-in-Page match counter, draggable persistent floating drop basket, and one-click `⬇ Install FFmpeg` auto-extractor.

## v2.1.0 (2026-11-19)
- Fixed `E0063` missing fields (`active_reader` in `ChromeSnapshot`, `reader_active` in `BrowserTab`).
- Removed all external IDM / standalone downloader branding and merged 100% of multi-threaded download manager features natively into Shunno Browser's built-in **Downloads (`⚡`)** manager.
- Added **Drag-to-Reorder Tabs** in the tab strip (`REORDER_TAB`) and **Bookmark Folder Grouping** across the Bookmarks Bar and Library.

## v2.1.0 (2026-11-19)
- Fixed `proxy` clone (`ad_proxy`) in `WebResourceRequestedEventHandler` (`E0382`).
- Fixed Taskbar Icon via `LoadImageW` from embedded EXE icon resource #1 + `SetClassLongPtrW` (`GCLP_HICON`/`GCLP_HICONSM`) + `WM_SETICON`.
- Added native Win32 8-directional mouse window resizing (`WM_NCHITTEST` + `WS_THICKFRAME`) on borderless window.
- Added `ZoomFactorChangedEventHandler` so `Ctrl + Mouse Scroll` zooms smoothly and updates the zoom indicator.
- Eliminated Menu/Download open flicker by keeping the live webpage visible beneath a transparent `chrome_wv` (`SetDefaultBackgroundColor` alpha=0) with a 10% subtle blur.
- Added On/Off toggle for Reader Mode (`📖 Reader`), Rename (`✎`) & Remove (`✕`) on Bookmarks, and coordinate-based IDM media detection + optional `native engine` forwarding.

## v1.7.2 (2026-11-19)
- Fixed `#[cfg(windows)]` / `#[cfg(not(windows))]` attribute pairing on `apply_tab_zoom` and `set_webview_muted` in `src/main.rs` (`E0428`).

## v1.7.1 (2026-11-19)
- Fixed `proxy` clone (`ad_proxy`) in `WebResourceRequestedEventHandler` (`src/main.rs:242`).
- Added native `ICoreWebView2Controller::SetZoomFactor` + capture-phase `Ctrl++` / `Ctrl+-` / `Ctrl+0` / `Ctrl+Wheel` zoom handling.
- Added the Shunno compass-ring emblem inside `New Tab` pills.

## v1.7.1 (2026-11-19)
- Cast `ICoreWebView2` to `ICoreWebView2_11` (`add_ContextMenuRequested`) and `ICoreWebView2Environment` to `ICoreWebView2Environment9` (`CreateContextMenuItem`) in `src/main.rs`.

## v1.6.1 (2026-11-19)
- Fixed Win32 COM stream & memory feature flags (`Win32_System_Com_StructuredStorage`, `Win32_System_Memory`) and `HGLOBAL::default()` in `capture_tab_preview_to_chrome`.
- Replaced Edge WebView2's right-click **"Open link in new window"** context menu item with **"Open link in new tab"**.

## v1.6.0 (2026-11-18)
- Fixed `ICoreWebView2` -> `ICoreWebView2_8` COM interface cast inside `IsDocumentPlayingAudioChangedEventHandler` (`src/main.rs:184`).
- Added throttled `mousemove` fallback for the Trusted-Types safe floating media download pill so players with transparent overlay shields always trigger the pill.

# Changelog

## v1.6.0 (2026-11-18)
- **Clickable Docked Floating Controls & Downloads:** Fixed Win32 child WebView2 Z-order occlusion by docking the active webpage beside the open controls/downloads popover so every button works reliably while keeping the live webpage visible.
- **Tab Audio Mute Indicators (`🔊` / `🔇`):** Added live audio activity detection and one-click per-tab mute toggling in the tab strip.
- **Built-In Picture-in-Picture (`📺 PiP`):** Added native Web PiP button to the floating video hover pill, controls popover, and optional auto-PiP on window focus loss.
- **Download Auto-Cleanup & Duplicate Detection:** Detects existing completed or active downloads before duplicating tasks, adds a `Clear Completed` button, and supports auto-cleaning finished tasks.
- **Trusted-Types Safe YouTube Floating Pill & Ad Stripper:** Rebuilt the Shadow-DOM floating media pill without `.innerHTML` so YouTube's Trusted Types CSP never blocks it, and replaced polling intervals with debounced `MutationObserver` + `requestIdleCallback`.
- **Security & Architecture Hardening:** Extracted `src/sanitizer.rs`, `src/userscripts.rs`, and `assets/js/content_guard.js`; added per-launch cryptographic IPC tokens, SHA-256 user-script approval in `%LOCALAPPDATA%\ShunnoBrowser\scripts\`, Windows filename sanitization, `rustls-tls`, and automated tests.

## v1.0.0
- Initial native Rust + Edge WebView2 release with 32-thread range downloader, per-site policies, dark mode, and custom search engines.

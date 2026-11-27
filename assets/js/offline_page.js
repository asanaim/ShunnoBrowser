(function () {
  var h = location.href;
  var host = '';
  try { host = new URL(h).hostname; } catch (e) {}
  var shown = (document.body && document.body.getAttribute('data-shunno-offline')) === '1';
  if (shown) return;
  var css = ':root{color-scheme:light dark}html,body{height:100%;margin:0}body{display:flex;align-items:center;justify-content:center;' +
    'font-family:system-ui,"Segoe UI",sans-serif;background:#fff;color:#1d1d1f}' +
    '@media(prefers-color-scheme:dark){body{background:#1c1c1e;color:#f5f5f7}.s{color:#98989d}button{background:#0a84ff}}' +
    '.c{max-width:420px;padding:24px}h1{font-size:22px;font-weight:600;margin:0 0 8px}.s{color:#6e6e73;font-size:14px;line-height:1.5;margin:0 0 20px}' +
    'button{font:inherit;font-weight:600;border:0;border-radius:10px;padding:8px 18px;background:#007aff;color:#fff;cursor:pointer}';
  var html = '<!doctype html><meta charset="utf-8"><title>No connection</title><style>' + css + '</style>' +
    '<body data-shunno-offline="1"><div class="c"><h1>You are offline</h1><p class="s">Shunno cannot reach <b id="h"></b>. ' +
    'Check the network connection, then try again.</p><button onclick="location.reload()">Try again</button></div>';
  document.open();
  document.write(html);
  document.close();
  var el = document.getElementById('h');
  if (el) el.textContent = host || 'this page';
  window.addEventListener('online', function () { location.reload(); });
})();

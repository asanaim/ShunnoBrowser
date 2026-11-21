# Third-party notices

Shunno Browser is proprietary software. It is built with the open-source Rust crates
listed in `Cargo.lock`, which are licensed under permissive licenses (predominantly
MIT and/or Apache-2.0). Their copyright notices and license texts must accompany
binary distributions.

Generate the complete, exact notice file for each release with:

```
cargo install cargo-about
cargo about generate about.hbs > THIRD_PARTY_LICENSES.html
```

The `adblock` crate (Brave Software) is MPL-2.0 and is used unmodified; its source is available at
https://github.com/brave/adblock-rust. If any of its files are ever modified, those files must be published.

Runtime components fetched on demand (not bundled):
- Microsoft Edge WebView2 Runtime (Microsoft Software License Terms), provided by Windows.
- FFmpeg essentials build from gyan.dev (GPL). Downloaded and run as a separate program.
- yt-dlp from github.com/yt-dlp/yt-dlp (Unlicense). Downloaded and run as a separate program.
- EasyList and EasyPrivacy filter lists (GPLv3 / CC BY-SA 3.0), downloaded weekly from easylist.to. Used as data, not bundled.

## Mozilla Readability 0.6.0

Used by Reader (`assets/js/readability.min.js`, and `isProbablyReaderable` in `assets/js/content_guard.js`), minified, otherwise unmodified.
Source: https://github.com/mozilla/readability

Copyright (c) 2010 Arc90 Inc. Licensed under the Apache License, Version 2.0 (https://www.apache.org/licenses/LICENSE-2.0).


## webview2-com 0.30.0 (vendored, modified)

Source: https://github.com/wravery/webview2-rs. Licence: MIT, Copyright (c) the webview2-rs authors.
Shunno ships a copy in `vendor/webview2-com` with one change in `src/options.rs`: the environment options
object also implements `ICoreWebView2EnvironmentOptions6` so browser extensions can be enabled.

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated
documentation files (the "Software"), to deal in the Software without restriction, including without limitation
the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and
to permit persons to whom the Software is furnished to do so, subject to the following conditions: The above
copyright notice and this permission notice shall be included in all copies or substantial portions of the
Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT
LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.

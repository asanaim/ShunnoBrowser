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

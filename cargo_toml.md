[package]
name = "shunno-browser"
version = "1.0.1"
edition = "2021"
description = "Shunno Browser — Fast, privacy-first Windows desktop browser with an integrated multi-threaded download engine"

[[bin]]
name = "ShunnoBrowser"
path = "src/main.rs"

[dependencies]
tao = "0.30"
wry = "0.44"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.40", features = ["full"] }
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "stream", "gzip", "brotli", "deflate", "cookies"] }
futures-util = "0.3"
url = "2.5"
dirs = "5.0"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.10", features = ["v4"] }
percent-encoding = "2.3"
once_cell = "1.19"
rfd = "0.14"

[target.'cfg(windows)'.dependencies]
windows-core = "0.56"
webview2-com = "0.30"

[build-dependencies]
winres = "0.1"

[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
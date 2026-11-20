fn main() {
    println!("cargo:rerun-if-changed=assets/app.ico");
    let target_windows = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows";
    // rc.exe only exists on Windows hosts; cross `cargo check` from Linux skips the icon.
    let host_windows = std::env::var("HOST").unwrap_or_default().contains("windows");
    if target_windows && host_windows {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/app.ico");
        res.compile().expect("Failed to compile Windows icon resource");
    }
}

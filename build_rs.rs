fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/app.ico");
        res.set("ProductName", "Shunno Browser");
        res.set("FileDescription", "Shunno Browser");
        res.compile().expect("Failed to compile Windows icon resource");
    }
}
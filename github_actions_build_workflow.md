name: Build Shunno Browser

on:
  push:
    branches: [ main, master ]
  workflow_dispatch:

permissions:
  contents: write

jobs:
  build-windows:
    runs-on: windows-latest

    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Install Rust stable toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: x86_64-pc-windows-msvc

      - name: Cache Cargo dependencies
        uses: Swatinem/rust-cache@v2

      - name: Build ShunnoBrowser.exe (Release)
        run: cargo build --release --target x86_64-pc-windows-msvc

      - name: Package portable release ZIP
        shell: pwsh
        run: |
          New-Item -ItemType Directory -Force -Path dist | Out-Null
          Copy-Item "target/x86_64-pc-windows-msvc/release/ShunnoBrowser.exe" "dist/ShunnoBrowser.exe"
          Copy-Item "README.md" "dist/README.md"
          Compress-Archive -Path "dist/*" -DestinationPath "ShunnoBrowser-win64.zip" -Force

      - name: Upload Build Artifact
        uses: actions/upload-artifact@v4
        with:
          name: ShunnoBrowser-win64
          path: |
            dist/ShunnoBrowser.exe
            ShunnoBrowser-win64.zip

      - name: Publish GitHub Release
        if: startsWith(github.ref, 'refs/tags/') || github.event_name == 'workflow_dispatch'
        uses: softprops/action-gh-release@v2
        with:
          tag_name: v1.0.1
          name: Shunno Browser v1.0.1
          files: |
            dist/ShunnoBrowser.exe
            ShunnoBrowser-win64.zip
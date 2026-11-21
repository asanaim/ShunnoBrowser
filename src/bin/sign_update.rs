//! Release signing for the auto-updater (used by CI; replaces the Python script there).
//!
//! `sign_update manifest <exe> <version> <url> <out.json>`
//! writes latest.json and signs it when the `UPDATE_SIGNING_KEY` environment variable holds the
//! Ed25519 private key as PKCS#8 PEM (the format `scripts/sign_update.py genkey` writes).
//!
//! Built from the crate's locked dependencies (ed25519-dalek, sha2), so the step that holds the
//! signing key downloads and runs no third-party package.

use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};

/// Same text the updater verifies (src/updater.rs `signed_message`).
fn message(version: &str, sha256: &str, url: &str) -> String {
    format!("shunno-update-v1\n{version}\n{sha256}\n{url}")
}

fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    };
    let clean: Vec<u8> = s.bytes().filter(|c| !c.is_ascii_whitespace()).collect();
    let body: Vec<u8> = clean.iter().copied().take_while(|&c| c != b'=').collect();
    let mut out = Vec::with_capacity(body.len() * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0;
    for c in body {
        acc = (acc << 6) | val(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// UPDATE_PUBLIC_KEY_HEX as written in src/updater.rs (empty when not set there).
fn embedded_public_key() -> String {
    const SRC: &str = include_str!("../updater.rs");
    SRC.split("UPDATE_PUBLIC_KEY_HEX: &str = \"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// The 32-byte seed from an unencrypted PKCS#8 Ed25519 PEM.
fn seed_from_pem(pem: &str) -> Result<[u8; 32], String> {
    let body: String = pem.lines().filter(|l| !l.starts_with("-----")).collect();
    let der = b64_decode(&body).ok_or("UPDATE_SIGNING_KEY is not valid PEM")?;
    // SEQUENCE { INTEGER 0, SEQUENCE { OID 1.3.101.112 }, OCTET STRING { OCTET STRING(32) } }
    const PREFIX: [u8; 16] = [0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20];
    if der.len() != 48 || der[..16] != PREFIX {
        return Err("UPDATE_SIGNING_KEY is not an unencrypted PKCS#8 Ed25519 key".into());
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&der[16..]);
    Ok(seed)
}

fn run(args: &[String]) -> Result<(), String> {
    let [_, cmd, exe, version, url, out] = args else {
        return Err("usage: sign_update manifest <exe> <version> <url> <out.json>".into());
    };
    if cmd != "manifest" {
        return Err("usage: sign_update manifest <exe> <version> <url> <out.json>".into());
    }
    let version = version.trim().trim_start_matches('v');
    let bytes = std::fs::read(exe).map_err(|e| format!("cannot read {exe}: {e}"))?;
    let sha = hex::encode(Sha256::digest(&bytes));
    let pem = std::env::var("UPDATE_SIGNING_KEY").unwrap_or_default();
    let signature = if pem.trim().is_empty() {
        String::new()
    } else {
        let key = SigningKey::from_bytes(&seed_from_pem(pem.trim())?);
        // A key that does not match the one compiled into the browser would publish an update no
        // installed copy accepts: stop the release instead.
        let built_in = embedded_public_key();
        let ours = hex::encode(key.verifying_key().to_bytes());
        if !built_in.is_empty() && ours != built_in {
            return Err(format!("UPDATE_SIGNING_KEY does not match UPDATE_PUBLIC_KEY_HEX in src/updater.rs (key {ours})"));
        }
        hex::encode(key.sign(message(version, &sha, url).as_bytes()).to_bytes())
    };
    let json = serde_json::json!({ "version": version, "url": url, "sha256": sha, "signature": signature });
    std::fs::write(out, json.to_string()).map_err(|e| format!("cannot write {out}: {e}"))?;
    println!(
        "latest.json: version={version} sha256={}... signed={}",
        &sha[..12],
        if signature.is_empty() { "no (notify-only)" } else { "yes" }
    );
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Err(e) = run(&args) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

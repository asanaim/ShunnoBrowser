"""Shunno Browser update signing.

  python scripts/sign_update.py genkey
      Creates update_signing_key.pem (KEEP SECRET, never commit) and prints the public key hex
      to paste into UPDATE_PUBLIC_KEY_HEX in src/updater.rs.

  python scripts/sign_update.py manifest <exe> <version> <url> <out.json>
      Writes latest.json. Signs it when the UPDATE_SIGNING_KEY environment variable holds the PEM.
"""
import hashlib, json, os, sys
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

def message(version, sha256, url):
    return f"shunno-update-v1\n{version}\n{sha256}\n{url}".encode()

def genkey():
    key = Ed25519PrivateKey.generate()
    pem = key.private_bytes(serialization.Encoding.PEM, serialization.PrivateFormat.PKCS8, serialization.NoEncryption())
    if os.path.exists("update_signing_key.pem"):
        sys.exit("update_signing_key.pem already exists; refusing to overwrite")
    with open("update_signing_key.pem", "wb") as f:
        f.write(pem)
    pub = key.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw).hex()
    print("Private key written to update_signing_key.pem (add its full text as GitHub secret UPDATE_SIGNING_KEY, then delete or store offline).")
    print("Public key for src/updater.rs UPDATE_PUBLIC_KEY_HEX:")
    print(pub)

def manifest(exe, version, url, out):
    version = version.strip().lstrip("v")
    with open(exe, "rb") as f:
        sha = hashlib.sha256(f.read()).hexdigest()
    sig = ""
    pem = os.environ.get("UPDATE_SIGNING_KEY", "").strip()
    if pem:
        key = serialization.load_pem_private_key(pem.encode(), password=None)
        sig = key.sign(message(version, sha, url)).hex()
    with open(out, "w", encoding="utf-8", newline="\n") as f:
        json.dump({"version": version, "url": url, "sha256": sha, "signature": sig}, f)
    print(f"latest.json: version={version} sha256={sha[:12]}... signed={'yes' if sig else 'no (notify-only)'}")

if __name__ == "__main__":
    if len(sys.argv) >= 2 and sys.argv[1] == "genkey":
        genkey()
    elif len(sys.argv) == 6 and sys.argv[1] == "manifest":
        manifest(*sys.argv[2:])
    else:
        sys.exit(__doc__)

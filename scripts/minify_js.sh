#!/usr/bin/env bash
# Regenerates assets/js/content_guard.min.js (what the browser embeds) from content_guard.js.
# Run after every change to content_guard.js; a unit test fails while the two differ.
# Needs node and npm (terser is fetched by npx). Function names are kept (the script spoofs
# Function.prototype.toString for the functions it replaces), top level names are not touched.
set -euo pipefail
cd "$(dirname "$0")/.."
src=assets/js/content_guard.js
out=assets/js/content_guard.min.js
hash=$(node -e '
const b = require("fs").readFileSync(process.argv[1]);
let h = 0xcbf29ce484222325n;
for (const x of b) { h ^= BigInt(x); h = (h * 0x100000001b3n) & 0xffffffffffffffffn; }
process.stdout.write(h.toString(16).padStart(16, "0"));
' "$src")
tmp=$(mktemp)
npx --yes terser "$src" --compress --mangle --keep-fnames --ecma 2020 -o "$tmp"
{ printf '/*src:%s*/' "$hash"; cat "$tmp"; } > "$out"
rm -f "$tmp"
echo "wrote $out ($(wc -c < "$out") bytes from $(wc -c < "$src"))"

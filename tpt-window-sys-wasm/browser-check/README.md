# Browser check for the web backend

Runs `tpt-window-sys-wasm` in a real browser and checks the events it reports for pointer,
keyboard, text, wheel and canvas resize. It is not part of the workspace and is not run in
CI, because it needs a browser.

## Run

Requires `wasm-bindgen-cli` 0.2.130 (the same version as the crate), Node.js and a
Chromium-based browser.

```text
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown --release
wasm-bindgen --target web --out-dir www/pkg target/wasm32-unknown-unknown/release/wasm_browser_check.wasm
npm install
python -m http.server 8123 --bind 127.0.0.1 --directory www &
CHROME_PATH=/path/to/chrome npm run check
```

Leave `CHROME_PATH` unset to use the browser build that Playwright installs
(`npx playwright install chromium`).

`check.mjs` prints the cumulative event counts after each step. It reports any page errors.
The only console error that can appear is a missing favicon, and the page avoids that.

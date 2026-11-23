# Lemon Studio in WebOS

Use nightly Rust 2025-11-06 with `rust-src` and `wasm32-unknown-unknown`, Emscripten 4.0.23, and wasm-bindgen 0.2.129. Run `tools/web-setup.sh` to install the pinned browser toolchain, then build with `tools/web-build.sh`. Its locked release command produces the browser directory and a separately verified ZIP, including third-party license files. The WebOS bundle uses `web/mount.mjs` with the Desktop workspace stylesheet and places the browser directory under `native/`.

Run the WebOS editor on HTTPS or localhost. Its same-origin iframe needs Cross-Origin-Opener-Policy `same-origin` and Cross-Origin-Embedder-Policy `require-corp` to use shared memory for the audio engine. The build serves the worker and AudioWorklet as same-origin modules. The small patch in `vendor/wasm_thread` uses the supplied worker URL directly. The vendored source is based on crates.io `wasm_thread` 0.3.3 and retains both of its licenses in the browser package.

Open and Save use WebOS Files. Songs without imported media use Lemon Studio's native `.lemon` format. Songs with imported samples use a ZIP with the editable `.lemon` project and its samples. Use the editor's Open button to reopen either format or to import an existing FLP. Use Save as to keep your edits in a separate file when Files contains a newer revision.

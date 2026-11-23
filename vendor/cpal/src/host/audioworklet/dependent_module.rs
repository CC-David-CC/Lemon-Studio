// This file is based on code from:
//   https://github.com/rustwasm/wasm-bindgen/blob/main/examples/wasm-audio-worklet/src/dependent_module.rs
//
// The original code is licensed under either of:
//   - MIT license (https://opensource.org/licenses/MIT)
//   - Apache License, Version 2.0 (https://www.apache.org/licenses/LICENSE-2.0)
// at your option.
//
// Copyright (c) 2017-2024 The wasm-bindgen Developers
//
// This file incorporates code from the above source under the Apache License, Version 2.0 license.
// Please see the original repository for more details.
//
// See this issue for a further explanation of what this file does: https://github.com/rustwasm/wasm-bindgen/issues/3019

use js_sys::{wasm_bindgen, JsString};
use wasm_bindgen::prelude::*;
use web_sys::Url;

// This is a not-so-clean approach to get the current bindgen ES module URL
// in Rust. This will fail at run time on bindgen targets not using ES modules.
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen]
    type ImportMeta;

    #[wasm_bindgen(method, getter)]
    fn url(this: &ImportMeta) -> JsString;

    #[wasm_bindgen(thread_local_v2, js_namespace = import, js_name = meta)]
    static IMPORT_META: ImportMeta;
}

/// The build places the AudioWorklet beside the wasm-bindgen module.
pub fn worklet_url() -> Result<String, JsValue> {
    let module = String::from(IMPORT_META.with(ImportMeta::url));
    Url::new_with_base("./audio-worklet.mjs", &module).map(|url| url.href())
}

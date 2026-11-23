// SPDX-License-Identifier: GPL-3.0-or-later

import initialize, {wasm_thread_entry_point} from './os_app.js';

self.onmessage = async ({data:[module, memory, task]}) => {
  await initialize({module_or_path:module, memory});
  wasm_thread_entry_point(task);
  self.close();
};

// SPDX-License-Identifier: GPL-3.0-or-later

function files() {
  return window.frameElement?.lemonWorkspaceFiles;
}

export function workspace_enabled() {
  return Boolean(files());
}

export async function workspace_open(initial) {
  const file = await files().open(initial);
  return file && {...file, bytes:new Uint8Array(file.bytes)};
}

export function workspace_save(receipt, name, bytes, zipped, choose) {
  return files().save(receipt, name, new Uint8Array(bytes), zipped, choose);
}

export function workspace_report(receipt, unsaved, message) {
  files()?.report(receipt, unsaved, message);
}

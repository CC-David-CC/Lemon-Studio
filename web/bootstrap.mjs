const status = document.querySelector('#status');
const workspace = window.frameElement?.lemonWorkspaceFiles;
function fail(error) {
  status.hidden = false;
  status.textContent = `Lemon Studio could not start: ${error?.message ?? error}`;
  workspace?.failed(status.textContent);
}
window.addEventListener('error', e => fail(e.error ?? e.message));
window.addEventListener('unhandledrejection', e => fail(e.reason));
try {
  if (!crossOriginIsolated || typeof SharedArrayBuffer === 'undefined') throw Error('This page requires Cross-Origin-Opener-Policy: same-origin and Cross-Origin-Embedder-Policy: require-corp headers.');
  if (typeof AudioContext === 'undefined' || typeof AudioWorkletNode === 'undefined') throw Error('This browser does not support AudioWorklet audio.');
  const app = await import('./os_app.js');
  await app.default();
  await app.start(new URL('./worker.mjs', import.meta.url).href);
  document.querySelector('#lemon-canvas').focus();
  status.hidden = true;
  document.documentElement.dataset.lemonReady = "true";
  workspace?.started(app.workspace_command);
} catch (error) { fail(error); }

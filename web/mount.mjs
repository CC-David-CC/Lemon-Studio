// SPDX-License-Identifier: GPL-3.0-or-later

const digest = /^[a-f0-9]{64}$/;
const invalidPart = /[/\\:\u0000-\u001f]/;
const extensions = ['.lemon', '.flp', '.zip'];
const maxProjectBytes = 256 * 1024 * 1024;

function element(tag, properties) {
  return Object.assign(document.createElement(tag), properties);
}

function workspacePath(value) {
  const path = String(value).replace(/^\//, '');
  if (path.split('/').some(part => !part || part === '.' || part === '..' || invalidPart.test(part))) {
    throw Error('Choose a project in your workspace.');
  }
  return path;
}

function basename(path) {
  return path.split('/').at(-1);
}

export function mount(host) {
  const owner = host.storage.workspaceOwner();
  const session = host.session();
  const controller = new AbortController();
  const {signal} = controller;
  const section = element('section', {className:'window-content binary-studio lemon-studio'});
  const styles = element('link', {rel:'stylesheet', href:new URL('./workspace.css', import.meta.url).href});
  const controlsStyle = element('link', {rel:'stylesheet', href:new URL('./controls.css', import.meta.url).href});
  const toolbar = element('div', {className:'binary-studio-tools'});
  const frame = element('iframe', {
    className:'binary-studio-native', title:'Lemon Studio', allow:'cross-origin-isolated; autoplay',
  });
  const status = element('p', {className:'binary-studio-status', textContent:'Starting Lemon Studio…'});
  status.setAttribute('role', 'status');
  section.append(styles, controlsStyle, toolbar, frame, status);
  const receipts = new Map();
  let nextReceipt = 0;
  let context, command, pending, unregister, binding;
  let initialized = false;
  let paused = false;
  let dirty = false;

  const buttons = [['Open', 'open'], ['Save', 'save'], ['Save as', 'save-as']].map(([label, action]) => {
    const button = element('button', {type:'button', textContent:label});
    button.addEventListener('click', () => {
      try {
        check();
        command(action);
      } catch (error) {
        status.textContent = error.message;
      }
    }, {signal});
    toolbar.append(button);
    return button;
  });

  function controls() {
    for (const button of buttons) button.disabled = !initialized || paused || Boolean(pending);
    frame.inert = !initialized || paused;
  }
  controls();

  function check() {
    if (signal.aborted || session !== host.session() || owner !== host.storage.workspaceOwner() ||
        !context?.valid()) throw new DOMException('This Files session ended.', 'AbortError');
    if (paused) throw Error('Files are being transferred.');
  }

  function transaction(action) {
    if (pending) throw Error('Lemon Studio is waiting for a Files operation.');
    pending = Promise.resolve().then(() => { check(); return action(); });
    controls();
    return pending.finally(() => {
      pending = null;
      controls();
      if (initialized && !paused && !signal.aborted) frame.contentDocument?.querySelector('#lemon-canvas')?.focus();
    });
  }

  function remember(path, sha256, action) {
    if (!digest.test(sha256)) throw Error('Files did not confirm this revision.');
    const receipt = String(++nextReceipt);
    receipts.set(receipt, {path, sha256, action});
    return receipt;
  }

  async function read(path, expected) {
    check();
    const file = await context.files.read(path, {signal});
    check();
    if (expected && expected !== file.sha256) throw Error('This project changed in Files. Open its current revision.');
    if (file.data.byteLength > maxProjectBytes) throw Error('This project exceeds the browser size limit.');
    return {name:basename(path), bytes:file.data, receipt:remember(path, file.sha256, 'Opened')};
  }

  const startup = new Promise((started, failed) => {
    const timeout = setTimeout(() => failed(Error('Lemon Studio took too long to start.')), 90000);
    const stop = () => {
      clearTimeout(timeout);
      failed(new DOMException('This Files session ended.', 'AbortError'));
    };
    signal.addEventListener('abort', stop, {once:true});
    frame.lemonWorkspaceFiles = {
      started(workspaceCommand) {
        clearTimeout(timeout);
        signal.removeEventListener('abort', stop);
        command = workspaceCommand;
        started();
      },
      failed(message) {
        clearTimeout(timeout);
        signal.removeEventListener('abort', stop);
        status.textContent = message;
        failed(Error(message));
      },
      async open(initial) {
        if (initial) {
          const launch = host.w.launchContext;
          if (!launch) return null;
          if (launch.storage !== 'account' || launch.owner !== owner) throw Error('Open a project from your Files.');
          return transaction(() => read(workspacePath(launch.path), launch.restore ? null : launch.sha256));
        }
        return transaction(async () => {
          const choice = await host.storage.pickWorkspaceFile({extensions, signal});
          return choice ? read(workspacePath(choice)) : null;
        });
      },
      async save(receipt, suggested, bytes, zipped, choose) {
        return transaction(async () => {
          if (bytes.byteLength > maxProjectBytes) throw Error('This project exceeds the browser size limit.');
          const ext = zipped ? '.zip' : '.lemon';
          const current = receipts.get(receipt);
          let target = current?.path;
          if (choose || !target?.toLowerCase().endsWith(ext)) {
            const folder = await host.storage.pickWorkspaceFile({foldersOnly:true, signal});
            if (!folder) return null;
            const value = (target ? basename(target) : suggested).replace(/\.(?:lemon|flp|zip)$/i, '') + ext;
            const choice = await host.ui.prompt('File name', {
              title:'Save Lemon Studio project', value, confirmLabel:'Save', signal,
            });
            if (choice == null) return null;
            let name = choice.trim();
            if (!name || invalidPart.test(name) || name === '.' || name === '..') throw Error('Enter one project name.');
            if (!name.toLowerCase().endsWith(ext)) name += ext;
            target = workspacePath((folder === '/' ? '' : folder + '/') + name);
          }
          check();
          const expected = current?.path === target ? current.sha256 : null;
          const type = zipped ? 'application/zip' : 'application/octet-stream';
          let result;
          try {
            result = await context.files.write(target, new Blob([bytes], {type}), expected, {signal});
          } catch (error) {
            if (error?.status === 409) throw Error('The file already exists or has changed. Use Save as and choose a new name.');
            throw error;
          }
          check();
          return {receipt:remember(target, result.sha256, 'Saved'), name:basename(target)};
        });
      },
      report(receipt, unsaved, message) {
        if (signal.aborted || session !== host.session() || !context?.valid()) return;
        const current = receipts.get(receipt);
        dirty = unsaved;
        if (current !== binding) {
          binding = current;
          host.desktop.updateAccountFileBinding(host.w, {owner, path:current?.path ?? null, sha256:current?.sha256});
          if (current) status.textContent = current.action + ' ' + basename(current.path);
          else status.textContent = 'Lemon Studio is ready';
          for (const key of receipts.keys()) {
            if (Number(key) < Number(receipt)) receipts.delete(key);
          }
        }
        if (message && /^(?:Saving to Files|Save cancelled|Files save failed|Could not open project|Earlier version saved)/.test(message)) {
          status.textContent = message;
        }
      },
    };
  });
  startup.catch(() => {});

  const ready = (async () => {
    context = await host.storage.workspaceFileContext();
    check();
    if (context.owner !== owner) throw Error('This editor belongs to another Files workspace.');
    frame.src = new URL('./native/index.html', import.meta.url).href;
    await startup;
    check();
    unregister = context.registerFileWriter?.(async () => {
      if (pending) await pending;
      if (dirty) throw Error('Save your Lemon Studio project before transferring Files.');
      paused = true;
      controls();
      return () => { paused = false; controls(); };
    });
    initialized = true;
    controls();
    if (status.textContent.startsWith('Starting Lemon Studio')) status.textContent = 'Lemon Studio is ready';
  })();
  ready.catch(error => { if (!signal.aborted) status.textContent = error.message; });

  return {
    element:section,
    ready,
    focus:() => frame.contentDocument?.querySelector('#lemon-canvas')?.focus(),
    inspect:() => ({path:binding?.path ?? null, dirty}),
    async requestClose() {
      if (pending) await pending.catch(() => {});
      return !dirty || host.ui.confirm('Discard unsaved changes to this project?', {confirmLabel:'Discard', signal});
    },
    dispose() {
      controller.abort();
      unregister?.();
    },
  };
}

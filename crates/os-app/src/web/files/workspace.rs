// SPDX-License-Identifier: GPL-3.0-or-later

use super::{App, SaveReceipt, error, get};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

#[wasm_bindgen(module = "/web/workspace.mjs")]
extern "C" {
    fn workspace_enabled() -> bool;
    #[wasm_bindgen(catch)]
    fn workspace_open(initial: bool) -> Result<js_sys::Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn workspace_save(receipt: Option<String>, name: &str, bytes: &[u8], zipped: bool, choose: bool)
        -> Result<js_sys::Promise, JsValue>;
    fn workspace_report(receipt: Option<String>, unsaved: bool, message: Option<String>);
}

const MAX_BYTES: usize = os_paths::fs::MAX_IMPORT_BYTES;
const MAX_ENTRIES: usize = 4096;

pub(super) struct Opened {
    name: PathBuf,
    bytes: Vec<u8>,
    receipt: String,
}

struct Binding {
    path: Option<PathBuf>,
    receipt: String,
}

#[derive(Clone, Copy)]
enum Command { Open, Save, SaveAs }

thread_local! {
    static BINDING: RefCell<Option<Binding>> = const { RefCell::new(None) };
    static COMMAND: std::cell::Cell<Option<Command>> = const { std::cell::Cell::new(None) };
}

#[wasm_bindgen]
pub fn workspace_command(action: &str) {
    let command = match action {
        "open" => Command::Open,
        "save" => Command::Save,
        "save-as" => Command::SaveAs,
        _ => return,
    };
    COMMAND.with(|pending| pending.set(Some(command)));
    crate::wake_ui();
}

pub(super) fn poll_command(app: &mut App) {
    match COMMAND.with(|pending| pending.take()) {
        Some(Command::Open) => super::pick(app, super::Import::Project),
        Some(Command::Save) => super::save(app, false),
        Some(Command::SaveAs) => super::save(app, true),
        None => {}
    }
}

pub(super) fn enabled() -> bool { workspace_enabled() }

pub(super) fn clear() {
    BINDING.with_borrow_mut(|slot| *slot = None);
}

pub(super) fn bind(path: Option<PathBuf>, receipt: String) {
    BINDING.with_borrow_mut(|slot| *slot = Some(Binding { path, receipt }));
}

fn receipt(app: &App) -> Option<String> {
    BINDING.with_borrow(|slot| {
        let binding = slot.as_ref()?;
        (binding.path == app.state.path).then(|| binding.receipt.clone())
    })
}

pub(super) fn report(app: &App, unsaved: bool) {
    workspace_report(receipt(app), unsaved, app.state.message.clone());
}

pub(super) async fn open(initial: bool) -> Result<Option<Opened>, String> {
    let value = JsFuture::from(workspace_open(initial).map_err(error)?).await.map_err(error)?;
    if value.is_null() {
        return Ok(None);
    }
    let name = get(&value, "name").as_string().ok_or("Missing file name")?;
    let name = os_paths::fs::relative_path(&name).map_err(|e| e.to_string())?;
    let bytes = js_sys::Uint8Array::new(&get(&value, "bytes"));
    if bytes.length() as usize > MAX_BYTES {
        return Err("Project is too large for the browser".into());
    }
    let receipt = get(&value, "receipt").as_string().ok_or("Missing Files revision")?;
    Ok(Some(Opened { name, bytes: bytes.to_vec(), receipt }))
}

pub(super) fn apply(app: &mut App, opened: Opened) -> Result<(), String> {
    let root = PathBuf::from(format!("/imports/{:.0}-{}", js_sys::Date::now(), super::current()));
    let (path, bytes, files) = if extension(&opened.name, "zip") {
        let Archive { project, bytes, files } = unpack(opened.bytes)?;
        (root.join(project), bytes, files)
    } else {
        (root.join(opened.name), opened.bytes, BTreeMap::new())
    };
    // Validate before touching the editor or replacing its sample library.
    let mut parsed = os_flp::FlpFile::parse(&bytes).map_err(|e| e.to_string())?;
    os_flp::project::upgrade(&mut parsed).and_then(|_| os_flp::project::load(&parsed))
        .map_err(|e| e.to_string())?;
    if !app.confirm_discard("opening another project") {
        return Ok(());
    }
    os_paths::fs::clear();
    for (name, bytes) in files {
        os_paths::fs::insert(root.join(name), bytes);
    }
    app.open_bytes(path, Ok(bytes));
    bind(app.state.path.clone(), opened.receipt);
    Ok(())
}

pub(super) fn save(app: &App, path: &Path, bytes: &[u8], choose: bool)
    -> Result<js_sys::Promise, String>
{
    let name = path.file_name().ok_or("Missing project name")?.to_string_lossy();
    let files = os_paths::fs::paths();
    let token = receipt(app);
    if files.is_empty() {
        return workspace_save(token, &name, bytes, false, choose).map_err(error);
    }
    let packed = pack(bytes, files)?;
    workspace_save(token, &name, &packed, true, choose).map_err(error)
}

pub(super) async fn saved(request: Result<js_sys::Promise, String>) -> Result<SaveReceipt, String> {
    let result = JsFuture::from(request?).await.map_err(error)?;
    if result.is_null() {
        return Ok(SaveReceipt::Cancelled);
    }
    let receipt = get(&result, "receipt").as_string().ok_or("Missing Files revision")?;
    let name = get(&result, "name").as_string().ok_or("Missing file name")?;
    Ok(SaveReceipt::Workspace { receipt, name })
}

fn extension(path: &Path, ext: &str) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

fn pack(project: &[u8], mut paths: Vec<PathBuf>) -> Result<Vec<u8>, String> {
    let root = os_paths::fs::root();
    if paths.len() >= MAX_ENTRIES || project.len() > MAX_BYTES {
        return Err("Project exceeds the browser archive limit".into());
    }
    paths.sort();
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("Project.lemon", options).map_err(|e| e.to_string())?;
    zip.write_all(project).map_err(|e| e.to_string())?;
    let mut total = project.len();
    for path in paths {
        let name = path.strip_prefix(&root).map_err(|_| "Move samples beside the project before saving")?;
        if name == Path::new("Project.lemon") {
            return Err("A sample name conflicts with the project file".into());
        }
        let bytes = os_paths::fs::read(&path).map_err(|e| e.to_string())?;
        total += bytes.len();
        if total > MAX_BYTES {
            return Err("Project and samples exceed the browser archive limit".into());
        }
        zip.start_file(name.to_string_lossy(), options).map_err(|e| e.to_string())?;
        zip.write_all(&bytes).map_err(|e| e.to_string())?;
    }
    let bytes = zip.finish().map_err(|e| e.to_string())?.into_inner();
    if bytes.len() > MAX_BYTES {
        return Err("Project and samples exceed the browser archive limit".into());
    }
    Ok(bytes)
}

struct Archive {
    project: PathBuf,
    bytes: Vec<u8>,
    files: BTreeMap<PathBuf, Vec<u8>>,
}

fn unpack(bytes: Vec<u8>) -> Result<Archive, String> {
    let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    if zip.len() > MAX_ENTRIES {
        return Err("Project has too many files".into());
    }
    let mut remaining = MAX_BYTES;
    let mut files = BTreeMap::new();
    for index in 0..zip.len() {
        let file = zip.by_index(index).map_err(|e| e.to_string())?;
        if file.is_dir() {
            continue;
        }
        let name = file.name().map_err(|e| e.to_string())?;
        let name = os_paths::fs::relative_path(&name).map_err(|e| e.to_string())?;
        if file.size() > remaining as u64 || files.contains_key(&name) {
            return Err("Project archive exceeds browser limits or contains duplicate paths".into());
        }
        let mut bytes = Vec::new();
        file.take(remaining as u64 + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        remaining = remaining.checked_sub(bytes.len()).ok_or("Project archive exceeds browser limits")?;
        files.insert(name, bytes);
    }
    let mut projects = files.keys().filter(|path| extension(path, "lemon") || extension(path, "flp"));
    let project = projects.next().cloned().ok_or("The archive contains no Lemon Studio project")?;
    if projects.next().is_some() {
        return Err("Choose an archive containing one project".into());
    }
    let bytes = files.remove(&project).expect("project came from this archive");
    Ok(Archive { project, bytes, files })
}

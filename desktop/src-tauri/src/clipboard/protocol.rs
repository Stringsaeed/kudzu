//! `kzclip://localhost/<what>/<id>[/<index>]` — clipboard media for the webviews.
//!
//! - `thumb/<id>`: the stored PNG thumbnail.
//! - `image/<id>`: the full copied image, in its original format.
//! - `file/<id>/<i>`: the i-th copied file, only when it's an image (for previews).
//! - `icon/<id>/<i>`: the Finder icon of the i-th copied file.
//!
//! Only entries in the history can be read, so a page can't use this to read arbitrary files.
//! Ids never change content, so responses are cached by the webview indefinitely.

use std::fs;
use std::sync::Arc;

use kudzu_core::clipboard::{image_mime_for_path, image_of};
use tauri::http::{Request, Response, StatusCode};
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder};
use uuid::Uuid;

use crate::state::AppCore;

const MAX_FILE_BYTES: u64 = 80 * 1024 * 1024;

fn respond(responder: UriSchemeResponder, status: StatusCode, mime: &str, body: Vec<u8>) {
    let response = Response::builder()
        .status(status)
        .header("Content-Type", mime)
        .header("Cache-Control", if status == StatusCode::OK { "private, max-age=31536000, immutable" } else { "no-store" })
        .header("Access-Control-Allow-Origin", "*")
        .body(body)
        .unwrap_or_default();
    responder.respond(response);
}

pub fn handle<R: Runtime>(ctx: UriSchemeContext<'_, R>, request: Request<Vec<u8>>, responder: UriSchemeResponder) {
    let Some(core) = ctx.app_handle().try_state::<Arc<AppCore>>().map(|s| s.inner().clone()) else {
        return respond(responder, StatusCode::SERVICE_UNAVAILABLE, "text/plain", vec![]);
    };
    let path = request.uri().path().trim_start_matches('/').to_string();
    // Disk reads and icon rendering happen off the webview's thread.
    std::thread::spawn(move || {
        let parts: Vec<&str> = path.split('/').collect();
        let (Some(kind), Some(id)) = (parts.first().copied(), parts.get(1).and_then(|s| Uuid::parse_str(s).ok())) else {
            return respond(responder, StatusCode::BAD_REQUEST, "text/plain", vec![]);
        };
        let index = parts.get(2).and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
        match serve(&core, kind, id, index) {
            Some((mime, body)) => respond(responder, StatusCode::OK, mime, body),
            None => respond(responder, StatusCode::NOT_FOUND, "text/plain", vec![]),
        }
    });
}

fn serve(core: &AppCore, kind: &str, id: Uuid, index: usize) -> Option<(&'static str, Vec<u8>)> {
    let (entry, thumb, items) = {
        let clip = core.clip.lock();
        let entry = clip.history.get(id)?.clone();
        let items = (kind == "image").then(|| clip.items(id)).flatten();
        (entry, clip.thumb_path(id), items)
    };
    match kind {
        "thumb" => fs::read(thumb).ok().map(|b| ("image/png", b)),
        "image" => {
            let items = items?;
            let (data, mime) = image_of(&items)?;
            Some((mime, data.to_vec()))
        }
        "file" => {
            let path = entry.files.get(index)?;
            let mime = image_mime_for_path(path)?;
            let meta = fs::metadata(path).ok().filter(|m| m.is_file() && m.len() <= MAX_FILE_BYTES)?;
            let _ = meta;
            fs::read(path).ok().map(|b| (mime, b))
        }
        "icon" => {
            let path = entry.files.get(index)?;
            kudzu_engine::system::app_icon_png(path, 64).map(|b| ("image/png", b))
        }
        _ => None,
    }
}

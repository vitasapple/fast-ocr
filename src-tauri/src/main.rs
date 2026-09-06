// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use tauri::Manager;
use tiny_http::{Header, Response, Server};

fn resolve_asset_dir(app: &tauri::AppHandle) -> PathBuf {
    // 1. Try production bundled resource directory
    if let Ok(resource_dir) = app.path().resource_dir() {
        let candidate = resource_dir.join("src");
        if candidate.join("index.html").exists() {
            return candidate;
        }
        if resource_dir.join("index.html").exists() {
            return resource_dir;
        }
    }

    // 2. Try development paths relative to current working directory or exe
    let dev_candidates = [
        PathBuf::from("../src"),
        PathBuf::from("src"),
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_default()
            .join("src"),
    ];

    for candidate in &dev_candidates {
        if candidate.join("index.html").exists() {
            if let Ok(canon) = candidate.canonicalize() {
                return canon;
            }
            return candidate.clone();
        }
    }

    PathBuf::from("src")
}

fn guess_mime(path_str: &str) -> &'static str {
    if path_str.ends_with("+esm") || path_str.ends_with(".mjs") || path_str.ends_with(".js") {
        "text/javascript; charset=utf-8"
    } else if path_str.ends_with(".wasm") {
        "application/wasm"
    } else if path_str.ends_with(".html") || path_str.ends_with(".htm") {
        "text/html; charset=utf-8"
    } else if path_str.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path_str.ends_with(".json") {
        "application/json; charset=utf-8"
    } else if path_str.ends_with(".tar") {
        "application/x-tar"
    } else if path_str.ends_with(".png") {
        "image/png"
    } else if path_str.ends_with(".jpg") || path_str.ends_with(".jpeg") {
        "image/jpeg"
    } else if path_str.ends_with(".svg") {
        "image/svg+xml"
    } else {
        "application/octet-stream"
    }
}

fn start_local_server(asset_dir: PathBuf) -> u16 {
    let server = Server::http("127.0.0.1:0").expect("Failed to bind local server");
    let port = server.server_addr().to_ip().map(|a| a.port()).unwrap_or(8000);

    thread::spawn(move || {
        for request in server.incoming_requests() {
            let url = request.url();
            let raw_path = url.split('?').next().unwrap_or("/");
            let decoded_path = percent_encoding::percent_decode_str(raw_path)
                .decode_utf8_lossy();
            let clean_path = decoded_path.trim_start_matches('/');

            let target_path = if clean_path.is_empty() || clean_path == "/" {
                asset_dir.join("index.html")
            } else {
                asset_dir.join(clean_path)
            };

            let (status, content_type, data) = if target_path.is_file() {
                let mime = guess_mime(&target_path.to_string_lossy());
                match fs::read(&target_path) {
                    Ok(bytes) => (200, mime, bytes),
                    Err(_) => (500, "text/plain", b"500 Internal Error".to_vec()),
                }
            } else {
                (404, "text/plain", b"404 Not Found".to_vec())
            };

            let mut response = Response::from_data(data).with_status_code(status);

            if let Ok(h) = Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes()) {
                response.add_header(h);
            }
            if let Ok(h) = Header::from_bytes(&b"Cross-Origin-Opener-Policy"[..], &b"same-origin"[..]) {
                response.add_header(h);
            }
            if let Ok(h) = Header::from_bytes(&b"Cross-Origin-Embedder-Policy"[..], &b"credentialless"[..]) {
                response.add_header(h);
            }
            if let Ok(h) = Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]) {
                response.add_header(h);
            }
            if content_type.starts_with("text/html") {
                if let Ok(h) = Header::from_bytes(&b"Cache-Control"[..], &b"no-cache"[..]) {
                    response.add_header(h);
                }
            } else {
                if let Ok(h) = Header::from_bytes(&b"Cache-Control"[..], &b"public, max-age=604800"[..]) {
                    response.add_header(h);
                }
            }

            let _ = request.respond(response);
        }
    });

    port
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let asset_dir = resolve_asset_dir(app.handle());
            let port = start_local_server(asset_dir);

            if let Some(window) = app.get_webview_window("main") {
                let target_url = format!("http://127.0.0.1:{}/index.html", port);
                if let Ok(parsed) = target_url.parse() {
                    let _ = window.navigate(parsed);
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::Manager;
use tiny_http::{Header, Method, Response, Server};

static ACTIVE_PORT: Mutex<u16> = Mutex::new(8000);
static RECEIVED_FILES: Mutex<Vec<ReceivedItem>> = Mutex::new(Vec::new());
static SHARED_FILES: Mutex<Vec<SharedItem>> = Mutex::new(Vec::new());
const MAX_STORE_ITEMS: usize = 50;

#[derive(Clone, Serialize, Deserialize)]
pub struct ReceivedItem {
    pub id: String,
    pub name: String,
    pub size: usize,
    pub category: String,
    pub target: String,
    pub mimetype: String,
    pub preview: String,
    pub timestamp: u64,
    #[serde(rename = "senderIp")]
    pub sender_ip: String,
    #[serde(skip)]
    pub data: Vec<u8>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SharedItem {
    pub id: String,
    pub name: String,
    pub size: usize,
    pub mimetype: String,
    pub timestamp: u64,
    #[serde(skip)]
    pub data: Vec<u8>,
}

fn generate_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let cnt = COUNTER.fetch_add(1, Ordering::Relaxed);
    let val = (now as u64) ^ (cnt.wrapping_mul(0x517cc1b727220a95));
    format!("{:08x}", val & 0xffffffff)
}

fn resolve_asset_dir(app: &tauri::AppHandle) -> PathBuf {
    // 1. Try production bundled resource directory
    if let Ok(resource_dir) = app.path().resource_dir() {
        let candidates = [
            resource_dir.join("src"),
            resource_dir.join("_up_").join("src"),
            resource_dir.clone(),
        ];
        for candidate in &candidates {
            if candidate.join("index.html").exists() {
                return candidate.clone();
            }
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

fn is_valid_physical_lan_ip(ip: &str) -> bool {
    if ip.is_empty() {
        return false;
    }
    if ip.starts_with("127.")
        || ip.starts_with("198.18.")
        || ip.starts_with("198.19.")
        || ip.starts_with("169.254.")
        || ip.starts_with("0.")
        || ip.starts_with("255.")
    {
        return false;
    }
    if ip.starts_with("100.") {
        let parts: Vec<&str> = ip.split('.').collect();
        if parts.len() == 4 {
            if let Ok(p2) = parts[1].parse::<u32>() {
                if (64..=127).contains(&p2) {
                    return false;
                }
            }
        }
    }
    if ip.starts_with("192.168.") || ip.starts_with("10.") {
        return true;
    }
    if ip.starts_with("172.") {
        let parts: Vec<&str> = ip.split('.').collect();
        if parts.len() == 4 {
            if let Ok(p2) = parts[1].parse::<u32>() {
                if (16..=31).contains(&p2) {
                    return true;
                }
            }
        }
    }
    false
}

fn get_lan_ips() -> Vec<String> {
    let mut candidates: Vec<String> = Vec::new();

    #[cfg(unix)]
    {
        use std::process::Command;
        if let Ok(output) = Command::new("ifconfig").output() {
            if let Ok(out_str) = String::from_utf8(output.stdout) {
                let mut current_iface = String::new();
                for line in out_str.lines() {
                    if !line.starts_with('\t') && !line.starts_with(' ') {
                        if let Some(colon_pos) = line.find(':') {
                            current_iface = line[..colon_pos].trim().to_lowercase();
                        } else {
                            current_iface.clear();
                        }
                    } else if !current_iface.is_empty() {
                        let skip = ["utun", "tun", "tap", "lo", "docker", "br-", "veth", "p2p", "awdl", "llw"];
                        if skip.iter().any(|pfx| current_iface.starts_with(pfx)) {
                            continue;
                        }
                        if let Some(inet_pos) = line.find("inet ") {
                            let rest = line[inet_pos + 5..].trim_start();
                            let ip = rest.split_whitespace().next().unwrap_or("");
                            if is_valid_physical_lan_ip(ip) && !candidates.contains(&ip.to_string()) {
                                if current_iface == "en0" || current_iface == "wlan0" || current_iface == "eth0" {
                                    candidates.insert(0, ip.to_string());
                                } else {
                                    candidates.push(ip.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[cfg(windows)]
    {
        use std::process::Command;
        if let Ok(output) = Command::new("ipconfig").output() {
            let out_str = String::from_utf8_lossy(&output.stdout);
            for line in out_str.lines() {
                let lower = line.to_lowercase();
                if lower.contains("ipv4") || lower.contains("ip address") || line.contains("IPv4 地址") {
                    if let Some(colon_pos) = line.rfind(':') {
                        let ip = line[colon_pos + 1..].trim();
                        if is_valid_physical_lan_ip(ip) && !candidates.contains(&ip.to_string()) {
                            candidates.push(ip.to_string());
                        }
                    }
                }
            }
        }
    }

    candidates.sort_by_key(|ip| {
        if ip.starts_with("192.168.") {
            0
        } else if ip.starts_with("10.") {
            1
        } else if ip.starts_with("172.") {
            2
        } else {
            3
        }
    });

    if candidates.is_empty() {
        candidates.push("127.0.0.1".to_string());
    }

    candidates
}

struct FormPart {
    name: String,
    filename: Option<String>,
    content_type: Option<String>,
    data: Vec<u8>,
}

fn extract_boundary(content_type: &str) -> Option<String> {
    for part in content_type.split(';') {
        let part = part.trim();
        if part.starts_with("boundary=") {
            let b = part["boundary=".len()..].trim().trim_matches('"');
            return Some(b.to_string());
        }
    }
    None
}

fn parse_multipart(body: &[u8], boundary: &str) -> Vec<FormPart> {
    let mut parts = Vec::new();
    let delim = format!("--{}", boundary).into_bytes();

    let mut indices = Vec::new();
    let mut i = 0;
    while i + delim.len() <= body.len() {
        if &body[i..i + delim.len()] == delim.as_slice() {
            indices.push(i);
            i += delim.len();
        } else {
            i += 1;
        }
    }

    if indices.len() < 2 {
        return parts;
    }

    for k in 0..indices.len() - 1 {
        let mut start = indices[k] + delim.len();
        if start + 2 <= body.len() && &body[start..start + 2] == b"--" {
            break;
        }
        if start + 2 <= body.len() && &body[start..start + 2] == b"\r\n" {
            start += 2;
        } else if start + 1 <= body.len() && body[start] == b'\n' {
            start += 1;
        }

        let mut end = indices[k + 1];
        if end >= start + 2 && &body[end - 2..end] == b"\r\n" {
            end -= 2;
        } else if end >= start + 1 && body[end - 1] == b'\n' {
            end -= 1;
        }

        if start >= end {
            continue;
        }

        let segment = &body[start..end];
        let (header_bytes, data_bytes) = if let Some(pos) = segment.windows(4).position(|w| w == b"\r\n\r\n") {
            (&segment[..pos], &segment[pos + 4..])
        } else if let Some(pos) = segment.windows(2).position(|w| w == b"\n\n") {
            (&segment[..pos], &segment[pos + 2..])
        } else {
            continue;
        };

        let header_str = String::from_utf8_lossy(header_bytes);
        let mut name = String::new();
        let mut filename = None;
        let mut content_type = None;

        for line in header_str.lines() {
            let line = line.trim();
            if line.to_lowercase().starts_with("content-disposition:") {
                if let Some(pos) = line.find("name=\"") {
                    let rest = &line[pos + 6..];
                    if let Some(end_quote) = rest.find('"') {
                        name = rest[..end_quote].to_string();
                    }
                }
                if let Some(pos) = line.find("filename=\"") {
                    let rest = &line[pos + 10..];
                    if let Some(end_quote) = rest.find('"') {
                        filename = Some(rest[..end_quote].to_string());
                    }
                }
            } else if line.to_lowercase().starts_with("content-type:") {
                let rest = line["content-type:".len()..].trim();
                content_type = Some(rest.to_string());
            }
        }

        if !name.is_empty() || filename.is_some() {
            parts.push(FormPart {
                name,
                filename,
                content_type,
                data: data_bytes.to_vec(),
            });
        }
    }

    parts
}

fn add_common_headers(response: &mut Response<std::io::Cursor<Vec<u8>>>, content_type: &str) {
    if let Ok(h) = Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes()) {
        response.add_header(h);
    }
    if let Ok(h) = Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]) {
        response.add_header(h);
    }
    if let Ok(h) = Header::from_bytes(&b"Access-Control-Allow-Methods"[..], &b"GET, POST, OPTIONS, DELETE"[..]) {
        response.add_header(h);
    }
    if let Ok(h) = Header::from_bytes(&b"Access-Control-Allow-Headers"[..], &b"Content-Type, Authorization, X-Requested-With"[..]) {
        response.add_header(h);
    }
    if let Ok(h) = Header::from_bytes(&b"Cross-Origin-Opener-Policy"[..], &b"same-origin"[..]) {
        response.add_header(h);
    }
    if let Ok(h) = Header::from_bytes(&b"Cross-Origin-Embedder-Policy"[..], &b"credentialless"[..]) {
        response.add_header(h);
    }
}

fn respond_json(request: tiny_http::Request, status: u16, json_val: &serde_json::Value) {
    let data = serde_json::to_vec(json_val).unwrap_or_default();
    let mut response = Response::from_data(data).with_status_code(status);
    add_common_headers(&mut response, "application/json; charset=utf-8");
    if let Ok(h) = Header::from_bytes(&b"Cache-Control"[..], &b"no-cache"[..]) {
        response.add_header(h);
    }
    let _ = request.respond(response);
}

fn get_header_value(request: &tiny_http::Request, name: &str) -> Option<String> {
    for header in request.headers() {
        if header.field.to_string().eq_ignore_ascii_case(name) {
            return Some(header.value.to_string());
        }
    }
    None
}

fn start_local_server(asset_dir: PathBuf) -> u16 {
    let server = Server::http("0.0.0.0:8000")
        .or_else(|_| Server::http("0.0.0.0:0"))
        .expect("Failed to bind local server");
    let port = server.server_addr().to_ip().map(|a| a.port()).unwrap_or(8000);
    if let Ok(mut lock) = ACTIVE_PORT.lock() {
        *lock = port;
    }

    thread::spawn(move || {
        for mut request in server.incoming_requests() {
            // Handle CORS preflight
            if request.method() == &Method::Options {
                let mut response = Response::from_data(Vec::new()).with_status_code(204);
                add_common_headers(&mut response, "text/plain");
                let _ = request.respond(response);
                continue;
            }

            let url = request.url().to_string();
            let raw_path = url.split('?').next().unwrap_or("/");
            let decoded_path = percent_encoding::percent_decode_str(raw_path).decode_utf8_lossy();
            let path = decoded_path.as_ref();

            // 1. GET /api/connect/info
            if request.method() == &Method::Get && path == "/api/connect/info" {
                let ips = get_lan_ips();
                let primary = ips.first().cloned().unwrap_or_else(|| "127.0.0.1".into());
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let val = serde_json::json!({
                    "status": "ok",
                    "port": port,
                    "primaryIp": primary,
                    "allIps": ips,
                    "serverTime": now,
                    "mobileUrl": format!("http://{}:{}/mobile.html", primary, port)
                });
                respond_json(request, 200, &val);
                continue;
            }

            // 2. GET /api/connect/events
            if request.method() == &Method::Get && path == "/api/connect/events" {
                let summary: Vec<serde_json::Value> = {
                    let lock = RECEIVED_FILES.lock().unwrap();
                    lock.iter()
                        .map(|item| {
                            serde_json::json!({
                                "id": item.id,
                                "name": item.name,
                                "size": item.size,
                                "category": item.category,
                                "target": item.target,
                                "mimetype": item.mimetype,
                                "preview": item.preview,
                                "timestamp": item.timestamp,
                                "senderIp": item.sender_ip,
                            })
                        })
                        .collect()
                };
                respond_json(request, 200, &serde_json::json!({ "events": summary }));
                continue;
            }

            // 3. GET /api/connect/file/<id>
            if request.method() == &Method::Get && path.starts_with("/api/connect/file/") {
                let file_id = &path["/api/connect/file/".len()..];
                let found = {
                    let lock = RECEIVED_FILES.lock().unwrap();
                    lock.iter()
                        .find(|item| item.id == file_id)
                        .map(|item| (item.data.clone(), item.mimetype.clone(), item.name.clone()))
                };

                if let Some((data, mime, name)) = found {
                    let mut response = Response::from_data(data).with_status_code(200);
                    add_common_headers(&mut response, &mime);
                    let safe_name = name.replace('"', "_").replace('\r', "").replace('\n', "");
                    let disp = format!("inline; filename=\"{}\"", safe_name);
                    if let Ok(h) = Header::from_bytes(&b"Content-Disposition"[..], disp.as_bytes()) {
                        response.add_header(h);
                    }
                    let _ = request.respond(response);
                } else {
                    let mut response = Response::from_data(b"File not found".to_vec()).with_status_code(404);
                    add_common_headers(&mut response, "text/plain");
                    let _ = request.respond(response);
                }
                continue;
            }

            // 4. GET /api/connect/shared
            if request.method() == &Method::Get && path == "/api/connect/shared" {
                let summary: Vec<serde_json::Value> = {
                    let lock = SHARED_FILES.lock().unwrap();
                    lock.iter()
                        .map(|item| {
                            serde_json::json!({
                                "id": item.id,
                                "name": item.name,
                                "size": item.size,
                                "mimetype": item.mimetype,
                                "timestamp": item.timestamp,
                            })
                        })
                        .collect()
                };
                respond_json(request, 200, &serde_json::json!({ "files": summary }));
                continue;
            }

            // 5. GET /api/connect/shared/<id>
            if request.method() == &Method::Get && path.starts_with("/api/connect/shared/") {
                let file_id = &path["/api/connect/shared/".len()..];
                let found = {
                    let lock = SHARED_FILES.lock().unwrap();
                    lock.iter()
                        .find(|item| item.id == file_id)
                        .map(|item| (item.data.clone(), item.mimetype.clone(), item.name.clone()))
                };

                if let Some((data, mime, name)) = found {
                    let mut response = Response::from_data(data).with_status_code(200);
                    add_common_headers(&mut response, &mime);
                    let safe_name = name.replace('"', "_").replace('\r', "").replace('\n', "");
                    let disp = format!("attachment; filename=\"{}\"", safe_name);
                    if let Ok(h) = Header::from_bytes(&b"Content-Disposition"[..], disp.as_bytes()) {
                        response.add_header(h);
                    }
                    let _ = request.respond(response);
                } else {
                    let mut response = Response::from_data(b"File not found".to_vec()).with_status_code(404);
                    add_common_headers(&mut response, "text/plain");
                    let _ = request.respond(response);
                }
                continue;
            }

            // 6. GET /mobile -> redirect to /mobile.html
            if request.method() == &Method::Get && (path == "/mobile" || path == "/mobile/") {
                let mut response = Response::from_data(Vec::new()).with_status_code(302);
                if let Ok(h) = Header::from_bytes(&b"Location"[..], &b"/mobile.html"[..]) {
                    response.add_header(h);
                }
                let _ = request.respond(response);
                continue;
            }

            // 7. POST /api/connect/upload
            if request.method() == &Method::Post && path == "/api/connect/upload" {
                let sender_ip = request
                    .remote_addr()
                    .map(|a| a.ip().to_string())
                    .unwrap_or_default();
                let content_type = get_header_value(&request, "Content-Type").unwrap_or_default();
                let mut body = Vec::new();
                let _ = request.as_reader().read_to_end(&mut body);

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);

                if content_type.contains("multipart/form-data") {
                    if let Some(boundary) = extract_boundary(&content_type) {
                        let parts = parse_multipart(&body, &boundary);
                        let mut target = "image".to_string();
                        for p in &parts {
                            if p.name == "target" {
                                if let Ok(s) = std::str::from_utf8(&p.data) {
                                    target = s.trim().to_string();
                                }
                            }
                        }

                        let mut created_ids = Vec::new();

                        // Check text field
                        for p in &parts {
                            if p.name == "text" && !p.data.is_empty() {
                                let raw_text = String::from_utf8_lossy(&p.data).to_string();
                                let item_id = generate_id();
                                let preview = if raw_text.chars().count() > 200 {
                                    raw_text.chars().take(200).collect::<String>()
                                } else {
                                    raw_text.clone()
                                };
                                let item = ReceivedItem {
                                    id: item_id.clone(),
                                    name: "手机便签.txt".to_string(),
                                    size: p.data.len(),
                                    category: "text".to_string(),
                                    target: target.clone(),
                                    mimetype: "text/plain; charset=utf-8".to_string(),
                                    preview,
                                    timestamp: now,
                                    sender_ip: sender_ip.clone(),
                                    data: p.data.clone(),
                                };
                                let mut lock = RECEIVED_FILES.lock().unwrap();
                                lock.insert(0, item);
                                created_ids.push(item_id);
                            }
                        }

                        // Check file fields
                        for p in &parts {
                            if (p.name == "files" || p.filename.is_some()) && !p.data.is_empty() {
                                let fname = p.filename.clone().unwrap_or_else(|| "unnamed_file".to_string());
                                let mime = p.content_type.clone().unwrap_or_else(|| "application/octet-stream".to_string());
                                let lower_fname = fname.to_lowercase();

                                let category = if mime.starts_with("image/")
                                    || lower_fname.ends_with(".png")
                                    || lower_fname.ends_with(".jpg")
                                    || lower_fname.ends_with(".jpeg")
                                    || lower_fname.ends_with(".webp")
                                    || lower_fname.ends_with(".bmp")
                                    || lower_fname.ends_with(".gif")
                                {
                                    "image"
                                } else if mime == "application/pdf" || lower_fname.ends_with(".pdf") {
                                    "pdf"
                                } else if lower_fname.ends_with(".txt")
                                    || lower_fname.ends_with(".md")
                                    || lower_fname.ends_with(".json")
                                    || lower_fname.ends_with(".log")
                                    || lower_fname.ends_with(".csv")
                                {
                                    "text"
                                } else {
                                    "file"
                                };

                                let item_id = generate_id();
                                let preview = if category == "text" {
                                    let text_str = String::from_utf8_lossy(&p.data);
                                    text_str.chars().take(200).collect::<String>()
                                } else {
                                    String::new()
                                };

                                let item = ReceivedItem {
                                    id: item_id.clone(),
                                    name: fname,
                                    size: p.data.len(),
                                    category: category.to_string(),
                                    target: target.clone(),
                                    mimetype: mime,
                                    preview,
                                    timestamp: now,
                                    sender_ip: sender_ip.clone(),
                                    data: p.data.clone(),
                                };

                                let mut lock = RECEIVED_FILES.lock().unwrap();
                                lock.insert(0, item);
                                created_ids.push(item_id);
                            }
                        }

                        let mut lock = RECEIVED_FILES.lock().unwrap();
                        while lock.len() > MAX_STORE_ITEMS {
                            lock.pop();
                        }

                        respond_json(
                            request,
                            200,
                            &serde_json::json!({
                                "success": true,
                                "ids": created_ids,
                                "count": created_ids.len()
                            }),
                        );
                        continue;
                    }
                } else if content_type.contains("application/json") {
                    if let Ok(payload) = serde_json::from_slice::<serde_json::Value>(&body) {
                        let text = payload["text"].as_str().unwrap_or("").to_string();
                        let title = payload["title"].as_str().unwrap_or("手机便签");
                        let target = payload["target"].as_str().unwrap_or("text");

                        if !text.is_empty() {
                            let item_id = generate_id();
                            let text_bytes = text.into_bytes();
                            let preview = if text_bytes.len() > 200 {
                                String::from_utf8_lossy(&text_bytes[..200]).to_string()
                            } else {
                                String::from_utf8_lossy(&text_bytes).to_string()
                            };

                            let item = ReceivedItem {
                                id: item_id.clone(),
                                name: format!("{}.txt", title),
                                size: text_bytes.len(),
                                category: "text".to_string(),
                                target: target.to_string(),
                                mimetype: "text/plain; charset=utf-8".to_string(),
                                preview,
                                timestamp: now,
                                sender_ip,
                                data: text_bytes,
                            };

                            let mut lock = RECEIVED_FILES.lock().unwrap();
                            lock.insert(0, item);
                            while lock.len() > MAX_STORE_ITEMS {
                                lock.pop();
                            }

                            respond_json(request, 200, &serde_json::json!({ "success": true, "id": item_id }));
                            continue;
                        }
                    }
                }

                respond_json(request, 400, &serde_json::json!({ "error": "Invalid upload format" }));
                continue;
            }

            // 8. POST /api/connect/share
            if request.method() == &Method::Post && path == "/api/connect/share" {
                let content_type = get_header_value(&request, "Content-Type").unwrap_or_default();
                let mut body = Vec::new();
                let _ = request.as_reader().read_to_end(&mut body);

                let mut shared_success = false;
                if let Some(boundary) = extract_boundary(&content_type) {
                    let parts = parse_multipart(&body, &boundary);
                    for p in parts {
                        if (p.name == "file" || p.filename.is_some()) && !p.data.is_empty() {
                            let fname = p.filename.unwrap_or_else(|| "shared_file".to_string());
                            let mime = p.content_type.unwrap_or_else(|| "application/octet-stream".to_string());
                            let item_id = generate_id();
                            let now = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .map(|d| d.as_secs())
                                .unwrap_or(0);

                            let item = SharedItem {
                                id: item_id.clone(),
                                name: fname.clone(),
                                size: p.data.len(),
                                mimetype: mime,
                                timestamp: now,
                                data: p.data,
                            };

                            let mut lock = SHARED_FILES.lock().unwrap();
                            lock.insert(0, item);
                            while lock.len() > MAX_STORE_ITEMS {
                                lock.pop();
                            }

                            respond_json(
                                request,
                                200,
                                &serde_json::json!({ "success": true, "id": item_id, "name": fname }),
                            );
                            shared_success = true;
                            break;
                        }
                    }
                }

                if !shared_success {
                    respond_json(request, 400, &serde_json::json!({ "error": "Invalid share upload" }));
                }
                continue;
            }

            // 9. POST /api/connect/clear
            if request.method() == &Method::Post && path == "/api/connect/clear" {
                RECEIVED_FILES.lock().unwrap().clear();
                respond_json(request, 200, &serde_json::json!({ "success": true }));
                continue;
            }

            // 10. Fallback: Static File Serving from asset_dir
            let clean_path = path.trim_start_matches('/');
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
            add_common_headers(&mut response, content_type);

            if content_type.starts_with("text/html") {
                if let Ok(h) = Header::from_bytes(&b"Cache-Control"[..], &b"no-cache"[..]) {
                    response.add_header(h);
                }
            } else if status == 200 {
                if let Ok(h) = Header::from_bytes(&b"Cache-Control"[..], &b"public, max-age=604800"[..]) {
                    response.add_header(h);
                }
            }

            let _ = request.respond(response);
        }
    });

    port
}

#[tauri::command]
fn get_lan_info() -> Result<serde_json::Value, String> {
    let ips = get_lan_ips();
    let primary = ips.first().cloned().unwrap_or_else(|| "127.0.0.1".into());
    let port = *ACTIVE_PORT.lock().unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    Ok(serde_json::json!({
        "status": "ok",
        "port": port,
        "primaryIp": primary,
        "allIps": ips,
        "serverTime": now,
        "mobileUrl": format!("http://{}:{}/mobile.html", primary, port)
    }))
}

#[tauri::command]
fn capture_screen() -> Result<Vec<u8>, String> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let temp_path = std::env::temp_dir().join("dali_snip.png");
        let _ = std::fs::remove_file(&temp_path);

        let status = Command::new("/usr/sbin/screencapture")
            .arg("-i")
            .arg(&temp_path)
            .status()
            .map_err(|e| format!("screencapture failed: {}", e))?;

        if status.success() && temp_path.exists() {
            let bytes = std::fs::read(&temp_path)
                .map_err(|e| format!("Failed to read screenshot: {}", e))?;
            let _ = std::fs::remove_file(&temp_path);
            Ok(bytes)
        } else {
            Err("Capture cancelled".to_string())
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Native screen capture is only supported on macOS".to_string())
    }
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![capture_screen, get_lan_info])
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

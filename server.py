#!/usr/bin/env python3
"""大梨OCR 本地与局域网协同 Web 服务器。

支持：
1. 静态资源托管（带 COOP/COEP 与 MIME 支持）
2. 局域网广播与手机扫码直传 (Dali Connect)
3. 移动端与桌面端双向文件/文本传输 API

用法：python3 server.py [端口]
默认地址：http://0.0.0.0:8000
"""
import cgi
import http.server
import json
import os
import re
import socket
import socketserver
import subprocess
import sys
import time
import uuid

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 8000
BASE_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "src")

# 局域网互联内存消息/文件队列
RECEIVED_FILES = []  # 手机端上传到电脑端的文件/文本
SHARED_FILES = []    # 电脑端共享给手机端下载的文件
MAX_STORE_ITEMS = 50


def is_valid_physical_lan_ip(ip: str) -> bool:
    """严格校验是否为真实的物理局域网 IPv4 地址。"""
    if not ip or not isinstance(ip, str):
        return False
    # 严格排除本地回环
    if ip.startswith("127."):
        return False
    # 严格排除 Clash / Surge / Shadowrocket Fake-IP (RFC 2544 198.18.0.0/15)
    if ip.startswith("198.18.") or ip.startswith("198.19."):
        return False
    # 排除 Link-Local (未获取到路由 DHCP 分配的自动私有 IP 169.254.0.0/16)
    if ip.startswith("169.254."):
        return False
    # 排除广播与非法
    if ip.startswith("0.") or ip.startswith("255."):
        return False
    # 排除 Tailscale / CGNAT (100.64.0.0/10)
    if ip.startswith("100."):
        parts = ip.split(".")
        if len(parts) == 4 and parts[1].isdigit():
            p2 = int(parts[1])
            if 64 <= p2 <= 127:
                return False

    # 检查是否在标准局域网私有地址段内 (192.168.*, 10.*, 172.16~31.*)
    if ip.startswith("192.168.") or ip.startswith("10."):
        return True
    if ip.startswith("172."):
        parts = ip.split(".")
        if len(parts) == 4 and parts[1].isdigit():
            p2 = int(parts[1])
            if 16 <= p2 <= 31:
                return True
    return False


def get_lan_ips():
    """获取本机所有有效的物理局域网 IPv4 地址（彻底排除 VPN/Clash TUN 虚拟网卡与回环）。"""
    candidates = []

    # 1. 针对 macOS / Linux：通过 ifconfig 优先精准提取物理网卡 (en*, eth*, wlan*)
    try:
        out = subprocess.check_output(["ifconfig"], text=True, stderr=subprocess.DEVNULL)
        current_iface = ""
        for line in out.splitlines():
            if not line.startswith("\t") and not line.startswith(" "):
                m = re.match(r"^([a-zA-Z0-9_-]+):", line)
                current_iface = m.group(1) if m else ""
            elif current_iface:
                lower_name = current_iface.lower()
                # 排除所有虚拟/隧道网卡与回环
                if any(lower_name.startswith(pfx) for pfx in ("utun", "tun", "tap", "lo", "docker", "br-", "veth", "p2p", "awdl", "llw")):
                    continue
                ip_match = re.search(r"inet\s+(\d+\.\d+\.\d+\.\d+)", line)
                if ip_match:
                    ip = ip_match.group(1)
                    if is_valid_physical_lan_ip(ip) and ip not in candidates:
                        # 物理无线网卡 en0 / wlan0 优先级最高，置于最前
                        if lower_name in ("en0", "wlan0", "eth0"):
                            candidates.insert(0, ip)
                        else:
                            candidates.append(ip)
    except Exception:
        pass

    # 2. 针对 Windows：通过 ipconfig 扫描非虚拟适配器
    if sys.platform == "win32":
        try:
            out = subprocess.check_output(["ipconfig", "/all"], text=True, stderr=subprocess.DEVNULL)
            sections = out.split("\n\n")
            for sec in sections:
                sec_lower = sec.lower()
                if any(v in sec_lower for v in ("virtual", "vmware", "vethernet", "tap", "tun", "wsl", "loopback")):
                    continue
                for m in re.finditer(r"(?:IPv4\s*Address|IPv4\s*地址|IP\s*Address)[\.\s]*:\s*([0-9\.]+)", sec):
                    ip = m.group(1).strip()
                    if is_valid_physical_lan_ip(ip) and ip not in candidates:
                        candidates.append(ip)
        except Exception:
            pass

    # 3. 通用 socket 主机名反查兜底
    try:
        for ip in socket.gethostbyname_ex(socket.gethostname())[2]:
            if is_valid_physical_lan_ip(ip) and ip not in candidates:
                candidates.append(ip)
    except Exception:
        pass

    # 4. 按优先级排序（192.168.x.x > 10.x.x.x > 172.x.x.x）
    def ip_score(ip):
        if ip.startswith("192.168."):
            return 100
        if ip.startswith("10."):
            return 80
        if ip.startswith("172."):
            return 60
        return 10

    candidates.sort(key=ip_score, reverse=True)
    return candidates or ["127.0.0.1"]


class ConnectHandler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".mjs": "text/javascript",
        ".wasm": "application/wasm",
    }

    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=BASE_DIR, **kwargs)

    def guess_type(self, path):
        if path.endswith("+esm"):
            return "text/javascript"
        return super().guess_type(path)

    def end_headers(self):
        # 允许跨域与多线程 WASM
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS, DELETE")
        self.send_header("Access-Control-Allow-Headers", "Content-Type, Authorization, X-Requested-With")
        self.send_header("Cross-Origin-Opener-Policy", "same-origin")
        self.send_header("Cross-Origin-Embedder-Policy", "credentialless")
        super().end_headers()

    def do_OPTIONS(self):
        self.send_response(204)
        self.end_headers()

    def do_GET(self):
        url_path = self.path.split("?")[0]

        # 1. 局域网状态探测
        if url_path == "/api/connect/info":
            ips = get_lan_ips()
            data = {
                "status": "ok",
                "port": PORT,
                "primaryIp": ips[0],
                "allIps": ips,
                "serverTime": int(time.time()),
                "mobileUrl": f"http://{ips[0]}:{PORT}/mobile.html"
            }
            self._send_json(200, data)
            return

        # 2. 桌面端拉取手机来件列表 (Inbox)
        if url_path == "/api/connect/events":
            summary = []
            for item in RECEIVED_FILES:
                summary.append({
                    "id": item["id"],
                    "name": item["name"],
                    "size": item["size"],
                    "category": item["category"],  # image, pdf, text, file
                    "target": item.get("target", "image"),
                    "mimetype": item.get("mimetype", "application/octet-stream"),
                    "preview": item.get("preview", ""),
                    "timestamp": item["timestamp"],
                    "senderIp": item.get("senderIp", "")
                })
            self._send_json(200, {"events": summary})
            return

        # 3. 手机/桌面拉取具体接收文件二进制内容
        if url_path.startswith("/api/connect/file/"):
            file_id = url_path[len("/api/connect/file/"):]
            target = next((f for f in RECEIVED_FILES if f["id"] == file_id), None)
            if not target:
                self.send_error(404, "File not found")
                return
            self.send_response(200)
            self.send_header("Content-Type", target.get("mimetype", "application/octet-stream"))
            self.send_header("Content-Length", str(len(target["data"])))
            encoded_name = target["name"].encode("utf-8", "ignore").decode("latin-1", "ignore")
            self.send_header("Content-Disposition", f'inline; filename="{encoded_name}"')
            self.end_headers()
            self.wfile.write(target["data"])
            return

        # 4. 手机端拉取电脑分享文件列表 (Outbox)
        if url_path == "/api/connect/shared":
            summary = []
            for item in SHARED_FILES:
                summary.append({
                    "id": item["id"],
                    "name": item["name"],
                    "size": item["size"],
                    "mimetype": item.get("mimetype", "application/octet-stream"),
                    "timestamp": item["timestamp"]
                })
            self._send_json(200, {"files": summary})
            return

        # 5. 手机端下载电脑分享的文件
        if url_path.startswith("/api/connect/shared/"):
            file_id = url_path[len("/api/connect/shared/"):]
            target = next((f for f in SHARED_FILES if f["id"] == file_id), None)
            if not target:
                self.send_error(404, "File not found")
                return
            self.send_response(200)
            self.send_header("Content-Type", target.get("mimetype", "application/octet-stream"))
            self.send_header("Content-Length", str(len(target["data"])))
            encoded_name = target["name"].encode("utf-8", "ignore").decode("latin-1", "ignore")
            self.send_header("Content-Disposition", f'attachment; filename="{encoded_name}"')
            self.end_headers()
            self.wfile.write(target["data"])
            return

        # 6. 便捷访问 /mobile -> /mobile.html
        if url_path == "/mobile" or url_path == "/mobile/":
            self.send_response(302)
            self.send_header("Location", "/mobile.html")
            self.end_headers()
            return

        # 默认静态文件处理
        super().do_GET()

    def do_POST(self):
        url_path = self.path.split("?")[0]
        content_type = self.headers.get("Content-Type", "")

        # A. 手机端上传文件 / 文本到电脑
        if url_path == "/api/connect/upload":
            sender_ip = self.client_address[0]
            if "multipart/form-data" in content_type:
                form = cgi.FieldStorage(
                    fp=self.rfile,
                    headers=self.headers,
                    environ={'REQUEST_METHOD': 'POST', 'CONTENT_TYPE': content_type}
                )
                
                target = form.getvalue("target", "image")
                created_items = []

                # 检查是否为文本提交
                if "text" in form and form["text"].value:
                    raw_text = form["text"].value
                    title = form.getvalue("title", "手机便签/剪贴板")
                    text_bytes = raw_text.encode("utf-8")
                    item_id = str(uuid.uuid4())[:8]
                    item = {
                        "id": item_id,
                        "name": f"{title}.txt",
                        "size": len(text_bytes),
                        "category": "text",
                        "target": target,
                        "mimetype": "text/plain; charset=utf-8",
                        "data": text_bytes,
                        "preview": raw_text[:200],
                        "timestamp": int(time.time()),
                        "senderIp": sender_ip
                    }
                    RECEIVED_FILES.insert(0, item)
                    created_items.append(item_id)

                # 检查是否为文件上传
                if "files" in form:
                    file_fields = form["files"]
                    if not isinstance(file_fields, list):
                        file_fields = [file_fields]
                    
                    for field in file_fields:
                        if not field.filename:
                            continue
                        file_bytes = field.file.read()
                        fname = os.path.basename(field.filename)
                        mime = field.type or "application/octet-stream"
                        
                        category = "file"
                        if mime.startswith("image/") or fname.lower().endswith((".png", ".jpg", ".jpeg", ".webp", ".bmp", ".gif")):
                            category = "image"
                        elif mime == "application/pdf" or fname.lower().endswith(".pdf"):
                            category = "pdf"
                        elif fname.lower().endswith((".txt", ".md", ".json", ".log", ".csv")):
                            category = "text"

                        item_id = str(uuid.uuid4())[:8]
                        item = {
                            "id": item_id,
                            "name": fname,
                            "size": len(file_bytes),
                            "category": category,
                            "target": target,
                            "mimetype": mime,
                            "data": file_bytes,
                            "preview": "",
                            "timestamp": int(time.time()),
                            "senderIp": sender_ip
                        }
                        RECEIVED_FILES.insert(0, item)
                        created_items.append(item_id)

                # 控制队列最大容量
                while len(RECEIVED_FILES) > MAX_STORE_ITEMS:
                    RECEIVED_FILES.pop()

                self._send_json(200, {"success": True, "ids": created_items, "count": len(created_items)})
                return
            elif "application/json" in content_type:
                length = int(self.headers.get("Content-Length", 0))
                body = self.rfile.read(length)
                try:
                    payload = json.loads(body.decode("utf-8"))
                    text = payload.get("text", "")
                    title = payload.get("title", "手机便签")
                    target = payload.get("target", "text")
                    if text:
                        text_bytes = text.encode("utf-8")
                        item_id = str(uuid.uuid4())[:8]
                        item = {
                            "id": item_id,
                            "name": f"{title}.txt",
                            "size": len(text_bytes),
                            "category": "text",
                            "target": target,
                            "mimetype": "text/plain; charset=utf-8",
                            "data": text_bytes,
                            "preview": text[:200],
                            "timestamp": int(time.time()),
                            "senderIp": sender_ip
                        }
                        RECEIVED_FILES.insert(0, item)
                        self._send_json(200, {"success": True, "id": item_id})
                        return
                except Exception as e:
                    self._send_json(400, {"error": str(e)})
                    return

            self._send_json(400, {"error": "Unsupported Content-Type"})
            return

        # B. 电脑端分享文件给手机 (Outbox)
        if url_path == "/api/connect/share":
            if "multipart/form-data" in content_type:
                form = cgi.FieldStorage(
                    fp=self.rfile,
                    headers=self.headers,
                    environ={'REQUEST_METHOD': 'POST', 'CONTENT_TYPE': content_type}
                )
                if "file" in form and form["file"].filename:
                    field = form["file"]
                    file_bytes = field.file.read()
                    fname = os.path.basename(field.filename)
                    mime = field.type or "application/octet-stream"
                    item_id = str(uuid.uuid4())[:8]
                    item = {
                        "id": item_id,
                        "name": fname,
                        "size": len(file_bytes),
                        "mimetype": mime,
                        "data": file_bytes,
                        "timestamp": int(time.time())
                    }
                    SHARED_FILES.insert(0, item)
                    while len(SHARED_FILES) > MAX_STORE_ITEMS:
                        SHARED_FILES.pop()
                    self._send_json(200, {"success": True, "id": item_id, "name": fname})
                    return
            self._send_json(400, {"error": "Invalid share upload"})
            return

        # C. 清理队列
        if url_path == "/api/connect/clear":
            RECEIVED_FILES.clear()
            self._send_json(200, {"success": True})
            return

        self.send_error(404, "Endpoint not found")

    def _send_json(self, status, payload):
        body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-cache, no-store")
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, fmt, *args):
        # 简化日志输出，屏蔽高频轮询干扰
        if "/api/connect/events" in fmt or "/api/connect/info" in fmt:
            return
        print("[%s] %s" % (self.log_date_time_string(), fmt % args))


class ThreadingServer(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


if __name__ == "__main__":
    ips = get_lan_ips()
    primary_ip = ips[0]
    with ThreadingServer(("0.0.0.0", PORT), ConnectHandler) as httpd:
        print("\n=======================================================")
        print("🚀 大梨OCR · 局域网协同服务器 (Dali Connect Engine) 已就绪！")
        print("-------------------------------------------------------")
        print(f"💻 本机控制台地址 : http://127.0.0.1:{PORT}")
        print(f"📱 局域网手机扫码 : http://{primary_ip}:{PORT}/mobile.html")
        if len(ips) > 1:
            print("🌐 其他可用局域网IP:")
            for alt_ip in ips[1:]:
                print(f"   http://{alt_ip}:{PORT}/mobile.html")
        print("=======================================================\n")
        print("按 Ctrl+C 停止服务。")
        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            print("\n服务器已停止。")
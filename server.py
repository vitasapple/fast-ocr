#!/usr/bin/env python3
"""静态 Web 服务器，用于本地运行 大梨OCR 网页工具。

用法：python3 server.py [端口]
默认地址：http://127.0.0.1:8000
"""
import http.server
import os
import socketserver
import sys

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 8000
BASE_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "src")


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".mjs": "text/javascript",
        ".wasm": "application/wasm",
    }

    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=BASE_DIR, **kwargs)

    def guess_type(self, path):
        # jsdelivr 的 +esm 文件没有扩展名，必须按 JS 返回，否则浏览器拒绝执行
        if path.endswith("+esm"):
            return "text/javascript"
        return super().guess_type(path)

    def end_headers(self):
        # COOP/COEP 让页面进入 crossOriginIsolated，从而允许 WASM 多线程推理
        self.send_header("Cross-Origin-Opener-Policy", "same-origin")
        self.send_header("Cross-Origin-Embedder-Policy", "credentialless")
        super().end_headers()

    def send_head(self):
        # 自定义发送逻辑：始终返回 200，永不返回 304，
        # 并带上 no-store 头，避免浏览器缓存旧响应（尤其是错误的 MIME 类型）
        path = self.translate_path(self.path)
        if os.path.isdir(path):
            return super().send_head()

        try:
            f = open(path, "rb")
        except OSError:
            self.send_error(404, "File not found")
            return None

        try:
            fs = os.fstat(f.fileno())
            self.send_response(200)
            self.send_header("Content-type", self.guess_type(path))
            self.send_header("Content-Length", str(fs[6]))
            self.send_header("Last-Modified", self.date_time_string(fs.st_mtime))
            ctype = self.guess_type(path)
            if ctype == "text/html":
                self.send_header("Cache-Control", "no-cache")
            else:
                # 资源文件（wasm/模型/第三方 JS）按路径版本化，可放心缓存
                self.send_header("Cache-Control", "public, max-age=604800")
            self.end_headers()
            return f
        except Exception:
            f.close()
            raise

    def log_message(self, fmt, *args):
        print("[%s] %s" % (self.log_date_time_string(), fmt % args))


class ThreadingServer(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


if __name__ == "__main__":
    with ThreadingServer(("127.0.0.1", PORT), Handler) as httpd:
        print("大梨OCR 网页工具已启动：http://127.0.0.1:%d" % PORT)
        print("按 Ctrl+C 停止。")
        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            print("\n已停止。")
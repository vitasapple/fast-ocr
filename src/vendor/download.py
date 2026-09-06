#!/usr/bin/env python3
"""下载 PaddleOCR.js 所需的前端资源到本地 vendor 目录，避免依赖外网 CDN。

用法：python3 vendor/download.py
"""
import json
import os
import urllib.request

BASE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")

NPM = "https://cdn.jsdelivr.net/npm/"
ESM = "+esm"
ORT_VERSION = "1.26.0"

JSDELIVR_API = "https://data.jsdelivr.com/v1/packages/npm/onnxruntime-web@{ver}"


def fetch(url, dest):
    dest = os.path.join(BASE, dest)
    os.makedirs(os.path.dirname(dest), exist_ok=True)
    if os.path.exists(dest) and os.path.getsize(dest) > 0:
        print("skip (exists):", dest)
        return
    print("download:", url)
    with urllib.request.urlopen(url, timeout=180) as r:
        data = r.read()
    with open(dest, "wb") as f:
        f.write(data)
    print("  ->", dest, len(data), "bytes")


def list_jsdelivr_files(pkg, ver):
    url = JSDELIVR_API.format(ver=ver)
    with urllib.request.urlopen(url, timeout=60) as r:
        spec = json.load(r)
    out = []
    def walk(files, prefix):
        for f in files:
            if f["type"] == "directory":
                walk(f.get("files", []), prefix + f["name"] + "/")
            else:
                out.append(prefix + f["name"])
    walk(spec.get("files", []), "")
    return out


def main():
    # PaddleOCR SDK + 依赖（+esm 以绝对路径 /npm/... 相互引用，须原样保存）
    fetch(NPM + "@paddleocr/paddleocr-js@0.4.2/" + ESM, "npm2/@paddleocr/paddleocr-js@0.4.2/+esm")
    fetch(NPM + "js-yaml@4.2.0/" + ESM, "npm2/js-yaml@4.2.0/+esm")
    fetch(NPM + "clipper-lib@6.4.2/" + ESM, "npm2/clipper-lib@6.4.2/+esm")
    fetch(
        NPM + "@techstark/opencv-js@4.10.0-release.1/" + ESM,
        "npm2/@techstark/opencv-js@4.10.0-release.1/+esm",
    )
    fetch(NPM + "onnxruntime-web@1.26.0/" + ESM, "npm2/onnxruntime-web@1.26.0/+esm")

    # onnxruntime 的 dist（仅运行所需的 dist 文件，跳过全部源文件与 .map 源码映射）
    dist_files = [
        f
        for f in list_jsdelivr_files("onnxruntime-web", ORT_VERSION)
        if f.startswith("dist/") and not f.endswith(".map")
    ]
    print("onnxruntime-web dist 文件 %d 个" % len(dist_files))
    for f in dist_files:
        fetch(
            NPM + "onnxruntime-web@{v}/{f}".format(v=ORT_VERSION, f=f),
            "npm2/onnxruntime-web@{v}/{f}".format(v=ORT_VERSION, f=f),
        )

    # 把包内硬编码的 /npm/ 引用改写为 /npm2/（新 URL，避免服务器改 MIME 前的旧缓存失效问题）
    renamed = 0
    for dirpath, _dirnames, filenames in os.walk(os.path.join(BASE, "npm2")):
        for fn in filenames:
            p = os.path.join(dirpath, fn)
            with open(p, "rb") as f:
                data = f.read()
            if b"/npm/" in data:
                with open(p, "wb") as f:
                    f.write(data.replace(b"/npm/", b"/npm2/"))
                renamed += 1
    print("改写 /npm2/ 引用文件 %d 个" % renamed)

    # pdf.js（用于 pdf.html 渲染 PDF 页面）
    fetch(NPM + "pdfjs-dist@4.10.38/build/pdf.min.mjs", "vendor/pdf.mjs")
    fetch(NPM + "pdfjs-dist@4.10.38/build/pdf.worker.min.mjs", "vendor/pdf.worker.mjs")

    # OCR 模型（本地同源加载，彻底绕过模型 CDN 的 CORS；SDK 解析 .tar 内的 inference.onnx/.yml）
    MODEL_BASE = "https://paddle-model-ecology.bj.bcebos.com/paddlex/official_inference_model/paddle3.0.0/"
    for m in [
        "PP-OCRv5_mobile_det",
        "PP-OCRv5_mobile_rec",
        "PP-OCRv6_small_det",
        "PP-OCRv6_small_rec",
        "PP-OCRv6_tiny_det",
        "PP-OCRv6_tiny_rec",
    ]:
        fetch(MODEL_BASE + m + "_onnx_infer.tar", "models/" + m + "_onnx_infer.tar")

    print("全部资源已下载完成。")


if __name__ == "__main__":
    main()
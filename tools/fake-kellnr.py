"""缓存版 kellnr：从本机 cargo 缓存提供 sparse index + download。

用途：内网 kellnr（192.168.2.12:16444）不可达时，让 docker 构建能取到
digua 系列 crate。index 段把 cargo 私有缓存格式（\x00 分隔的 vers+json 对）
还原为标准 sparse index JSON 行；download 段直接回 .crate 文件。
只对本机构建用，不入发布链路。
"""

import http.server
import os
import re

HOME = os.path.expanduser("~")
INDEX_ROOT = os.path.join(
    HOME, ".cargo", "registry", "index", "192.168.2.12-726746185323252c", ".cache"
)
CACHE_ROOT = os.path.join(
    HOME, ".cargo", "registry", "cache", "192.168.2.12-726746185323252c"
)


def index_json(cache_path: str) -> bytes:
    """cargo 缓存文件 → 标准 sparse index JSON 行。

    缓存格式：[5 字节头][etag: <hex>][\\x00 vers][\\x00 {json}]…
    取每个 \\x00 段中以 "{" 开头的 JSON 片段，按行拼接。
    """
    with open(cache_path, "rb") as f:
        data = f.read()
    text = data.decode("utf-8", errors="replace")
    lines = []
    for segment in text.split("\x00"):
        segment = segment.strip()
        if segment.startswith("{"):
            lines.append(segment)
    return ("\n".join(lines) + "\n").encode("utf-8")


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        # registry config（sparse 协议握手）：声明下载基址
        if re.fullmatch(r"/api/v1/crates/config\.json", self.path):
            body = b'{"dl": "http://192.168.2.103:16444/api/v1/crates", "api": "http://192.168.2.103:16444/api/v1/crates/"}'
            self.send_response(200)
            self.send_header("Content-Type", "application/json; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        # download: /api/v1/crates/<name>/<version>/download（先于 index 判定）
        m = re.fullmatch(r"/api/v1/crates/([^/]+)/([^/]+)/download", self.path)
        if m:
            path = os.path.join(CACHE_ROOT, f"{m.group(1)}-{m.group(2)}.crate")
            if not os.path.isfile(path):
                self.send_error(404)
                return
            with open(path, "rb") as f:
                body = f.read()
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        # sparse index: /api/v1/crates/<a2>/<b2>/<name>
        m = re.fullmatch(r"/api/v1/crates/([^/]+)/([^/]+)/([^/]+)", self.path)
        if m:
            path = os.path.join(INDEX_ROOT, m.group(1), m.group(2), m.group(3))
            if not os.path.isfile(path):
                self.send_error(404)
                return
            body = index_json(path)
            self.send_response(200)
            self.send_header("Content-Type", "application/json; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        self.send_error(404)

    def log_message(self, fmt, *args):  # noqa: A003
        print(fmt % args)


if __name__ == "__main__":
    server = http.server.ThreadingHTTPServer(("0.0.0.0", 16444), Handler)
    print("fake kellnr on :16444")
    server.serve_forever()

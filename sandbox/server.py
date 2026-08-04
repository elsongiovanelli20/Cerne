# -*- coding: utf-8 -*-
"""Executor isolado do Cerne. Roda código de agente em CONTAINER separado, non-root, sem segredos,
com limites (mem/pids via compose), rootfs read-only + tmpfs /work, cada execução por subprocesso.
Endpoints: GET /health · POST /run {workdir,linguagem,codigo,timeout} · /write · /read · /reset."""
import http.server
import socketserver
import subprocess
import json
import os
import shutil

WORK = "/work"
SAFE_ENV = {"PATH": "/usr/local/bin:/usr/bin:/bin", "LANG": "C.UTF-8", "LC_ALL": "C.UTF-8",
            "HOME": "/work", "PYTHONDONTWRITEBYTECODE": "1", "PYTHONUNBUFFERED": "1",
            "U2NET_HOME": "/models/u2net", "NUMBA_CACHE_DIR": "/work", "MPLCONFIGDIR": "/work",
            "OMP_NUM_THREADS": "2", "ORT_DISABLE_ALL_OPTIMIZATION": "0"}


def _rundir(rid):
    safe = "".join(c for c in (rid or "default") if c.isalnum() or c in "-_")[:48] or "default"
    d = os.path.join(WORK, safe)
    os.makedirs(d, exist_ok=True)
    return d


class H(http.server.BaseHTTPRequestHandler):
    def _send(self, code, obj):
        b = json.dumps(obj, ensure_ascii=False).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(b)))
        self.end_headers()
        self.wfile.write(b)

    def do_GET(self):
        self._send(200, {"ok": True}) if self.path == "/health" else self._send(404, {"error": "nao encontrado"})

    def do_POST(self):
        try:
            ln = int(self.headers.get("Content-Length", 0) or 0)
            data = json.loads(self.rfile.read(ln) or b"{}")
        except Exception:
            return self._send(400, {"error": "json invalido"})
        d = _rundir(data.get("workdir"))
        if self.path == "/run":
            lang = (data.get("linguagem") or "python").lower()
            codigo = data.get("codigo") or ""
            timeout = min(max(int(data.get("timeout") or 15), 1), 180)
            try:
                if lang in ("python", "py"):
                    open(os.path.join(d, "_run.py"), "w", encoding="utf-8").write(codigo)
                    cmd = ["python", "-I", "_run.py"]
                elif lang in ("bash", "sh", "shell"):
                    open(os.path.join(d, "_run.sh"), "w", encoding="utf-8").write(codigo)
                    cmd = ["bash", "_run.sh"]
                else:
                    return self._send(200, {"saida": f"(linguagem '{lang}' nao suportada; use python ou bash)"})
                r = subprocess.run(cmd, cwd=d, env=SAFE_ENV, capture_output=True, text=True, timeout=timeout)
                out = (r.stdout or "") + (("\n[stderr]\n" + r.stderr) if r.stderr else "")
                self._send(200, {"saida": out[:8000] or f"(sem saida · retorno {r.returncode})",
                                 "retorno": r.returncode, "arquivos": sorted(os.listdir(d))[:60]})
            except subprocess.TimeoutExpired:
                self._send(200, {"saida": f"(tempo esgotado — o codigo passou de {timeout}s)"})
            except Exception as e:
                self._send(200, {"saida": f"(erro ao executar: {str(e)[:200]})"})
        elif self.path == "/write":
            nome = os.path.basename(data.get("nome") or "arquivo.txt")
            try:
                open(os.path.join(d, nome), "w", encoding="utf-8").write(data.get("conteudo") or "")
                self._send(200, {"ok": True, "nome": nome})
            except Exception as e:
                self._send(200, {"erro": str(e)[:120]})
        elif self.path == "/read":
            nome = os.path.basename(data.get("nome") or "")
            try:
                self._send(200, {"conteudo": open(os.path.join(d, nome), encoding="utf-8", errors="ignore").read()[:8000]})
            except Exception as e:
                self._send(200, {"erro": str(e)[:120]})
        elif self.path == "/list":
            # artefatos gerados pelo agente (exclui os scripts _run.*), com tamanho em bytes
            itens = []
            for n in sorted(os.listdir(d)):
                if n.startswith("_run."):
                    continue
                fp = os.path.join(d, n)
                if os.path.isfile(fp):
                    itens.append({"nome": n, "bytes": os.path.getsize(fp)})
            self._send(200, {"arquivos": itens})
        elif self.path == "/download":
            import base64
            nome = os.path.basename(data.get("nome") or "")
            fp = os.path.join(d, nome)
            try:
                if not os.path.isfile(fp) or os.path.getsize(fp) > 25 * 1024 * 1024:
                    return self._send(200, {"erro": "arquivo inexistente ou grande demais (>25MB)"})
                self._send(200, {"nome": nome, "b64": base64.b64encode(open(fp, "rb").read()).decode(),
                                 "bytes": os.path.getsize(fp)})
            except Exception as e:
                self._send(200, {"erro": str(e)[:120]})
        elif self.path == "/reset":
            shutil.rmtree(d, ignore_errors=True)
            self._send(200, {"ok": True})
        else:
            self._send(404, {"error": "nao encontrado"})

    def log_message(self, *a):
        pass


class TS(socketserver.ThreadingMixIn, http.server.HTTPServer):
    daemon_threads = True
    allow_reuse_address = True


if __name__ == "__main__":
    os.makedirs(WORK, exist_ok=True)
    TS(("0.0.0.0", 8090), H).serve_forever()

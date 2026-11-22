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


def _env(d):
    """Ambiente da execução: pacotes instalados pelo agente ficam DENTRO do workdir (.pkgs / node_modules),
    pois o rootfs é read-only. `pip install x` vai para .pkgs automaticamente (PIP_TARGET)."""
    e = dict(SAFE_ENV)
    pk = os.path.join(d, ".pkgs")
    e.update({"PYTHONPATH": pk, "PIP_TARGET": pk, "PIP_DISABLE_PIP_VERSION_CHECK": "1", "PIP_NO_WARN_SCRIPT_LOCATION": "1",
              "PIP_ROOT_USER_ACTION": "ignore", "PATH": pk + "/bin:" + d + "/node_modules/.bin:" + SAFE_ENV["PATH"],
              "npm_config_cache": "/work/.npm", "npm_config_update_notifier": "false",
              "GIT_AUTHOR_NAME": "Cerne Agente", "GIT_AUTHOR_EMAIL": "agente@cerne.local",
              "GIT_COMMITTER_NAME": "Cerne Agente", "GIT_COMMITTER_EMAIL": "agente@cerne.local"})
    return e


def _rel(d, nome):
    """Caminho relativo seguro dentro do workdir (aceita subpastas; bloqueia .. e absolutos)."""
    n = (nome or "").replace("\\", "/").strip().lstrip("/")
    partes = [p for p in n.split("/") if p not in ("", ".")]
    if not partes or any(p == ".." for p in partes):
        return None
    fp = os.path.realpath(os.path.join(d, *partes))
    return fp if fp.startswith(os.path.realpath(d) + os.sep) else None


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
            timeout = min(max(int(data.get("timeout") or 15), 1), 600)
            try:
                if lang in ("python", "py"):
                    open(os.path.join(d, "_run.py"), "w", encoding="utf-8").write(codigo)
                    # -I (isolado) + os pacotes que o agente instalou em .pkgs
                    cmd = ["python", "-I", "-c", "import sys,runpy;sys.path[:0]=['.pkgs','.'];sys.argv=['_run.py'];"
                           "runpy.run_path('_run.py',run_name='__main__')"]
                elif lang in ("bash", "sh", "shell"):
                    open(os.path.join(d, "_run.sh"), "w", encoding="utf-8").write(codigo)
                    cmd = ["bash", "_run.sh"]
                else:
                    return self._send(200, {"saida": f"(linguagem '{lang}' nao suportada; use python ou bash)"})
                r = subprocess.run(cmd, cwd=d, env=_env(d), capture_output=True, text=True, timeout=timeout)
                out = (r.stdout or "") + (("\n[stderr]\n" + r.stderr) if r.stderr else "")
                if len(out) > 8000:                     # guarda o começo E o fim (erros de teste ficam no fim)
                    out = out[:2500] + "\n…[saída cortada]…\n" + out[-5400:]
                self._send(200, {"saida": (out or f"(sem saida · retorno {r.returncode})") + f"\n[retorno {r.returncode}]",
                                 "retorno": r.returncode, "arquivos": sorted(os.listdir(d))[:60]})
            except subprocess.TimeoutExpired:
                self._send(200, {"saida": f"(tempo esgotado — o codigo passou de {timeout}s)"})
            except Exception as e:
                self._send(200, {"saida": f"(erro ao executar: {str(e)[:200]})"})
        elif self.path == "/write":
            fp = _rel(d, data.get("nome") or "arquivo.txt")
            if not fp:
                return self._send(200, {"erro": "caminho inválido"})
            try:
                os.makedirs(os.path.dirname(fp), exist_ok=True)
                open(fp, "w", encoding="utf-8").write(data.get("conteudo") or "")
                self._send(200, {"ok": True, "nome": os.path.relpath(fp, d)})
            except Exception as e:
                self._send(200, {"erro": str(e)[:120]})
        elif self.path == "/read":
            fp = _rel(d, data.get("nome") or "")
            if not fp:
                return self._send(200, {"erro": "caminho inválido"})
            try:
                if os.path.isdir(fp):
                    return self._send(200, {"conteudo": "\n".join(sorted(os.listdir(fp))[:300])})
                self._send(200, {"conteudo": open(fp, encoding="utf-8", errors="ignore").read()[:30000]})
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
        elif self.path == "/extract":
            # tarball (.tar.gz, base64) → <workdir>/<dest>, removendo a pasta-raiz do GitHub e criando o
            # snapshot git inicial (base p/ calcular as alterações do PR depois)
            import base64, tarfile, io
            dest = _rel(d, data.get("dest") or "repo")
            if not dest:
                return self._send(200, {"erro": "destino inválido"})
            try:
                shutil.rmtree(dest, ignore_errors=True); os.makedirs(dest)
                tf = tarfile.open(fileobj=io.BytesIO(base64.b64decode(data.get("b64") or "")), mode="r:*")
                membros = []
                for m in tf.getmembers():
                    partes = m.name.split("/", 1)
                    if len(partes) < 2 or not partes[1] or not (m.isfile() or m.isdir()):
                        continue
                    m.name = partes[1]
                    membros.append(m)
                tf.extractall(dest, members=membros, filter="data")
                n = sum(1 for m in membros if m.isfile())
                e = _env(d)
                for c in (["git", "init", "-q"], ["git", "add", "-A"], ["git", "commit", "-q", "-m", "base", "--no-gpg-sign"]):
                    subprocess.run(c, cwd=dest, env=e, capture_output=True, timeout=120)
                self._send(200, {"ok": True, "arquivos": n})
            except Exception as ex:
                self._send(200, {"erro": str(ex)[:200]})
        elif self.path == "/git_changes":
            import base64
            rd = _rel(d, data.get("dir") or "repo")
            if not rd or not os.path.isdir(os.path.join(rd, ".git")):
                return self._send(200, {"erro": "repositório não encontrado"})
            e = _env(d)
            subprocess.run(["git", "add", "-A"], cwd=rd, env=e, capture_output=True, timeout=120)
            r = subprocess.run(["git", "diff", "--cached", "--name-status", "-z", "--no-renames", "HEAD"],
                               cwd=rd, env=e, capture_output=True, timeout=120)
            itens = r.stdout.decode("utf-8", "replace").split("\0")
            mud = []
            for st, caminho in zip(itens[0::2], itens[1::2]):
                if not caminho or caminho.startswith((".pkgs/", "node_modules/")):
                    continue
                m = {"caminho": caminho, "removido": st.startswith("D")}
                fp = os.path.join(rd, caminho)
                if not m["removido"] and os.path.isfile(fp):
                    m["modo"] = "100755" if os.access(fp, os.X_OK) else "100644"
                    if data.get("conteudo") and os.path.getsize(fp) <= 20 * 1024 * 1024:
                        m["b64"] = base64.b64encode(open(fp, "rb").read()).decode()
                mud.append(m)
            self._send(200, {"mudancas": mud})
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

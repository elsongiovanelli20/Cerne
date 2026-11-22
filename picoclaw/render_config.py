#!/usr/bin/env python3
"""Gera picoclaw/data/config.json a partir do config.template.json + .env do Cerne (segredos ficam fora do git).

Uso (na pasta do Cerne):  python3 picoclaw/render_config.py [.env]

- O canal Telegram só é ligado se TELEGRAM_BOT_TOKEN **e** TELEGRAM_ALLOW_FROM estiverem preenchidos
  (nunca abre o bot para qualquer pessoa: allow_from vazio = todos, por isso é exigido).
- TELEGRAM_ALLOW_FROM: IDs numéricos do Telegram separados por vírgula (descubra o seu com o @userinfobot).
"""
import json
import os
import re
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
ENV_PATH = sys.argv[1] if len(sys.argv) > 1 else os.path.join(AQUI, "..", ".env")


def ler_env(path):
    env = {}
    with open(path, encoding="utf-8") as f:
        for linha in f:
            linha = linha.strip()
            if not linha or linha.startswith("#") or "=" not in linha:
                continue
            k, v = linha.split("=", 1)
            v = v.split(" #", 1)[0].strip().strip('"').strip("'")
            env[k.strip()] = v
    return env


def main():
    env = ler_env(ENV_PATH)
    faltando = [k for k in ("CERNE_MCP_TOKEN",) if not env.get(k)]
    if faltando:
        sys.exit("faltam no .env: " + ", ".join(faltando))
    ids = [i.strip() for i in env.get("TELEGRAM_ALLOW_FROM", "").split(",") if i.strip()]
    tg_on = bool(env.get("TELEGRAM_BOT_TOKEN")) and bool(ids)
    tipados = {"TELEGRAM_ENABLED": tg_on, "TELEGRAM_ALLOW_FROM": ids}

    tpl = json.load(open(os.path.join(AQUI, "config.template.json"), encoding="utf-8"))

    def sub(o):
        if isinstance(o, dict):                       # chaves "_..." são só comentários do modelo (o PicoClaw
            return {k: sub(v) for k, v in o.items() if not k.startswith("_")}   # rejeita campos desconhecidos)
        if isinstance(o, list):
            return [sub(v) for v in o]
        if isinstance(o, str):
            m = re.fullmatch(r"\$\{(\w+)\}", o)
            if m and m.group(1) in tipados:
                return tipados[m.group(1)]
            return re.sub(r"\$\{(\w+)\}", lambda mm: env.get(mm.group(1), ""), o)
        return o

    cfg = sub(tpl)
    os.makedirs(os.path.join(AQUI, "data"), exist_ok=True)
    out = os.path.join(AQUI, "data", "config.json")
    tmp = out + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(cfg, f, ensure_ascii=False, indent=2)
    os.chmod(tmp, 0o600)
    os.replace(tmp, out)
    print(f"ok: {out} · telegram {'LIGADO' if tg_on else 'desligado (falta TELEGRAM_BOT_TOKEN e/ou TELEGRAM_ALLOW_FROM)'}")


if __name__ == "__main__":
    main()

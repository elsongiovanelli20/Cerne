"""Gera (uma única vez) os segredos do Langfuse self-hosted e acrescenta no .env do Cerne.

Uso (na pasta do projeto):  python scripts/langfuse-setup.py
Não sobrescreve nada que já exista no .env. Mostra no final o e-mail/senha do 1º login.
"""
import os
import secrets
import sys

ENV = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".env")


def main():
    atual = {}
    linhas = []
    if os.path.exists(ENV):
        with open(ENV, encoding="utf-8") as f:
            linhas = f.read().splitlines()
        for ln in linhas:
            if "=" in ln and not ln.lstrip().startswith("#"):
                k, v = ln.split("=", 1)
                atual[k.strip()] = v.strip()
    novos = {
        "LANGFUSE_NEXTAUTH_SECRET": secrets.token_urlsafe(32),
        "LANGFUSE_SALT": secrets.token_urlsafe(32),
        "LANGFUSE_ENCRYPTION_KEY": secrets.token_hex(32),          # 64 hex, exigido pelo Langfuse
        "LANGFUSE_DB_PASSWORD": secrets.token_urlsafe(24),
        "LANGFUSE_CLICKHOUSE_PASSWORD": secrets.token_urlsafe(24),
        "LANGFUSE_REDIS_AUTH": secrets.token_hex(24),
        "LANGFUSE_MINIO_PASSWORD": secrets.token_urlsafe(24),
        "LANGFUSE_PUBLIC_KEY": "pk-lf-" + secrets.token_hex(16),
        "LANGFUSE_SECRET_KEY": "sk-lf-" + secrets.token_hex(16),
        "LANGFUSE_INIT_USER_EMAIL": "admin@cerne.local",
        "LANGFUSE_INIT_USER_PASSWORD": secrets.token_urlsafe(14),
        "LANGFUSE_PUBLIC_URL": "http://localhost:3300",
        "LANGFUSE_CAPTURE_CONTENT": "false",
    }
    add = {k: v for k, v in novos.items() if not atual.get(k)}
    if not add:
        print("Segredos do Langfuse já existem no .env — nada alterado.")
    else:
        with open(ENV, "a", encoding="utf-8") as f:
            if linhas and linhas[-1].strip():
                f.write("\n")
            f.write("\n# ---- Langfuse self-hosted (gerado por scripts/langfuse-setup.py) ----\n")
            for k, v in add.items():
                f.write(f"{k}={v}\n")
        print(f"{len(add)} variáveis do Langfuse acrescentadas ao .env.")
    final = {**novos, **atual}
    print("\nLogin do Langfuse:", final["LANGFUSE_INIT_USER_EMAIL"], "/", final["LANGFUSE_INIT_USER_PASSWORD"])
    print("Suba com: docker compose -f docker-compose.yml -f docker-compose.langfuse.yml up -d")
    return 0


if __name__ == "__main__":
    sys.exit(main())

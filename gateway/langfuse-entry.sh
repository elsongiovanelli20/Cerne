#!/bin/sh
# Entrada do gateway quando o docker-compose.langfuse.yml está em uso.
# Liga o callback "langfuse_otel" do LiteLLM SEM duplicar o litellm-config.yaml: gera uma cópia em /tmp
# com o callback acrescentado. Sem chaves (ou se a geração falhar) sobe com a config original — o gateway
# nunca fica fora do ar por causa do Langfuse.
SRC=/app/config.yaml
DST=/tmp/litellm-config.yaml
cp "$SRC" "$DST"
if [ -n "$LANGFUSE_PUBLIC_KEY" ] && [ -n "$LANGFUSE_SECRET_KEY" ]; then
  PY=$(command -v python3 || command -v python)
  if "$PY" - "$SRC" "$DST" <<'EOF'
import sys, yaml
cfg = yaml.safe_load(open(sys.argv[1])) or {}
ls = cfg.get("litellm_settings") or {}
cb = ls.get("callbacks") or []
if isinstance(cb, str):
    cb = [cb]
if "langfuse_otel" not in cb:
    cb.append("langfuse_otel")
ls["callbacks"] = cb
cfg["litellm_settings"] = ls
yaml.safe_dump(cfg, open(sys.argv[2], "w"), sort_keys=False, allow_unicode=True)
EOF
  then
    echo "[cerne] Langfuse ligado: traces -> ${LANGFUSE_HOST}"
  else
    cp "$SRC" "$DST"
    echo "[cerne] AVISO: não foi possível ligar o Langfuse; gateway segue sem traces"
  fi
  if [ "${LANGFUSE_CAPTURE_CONTENT:-false}" = "true" ]; then
    export OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT=span_only   # grava prompt/resposta (ver aviso de PII)
  fi
else
  echo "[cerne] Langfuse sem LANGFUSE_PUBLIC_KEY/SECRET_KEY: gateway sem traces"
fi
exec litellm --config "$DST" "$@"

# ---- Estágio 1: conteúdo do Open Design Agent Kit (design systems, receitas, exemplos, craft) ----
# Kit: https://github.com/feimacode/open-design-agent-kit (MIT) — conteúdo vendorizado do
# nexu-io/open-design (Apache-2.0). Pinado num commit p/ builds reprodutíveis; troque ODKIT_REF para atualizar.
# Só o conteúdo (~20 MB) é baixado (sparse checkout); nada do kit roda em runtime — o Cerne lê os arquivos.
FROM alpine/git:latest AS odkit
ARG ODKIT_REF=2e92c00ae4fdb025e09ad8c13f39fdf09a2dfa4b
RUN git clone --filter=blob:none --no-checkout https://github.com/feimacode/open-design-agent-kit.git /odak \
 && cd /odak \
 && git sparse-checkout set --no-cone "/packages/content/assets/open-design/" "/LICENSE" \
 && git checkout ${ODKIT_REF} \
 && cp LICENSE packages/content/assets/open-design/LICENSE-open-design-agent-kit.txt

# ---- Estágio 2: app ----
FROM python:3.11-slim
WORKDIR /srv
# OCR local (Tesseract + idioma português) para extrair texto de imagens/scans
RUN apt-get update && apt-get install -y --no-install-recommends \
    tesseract-ocr tesseract-ocr-por \
    unar \
 && rm -rf /var/lib/apt/lists/*
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt
COPY --from=odkit /odak/packages/content/assets/open-design /opt/odkit
ENV ODKIT_DIR=/opt/odkit
COPY odkit.py main.py ./
EXPOSE 8000
CMD ["uvicorn", "main:app", "--host", "0.0.0.0", "--port", "8000"]

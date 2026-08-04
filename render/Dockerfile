# Renderizador Chromium do Cerne (Playwright + FFmpeg) — imagem multi-arch (amd64/arm64)
FROM mcr.microsoft.com/playwright/python:v1.47.0-jammy

RUN apt-get update && apt-get install -y --no-install-recommends ffmpeg \
    && rm -rf /var/lib/apt/lists/*

RUN pip install --no-cache-dir "fastapi==0.111.*" "uvicorn[standard]==0.30.*" "playwright==1.47.0"

WORKDIR /srv
COPY server.py /srv/server.py
EXPOSE 8091
CMD ["uvicorn", "server:app", "--host", "0.0.0.0", "--port", "8091", "--workers", "1"]

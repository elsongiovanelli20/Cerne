FROM python:3.12-slim

# Fontes (DejaVu) p/ texto nos criativos + ffmpeg p/ o Animate (motion graphics -> MP4).
RUN apt-get update && apt-get install -y --no-install-recommends fonts-dejavu-core ffmpeg \
    && rm -rf /var/lib/apt/lists/*

# Office/dados/imagem (Pillow via python-pptx) + rembg (remoção de fundo do Photoshoot, roda em CPU).
RUN pip install --no-cache-dir openpyxl python-docx python-pptx pandas requests rembg onnxruntime \
    && useradd -u 1000 -m sandbox

# Pré-baixa o modelo do rembg (u2net) num caminho FIXO — em runtime o rootfs é read-only.
ENV U2NET_HOME=/models/u2net
RUN mkdir -p /models/u2net \
    && python -c "import urllib.request; urllib.request.urlretrieve('https://github.com/danielgatis/rembg/releases/download/v0.0.0/u2net.onnx','/models/u2net/u2net.onnx')" \
    && chmod -R 755 /models

COPY server.py /server.py
WORKDIR /work
USER sandbox
EXPOSE 8090
CMD ["python", "-I", "/server.py"]

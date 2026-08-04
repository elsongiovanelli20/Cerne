# -*- coding: utf-8 -*-
"""Renderizador soberano do Cerne: HTML → PNG/PDF/MP4 via Chromium (Playwright) + FFmpeg.
Container separado, SEM segredos. Recebe HTML autossuficiente do app e devolve os bytes."""
import os, subprocess, tempfile, shutil
from fastapi import FastAPI, Response, HTTPException
from pydantic import BaseModel
from playwright.async_api import async_playwright

app = FastAPI()


class RenderReq(BaseModel):
    html: str = ""
    format: str = "png"        # png | pdf | mp4
    width: int = 1280
    height: int = 800
    dur: float = 8.0           # segundos (mp4)
    full: bool = True          # png de página inteira


@app.get("/health")
async def health():
    return {"ok": True}


@app.post("/render")
async def render(req: RenderReq):
    if not (req.html or "").strip():
        raise HTTPException(status_code=400, detail="html vazio")
    w = max(320, min(int(req.width or 1280), 1920)); h = max(320, min(int(req.height or 800), 1200))
    w -= w % 2; h -= h % 2
    async with async_playwright() as p:
        browser = await p.chromium.launch(args=["--no-sandbox", "--disable-dev-shm-usage", "--disable-gpu"])
        try:
            page = await browser.new_page(viewport={"width": w, "height": h}, device_scale_factor=1)
            try:
                await page.set_content(req.html, wait_until="networkidle", timeout=25000)
            except Exception:
                await page.set_content(req.html, wait_until="load", timeout=15000)
            await page.wait_for_timeout(700)
            if req.format == "pdf":
                data = await page.pdf(print_background=True, prefer_css_page_size=True)
                return Response(content=data, media_type="application/pdf")
            if req.format == "mp4":
                tmp = tempfile.mkdtemp()
                try:
                    total_h = await page.evaluate("Math.max(document.body.scrollHeight, document.documentElement.scrollHeight)")
                    total = max(0, int(total_h) - h)
                    fps = 24; n = max(fps, int(fps * max(2.0, min(req.dur, 20))))
                    for i in range(n):
                        y = int(total * (i / max(1, n - 1))) if total else 0
                        await page.evaluate("window.scrollTo(0, %d)" % y)
                        await page.wait_for_timeout(16)
                        await page.screenshot(path=os.path.join(tmp, "f%05d.png" % i))
                    out = os.path.join(tmp, "out.mp4")
                    subprocess.run(["ffmpeg", "-y", "-framerate", str(fps), "-i", os.path.join(tmp, "f%05d.png"),
                                    "-vf", "scale=trunc(iw/2)*2:trunc(ih/2)*2", "-c:v", "libx264", "-pix_fmt", "yuv420p",
                                    "-movflags", "+faststart", "-loglevel", "error", out], check=True, timeout=150)
                    return Response(content=open(out, "rb").read(), media_type="video/mp4")
                finally:
                    shutil.rmtree(tmp, ignore_errors=True)
            data = await page.screenshot(full_page=bool(req.full), type="png")
            return Response(content=data, media_type="image/png")
        finally:
            await browser.close()

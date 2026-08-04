"""
Cerne — Reranker neural (cross-encoder BGE) via fastembed/ONNX.

Recebe (query, documentos) e devolve a ordem por relevância. Roda em ONNX
(leve, sem torch), multilíngue (bom em pt). O app usa isto para reordenar os
candidatos vindos do Qdrant antes de montar o contexto do RAG.
"""
import os
from fastapi import FastAPI
from pydantic import BaseModel
from fastembed.rerank.cross_encoder import TextCrossEncoder

MODEL = os.getenv("RERANK_MODEL", "BAAI/bge-reranker-base")  # multilíngue
model = TextCrossEncoder(model_name=MODEL, cache_dir="/cache")

app = FastAPI(title="Cerne — Reranker (cross-encoder)")


class RerankReq(BaseModel):
    query: str
    documents: list[str]


@app.get("/health")
def health():
    return {"status": "ok", "model": MODEL}


@app.post("/rerank")
def rerank(r: RerankReq):
    if not r.documents:
        return {"order": [], "scores": []}
    scores = list(model.rerank(r.query, r.documents))          # 1 score por documento
    order = sorted(range(len(r.documents)), key=lambda i: scores[i], reverse=True)
    return {"order": order, "scores": [float(scores[i]) for i in order]}

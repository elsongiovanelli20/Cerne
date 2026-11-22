"""
odkit — ponte entre o Design Studio do Cerne e o Open Design Agent Kit.

O kit (https://github.com/feimacode/open-design-agent-kit, MIT; conteúdo vendorizado do
nexu-io/open-design, Apache-2.0) é usado aqui como BIBLIOTECA DE CONTEÚDO, 100% local:

  design-systems/<id>/DESIGN.md + tokens.css   → design systems de marca/gênero (contrato + tokens CSS)
  design-templates/<id>/SKILL.md, skills/<id>  → "receitas" por tipo de artefato (deck, dashboard, landing…)
  examples/<id>/example.html + SKILL.md        → artefatos prontos para REMIXAR
  craft/*.md                                   → regras universais (anti-AI-slop, cor, tipografia, a11y…)
  design-system-descriptors.json               → humor/indústria/superfície de cada sistema (recomendação)
  surfaces.json                                → "o que dá pra fazer" → receitas por superfície

Nada sai da máquina: o conteúdo é lido do disco (ODKIT_DIR, padrão /opt/odkit — copiado no build do
container) e injetado no prompt do modelo que o Cerne já usa. Se a pasta não existir, tudo degrada
silenciosamente e o Design Studio continua funcionando como antes.
"""
from __future__ import annotations

import colorsys
import json
import os
import re
import threading

ODKIT_DIR = os.getenv("ODKIT_DIR", "/opt/odkit")
_LOCK = threading.Lock()
_CACHE: dict | None = None

# Tipo de design do Cerne → receita Open Design padrão + superfícies do catálogo (surfaces.json)
TIPO_MAP = {
    "landing":   {"receita": "saas-landing",     "surfaces": ["prototype", "portfolio"],       "ds_surface": "web"},
    "deck":      {"receita": "simple-deck",      "surfaces": ["slides"],                       "ds_surface": "slides"},
    "dashboard": {"receita": "dashboard",        "surfaces": ["data-report", "app-screens"],   "ds_surface": "dashboard"},
    "onepager":  {"receita": "docs-page",        "surfaces": ["document", "research"],         "ds_surface": "document"},
    "email":     {"receita": "email-campaign",   "surfaces": ["email"],                        "ds_surface": "web"},
    "prototype": {"receita": "mobile-app",       "surfaces": ["mobile", "app-screens", "wireframe"], "ds_surface": "mobile"},
    "social":    {"receita": "social-carousel",  "surfaces": ["social", "cards"],              "ds_surface": "web"},
    "blog":      {"receita": "blog-post",        "surfaces": ["document"],                     "ds_surface": "document"},
    "curriculo": {"receita": "resume-modern",    "surfaces": ["resume", "portfolio"],          "ds_surface": "document"},
    "cardapio":  {"receita": "",                 "surfaces": ["poster", "document"],           "ds_surface": "document"},
    "convite":   {"receita": "poster-hero",      "surfaces": ["poster", "campaign"],           "ds_surface": "web"},
}

# Regras de craft sempre aplicadas (as demais vêm do od.craft.requires da receita)
CRAFT_BASE = ["anti-ai-slop", "color", "typography"]

# Português → vocabulário do design-system-descriptors.json (recomendação de design system)
_KW = {
    "moods": {
        "minimal": ["minimal", "minimalista", "limpo", "clean", "simples", "enxuto"],
        "calm": ["calmo", "tranquil", "sereno", "zen", "suave"],
        "warm": ["acolhedor", "quente", "aconcheg", "caloroso", "terroso"],
        "friendly": ["amigável", "amigavel", "simpático", "acessível", "leve"],
        "playful": ["divertido", "lúdico", "ludico", "brincalh", "infantil", "criança", "jogo"],
        "bold": ["ousado", "forte", "impactante", "chamativo", "vibrante", "energ"],
        "premium": ["premium", "luxo", "sofisticad", "exclusiv", "alto padrão"],
        "elegant": ["elegante", "refinad", "chique", "clássico", "classico"],
        "technical": ["técnico", "tecnico", "dev ", "devs", "engenharia", "api", "código", "codigo", "infra"],
        "editorial": ["editorial", "revista", "jornal", "artigo", "ensaio", "notícia", "noticia", "blog"],
        "retro": ["retrô", "retro", "vintage", "anos 80", "anos 90", "nostalg"],
        "futuristic": ["futurist", "sci-fi", "neon", "cyber", "espacial"],
        "corporate": ["corporativ", "institucional", "empresa", "b2b", "banco", "jurídic", "juridic", "governo"],
        "artistic": ["artístic", "artistic", "arte", "galeria", "criativ", "portfólio", "portfolio"],
        "romantic": ["romântic", "romantic", "casamento", "noiva", "amor"],
    },
    "industries": {
        "saas": ["saas", "software", "plataforma", "app web", "assinatura"],
        "developer": ["dev ", "devs", "desenvolvedor", "api", "sdk", "código", "codigo", "open source"],
        "ai": [" ia ", " ia,", " ia.", "inteligência artificial", "inteligencia artificial", " ai ", "llm", "agentes de ia", "agente de ia"],
        "finance": ["finan", "banco", "fintech", "pagamento", "investimento", "crédito", "credito", "contábil", "contabil"],
        "commerce": ["loja", "e-commerce", "ecommerce", "produto", "varejo", "venda", "marketplace"],
        "consumer": ["consumidor", "app de", "cliente final"],
        "media": ["mídia", "midia", "vídeo", "video", "música", "musica", "podcast", "streaming"],
        "editorial": ["revista", "jornal", "editorial", "artigo", "blog", "newsletter"],
        "brand": ["marca", "branding", "identidade"],
        "creative": ["agência", "agencia", "estúdio", "estudio", "design", "criativ", "portfólio", "portfolio"],
        "dashboard": ["dashboard", "painel", "kpi", "métrica", "metrica", "relatório", "relatorio", "analytics"],
        "enterprise": ["enterprise", "corporativ", "erp", "b2b", "governança", "governanca"],
        "wellness": ["saúde", "saude", "bem-estar", "yoga", "clínica", "clinica", "meditação", "academia", "fitness"],
        "education": ["curso", "escola", "educa", "aula", "ensino", "faculdade", "treinamento"],
        "automotive": ["carro", "automot", "veículo", "veiculo", "moto"],
        "travel": ["viagem", "turismo", "hotel", "pousada", "passagem", "hospedagem"],
        "food": ["restaurante", "comida", "cardápio", "cardapio", "café", "cafe", "bar ", "bares", "barista", "padaria", "delivery", "gastron"],
        "social": ["social", "comunidade", "rede social", "redes sociais", "instagram", "carrossel"],
        "gaming": ["game", "jogo", "gamer", "e-sports", "esports"],
        "dating": ["namoro", "dating", "encontro"],
    },
}

_AI_INDIGO = {"#6366f1", "#4f46e5", "#4338ca", "#3730a3", "#8b5cf6", "#7c3aed", "#a855f7"}
_PLACEHOLDER_CDN = re.compile(r"(unsplash\.com|placehold\.co|placeholder\.com|via\.placeholder|placekitten\.com|picsum\.photos|dummyimage\.com|loremflickr)", re.I)
_FILLER = re.compile(r"(lorem ipsum|dolor sit amet|placeholder text|sample content|texto de exemplo|feature (one|two|three)|recurso (um|dois|três|tres)\b)", re.I)
_INVENTED = re.compile(r"(\b\d+(?:[.,]\d+)?\s*[x×]\s*(?:mais|faster|more|melhor|rápido|rapido)|99[.,]9+\s*%\s*(?:uptime|disponibilidade)?)", re.I)
_EMOJI = re.compile("[\U0001F300-\U0001FAFF☀-➿⭐✨]")


# ───────────────────────────── utilidades ─────────────────────────────

def _read(path, limit=None):
    try:
        with open(path, encoding="utf-8", errors="replace") as f:
            return f.read() if limit is None else f.read(limit)
    except Exception:
        return ""


def _json(path, default=None):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except Exception:
        return default


def _frontmatter(text):
    """Front-matter YAML mínimo (sem dependência): devolve (meta_dict, corpo). Usa PyYAML se houver."""
    m = re.match(r"^---\s*\n(.*?)\n---\s*\n?(.*)$", text or "", re.S)
    if not m:
        return {}, text or ""
    raw, body = m.group(1), m.group(2)
    try:
        import yaml  # type: ignore
        meta = yaml.safe_load(raw) or {}
        if isinstance(meta, dict):
            return meta, body
    except Exception:
        pass
    meta = {}
    for ln in raw.splitlines():
        mm = re.match(r"^([A-Za-z_][\w-]*):\s*(.*)$", ln)
        if mm and mm.group(2) and mm.group(2) not in ("|", ">"):
            meta[mm.group(1)] = mm.group(2).strip().strip('"\'')
    craft = re.search(r"craft:\s*\n\s*requires:\s*\[([^\]]*)\]", raw)
    if craft:
        meta.setdefault("od", {})["craft"] = {"requires": [x.strip() for x in craft.group(1).split(",") if x.strip()]}
    return meta, body


def _trim(text, n):
    text = (text or "").strip()
    if len(text) <= n:
        return text
    cut = text[:n]
    p = cut.rfind("\n## ")
    if p > n * 0.6:
        cut = cut[:p]
    return cut.rstrip() + "\n…(trecho truncado)"


def _hex_norm(h):
    h = (h or "").strip().lower()
    if re.fullmatch(r"#[0-9a-f]{3}", h):
        h = "#" + "".join(c * 2 for c in h[1:])
    return h if re.fullmatch(r"#[0-9a-f]{6}", h) else ""


def _lum(h):
    h = _hex_norm(h)
    if not h:
        return None
    def ch(v):
        v = v / 255
        return v / 12.92 if v <= 0.03928 else ((v + 0.055) / 1.055) ** 2.4
    r, g, b = (int(h[i:i + 2], 16) for i in (1, 3, 5))
    return 0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b)


def contraste(a, b):
    la, lb = _lum(a), _lum(b)
    if la is None or lb is None:
        return None
    hi, lo = max(la, lb), min(la, lb)
    return (hi + 0.05) / (lo + 0.05)


def _hue(h):
    h = _hex_norm(h)
    if not h:
        return None, 0
    r, g, b = (int(h[i:i + 2], 16) / 255 for i in (1, 3, 5))
    hh, _l, s = colorsys.rgb_to_hls(r, g, b)
    return hh * 360, s


# ───────────────────────────── carga do catálogo ─────────────────────────────

def _root_block(css):
    m = re.search(r":root\s*\{.*?\n\}", css or "", re.S)
    if not m:
        return ""
    blk = re.sub(r"/\*.*?\*/", "", m.group(0), flags=re.S)
    return "\n".join(ln.rstrip() for ln in blk.splitlines() if ln.strip())


def _tokens(root):
    vals = dict(re.findall(r"(--[\w-]+)\s*:\s*([^;]+);", root or "", re.S))
    def res(k, depth=0):
        v = (vals.get(k) or "").strip()
        mm = re.match(r"var\((--[\w-]+)\)", v)
        return res(mm.group(1), depth + 1) if mm and depth < 4 else v
    return {k: re.sub(r"\s+", " ", res(k)) for k in vals}


def _first_font(stack):
    for f in (stack or "").split(","):
        f = f.strip().strip('"\'')
        if f and not f.startswith("-") and f.lower() not in ("system-ui", "sans-serif", "serif", "monospace", "ui-monospace", "inherit"):
            return f
    return ""


def _load_design_systems(base, descr):
    out = {}
    d = os.path.join(base, "design-systems")
    if not os.path.isdir(d):
        return out
    for sid in sorted(os.listdir(d)):
        p = os.path.join(d, sid)
        md = _read(os.path.join(p, "DESIGN.md"))
        if not md:
            continue
        man = _json(os.path.join(p, "manifest.json"), {}) or {}
        root = _root_block(_read(os.path.join(p, "tokens.css")))
        tk = _tokens(root)
        title = re.search(r"^#\s+(.+)$", md, re.M)
        cat = re.search(r"^>\s*Category:\s*(.+)$", md, re.M)
        blurb = re.search(r"^>[ \t]*(?!Category:)(\S.+)$", md, re.M)
        nome = man.get("name") or (title.group(1).replace("Design System Inspired by", "").strip() if title else sid)
        paleta = []
        for k in ("--accent", "--bg", "--fg", "--surface-warm", "--accent-hover", "--success"):
            h = _hex_norm(tk.get(k, ""))
            if h and h not in paleta:
                paleta.append(h)
        fontes = [x for x in (_first_font(tk.get("--font-display")), _first_font(tk.get("--font-body"))) if x]
        fontes = list(dict.fromkeys(fontes))
        bg = _lum(tk.get("--bg", "#ffffff"))
        de = (descr.get("systems") or {}).get(sid) or {}
        out[sid] = {
            "id": "od:" + sid, "slug": sid, "nome": nome,
            "categoria": (man.get("category") or (cat.group(1).strip() if cat else "Open Design")),
            "blurb": (blurb.group(1).strip() if blurb else "")[:200],
            "paleta": paleta[:5], "fontes": fontes or ["system-ui"],
            "dark": bool(bg is not None and bg < 0.2),
            "kind": de.get("kind", ""), "moods": de.get("moods", []),
            "industries": de.get("industries", []), "surfaces": de.get("surfaces", []),
            "_root": root, "_md_path": os.path.join(p, "DESIGN.md"),
            "_components": os.path.join(p, "components.html") if os.path.exists(os.path.join(p, "components.html")) else "",
        }
    return out


def _is_stub(body):
    return "install the upstream" in body and "## Workflow" not in body


def _load_recipes(base, craft_req):
    out = {}
    for sub, src in (("design-templates", "template"), ("skills", "skill")):
        d = os.path.join(base, sub)
        if not os.path.isdir(d):
            continue
        for rid in sorted(os.listdir(d)):
            path = os.path.join(d, rid, "SKILL.md")
            txt = _read(path)
            if not txt:
                continue
            meta, body = _frontmatter(txt)
            if _is_stub(body) or rid in out:
                continue
            od = meta.get("od") if isinstance(meta.get("od"), dict) else {}
            craft = list(((od.get("craft") or {}).get("requires") or [])) if isinstance(od.get("craft"), dict) else []
            craft += [c for c in (craft_req.get(rid) or []) if c not in craft]
            desc = meta.get("description") or ""
            desc = re.sub(r"\s+", " ", str(desc)).strip()
            out[rid] = {"id": rid, "nome": meta.get("en_name") or rid.replace("-", " ").title(),
                        "desc": desc[:240], "mode": od.get("mode", ""), "fonte": src,
                        "ds_requer": bool((od.get("design_system") or {}).get("requires")) if isinstance(od.get("design_system"), dict) else False,
                        "craft": craft, "_path": path}
    return out


def _load_examples(base):
    out = {}
    d = os.path.join(base, "examples")
    if not os.path.isdir(d):
        return out
    for eid in sorted(os.listdir(d)):
        p = os.path.join(d, eid)
        html = os.path.join(p, "example.html")
        if not os.path.exists(html):
            continue
        man = _json(os.path.join(p, "open-design.json"), {}) or {}
        meta, _body = _frontmatter(_read(os.path.join(p, "SKILL.md"), 6000))
        od = man.get("od") or {}
        ti = man.get("title_i18n") or {}
        di = man.get("description_i18n") or {}
        q = ((od.get("useCase") or {}).get("query") or {})
        odm = meta.get("od") if isinstance(meta.get("od"), dict) else {}
        out[eid] = {
            "id": eid,
            "nome": ti.get("pt-BR") or man.get("title") or meta.get("en_name") or eid,
            "desc": (di.get("pt-BR") or man.get("description") or str(meta.get("description") or ""))[:260],
            "mode": od.get("mode") or odm.get("mode") or "",
            "surface": od.get("surface") or odm.get("surface") or "",
            "scenario": od.get("scenario") or odm.get("scenario") or "",
            "tags": [t for t in (man.get("tags") or []) if t not in ("example", "first-party")][:8],
            "prompt": q.get("pt-BR") or q.get("en") or str(odm.get("example_prompt") or ""),
            "kb": round(os.path.getsize(html) / 1024),
            "_html": html, "_skill": os.path.join(p, "SKILL.md"),
        }
    return out


def _load():
    base = ODKIT_DIR
    if not os.path.isdir(base):
        return {"ok": False, "base": base}
    descr = _json(os.path.join(base, "design-system-descriptors.json"), {}) or {}
    craft_req = {k: v for k, v in (_json(os.path.join(base, "craft-requires.json"), {}) or {}).items() if not k.startswith("$")}
    surfaces = [s for s in ((_json(os.path.join(base, "surfaces.json"), {}) or {}).get("surfaces") or []) if s.get("status") == "ready"]
    craft = {}
    cd = os.path.join(base, "craft")
    if os.path.isdir(cd):
        for f in sorted(os.listdir(cd)):
            if f.endswith(".md"):
                craft[f[:-3]] = os.path.join(cd, f)
    return {
        "ok": True, "base": base,
        "manifest": _json(os.path.join(base, "MANIFEST.json"), {}) or {},
        "sistemas": _load_design_systems(base, descr),
        "receitas": _load_recipes(base, craft_req),
        "exemplos": _load_examples(base),
        "craft": craft, "surfaces": surfaces,
        "prompts": {f[:-3]: os.path.join(base, "prompts", f) for f in (os.listdir(os.path.join(base, "prompts")) if os.path.isdir(os.path.join(base, "prompts")) else []) if f.endswith(".md")},
    }


def kit():
    global _CACHE
    if _CACHE is None:
        with _LOCK:
            if _CACHE is None:
                try:
                    _CACHE = _load()
                except Exception as e:  # nunca derruba o app
                    print("[cerne] odkit: falha ao carregar o catálogo:", str(e)[:200])
                    _CACHE = {"ok": False, "base": ODKIT_DIR, "erro": str(e)[:200]}
    return _CACHE


def recarregar():
    global _CACHE
    with _LOCK:
        _CACHE = None
    return kit()


def disponivel():
    return bool(kit().get("ok"))


def _pub(d):
    return {k: v for k, v in d.items() if not k.startswith("_")}


# ───────────────────────────── API pública ─────────────────────────────

def info():
    k = kit()
    if not k.get("ok"):
        return {"ok": False, "base": k.get("base"), "erro": k.get("erro", "conteúdo do kit não encontrado")}
    man = k.get("manifest") or {}
    return {"ok": True, "origem": man.get("sourceRepo", ""), "versao": man.get("sourceRef", ""),
            "sincronizado": man.get("syncedAt", ""),
            "contagem": {"sistemas": len(k["sistemas"]), "receitas": len(k["receitas"]),
                         "exemplos": len(k["exemplos"]), "craft": len(k["craft"])}}


def sistemas():
    return [_pub(s) for s in kit().get("sistemas", {}).values()]


def sistema(sid):
    sid = (sid or "").removeprefix("od:")
    return kit().get("sistemas", {}).get(sid)


def sistema_contrato(sid, rico=True):
    """Contrato do design system p/ o prompt: bloco :root (tokens) + DESIGN.md (resumido)."""
    s = sistema(sid)
    if not s:
        return ""
    md = _read(s["_md_path"])
    md = re.sub(r"^#\s+.*\n", "", md, count=1)
    return ("\nDESIGN SYSTEM OPEN DESIGN — " + s["nome"] + " (" + s["categoria"] + "). Siga-o como fonte de verdade.\n"
            "1) Cole este bloco :root LITERALMENTE no primeiro <style> do HTML e use as cores/fontes/raios SEMPRE via var(--nome) "
            "— nunca reescreva os hex soltos fora do :root:\n" + s["_root"] + "\n"
            "2) DESIGN.md (atmosfera, tipografia, componentes, do/don't):\n" + _trim(md, 9000 if rico else 2400))


def sistema_components(sid):
    s = sistema(sid)
    return _read(s["_components"]) if s and s.get("_components") else ""


def receitas(tipo=""):
    k = kit()
    rec = k.get("receitas", {})
    if not tipo:
        return [_pub(r) for r in rec.values()]
    tm = TIPO_MAP.get(tipo) or {}
    ids = []
    if tm.get("receita"):
        ids.append(tm["receita"])
    for s in k.get("surfaces", []):
        if s.get("id") in tm.get("surfaces", []):
            ids += s.get("entries") or []
    seen, out = set(), []
    for i in ids:
        if i in rec and i not in seen:
            seen.add(i)
            out.append(_pub(rec[i]))
    return out


def receita_padrao(tipo):
    r = (TIPO_MAP.get(tipo) or {}).get("receita") or ""
    return r if r in kit().get("receitas", {}) else ""


def receita_txt(rid, rico=True):
    r = kit().get("receitas", {}).get(rid or "")
    if not r:
        return ""
    _m, body = _frontmatter(_read(r["_path"]))
    return "\nRECEITA OPEN DESIGN «" + r["id"] + "» (siga o workflow e a checklist):\n" + _trim(body, 7000 if rico else 1800)


def craft_ids(rid=""):
    r = kit().get("receitas", {}).get(rid or "") or {}
    ids = list(CRAFT_BASE)
    for c in r.get("craft") or []:
        if c not in ids:
            ids.append(c)
    return [c for c in ids if c in kit().get("craft", {})]


def craft_txt(ids, rico=True):
    k = kit().get("craft", {})
    budget = 14000 if rico else 3200
    partes = []
    for i, cid in enumerate(ids):
        if cid not in k or budget <= 400:
            continue
        lim = min(budget, 5000 if rico else (2400 if i == 0 else 800))
        t = _trim(_read(k[cid]), lim)
        budget -= len(t)
        partes.append(f"### craft/{cid}\n{t}")
    return ("\nREGRAS DE CRAFT (Open Design) — obrigatórias, o resultado é checado contra elas:\n" + "\n\n".join(partes)) if partes else ""


def exemplos(q="", tipo="", limite=500):
    ex = list(kit().get("exemplos", {}).values())
    q = (q or "").lower().strip()
    if q:
        ex = [e for e in ex if q in (e["nome"] + " " + e["desc"] + " " + e["id"] + " " + " ".join(e["tags"])).lower()]
    if tipo:
        tm = TIPO_MAP.get(tipo) or {}
        alvo = set()
        for s in kit().get("surfaces", []):
            if s.get("id") in tm.get("surfaces", []):
                alvo |= set(s.get("entries") or [])
        modo = {"deck": "deck", "prototype": "prototype", "dashboard": "prototype"}.get(tipo)
        pri = [e for e in ex if e["id"] in alvo]
        resto = [e for e in ex if e["id"] not in alvo and (not modo or e["mode"] == modo)]
        ex = pri + resto
    return [_pub(e) for e in ex[:limite]]


def exemplo(eid):
    return kit().get("exemplos", {}).get(eid or "")


def exemplo_html(eid):
    e = exemplo(eid)
    return _read(e["_html"]) if e else ""


def exemplo_skill(eid, rico=True):
    e = exemplo(eid)
    if not e:
        return ""
    _m, body = _frontmatter(_read(e["_skill"]))
    return _trim(body, 4000 if rico else 1200)


def prompt_txt(nome, n=6000):
    p = kit().get("prompts", {}).get(nome)
    return _trim(_read(p), n) if p else ""


# ───────────────────────────── recomendação ─────────────────────────────

def _tem(t, p):
    """Palavra-chave com fronteira no INÍCIO da palavra ('arte' não casa 'parte'; 'criativ' casa 'criativo')."""
    if p != p.strip():
        return p in t
    return re.search(r"(?<!\w)" + re.escape(p), t) is not None


def recomendar(texto, tipo="", n=5):
    """Ranqueia design systems para um briefing (determinístico). Marca citada pelo nome vem primeiro."""
    ss = list(kit().get("sistemas", {}).values())
    if not ss:
        return []
    t = " " + (texto or "").lower() + " "
    want = {"moods": [], "industries": []}
    for grupo, mp in _KW.items():
        for alvo, palavras in mp.items():
            if any(_tem(t, p) for p in palavras):
                want[grupo].append(alvo)
    surf = (TIPO_MAP.get(tipo) or {}).get("ds_surface", "")
    if any(w in t for w in (" escuro", " dark", "noturno", "preto")):
        want["dark"] = True
    if any(w in t for w in (" claro", " light", "branco")):
        want["dark"] = False
    out = []
    for s in ss:
        sc, why = 0.0, []
        nome = s["nome"].lower()
        if s["kind"] != "genre" and len(nome) > 2 and re.search(r"\b" + re.escape(nome) + r"\b", t):
            sc += 50; why.append("marca citada no briefing")
        elif s["kind"] == "brand":
            continue     # looks "inspirados em <marca>" só quando a marca é citada (regra do Open Design)
        ind = [i for i in s["industries"] if i in want["industries"]]
        mood = [m for m in s["moods"] if m in want["moods"]]
        sc += 3 * len(ind) + 2 * len(mood)
        if ind: why.append("setor: " + ", ".join(ind))
        if mood: why.append("tom: " + ", ".join(mood))
        if surf and surf in s["surfaces"]:
            sc += 1.5; why.append("feito p/ " + surf)
        if "dark" in want and s["dark"] == want["dark"]:
            sc += 1.5; why.append("tema " + ("escuro" if s["dark"] else "claro"))
        out.append((sc, s["slug"], s, why))
    out.sort(key=lambda x: (-x[0], x[1]))
    res = []
    for sc, _slug, s, why in out[:n]:
        p = _pub(s)
        p["score"] = round(sc, 1)
        p["motivo"] = "; ".join(why) or "boa escolha neutra para o tipo"
        res.append(p)
    return res


# ───────────────────────────── checagem (lint) ─────────────────────────────

def _strip_tags(s):
    return re.sub(r"<[^>]+>", " ", s or "")


def checar(html, sistema_id="", tipo=""):
    """Lint determinístico inspirado no lint-artifact do Open Design + craft/anti-ai-slop.
    Retorna {score, achados:[{sev, regra, msg, qtd}]}. P0 = deve corrigir, P1 = deveria, P2 = polimento."""
    h = html or ""
    low = h.lower()
    ach = []

    def add(sev, regra, msg, qtd=1):
        ach.append({"sev": sev, "regra": regra, "msg": msg, "qtd": qtd})

    s = sistema(sistema_id) if sistema_id else None
    pal = set(s["paleta"]) if s else set()

    # P0 — os "sete pecados" do anti-AI-slop
    indigo = sorted({x for x in (_hex_norm(m) for m in re.findall(r"#[0-9a-fA-F]{6}\b", h)) if x in _AI_INDIGO} - pal)
    if indigo:
        add("P0", "anti-ai-slop/indigo", "Índigo/violeta padrão do Tailwind como acento (" + ", ".join(indigo) + ") — a marca típica de IA. Use o acento do design system.", len(indigo))
    texto = _strip_tags(re.sub(r"(?is)<(script|style)[^>]*>.*?</\1>", " ", h))
    fil = _FILLER.findall(texto)
    if fil:
        add("P0", "anti-ai-slop/filler", "Texto de enchimento (lorem ipsum / 'feature one' / placeholder).", len(fil))
    emo = 0
    for m in re.finditer(r"(?is)<(h[1-6]|button|li)\b[^>]*>(.*?)</\1>", h):
        if _EMOJI.search(_strip_tags(m.group(2))):
            emo += 1
    if emo:
        add("P0", "anti-ai-slop/emoji-icones", "Emoji usado como ícone em títulos/botões/listas — troque por SVG monoline (stroke 1.6–1.8, currentColor).", emo)
    inv = _INVENTED.findall(texto)
    if inv:
        add("P1", "anti-ai-slop/metricas", "Métricas com cara de inventadas ('10x mais rápido', '99,9% uptime') — use dado real ou um rótulo de exemplo.", len(inv))
    grad = 0
    for g in re.findall(r"linear-gradient\(([^)]*)\)", h, re.I):
        hs = [_hue(x) for x in re.findall(r"#[0-9a-fA-F]{6}\b", g)]
        hs = [x for x in hs if x[0] is not None and x[1] > 0.35]
        if len(hs) >= 2 and all(180 <= x[0] <= 300 for x in hs[:2]) and abs(hs[0][0] - hs[1][0]) > 25:
            grad += 1
    if grad:
        add("P1", "anti-ai-slop/gradiente", "Gradiente de duas paradas roxo→azul/ciano (o 'gradiente de confiança' genérico). Prefira superfície chapada + tipografia intencional.", grad)

    # P1 — estrutura, acessibilidade e tokens
    if not re.search(r"<meta[^>]+name=[\"']viewport", low):
        add("P1", "responsivo/viewport", "Falta <meta name=\"viewport\"> — o layout não se adapta ao celular.")
    if not re.search(r"<html[^>]*\blang=", low):
        add("P1", "a11y/lang", "Falta o atributo lang no <html> (ex.: lang=\"pt-BR\").")
    if not re.search(r"<meta[^>]+charset", low):
        add("P1", "html/charset", "Falta <meta charset=\"utf-8\"> — acentos podem aparecer quebrados.")
    if "<title" not in low:
        add("P2", "a11y/title", "Falta <title> no documento.")
    sem_alt = len([m for m in re.findall(r"<img\b[^>]*>", h, re.I) if not re.search(r"\balt\s*=", m, re.I)])
    if sem_alt:
        add("P1", "a11y/alt", "Imagens sem texto alternativo (alt).", sem_alt)
    cdn = _PLACEHOLDER_CDN.findall(h)
    if cdn:
        add("P1", "anti-ai-slop/cdn-placeholder", "Imagens de CDN de placeholder (" + ", ".join(sorted(set(c if isinstance(c, str) else c[0] for c in cdn))) + ") — frágil e óbvio; use placeholder desenhado em CSS/SVG.", len(cdn))
    css = " ".join(re.findall(r"(?is)<style[^>]*>(.*?)</style>", h) + re.findall(r"(?i)\sstyle=\"([^\"]*)\"", h))
    css = re.sub(r"(?s):root\s*\{.*?\}", "", css)
    hexes = len(set(x.lower() for x in re.findall(r"#[0-9a-fA-F]{6}\b|#[0-9a-fA-F]{3}\b(?![0-9a-fA-F])", css)))
    if hexes > 12:
        add("P1", "tokens/hex-soltos", f"{hexes} cores hex distintas no CSS fora do :root — defina tokens (--accent, --fg, --bg…) e use var().", hexes)
    if s and "--accent" not in h:
        add("P1", "tokens/design-system", "O bloco :root do design system escolhido não foi aplicado (sem --accent).")
    ruins = []
    for blk in re.findall(r"\{([^{}]*)\}", h):
        cor = re.search(r"(?<![-\w])color\s*:\s*(#[0-9a-fA-F]{3,6})\b", blk)
        fundo = re.search(r"background(?:-color)?\s*:\s*(#[0-9a-fA-F]{3,6})\b", blk)
        if cor and fundo:
            c = contraste(cor.group(1), fundo.group(1))
            if c is not None and c < 4.5:
                ruins.append(f"{cor.group(1)} sobre {fundo.group(1)} ({c:.1f}:1)")
    if ruins:
        add("P1", "a11y/contraste", "Contraste abaixo de 4.5:1: " + "; ".join(ruins[:4]) + ("…" if len(ruins) > 4 else ""), len(ruins))
    fams = set()
    for m in re.findall(r"fonts\.googleapis\.com/css2?\?([^\"' >]+)", h):
        fams |= {f.split(":")[0].replace("+", " ") for f in re.findall(r"family=([^&]+)", m)}
    if len(fams) > 3:
        add("P1", "tipografia/familias", f"{len(fams)} famílias de fonte ({', '.join(sorted(fams))}) — limite a 2–3.", len(fams))

    # P2 — polimento
    if re.search(r"<(button|a|input)\b", low) and ":focus" not in low:
        add("P2", "a11y/foco", "Sem estilo de foco (:focus-visible) para navegação por teclado.")
    if re.search(r"@keyframes|animation\s*:|transition\s*:", low) and "prefers-reduced-motion" not in low:
        add("P2", "movimento/reduced-motion", "Há animações mas falta @media (prefers-reduced-motion: reduce).")
    if len(h) > 300_000:
        add("P2", "performance/peso", f"HTML com {len(h)//1024} KB — considere enxugar.")
    if tipo == "email" and re.search(r"display\s*:\s*(flex|grid)", low):
        add("P1", "email/layout", "E-mail usando flex/grid — muitos clientes de e-mail ignoram; use tabelas.")

    peso = {"P0": 15, "P1": 6, "P2": 2}
    score = max(0, 100 - sum(peso[a["sev"]] for a in ach))
    ordem = {"P0": 0, "P1": 1, "P2": 2}
    ach.sort(key=lambda a: ordem[a["sev"]])
    return {"score": score, "achados": ach,
            "resumo": {sv: len([a for a in ach if a["sev"] == sv]) for sv in ("P0", "P1", "P2")}}


def instrucao_correcao(resultado):
    """Converte os achados da checagem numa instrução de iteração para o modelo."""
    itens = [a for a in (resultado or {}).get("achados", []) if a["sev"] in ("P0", "P1")] or (resultado or {}).get("achados", [])
    if not itens:
        return ""
    linhas = "\n".join(f"- [{a['sev']}] {a['msg']}" for a in itens[:12])
    return ("Corrija TODOS os problemas abaixo apontados pela checagem de qualidade Open Design, sem mudar o conteúdo, "
            "a estrutura geral nem a identidade visual além do necessário:\n" + linhas)

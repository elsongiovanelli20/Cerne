// Cerne — Copiloto do Navegador (service worker, MV3)
// Conecta-se ao SEU servidor Cerne (soberano) por long-poll, executa comandos no navegador que você já usa
// e mostra uma moldura enquanto atua. Sem token pareado, fica ocioso (não faz nada).

importScripts("linkedin.js");   // automação do LinkedIn (aba própria em segundo plano)

const DEFAULTS = { base: "https://cerne.orqion-ia.com.br", token: "", ligado: true };
let cfg = { ...DEFAULTS };
let polling = false;
let controlledTabId = null;

async function loadCfg() {
  const s = await chrome.storage.local.get(["base", "token", "ligado"]);
  cfg = { base: (s.base || DEFAULTS.base).replace(/\/+$/, ""), token: s.token || "", ligado: s.ligado !== false };
  return cfg;
}
chrome.storage.onChanged.addListener(async (ch, area) => {
  if (area === "local") { await loadCfg(); if (cfg.token && cfg.ligado) kick(); setBadge(); }
});

function setBadge() {
  const on = cfg.token && cfg.ligado;
  chrome.action.setBadgeBackgroundColor({ color: on ? "#38d6d6" : "#555" });
  chrome.action.setBadgeText({ text: on ? "on" : "" });
}

// ---- moldura (overlay) do Cerne na aba ativa ----
async function overlay(tabId, on, label) {
  if (tabId == null) return;
  try {
    await chrome.tabs.sendMessage(tabId, { cerne: true, tipo: "overlay", on, label: label || "Cerne está operando este navegador" });
  } catch (e) {
    // content script pode não ter carregado ainda (ex.: recém-navegado): injeta e tenta de novo
    try {
      await chrome.scripting.executeScript({ target: { tabId }, files: ["content.js"] });
      await chrome.tabs.sendMessage(tabId, { cerne: true, tipo: "overlay", on, label: label || "Cerne está operando este navegador" });
    } catch (_) {}
  }
}

async function abaAlvo() {
  if (controlledTabId != null) {
    try { const t = await chrome.tabs.get(controlledTabId); if (t) return t; } catch (_) { controlledTabId = null; }
  }
  const [t] = await chrome.tabs.query({ active: true, lastFocusedWindow: true });
  if (t) controlledTabId = t.id;
  return t || null;
}

function esperarCarregar(tabId, timeout = 25000) {
  return new Promise((resolve) => {
    let done = false;
    const fim = (v) => { if (!done) { done = true; chrome.tabs.onUpdated.removeListener(li); resolve(v); } };
    const li = (id, info) => { if (id === tabId && info.status === "complete") fim(true); };
    chrome.tabs.onUpdated.addListener(li);
    setTimeout(() => fim(false), timeout);
    chrome.tabs.get(tabId).then((t) => { if (t && t.status === "complete") fim(true); }).catch(() => {});
  });
}

// funções injetadas na página (rodam no contexto do DOM)
function _lerPagina(seletor) {
  try {
    const el = seletor ? document.querySelector(seletor) : document.body;
    if (!el) return { erro: "elemento não encontrado: " + seletor };
    const txt = (el.innerText || el.textContent || "").replace(/\n{3,}/g, "\n\n").trim();
    return { titulo: document.title, url: location.href, texto: txt.slice(0, 12000), tamanho: txt.length };
  } catch (e) { return { erro: String(e).slice(0, 200) }; }
}
function _clicar(seletor) {
  try { const el = document.querySelector(seletor); if (!el) return { erro: "não achei: " + seletor }; el.scrollIntoView({ block: "center" }); el.click(); return { ok: true, clicado: seletor }; }
  catch (e) { return { erro: String(e).slice(0, 200) }; }
}
function _preencher(seletor, valor) {
  try {
    const el = document.querySelector(seletor); if (!el) return { erro: "não achei: " + seletor };
    el.scrollIntoView({ block: "center" }); el.focus();
    if (el.isContentEditable) { el.textContent = valor; }
    else {
      // setter NATIVO: React/Vue/Angular só enxergam a mudança assim (el.value = x é ignorado pelo React)
      const proto = el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : el instanceof HTMLSelectElement ? HTMLSelectElement.prototype : HTMLInputElement.prototype;
      const d = Object.getOwnPropertyDescriptor(proto, "value");
      if (d && d.set) d.set.call(el, valor); else el.value = valor;
    }
    el.dispatchEvent(new Event("input", { bubbles: true })); el.dispatchEvent(new Event("change", { bubbles: true }));
    return { ok: true, preenchido: seletor };
  } catch (e) { return { erro: String(e).slice(0, 200) }; }
}
// MAPA: lista os elementos interativos VISÍVEIS com um seletor estável (data-cerne) + rótulo humano,
// para o agente saber o que pode clicar/preencher sem adivinhar CSS.
function _mapa(comTexto) {
  try {
    const vis = (el) => { const r = el.getBoundingClientRect(); const s = getComputedStyle(el); return r.width > 0 && r.height > 0 && s.visibility !== "hidden" && s.display !== "none"; };
    const rotulo = (el) => {
      let t = el.getAttribute("aria-label") || "";
      if (!t && el.id) { const l = document.querySelector('label[for="' + CSS.escape(el.id) + '"]'); if (l) t = l.innerText; }
      if (!t && el.closest("label")) t = el.closest("label").innerText;
      t = t || el.getAttribute("placeholder") || el.getAttribute("title") || el.getAttribute("name") || el.innerText || el.value || el.getAttribute("alt") || "";
      return String(t).replace(/\s+/g, " ").trim().slice(0, 90);
    };
    const els = [...document.querySelectorAll('a[href], button, input:not([type=hidden]), textarea, select, [role=button], [role=link], [role=tab], [role=menuitem], [role=checkbox], [contenteditable=true], [onclick]')].filter(vis).slice(0, 150);
    let n = 0;
    const out = els.map((el) => {
      let id = el.getAttribute("data-cerne"); if (!id) { id = "c" + (++n) + Math.random().toString(36).slice(2, 5); el.setAttribute("data-cerne", id); }
      const tag = el.tagName.toLowerCase();
      const tipo = tag === "input" ? "campo:" + (el.type || "text") : tag === "a" ? "link" : tag === "select" ? "lista:" + [...el.options].slice(0, 8).map(o => o.text.trim()).join("|") : tag === "textarea" ? "campo:texto-longo" : (el.getAttribute("role") || tag);
      const valor = (tag === "input" || tag === "textarea") && el.type !== "password" ? (el.value || "") : (tag === "select" ? (el.selectedOptions[0] || {}).text || "" : "");
      return { sel: '[data-cerne="' + id + '"]', tipo, rotulo: rotulo(el), valor };
    });
    const r = { titulo: document.title, url: location.href, elementos: out };
    if (comTexto) r.texto = (document.body.innerText || "").replace(/\n{3,}/g, "\n\n").trim().slice(0, 1500);
    return r;
  } catch (e) { return { erro: String(e).slice(0, 200) }; }
}
function _clicarTexto(texto) {
  try {
    const alvo = String(texto || "").toLowerCase().trim(); if (!alvo) return { erro: "faltou o texto" };
    const cands = [...document.querySelectorAll('button, a, [role=button], [role=link], [role=tab], [role=menuitem], input[type=submit], input[type=button], label, summary')];
    const txt = (el) => (el.innerText || el.value || el.getAttribute("aria-label") || "").replace(/\s+/g, " ").trim().toLowerCase();
    const el = cands.find(e => txt(e) === alvo) || cands.find(e => txt(e).includes(alvo));
    if (!el) return { erro: "não achei elemento com o texto: " + texto };
    el.scrollIntoView({ block: "center" }); el.click(); return { ok: true, clicado: txt(el).slice(0, 80) };
  } catch (e) { return { erro: String(e).slice(0, 200) }; }
}
function _selecionar(seletor, valor) {
  try {
    const el = document.querySelector(seletor); if (!el) return { erro: "não achei: " + seletor };
    if (el.tagName !== "SELECT") return { erro: "o elemento não é uma lista (select) — use clicar/clicar_texto" };
    const v = String(valor || "").toLowerCase().trim();
    const op = [...el.options].find(o => o.value.toLowerCase() === v || o.text.trim().toLowerCase() === v) || [...el.options].find(o => o.text.toLowerCase().includes(v));
    if (!op) return { erro: "opção não encontrada: " + valor + " (há: " + [...el.options].slice(0, 15).map(o => o.text.trim()).join(" | ") + ")" };
    el.value = op.value; el.dispatchEvent(new Event("input", { bubbles: true })); el.dispatchEvent(new Event("change", { bubbles: true }));
    return { ok: true, selecionado: op.text.trim() };
  } catch (e) { return { erro: String(e).slice(0, 200) }; }
}
function _tecla(seletor, tecla) {
  try {
    const el = (seletor && document.querySelector(seletor)) || document.activeElement || document.body;
    const k = String(tecla || "Enter"); const code = { Enter: 13, Tab: 9, Escape: 27, ArrowDown: 40, ArrowUp: 38, Backspace: 8, " ": 32 }[k] || 0;
    el.focus && el.focus();
    for (const tipo of ["keydown", "keypress", "keyup"]) el.dispatchEvent(new KeyboardEvent(tipo, { key: k, code: k, keyCode: code, which: code, bubbles: true, cancelable: true }));
    if (k === "Enter" && el.form && el.tagName === "INPUT") { if (el.form.requestSubmit) el.form.requestSubmit(); else el.form.submit(); }
    return { ok: true, tecla: k };
  } catch (e) { return { erro: String(e).slice(0, 200) }; }
}
function _rolar(direcao) { window.scrollBy({ top: (direcao === "cima" ? -1 : 1) * Math.round(window.innerHeight * 0.85), behavior: "instant" }); return { ok: true, y: Math.round(window.scrollY) }; }
function _existe(seletor) { try { return !!document.querySelector(seletor); } catch (e) { return false; } }
function _acharLinks(termo) {
  try {
    const as = [...document.querySelectorAll("a[href]")];
    const f = as.filter(a => !termo || (a.innerText || "").toLowerCase().includes(String(termo).toLowerCase()) || a.href.toLowerCase().includes(String(termo).toLowerCase()));
    return { links: f.slice(0, 30).map(a => ({ texto: (a.innerText || "").trim().slice(0, 80), href: a.href })) };
  } catch (e) { return { erro: String(e).slice(0, 200) }; }
}

async function injetar(tabId, fn, args) {
  const r = await chrome.scripting.executeScript({ target: { tabId }, func: fn, args: args || [] });
  return (r && r[0] && r[0].result) || null;
}

// ---- executa um comando vindo do Cerne ----
async function executar(cmd) {
  const acao = (cmd.acao || "").toLowerCase();
  if (acao.startsWith("li_")) return await liExecutar(cmd);   // LinkedIn: não mexe na aba que você está usando
  const t = await abaAlvo();
  const tabId = t ? t.id : null;
  if (acao !== "status" && tabId != null) overlay(tabId, true, cmd.rotulo || "Cerne está operando este navegador");
  try {
    if (acao === "abrir") {
      if (!cmd.url) return { erro: "faltou a url" };
      let url = cmd.url; if (!/^https?:\/\//i.test(url)) url = "https://" + url;
      if (tabId == null) { const nt = await chrome.tabs.create({ url }); controlledTabId = nt.id; await esperarCarregar(nt.id); overlay(nt.id, true, cmd.rotulo); return (await injetar(nt.id, _mapa, [true])) || { ok: true, url }; }
      await chrome.tabs.update(tabId, { url }); await esperarCarregar(tabId);
      overlay(tabId, true, cmd.rotulo);
      return (await injetar(tabId, _mapa, [true])) || { ok: true, url };   // já devolve o mapa: economiza um passo
    }
    if (tabId == null && ["mapa", "clicar_texto", "selecionar", "tecla", "rolar", "esperar_elemento", "voltar"].includes(acao)) return { erro: "nenhuma aba" };
    if (acao === "mapa") return await injetar(tabId, _mapa, [true]);
    if (acao === "clicar_texto") { const r = await injetar(tabId, _clicarTexto, [cmd.texto || cmd.valor || ""]); await esperarCarregar(tabId, 6000); return r; }
    if (acao === "selecionar") return await injetar(tabId, _selecionar, [cmd.seletor, cmd.valor || cmd.texto || ""]);
    if (acao === "tecla") { const r = await injetar(tabId, _tecla, [cmd.seletor || null, cmd.tecla || cmd.valor || "Enter"]); await esperarCarregar(tabId, 5000); return r; }
    if (acao === "rolar") return await injetar(tabId, _rolar, [cmd.direcao || "baixo"]);
    if (acao === "voltar") { await chrome.tabs.goBack(tabId); await esperarCarregar(tabId); return await injetar(tabId, _mapa, [true]); }
    if (acao === "esperar_elemento") {
      const fim = Date.now() + Math.min(cmd.ms || 10000, 30000);
      while (Date.now() < fim) { if (await injetar(tabId, _existe, [cmd.seletor || "body"])) return { ok: true, apareceu: cmd.seletor }; await new Promise(r => setTimeout(r, 500)); }
      return { erro: "o elemento não apareceu a tempo: " + cmd.seletor };
    }
    if (acao === "ler") { if (tabId == null) return { erro: "nenhuma aba" }; return await injetar(tabId, _lerPagina, [cmd.seletor || null]); }
    if (acao === "clicar") { if (tabId == null) return { erro: "nenhuma aba" }; const r = await injetar(tabId, _clicar, [cmd.seletor]); await esperarCarregar(tabId, 6000); return r; }
    if (acao === "preencher") { if (tabId == null) return { erro: "nenhuma aba" }; return await injetar(tabId, _preencher, [cmd.seletor, cmd.valor || ""]); }
    if (acao === "links") { if (tabId == null) return { erro: "nenhuma aba" }; return await injetar(tabId, _acharLinks, [cmd.termo || ""]); }
    if (acao === "baixar") {
      let url = cmd.url; if (!url && t) url = t.url;
      if (!url) return { erro: "faltou a url do download" };
      const id = await chrome.downloads.download({ url, saveAs: false });
      return { ok: true, download_id: id, url };
    }
    if (acao === "screenshot") {
      try { const data = await chrome.tabs.captureVisibleTab({ format: "png" }); return { ok: true, bytes: data.length, imagem_dataurl: data.slice(0, 300000) }; }
      catch (e) { return { erro: "screenshot falhou: " + String(e).slice(0, 120) }; }
    }
    if (acao === "esperar") { await new Promise(r => setTimeout(r, Math.min(cmd.ms || 1000, 15000))); return { ok: true }; }
    if (acao === "status") { return { ok: true, url: t ? t.url : null, titulo: t ? t.title : null, navegador: navigator.userAgent }; }
    return { erro: "ação desconhecida: " + acao };
  } finally {
    if (tabId != null) setTimeout(() => overlay(tabId, false), 4500);   // esmaece a moldura depois de agir
  }
}

// ---- laço de long-poll com o Cerne ----
async function pollLoop() {
  if (polling) return; polling = true;
  try {
    while (cfg.ligado && cfg.token) {
      let cmd = null;
      try {
        const r = await fetch(cfg.base + "/ext/poll", {
          method: "POST", headers: { "Content-Type": "application/json", "X-Ext-Token": cfg.token },
          body: JSON.stringify({ v: "1.2.0" })
        });
        if (r.status === 401) { await chrome.storage.local.set({ token: "" }); break; }
        if (r.ok) { const j = await r.json(); cmd = j && j.comando ? j.comando : null; }
        else { await new Promise(s => setTimeout(s, 3000)); }
      } catch (e) { await new Promise(s => setTimeout(s, 3000)); continue; }
      if (cmd) {
        let res;
        try { res = await executar(cmd); } catch (e) { res = { erro: String(e).slice(0, 200) }; }
        try {
          await fetch(cfg.base + "/ext/result", {
            method: "POST", headers: { "Content-Type": "application/json", "X-Ext-Token": cfg.token },
            body: JSON.stringify({ id: cmd.id, resultado: res })
          });
        } catch (_) {}
      }
    }
  } finally { polling = false; }
}

function kick() { if (!polling) pollLoop(); }

chrome.runtime.onInstalled.addListener(async () => { await loadCfg(); setBadge(); kick(); });
chrome.runtime.onStartup.addListener(async () => { await loadCfg(); setBadge(); kick(); });
chrome.alarms.create("cerne-keepalive", { periodInMinutes: 0.4 });
chrome.alarms.onAlarm.addListener(async () => { await loadCfg(); setBadge(); kick(); });

// mensagens do popup (pareamento/teste)
chrome.runtime.onMessage.addListener((msg, sender, reply) => {
  if (msg && msg.tipo === "cerne-ping") { loadCfg().then(() => { kick(); reply({ ok: true, cfg: { base: cfg.base, pareado: !!cfg.token, ligado: cfg.ligado } }); }); return true; }
  return false;
});

loadCfg().then(() => { setBadge(); kick(); });

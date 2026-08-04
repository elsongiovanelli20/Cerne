// Cerne — Copiloto do Navegador (service worker, MV3)
// Conecta-se ao SEU servidor Cerne (soberano) por long-poll, executa comandos no navegador que você já usa
// e mostra uma moldura enquanto atua. Sem token pareado, fica ocioso (não faz nada).

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
    el.focus(); el.value = valor;
    el.dispatchEvent(new Event("input", { bubbles: true })); el.dispatchEvent(new Event("change", { bubbles: true }));
    return { ok: true, preenchido: seletor };
  } catch (e) { return { erro: String(e).slice(0, 200) }; }
}
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
  const t = await abaAlvo();
  const tabId = t ? t.id : null;
  if (acao !== "status" && tabId != null) overlay(tabId, true, cmd.rotulo || "Cerne está operando este navegador");
  try {
    if (acao === "abrir") {
      if (!cmd.url) return { erro: "faltou a url" };
      let url = cmd.url; if (!/^https?:\/\//i.test(url)) url = "https://" + url;
      if (tabId == null) { const nt = await chrome.tabs.create({ url }); controlledTabId = nt.id; await esperarCarregar(nt.id); overlay(nt.id, true, cmd.rotulo); const info = await injetar(nt.id, _lerPagina, [null]); return { ok: true, url: info && info.url, titulo: info && info.titulo }; }
      await chrome.tabs.update(tabId, { url }); await esperarCarregar(tabId);
      overlay(tabId, true, cmd.rotulo);
      const info = await injetar(tabId, _lerPagina, [null]);
      return { ok: true, url: (info && info.url) || url, titulo: info && info.titulo };
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
          body: JSON.stringify({ v: "1.0.0" })
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

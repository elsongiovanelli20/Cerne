// Cerne — automação do LinkedIn (carregado pelo background.js via importScripts).
// Tudo roda numa ABA PRÓPRIA, fixada e em segundo plano, na sessão do LinkedIn que você já tem logada.
// O Cerne nunca vê sua senha nem seus cookies: ele só manda o comando ("conectar com fulano") e recebe o resultado.
// As esperas ficam aqui no service worker (as abas ocultas têm timers estrangulados pelo Chrome); as funções
// injetadas na página são SÍNCRONAS e de um passo só (ler / clicar / preencher).

const LI_HOME = "https://www.linkedin.com/feed/";
let liTabId = null;

const liSleep = (ms) => new Promise((r) => setTimeout(r, ms));
const liRnd = (a, b) => Math.floor(a + Math.random() * (b - a));

async function liTab() {
  if (liTabId == null) {
    try { const s = await chrome.storage.session.get("liTabId"); liTabId = s.liTabId ?? null; } catch (_) {}
  }
  if (liTabId != null) {
    try { const t = await chrome.tabs.get(liTabId); if (t) return t; } catch (_) { liTabId = null; }
  }
  const t = await chrome.tabs.create({ url: LI_HOME, active: false, pinned: true });
  liTabId = t.id;
  try { await chrome.storage.session.set({ liTabId }); } catch (_) {}
  await esperarCarregar(t.id, 30000);
  await liSleep(2500);
  return t;
}

async function liGo(url, extraWait) {
  const t = await liTab();
  await chrome.tabs.update(t.id, { url });
  await esperarCarregar(t.id, 35000);
  await liSleep(extraWait || liRnd(1800, 3200));
  return t.id;
}

async function liDom(tabId, op, a) {
  try {
    const r = await chrome.scripting.executeScript({ target: { tabId }, func: _liDom, args: [op, a || {}] });
    return (r && r[0] && r[0].result) || {};
  } catch (e) { return { erro: "falha ao acessar a página: " + String(e).slice(0, 140) }; }
}

// espera (no service worker) até a checagem na página devolver algo "verdadeiro" em `campo`
async function liEsperar(tabId, op, a, campo, timeout) {
  const fim = Date.now() + (timeout || 15000);
  let r = {};
  while (Date.now() < fim) {
    r = await liDom(tabId, op, a);
    if (r && (campo ? r[campo] : r.ok)) return r;
    await liSleep(600);
  }
  return r;
}

async function liChecaLogin(tabId) {
  const e = await liDom(tabId, "estado");
  if (e && e.logado === false) return { erro: "nao_logado: entre no LinkedIn neste navegador" };
  return null;
}

// ===================== ações =====================
async function liExecutar(cmd) {
  const acao = cmd.acao;
  try {
    if (acao === "li_status") {
      const tabId = await liGo(LI_HOME);
      const e = await liEsperar(tabId, "estado", {}, "pronto", 12000);
      if (e.logado === false) return { ok: true, logado: false };
      const me = await liEsperar(tabId, "eu", {}, "nome", 8000);
      return { ok: true, logado: true, nome: me.nome || "", url: me.url || "" };
    }
    if (acao === "li_visitar") return await liVisitar(cmd.url);
    if (acao === "li_conectar") return await liConectar(cmd.url, cmd.nota || "");
    if (acao === "li_mensagem") return await liMensagem(cmd.url, cmd.texto || "");
    if (acao === "li_curtir") return await liCurtir(cmd.url);
    if (acao === "li_extrair_busca") return await liExtrairBusca(cmd.url, cmd.max || 25);
    if (acao === "li_extrair_post") return await liExtrairPost(cmd.url, cmd.modo || "ambos", cmd.max || 100);
    if (acao === "li_conexoes") return await liConexoes(cmd.max || 80);
    if (acao === "li_conversas") return await liConversas(cmd.max || 40);
    if (acao === "li_meus_posts") return await liMeusPosts(cmd.max || 30);
    if (acao === "li_publicar") return await liPublicar(cmd.texto || "", cmd.midia || null);
    return { erro: "ação do LinkedIn desconhecida: " + acao };
  } catch (e) {
    return { erro: String(e && e.message || e).slice(0, 200) };
  }
}

async function liAbrirPerfil(url) {
  const tabId = await liGo(url);
  const lg = await liChecaLogin(tabId); if (lg) return { lg };
  const p = await liEsperar(tabId, "perfil", {}, "nome", 15000);
  if (!p.nome) return { lg: { erro: p.erro || "perfil não carregou (link inválido ou perfil indisponível)" } };
  return { tabId, perfil: p };
}

async function liVisitar(url) {
  const { tabId, perfil, lg } = await liAbrirPerfil(url); if (lg) return lg;
  for (let i = 0; i < 3; i++) { await liDom(tabId, "rolar", { px: liRnd(350, 700) }); await liSleep(liRnd(1200, 2600)); }
  return { ok: true, perfil };
}

async function liConectar(url, nota) {
  const { tabId, perfil, lg } = await liAbrirPerfil(url); if (lg) return lg;
  if (perfil.conexao === "conectado") return { ok: true, ja: "conectado", perfil };
  if (perfil.conexao === "pendente") return { ok: true, ja: "pendente", perfil };
  let r = await liDom(tabId, "clicar_conectar");
  if (r.menu) { await liSleep(900); r = await liDom(tabId, "clicar_menu_conectar"); }
  if (r.ja) return { ok: true, ja: r.ja, perfil };
  if (!r.clicou) return { erro: r.erro || "não encontrei o botão Conectar neste perfil", perfil };
  let m = await liEsperar(tabId, "modal", {}, "aberto", 8000);
  if (m.limite) return { erro: "limite_semanal: o LinkedIn avisou o limite de convites", perfil };
  if (m.email) { await liDom(tabId, "fechar_dialogo"); return { erro: "o LinkedIn pede o e-mail da pessoa para conectar", perfil }; }
  if (!m.aberto) {                                           // alguns perfis enviam direto, sem modal
    const p2 = await liDom(tabId, "perfil");
    if (p2.conexao === "pendente") return { ok: true, perfil };
    return { erro: "o convite não abriu a janela de envio", perfil };
  }
  let semNota = !nota;
  if (nota) {
    if (m.btnNota) { await liDom(tabId, "clicar_no_dialogo", { re: "^(adicionar nota|add a note)$" }); await liSleep(900); }
    const t = await liEsperar(tabId, "modal", {}, "textarea", 5000);
    if (t.textarea) {
      await liDom(tabId, "preencher_nota", { texto: nota });
      await liSleep(liRnd(900, 1600));
      const e = await liDom(tabId, "clicar_no_dialogo", { re: "^(enviar|send|enviar convite|send invitation|enviar agora|send now)$" });
      if (!e.clicou) return { erro: "não achei o botão Enviar do convite", perfil };
    } else {
      // conta gratuita sem notas disponíveis (upsell do Premium): manda sem nota
      await liDom(tabId, "fechar_upsell");
      await liSleep(800);
      semNota = true;
      m = await liEsperar(tabId, "modal", {}, "aberto", 4000);
      if (!m.aberto) {
        let r2 = await liDom(tabId, "clicar_conectar"); if (r2.menu) { await liSleep(800); r2 = await liDom(tabId, "clicar_menu_conectar"); }
        await liEsperar(tabId, "modal", {}, "aberto", 6000);
      }
    }
  }
  if (semNota) {
    const e = await liDom(tabId, "clicar_no_dialogo", { re: "^(enviar sem nota|send without a note|enviar|send)$" });
    if (!e.clicou) return { erro: "não achei o botão de envio do convite", perfil };
  }
  await liSleep(1800);
  const fim = await liDom(tabId, "modal");
  if (fim.limite) return { erro: "limite_semanal: o LinkedIn avisou o limite de convites", perfil, contou: true };
  if (fim.aberto) await liDom(tabId, "fechar_dialogo");
  return { ok: true, perfil, aviso: (nota && semNota) ? "enviado sem nota (o plano não permite notas agora)" : "" };
}

async function liMensagem(url, texto) {
  if (!texto.trim()) return { erro: "mensagem vazia" };
  const { tabId, perfil, lg } = await liAbrirPerfil(url); if (lg) return lg;
  await liDom(tabId, "fechar_conversas");
  const c = await liDom(tabId, "clicar_mensagem");
  if (!c.clicou) return { erro: "não achei o botão Mensagem (a pessoa é sua conexão?)", perfil };
  const ed = await liEsperar(tabId, "editor_msg", {}, "pronto", 10000);
  if (!ed.pronto) {
    await liDom(tabId, "fechar_dialogo");
    return { erro: ed.premium ? "nao_conectado: só dá para enviar InMail (Premium) — a pessoa não é conexão" : "a caixa de mensagem não abriu", perfil };
  }
  await liDom(tabId, "digitar_msg", { texto });
  await liSleep(liRnd(1200, 2200));
  const s = await liEsperar(tabId, "enviar_msg", {}, "clicou", 5000);
  if (!s.clicou) return { erro: "não consegui clicar em Enviar", perfil };
  const ok = await liEsperar(tabId, "msg_enviada", {}, "ok", 8000);
  await liSleep(800);
  await liDom(tabId, "fechar_conversas");
  return ok.ok ? { ok: true, perfil } : { erro: "a mensagem pode não ter sido enviada (o editor não esvaziou)", perfil };
}

async function liCurtir(url) {
  const base = url.replace(/\/+$/, "");
  const tabId = await liGo(base + "/recent-activity/all/");
  const lg = await liChecaLogin(tabId); if (lg) return lg;
  const r = await liEsperar(tabId, "curtir", {}, "fim", 12000);
  if (r.clicou) return { ok: true };
  if (r.semPosts) return { ok: true, ja: "sem_posts" };
  return { erro: r.erro || "não achei publicação para curtir" };
}

async function liExtrairBusca(url, max) {
  if (/\/sales\//.test(url)) return { erro: "Sales Navigator: a extração ainda não está disponível — use a busca clássica (LinkedIn Search)" };
  const pessoas = []; const vistos = new Set();
  let pagina = 1; let vazias = 0;
  while (pessoas.length < max && pagina <= 50) {
    const u = new URL(url); u.searchParams.set("page", String(pagina));
    const tabId = await liGo(u.toString());
    const lg = await liChecaLogin(tabId); if (lg) return pessoas.length ? { ok: true, pessoas, aviso: "parou: sessão do LinkedIn caiu" } : lg;
    await liEsperar(tabId, "busca", {}, "pronto", 15000);
    for (let i = 0; i < 4; i++) { await liDom(tabId, "rolar", { px: 900 }); await liSleep(700); }
    const r = await liDom(tabId, "busca");
    let novos = 0;
    for (const p of r.pessoas || []) { if (!vistos.has(p.url)) { vistos.add(p.url); pessoas.push(p); novos++; if (pessoas.length >= max) break; } }
    if (!novos) { vazias++; if (vazias >= 2 || r.fim) break; } else vazias = 0;
    if (r.fim) break;
    pagina++;
    await liSleep(liRnd(2500, 5500));
  }
  return { ok: true, pessoas };
}

async function liExtrairPost(url, modo, max) {
  const tabId = await liGo(url, 3500);
  const lg = await liChecaLogin(tabId); if (lg) return lg;
  await liEsperar(tabId, "post_pronto", {}, "pronto", 15000);
  const mapa = new Map();
  if (modo === "comentarios" || modo === "ambos") {
    for (let i = 0; i < 25 && mapa.size < max; i++) {
      const c = await liDom(tabId, "comentarios");
      for (const p of c.pessoas || []) if (!mapa.has(p.url)) mapa.set(p.url, p);
      const m = await liDom(tabId, "mais_comentarios");
      if (!m.clicou) break;
      await liSleep(liRnd(1500, 2600));
    }
  }
  if ((modo === "reacoes" || modo === "ambos") && mapa.size < max) {
    const a = await liDom(tabId, "abrir_reacoes");
    if (a.clicou) {
      await liEsperar(tabId, "reacoes", {}, "aberto", 8000);
      let semNovos = 0;
      for (let i = 0; i < 60 && mapa.size < max && semNovos < 4; i++) {
        const r = await liDom(tabId, "reacoes", { rolar: true });
        let novos = 0;
        for (const p of r.pessoas || []) if (!mapa.has(p.url)) { mapa.set(p.url, p); novos++; }
        semNovos = novos ? 0 : semNovos + 1;
        await liSleep(liRnd(900, 1600));
      }
      await liDom(tabId, "fechar_dialogo");
    }
  }
  const pessoas = [...mapa.values()].slice(0, max);
  return { ok: true, pessoas, aviso: pessoas.length ? "" : "nenhuma pessoa encontrada (confira se é o link da publicação original)" };
}

async function liConexoes(max) {
  const tabId = await liGo("https://www.linkedin.com/mynetwork/invite-connect/connections/");
  const lg = await liChecaLogin(tabId); if (lg) return lg;
  await liEsperar(tabId, "conexoes", {}, "pronto", 15000);
  for (let i = 0; i < 3; i++) { await liDom(tabId, "rolar", { px: 1400 }); await liSleep(1100); }
  const r = await liDom(tabId, "conexoes");
  return { ok: true, conexoes: (r.pessoas || []).slice(0, max) };
}

async function liConversas(max) {
  const tabId = await liGo("https://www.linkedin.com/messaging/");
  const lg = await liChecaLogin(tabId); if (lg) return lg;
  const r = await liEsperar(tabId, "conversas", {}, "pronto", 15000);
  return { ok: true, conversas: (r.conversas || []).slice(0, max) };
}

async function liMeusPosts(max) {
  const tabId = await liGo("https://www.linkedin.com/in/me/recent-activity/shares/", 3500);
  const lg = await liChecaLogin(tabId); if (lg) return lg;
  await liEsperar(tabId, "meus_posts", {}, "pronto", 15000);
  for (let i = 0; i < 4; i++) { await liDom(tabId, "rolar", { px: 1600 }); await liSleep(1300); }
  const r = await liDom(tabId, "meus_posts");
  return { ok: true, posts: (r.posts || []).slice(0, max) };
}

async function liPublicar(texto, midia) {
  const tabId = await liGo(LI_HOME, 3000);
  const lg = await liChecaLogin(tabId); if (lg) return lg;
  const a = await liEsperar(tabId, "share_abrir", {}, "clicou", 12000);
  if (!a.clicou) return { erro: "não achei o botão 'Começar publicação'" };
  const ed = await liEsperar(tabId, "share_editor", {}, "pronto", 10000);
  if (!ed.pronto) return { erro: "o editor de publicação não abriu" };
  await liDom(tabId, "share_digitar", { texto });
  await liSleep(1200);
  if (midia && midia.b64) {
    await liDom(tabId, "share_midia_botao");
    const inp = await liEsperar(tabId, "share_midia_input", {}, "achou", 8000);
    if (!inp.achou) return { erro: "não consegui anexar a imagem (campo de mídia não apareceu)" };
    const f = await liDom(tabId, "share_midia_set", midia);
    if (!f.ok) return { erro: f.erro || "falha ao anexar a imagem" };
    await liSleep(3500);
    for (let i = 0; i < 3; i++) {                            // editor de mídia: Avançar / Concluído
      const n = await liDom(tabId, "clicar_no_dialogo", { re: "^(avançar|next|concluído|concluir|done)$" });
      if (!n.clicou) break;
      await liSleep(2200);
    }
    const ed2 = await liEsperar(tabId, "share_editor", {}, "pronto", 8000);
    if (!ed2.pronto) return { erro: "o editor não voltou depois de anexar a imagem" };
  }
  const p = await liEsperar(tabId, "share_publicar", {}, "clicou", 8000);
  if (!p.clicou) return { erro: "não consegui clicar em Publicar" };
  const fim = await liEsperar(tabId, "share_fechado", {}, "ok", 25000);
  if (!fim.ok) return { erro: "a publicação não confirmou (confira no LinkedIn)" };
  return { ok: true, url: fim.url || "" };
}

// ===================== funções injetadas na página (síncronas) =====================
function _liDom(op, a) {
  const T = (el) => ((el && (el.innerText || el.textContent)) || "").replace(/\s+/g, " ").trim();
  const vis = (el) => { if (!el) return false; const r = el.getBoundingClientRect(); return r.width > 0 && r.height > 0; };
  const rx = (s) => new RegExp(s, "i");
  const rotulo = (el) => (el.getAttribute("aria-label") || T(el) || "").trim();
  const botoes = (root) => [...(root || document).querySelectorAll("button, [role=button], a[role=button], div[role=menuitem], li[role=menuitem] div, a")].filter(vis);
  const acha = (root, re) => botoes(root).find((b) => re.test(rotulo(b)) || re.test(T(b)));
  const limpaUrl = (h) => { try { const u = new URL(h, location.origin); const m = u.pathname.match(/^\/in\/([^/]+)/); return m ? "https://www.linkedin.com/in/" + decodeURIComponent(m[1]) + "/" : ""; } catch (e) { return ""; } };
  const num = (s) => { s = String(s || "").toLowerCase().replace(/\s/g, ""); const m = s.match(/([\d.,]+)(mil|k|mi|m)?/); if (!m) return 0; let n = m[1]; let mult = m[2] === "mil" || m[2] === "k" ? 1000 : (m[2] === "mi" || m[2] === "m") ? 1e6 : 1; if (mult > 1) n = n.replace(",", "."); else n = n.replace(/[.,]/g, ""); return Math.round(parseFloat(n) * mult) || 0; };
  const dialogo = () => [...document.querySelectorAll("[role=dialog], .artdeco-modal, [role=alertdialog]")].filter(vis).pop() || null;
  const topo = () => { const h = document.querySelector("main h1"); return h ? (h.closest("section") || h.parentElement.parentElement) : null; };
  const logadoFora = () => /\/(login|checkpoint|authwall|uas\/login|signup)/.test(location.pathname) || !!document.querySelector("#session_key, form.login__form");
  const setNativo = (el, v) => { const proto = el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype; const d = Object.getOwnPropertyDescriptor(proto, "value"); d && d.set ? d.set.call(el, v) : (el.value = v); el.dispatchEvent(new Event("input", { bubbles: true })); el.dispatchEvent(new Event("change", { bubbles: true })); };
  const digitar = (ed, texto) => {
    ed.focus();
    try { const s = window.getSelection(); s.selectAllChildren(ed); s.collapseToEnd(); } catch (e) {}
    const linhas = String(texto).split("\n");
    linhas.forEach((ln, i) => { if (ln) document.execCommand("insertText", false, ln); if (i < linhas.length - 1) document.execCommand("insertParagraph", false); });
    ed.dispatchEvent(new Event("input", { bubbles: true }));
  };
  try {
    if (op === "estado") return { pronto: document.readyState === "complete", logado: !logadoFora(), url: location.href };
    if (op === "rolar") { window.scrollBy(0, a.px || 800); const d = dialogo(); return { ok: true, y: window.scrollY, d: !!d }; }
    if (op === "eu") {
      const img = document.querySelector("img.global-nav__me-photo, .global-nav__me img");
      const nome = img ? (img.getAttribute("alt") || "").trim() : "";
      const lk = [...document.querySelectorAll("a[href*='/in/']")].find((x) => x.closest(".feed-identity-module, .profile-card, aside"));
      return { nome, url: lk ? limpaUrl(lk.href) : "" };
    }
    if (op === "perfil") {
      if (logadoFora()) return { erro: "nao_logado" };
      const h1 = document.querySelector("main h1");
      if (!h1) return {};
      const tp = topo() || document;
      const headline = T(tp.querySelector(".text-body-medium")) || "";
      const empBtn = tp.querySelector("button[aria-label^='Empresa atual'], button[aria-label^='Current company'], [aria-label*='Empresa atual']");
      const empresa = empBtn ? (empBtn.getAttribute("aria-label") || "").replace(/^(Empresa atual|Current company)\s*:?\s*/i, "").split(".")[0].trim() : "";
      const txt = T(tp);
      let conexao = "nao_conectado";
      if (acha(tp, /^(pendente|pending)$/) || acha(tp, /convite pendente|pending invitation|retirar convite|withdraw/)) conexao = "pendente";
      else if (/·\s*1(º|st)\b|\b1st\b|1º grau|1st degree/.test(txt)) conexao = "conectado";
      return { nome: T(h1), cargo: headline.slice(0, 200), empresa: empresa.slice(0, 120), conexao, url: location.href };
    }
    if (op === "clicar_conectar") {
      const tp = topo(); if (!tp) return { erro: "perfil não carregado" };
      if (acha(tp, /^(pendente|pending)$/)) return { ja: "pendente" };
      let b = botoes(tp).find((x) => /(convidar|invite) .* (para se conectar|to connect)/i.test(x.getAttribute("aria-label") || "")) || acha(tp, /^(conectar|connect)$/);
      if (b) { b.click(); return { clicou: true }; }
      const mais = botoes(tp).find((x) => /^(mais|more)$|mais ações|more actions/i.test(rotulo(x)));
      if (mais) { mais.click(); return { menu: true }; }
      return { erro: "botão Conectar não encontrado" };
    }
    if (op === "clicar_menu_conectar") {
      const menus = [...document.querySelectorAll(".artdeco-dropdown__content, [role=menu]")].filter(vis);
      for (const m of menus) {
        const it = [...m.querySelectorAll("[role=button], div[aria-label], li, span")].filter(vis).find((x) => /(convidar|invite) .* (para se conectar|to connect)/i.test(x.getAttribute("aria-label") || "") || /^(conectar|connect)$/i.test(T(x)));
        if (it) { (it.closest("[role=button]") || it).click(); return { clicou: true }; }
        if ([...m.querySelectorAll("*")].some((x) => /^(remover conexão|remove connection)$/i.test(T(x)))) return { ja: "conectado" };
        if ([...m.querySelectorAll("*")].some((x) => /^(pendente|pending|retirar|withdraw)/i.test(T(x)))) return { ja: "pendente" };
      }
      return { erro: "opção Conectar não encontrada no menu 'Mais'" };
    }
    if (op === "modal") {
      const d = dialogo(); const t = T(d || document.body);
      const limite = /limite semanal|weekly invitation limit|atingiu o limite|reached the weekly|limite de convites/i.test(t);
      if (!d) return { aberto: false, limite };
      return { aberto: true, limite, texto: t.slice(0, 300), email: !!d.querySelector("input[type=email], input[name=email]"),
               btnNota: !!acha(d, /^(adicionar nota|add a note)$/), textarea: !!d.querySelector("textarea"),
               semNota: !!acha(d, /^(enviar sem nota|send without a note)$/) };
    }
    if (op === "clicar_no_dialogo") {
      const d = dialogo() || document; const b = botoes(d).find((x) => rx(a.re).test(rotulo(x)) || rx(a.re).test(T(x)));
      if (b && !b.disabled) { b.click(); return { clicou: true, rotulo: rotulo(b).slice(0, 60) }; }
      return { clicou: false };
    }
    if (op === "preencher_nota") { const d = dialogo(); const ta = d && d.querySelector("textarea"); if (!ta) return { erro: "sem campo de nota" }; ta.focus(); setNativo(ta, a.texto || ""); return { ok: true }; }
    if (op === "fechar_upsell") { const d = dialogo(); const x = d && botoes(d).find((b) => /^(fechar|dispensar|close|dismiss)/i.test(rotulo(b))); if (x) x.click(); return { ok: true }; }
    if (op === "fechar_dialogo") { const d = dialogo(); const x = d && botoes(d).find((b) => /^(fechar|dispensar|close|dismiss)/i.test(rotulo(b))); if (x) { x.click(); return { ok: true }; } document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true })); return { ok: true }; }
    if (op === "clicar_mensagem") {
      const tp = topo(); if (!tp) return { erro: "perfil não carregado" };
      const b = botoes(tp).find((x) => /^(mensagem|message)\b/i.test(rotulo(x)) || /^(enviar mensagem|message) /i.test(x.getAttribute("aria-label") || ""));
      if (b) { b.click(); return { clicou: true }; }
      return { clicou: false };
    }
    if (op === "editor_msg") {
      const eds = [...document.querySelectorAll(".msg-form__contenteditable[contenteditable=true], .msg-overlay-conversation-bubble [contenteditable=true]")].filter(vis);
      if (eds.length) return { pronto: true };
      const d = dialogo(); const premium = !!(d && /inmail|premium/i.test(T(d)));
      return { pronto: false, premium };
    }
    if (op === "digitar_msg") {
      const ed = [...document.querySelectorAll(".msg-form__contenteditable[contenteditable=true], .msg-overlay-conversation-bubble [contenteditable=true]")].filter(vis).pop();
      if (!ed) return { erro: "editor não encontrado" };
      digitar(ed, a.texto); return { ok: true };
    }
    if (op === "enviar_msg") {
      const ed = [...document.querySelectorAll(".msg-form__contenteditable[contenteditable=true]")].filter(vis).pop();
      const form = ed ? ed.closest("form") : document;
      const b = (form && (form.querySelector(".msg-form__send-button:not([disabled]), button[type=submit]:not([disabled])"))) || acha(form, /^(enviar|send)$/);
      if (b && !b.disabled) { b.click(); return { clicou: true }; }
      return { clicou: false };
    }
    if (op === "msg_enviada") {
      const ed = [...document.querySelectorAll(".msg-form__contenteditable[contenteditable=true]")].filter(vis).pop();
      return { ok: !ed || !T(ed) };
    }
    if (op === "fechar_conversas") {
      [...document.querySelectorAll(".msg-overlay-bubble-header__controls button, .msg-overlay-conversation-bubble button")].filter((b) => /fechar sua conversa|close your conversation|fechar conversa/i.test(b.getAttribute("aria-label") || "") || /close/i.test((b.querySelector("use") || {}).href?.baseVal || "")).forEach((b) => { try { b.click(); } catch (e) {} });
      return { ok: true };
    }
    if (op === "curtir") {
      if (logadoFora()) return { fim: true, erro: "nao_logado" };
      const posts = document.querySelectorAll("[data-urn^='urn:li:activity'], .feed-shared-update-v2");
      if (!posts.length) { const vazio = /ainda não publicou|hasn.t posted|no recent activity|nenhuma atividade/i.test(T(document.querySelector("main"))); return vazio ? { fim: true, semPosts: true } : {}; }
      const b = [...document.querySelectorAll("button[aria-pressed='false']")].filter(vis).find((x) => /(gostei|like)/i.test(x.getAttribute("aria-label") || "") && !/coment/i.test(x.getAttribute("aria-label") || ""));
      if (b) { b.scrollIntoView({ block: "center" }); b.click(); return { fim: true, clicou: true }; }
      if ([...document.querySelectorAll("button[aria-pressed='true']")].some((x) => /(gostei|like)/i.test(x.getAttribute("aria-label") || ""))) return { fim: true, clicou: true, ja: true };
      return {};
    }
    if (op === "busca") {
      const main = document.querySelector("main") || document.body;
      const vazio = /nenhum resultado|no results found/i.test(T(main));
      const cards = [...main.querySelectorAll("li, div[data-chameleon-result-urn], div[data-view-name='search-entity-result-universal-template']")];
      const pessoas = []; const vistos = new Set();
      for (const card of cards) {
        const ln = card.querySelector("a[href*='/in/']"); if (!ln) continue;
        const url = limpaUrl(ln.href); if (!url || vistos.has(url)) continue;
        if (card.querySelector("li a[href*='/in/']") && card.tagName === "LI" && card.querySelectorAll("a[href*='/in/']").length > 4) continue; // lista-mãe
        let nome = T(ln.querySelector("span[aria-hidden=true]")) || T(ln).split("\n")[0];
        nome = nome.replace(/\s*(•|·)\s*\d.*$/, "").replace(/Ver perfil de .*|View .* profile/i, "").trim();
        if (!nome || /membro do linkedin|linkedin member/i.test(nome)) continue;
        const linhas = (card.innerText || "").split("\n").map((s) => s.trim()).filter((s) => s && s !== nome && !/^(•|·)?\s*\d(º|st|nd|rd|th)/.test(s) && !/^(conectar|connect|seguir|follow|mensagem|message|enviar|status)/i.test(s) && !/^Ver perfil|^View /i.test(s));
        const cargo = (linhas[0] || "").slice(0, 200);
        vistos.add(url); pessoas.push({ url, nome: nome.slice(0, 120), cargo, empresa: "" });
      }
      const prox = acha(main, /^(avançar|next|próxima)$/);
      return { pronto: pessoas.length > 0 || vazio, pessoas, fim: vazio || (pessoas.length > 0 && (!prox || prox.disabled)) };
    }
    if (op === "post_pronto") { return { pronto: !!document.querySelector("main [data-urn], main .feed-shared-update-v2, main article") }; }
    if (op === "comentarios") {
      const its = [...document.querySelectorAll("article.comments-comment-entity, article.comments-comment-item, .comments-comment-item, [data-id^='urn:li:comment']")];
      const pessoas = [];
      for (const c of its) {
        const ln = c.querySelector("a[href*='/in/']"); if (!ln) continue;
        const url = limpaUrl(ln.href); if (!url) continue;
        const nome = T(c.querySelector(".comments-comment-meta__description-title, .comments-post-meta__name-text, .comments-comment-meta__description span[aria-hidden=true]")) || T(ln).split("\n")[0];
        const cargo = T(c.querySelector(".comments-comment-meta__description-subtitle, .comments-post-meta__headline"));
        const comentario = T(c.querySelector(".comments-comment-item__main-content, .comments-comment-item-content-body, .update-components-text"));
        pessoas.push({ url, nome: nome.slice(0, 120), cargo: cargo.slice(0, 200), comentario: comentario.slice(0, 500) });
      }
      return { pessoas };
    }
    if (op === "mais_comentarios") {
      const b = botoes(document).find((x) => /carregar mais comentários|load more comments|ver mais comentários|see more comments|mais comentários|previous comments|comentários anteriores/i.test(rotulo(x)));
      if (b) { b.scrollIntoView({ block: "center" }); b.click(); return { clicou: true }; }
      return { clicou: false };
    }
    if (op === "abrir_reacoes") {
      const b = [...document.querySelectorAll("button, [role=button]")].filter(vis).find((x) => /rea(ç|c)(ões|oes|ão)|reactions?/i.test(x.getAttribute("aria-label") || "") && /\d/.test(x.getAttribute("aria-label") || T(x)))
        || document.querySelector(".social-details-social-counts__reactions-count, button.social-details-social-counts__count-value");
      if (b) { (b.closest("button") || b).click(); return { clicou: true }; }
      return { clicou: false };
    }
    if (op === "reacoes") {
      const d = dialogo(); if (!d) return { aberto: false };
      const pessoas = []; const vistos = new Set();
      for (const ln of d.querySelectorAll("a[href*='/in/']")) {
        const url = limpaUrl(ln.href); if (!url || vistos.has(url)) continue; vistos.add(url);
        const ls = (ln.innerText || "").split("\n").map((s) => s.trim()).filter(Boolean).filter((s) => !/^(•|·)?\s*\d(º|st|nd|rd)/.test(s) && !/^Ver perfil|^View /i.test(s));
        pessoas.push({ url, nome: (ls[0] || "").slice(0, 120), cargo: (ls[1] || "").slice(0, 200) });
      }
      if (a.rolar) {
        const sc = [...d.querySelectorAll("*")].find((x) => x.scrollHeight > x.clientHeight + 30 && /(auto|scroll)/.test(getComputedStyle(x).overflowY));
        if (sc) sc.scrollTop = sc.scrollHeight;
        const mais = acha(d, /mostrar mais resultados|show more results|carregar mais|load more/);
        if (mais) mais.click();
      }
      return { aberto: true, pessoas };
    }
    if (op === "conexoes") {
      const main = document.querySelector("main") || document.body;
      const pessoas = []; const vistos = new Set();
      for (const ln of main.querySelectorAll("a[href*='/in/']")) {
        const url = limpaUrl(ln.href); if (!url || vistos.has(url)) continue;
        const nome = (ln.innerText || "").split("\n").map((s) => s.trim()).filter(Boolean)[0] || "";
        if (!nome) continue;
        vistos.add(url); pessoas.push({ url, nome: nome.slice(0, 120) });
      }
      return { pronto: pessoas.length > 0, pessoas };
    }
    if (op === "conversas") {
      const lis = [...document.querySelectorAll("li.msg-conversation-listitem, .msg-conversations-container__conversations-list > li, li.msg-conversation-card")];
      const conversas = [];
      for (const li of lis) {
        const nome = T(li.querySelector(".msg-conversation-listitem__participant-names, .msg-conversation-card__participant-names, h3"));
        if (!nome) continue;
        let snip = T(li.querySelector(".msg-conversation-card__message-snippet, .msg-conversation-card__message-snippet-body, p"));
        const meu = /^(você|you)\s*:/i.test(snip);
        snip = snip.replace(/^[^:]{1,40}:\s*/, "");
        const naoLida = /unread/.test(li.className + " " + (li.querySelector("[class*=unread]") || {}).className) || !!li.querySelector(".msg-conversation-card__unread-count, .notification-badge--show");
        const th = li.querySelector("a[href*='/messaging/thread/']");
        conversas.push({ nome: nome.slice(0, 120), texto: snip.slice(0, 400), ultima_de: meu ? "eu" : "contato", nao_lida: naoLida, thread: th ? th.href : "" });
      }
      return { pronto: conversas.length > 0 || /nenhuma mensagem|no messages/i.test(T(document.querySelector("main"))), conversas };
    }
    if (op === "meus_posts") {
      const els = [...document.querySelectorAll("[data-urn^='urn:li:activity']")];
      const posts = [];
      for (const el of els) {
        const urn = el.getAttribute("data-urn");
        const head = T(el.querySelector(".update-components-header, .feed-shared-header"));
        if (/(repostou|reposted|gostou|likes this|comentou|commented|celebra)/i.test(head)) continue;
        const texto = T(el.querySelector(".update-components-text, .feed-shared-inline-show-more-text, .feed-shared-text"));
        const reac = num(T(el.querySelector(".social-details-social-counts__reactions-count")) || (el.querySelector("[aria-label*='reaç'], [aria-label*='reaction']") || {}).getAttribute?.("aria-label"));
        const lab = [...el.querySelectorAll("button, li, span")].map((x) => (x.getAttribute("aria-label") || T(x))).join(" | ");
        const com = num((lab.match(/([\d.,]+\s*(mil|k)?)\s*(comentários|comentário|comments?)/i) || [])[1]);
        const rep = num((lab.match(/([\d.,]+\s*(mil|k)?)\s*(compartilhamentos|compartilhamento|reposts?)/i) || [])[1]);
        const imp = num((T(el).match(/([\d.,]+\s*(mil|k)?)\s*(impressões|impressions)/i) || [])[1]);
        posts.push({ urn, url: "https://www.linkedin.com/feed/update/" + urn + "/", texto: texto.slice(0, 3000), reacoes: reac, comentarios: com, compartilhamentos: rep, impressoes: imp });
      }
      return { pronto: posts.length > 0 || /ainda não publicou|hasn.t posted/i.test(T(document.querySelector("main"))), posts };
    }
    if (op === "share_abrir") {
      const b = document.querySelector("button.share-box-feed-entry__trigger") || acha(document.querySelector("main") || document, /^(começar (uma )?publicação|start a post|criar uma publicação)/);
      if (b) { b.click(); return { clicou: true }; }
      return { clicou: false };
    }
    if (op === "share_editor") {
      const d = dialogo(); const ed = d && d.querySelector(".ql-editor[contenteditable=true], [role=textbox][contenteditable=true]");
      return { pronto: !!ed };
    }
    if (op === "share_digitar") {
      const d = dialogo(); const ed = d && d.querySelector(".ql-editor[contenteditable=true], [role=textbox][contenteditable=true]");
      if (!ed) return { erro: "editor não encontrado" };
      digitar(ed, a.texto); return { ok: true };
    }
    if (op === "share_midia_botao") {
      const d = dialogo() || document;
      const b = botoes(d).find((x) => /adicionar (mídia|uma foto|foto)|add (media|a photo)|adicionar imagem/i.test(x.getAttribute("aria-label") || ""));
      if (b) { b.click(); return { clicou: true }; }
      return { clicou: false };
    }
    if (op === "share_midia_input") { return { achou: !!document.querySelector("input[type=file]") }; }
    if (op === "share_midia_set") {
      const inp = [...document.querySelectorAll("input[type=file]")].pop(); if (!inp) return { erro: "campo de arquivo não encontrado" };
      const bin = atob(a.b64); const u8 = new Uint8Array(bin.length); for (let i = 0; i < bin.length; i++) u8[i] = bin.charCodeAt(i);
      const f = new File([u8], a.nome || "imagem.png", { type: a.tipo || "image/png" });
      const dt = new DataTransfer(); dt.items.add(f); inp.files = dt.files;
      inp.dispatchEvent(new Event("change", { bubbles: true })); inp.dispatchEvent(new Event("input", { bubbles: true }));
      return { ok: true };
    }
    if (op === "share_publicar") {
      const d = dialogo(); if (!d) return { clicou: false };
      const b = d.querySelector("button.share-actions__primary-action:not([disabled])") || botoes(d).find((x) => /^(publicar|post)$/i.test(T(x)) && !x.disabled);
      if (b) { b.click(); return { clicou: true }; }
      return { clicou: false };
    }
    if (op === "share_fechado") {
      const d = dialogo(); const ed = d && d.querySelector(".ql-editor[contenteditable=true], [role=textbox][contenteditable=true]");
      if (ed) return { ok: false };
      const toast = [...document.querySelectorAll(".artdeco-toast-item a[href*='/feed/update/'], a[href*='/feed/update/urn:li:']")].pop();
      return { ok: true, url: toast ? toast.href.split("?")[0] : "" };
    }
    return { erro: "op desconhecida: " + op };
  } catch (e) { return { erro: String(e && e.message || e).slice(0, 200) }; }
}

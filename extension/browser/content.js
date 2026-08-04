// Cerne — content script: desenha a MOLDURA esfumaçada (cores do Cerne) enquanto o agente opera a página.
(function () {
  if (window.__cerneOverlayReady) return;      // evita injeção dupla
  window.__cerneOverlayReady = true;

  const ID = "cerne-operando-overlay";
  let hideTimer = null;

  function css() {
    if (document.getElementById(ID + "-style")) return;
    const st = document.createElement("style");
    st.id = ID + "-style";
    st.textContent = `
      #${ID}{position:fixed;inset:0;z-index:2147483646;pointer-events:none;opacity:0;transition:opacity .35s ease;}
      #${ID}.on{opacity:1;}
      #${ID} .cerne-frame{position:absolute;inset:0;border-radius:6px;
        box-shadow:inset 0 0 0 3px rgba(56,214,214,.85), inset 0 0 34px 10px rgba(56,214,214,.30),
                   inset 0 0 70px 26px rgba(108,140,255,.22), 0 0 22px 4px rgba(56,214,214,.28);
        animation:cernePulse 2.6s ease-in-out infinite;}
      #${ID} .cerne-smoke{position:absolute;inset:0;
        background:
          radial-gradient(120% 90px at 50% 0%, rgba(56,214,214,.22), rgba(56,214,214,0) 70%),
          radial-gradient(120% 90px at 50% 100%, rgba(108,140,255,.22), rgba(108,140,255,0) 70%),
          radial-gradient(90px 120% at 0% 50%, rgba(56,214,214,.20), rgba(56,214,214,0) 70%),
          radial-gradient(90px 120% at 100% 50%, rgba(108,140,255,.20), rgba(108,140,255,0) 70%);
        filter:blur(2px);animation:cerneSmoke 5s ease-in-out infinite alternate;}
      #${ID} .cerne-chip{position:absolute;top:14px;left:50%;transform:translateX(-50%);
        display:flex;align-items:center;gap:9px;pointer-events:none;
        background:rgba(8,14,20,.86);color:#dff;border:1px solid rgba(56,214,214,.6);
        border-radius:999px;padding:8px 15px;font:600 12.5px/1 -apple-system,Segoe UI,Roboto,sans-serif;
        box-shadow:0 6px 24px rgba(0,0,0,.35);backdrop-filter:blur(4px);}
      #${ID} .cerne-dot{width:9px;height:9px;border-radius:50%;background:#38d6d6;box-shadow:0 0 10px 2px #38d6d6;animation:cerneBlink 1.1s infinite;}
      @keyframes cernePulse{0%,100%{opacity:.9}50%{opacity:.55}}
      @keyframes cerneBlink{0%,100%{opacity:1}50%{opacity:.35}}
      @keyframes cerneSmoke{0%{opacity:.7;transform:scale(1)}100%{opacity:1;transform:scale(1.015)}}`;
    (document.head || document.documentElement).appendChild(st);
  }

  function build() {
    css();
    let ov = document.getElementById(ID);
    if (!ov) {
      ov = document.createElement("div");
      ov.id = ID;
      ov.innerHTML = `<div class="cerne-smoke"></div><div class="cerne-frame"></div>
        <div class="cerne-chip"><span class="cerne-dot"></span><span class="cerne-label">Cerne está operando este navegador</span></div>`;
      (document.body || document.documentElement).appendChild(ov);
    }
    return ov;
  }

  function show(label) {
    const ov = build();
    if (label) { const l = ov.querySelector(".cerne-label"); if (l) l.textContent = label; }
    requestAnimationFrame(() => ov.classList.add("on"));
    if (hideTimer) clearTimeout(hideTimer);
    hideTimer = setTimeout(hide, 12000);   // rede de segurança: some sozinho se o Cerne parar de enviar
  }
  function hide() {
    const ov = document.getElementById(ID);
    if (ov) { ov.classList.remove("on"); }
    if (hideTimer) { clearTimeout(hideTimer); hideTimer = null; }
  }

  chrome.runtime.onMessage.addListener((msg) => {
    if (!msg || !msg.cerne || msg.tipo !== "overlay") return;
    if (msg.on) show(msg.label); else hide();
  });
})();

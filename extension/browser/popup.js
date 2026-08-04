const $ = (id) => document.getElementById(id);
const DEF_BASE = "https://cerne.orqion-ia.com.br";

async function carregar() {
  const s = await chrome.storage.local.get(["base", "token", "ligado"]);
  $("base").value = s.base || DEF_BASE;
  $("token").value = s.token || "";
  setToggle(s.ligado !== false);
  status();
}
function setToggle(on) { $("tg").classList.toggle("on", on); $("tg").dataset.on = on ? "1" : "0"; }
$("tg").addEventListener("click", async () => {
  const on = $("tg").dataset.on !== "1"; setToggle(on);
  await chrome.storage.local.set({ ligado: on }); status();
});

async function status() {
  const s = await chrome.storage.local.get(["base", "token", "ligado"]);
  const on = s.ligado !== false;
  if (!s.token) { $("dot").classList.remove("on"); $("stTxt").textContent = "não pareado"; return; }
  if (!on) { $("dot").classList.remove("on"); $("stTxt").textContent = "pareado · pausado"; return; }
  $("dot").classList.add("on"); $("stTxt").textContent = "conectado · aguardando o Cerne";
}

$("save").addEventListener("click", async () => {
  const base = ($("base").value || DEF_BASE).trim().replace(/\/+$/, "");
  const token = ($("token").value || "").trim();
  await chrome.storage.local.set({ base, token, ligado: true });
  setToggle(true);
  $("stTxt").textContent = "salvo · conectando…";
  chrome.runtime.sendMessage({ tipo: "cerne-ping" }, () => setTimeout(status, 500));
});

$("test").addEventListener("click", async () => {
  const base = ($("base").value || DEF_BASE).trim().replace(/\/+$/, "");
  const token = ($("token").value || "").trim();
  if (!token) { $("stTxt").textContent = "cole o código primeiro"; return; }
  $("stTxt").textContent = "testando…";
  try {
    const r = await fetch(base + "/ext/hello", { method: "POST", headers: { "Content-Type": "application/json", "X-Ext-Token": token }, body: "{}" });
    const j = await r.json().catch(() => ({}));
    if (r.ok) { $("dot").classList.add("on"); $("stTxt").textContent = "✓ pareado como " + (j.usuario || "usuário"); }
    else { $("dot").classList.remove("on"); $("stTxt").textContent = "✗ " + (j.detail || j.error || ("erro " + r.status)); }
  } catch (e) { $("stTxt").textContent = "✗ sem conexão com o servidor"; }
});

carregar();

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod computador;
mod projeto;

use std::sync::Mutex;
use std::time::Duration;
use serde::Serialize;
use tauri::{Manager, Emitter, State, AppHandle, WebviewUrl, WebviewWindowBuilder};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_updater::UpdaterExt;
use enigo::{Enigo, Keyboard, Settings, Key, Direction::Click};

const APP_URL: &str = "https://cerne.orqion-ia.com.br";

#[derive(Clone)]
struct Sess {
    base: String,
    cookie: String,
}

#[derive(Default)]
struct AppState(Mutex<Option<Sess>>);

// ===================== Conexão + ingestão de pasta local =====================

#[tauri::command]
fn conectar(state: State<AppState>, base_url: String, email: String, senha: String) -> Result<String, String> {
    let base = base_url.trim().trim_end_matches('/').to_string();
    let resp = ureq::post(&format!("{}/auth/login", base))
        .timeout(Duration::from_secs(30))
        .send_json(serde_json::json!({ "email": email, "senha": senha }));
    let r = match resp {
        Ok(r) => r,
        Err(ureq::Error::Status(code, _)) => {
            return Err(format!("login recusado ({}) — confira e-mail e senha", code));
        }
        Err(e) => return Err(format!("não consegui conectar em {}: {}", base, e)),
    };
    let mut cookie = String::new();
    for h in r.all("set-cookie") {
        if h.starts_with("cerne_sess=") {
            cookie = h.split(';').next().unwrap_or("").to_string();
        }
    }
    if cookie.is_empty() {
        return Err("o servidor não retornou uma sessão de login".into());
    }
    *state.0.lock().unwrap() = Some(Sess { base, cookie });
    Ok("conectado".into())
}

#[tauri::command]
fn escolher_pasta(app: AppHandle) -> Option<String> {
    app.dialog().file().blocking_pick_folder().map(|p| p.to_string())
}

#[derive(Serialize)]
struct IngestResult {
    total: usize,
    ok: usize,
    falhas: usize,
    chunks: i64,
}

#[tauri::command]
fn ingerir_pasta(app: AppHandle, state: State<AppState>, colecao: String, pasta: String) -> Result<IngestResult, String> {
    let sess = state.0.lock().unwrap().clone().ok_or("conecte-se ao servidor primeiro")?;
    let colq = colecao.trim().replace(' ', "%20");
    let exts = ["pdf", "txt", "md", "csv", "docx", "doc", "rtf", "html", "htm", "png", "jpg", "jpeg", "json"];
    let mut arquivos: Vec<std::path::PathBuf> = Vec::new();
    for entry in walkdir::WalkDir::new(&pasta).max_depth(6).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                if exts.contains(&ext.to_lowercase().as_str()) {
                    arquivos.push(entry.path().to_path_buf());
                }
            }
        }
    }
    let total = arquivos.len();
    let (mut ok, mut falhas, mut chunks) = (0usize, 0usize, 0i64);
    let boundary = "----CerneDesktopBoundary7MA4YWxkTrZu";
    for (i, path) in arquivos.iter().enumerate() {
        let nome = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("arquivo")
            .replace(['"', '\r', '\n'], "_");
        let _ = app.emit("ingest-progress", serde_json::json!({"i": i + 1, "total": total, "arquivo": nome}));
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(_) => {
                falhas += 1;
                continue;
            }
        };
        let mut body: Vec<u8> = Vec::with_capacity(bytes.len() + 256);
        body.extend_from_slice(
            format!(
                "--{b}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{n}\"\r\nContent-Type: application/octet-stream\r\n\r\n",
                b = boundary, n = nome
            )
            .as_bytes(),
        );
        body.extend_from_slice(&bytes);
        body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());
        let url = format!("{}/ingest-file?colecao={}", sess.base, colq);
        let resp = ureq::post(&url)
            .set("Cookie", &sess.cookie)
            .set("Content-Type", &format!("multipart/form-data; boundary={}", boundary))
            .timeout(Duration::from_secs(240))
            .send_bytes(&body);
        match resp {
            Ok(r) => {
                ok += 1;
                if let Ok(j) = r.into_json::<serde_json::Value>() {
                    chunks += j.get("chunks_indexed").and_then(|v| v.as_i64()).unwrap_or(0);
                }
            }
            Err(_) => falhas += 1,
        }
    }
    Ok(IngestResult { total, ok, falhas, chunks })
}

// ===================== Agente local (arquivos / apps / entrada) =====================

#[derive(Serialize)]
struct Entrada {
    nome: String,
    tipo: String,
    tamanho: u64,
}

#[tauri::command]
fn listar_dir(caminho: String) -> Result<Vec<Entrada>, String> {
    let mut v = Vec::new();
    for e in std::fs::read_dir(&caminho).map_err(|x| x.to_string())? {
        if let Ok(e) = e {
            let md = e.metadata().ok();
            let is_dir = md.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            v.push(Entrada {
                nome: e.file_name().to_string_lossy().to_string(),
                tipo: if is_dir { "pasta".into() } else { "arquivo".into() },
                tamanho: md.map(|m| m.len()).unwrap_or(0),
            });
        }
    }
    v.sort_by(|a, b| (a.tipo.clone(), a.nome.to_lowercase()).cmp(&(b.tipo.clone(), b.nome.to_lowercase())));
    Ok(v)
}

#[tauri::command]
fn ler_arquivo(caminho: String) -> Result<String, String> {
    let bytes = std::fs::read(&caminho).map_err(|x| x.to_string())?;
    Ok(String::from_utf8_lossy(&bytes[..bytes.len().min(200_000)]).to_string())
}

#[tauri::command]
fn escrever_arquivo(caminho: String, conteudo: String) -> Result<(), String> {
    std::fs::write(&caminho, conteudo).map_err(|x| x.to_string())
}

#[tauri::command]
fn abrir_caminho(app: AppHandle, caminho: String) -> Result<(), String> {
    use tauri_plugin_shell::ShellExt;
    app.shell().open(&caminho, None).map_err(|x| x.to_string())
}

#[derive(Serialize)]
struct CmdOut {
    codigo: i32,
    saida: String,
}

#[tauri::command]
fn rodar_comando(programa: String, args: Vec<String>) -> Result<CmdOut, String> {
    let out = std::process::Command::new(&programa)
        .args(&args)
        .output()
        .map_err(|x| format!("não consegui executar '{}': {}", programa, x))?;
    let mut saida = String::from_utf8_lossy(&out.stdout).to_string();
    if !out.stderr.is_empty() {
        saida.push_str(&String::from_utf8_lossy(&out.stderr));
    }
    Ok(CmdOut { codigo: out.status.code().unwrap_or(-1), saida: saida.chars().take(8000).collect() })
}

#[tauri::command]
fn digitar_texto(texto: String) -> Result<(), String> {
    let mut e = Enigo::new(&Settings::default()).map_err(|x| x.to_string())?;
    e.text(&texto).map_err(|x| x.to_string())
}

#[tauri::command]
fn pressionar_tecla(tecla: String) -> Result<(), String> {
    let mut e = Enigo::new(&Settings::default()).map_err(|x| x.to_string())?;
    let k = match tecla.to_lowercase().as_str() {
        "enter" | "return" => Key::Return,
        "tab" => Key::Tab,
        "esc" | "escape" => Key::Escape,
        "space" => Key::Space,
        "backspace" => Key::Backspace,
        "delete" => Key::Delete,
        _ => return Err("tecla não suportada".into()),
    };
    e.key(k, Click).map_err(|x| x.to_string())
}

// ===================== Auto-update =====================

#[tauri::command]
async fn verificar_atualizacao(app: AppHandle) -> Result<String, String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await {
        Ok(Some(update)) => {
            let v = update.version.clone();
            update
                .download_and_install(|_chunk, _total| {}, || {})
                .await
                .map_err(|e| e.to_string())?;
            Ok(format!("Atualizado para {}. Reinicie o Cerne Desktop para concluir.", v))
        }
        Ok(None) => Ok("Você já está na versão mais recente.".into()),
        Err(e) => Err(format!("falha ao verificar atualização: {}", e)),
    }
}

fn checar_update_bandeja(app: &AppHandle) {
    let h = app.clone();
    tauri::async_runtime::spawn(async move {
        let res = match h.updater() {
            Ok(u) => match u.check().await {
                Ok(Some(update)) => {
                    let v = update.version.clone();
                    match update.download_and_install(|_a, _b| {}, || {}).await {
                        Ok(_) => format!("Cerne Desktop atualizado para {}. Reinicie para concluir.", v),
                        Err(e) => format!("Falha ao instalar a atualização: {}", e),
                    }
                }
                Ok(None) => "Você já está na versão mais recente.".into(),
                Err(e) => format!("Erro ao verificar: {}", e),
            },
            Err(e) => format!("Updater indisponível: {}", e),
        };
        h.dialog().message(res).blocking_show();
    });
}

// ===================== Navegador do usuário (abrir Chrome/Edge quando fechado) =====================

fn launch_browser(qual: &str, url: &str) -> Result<String, String> {
    let url = if url.trim().is_empty() { APP_URL } else { url };
    #[cfg(target_os = "windows")]
    {
        let pf = std::env::var("ProgramFiles").unwrap_or_default();
        let pf86 = std::env::var("ProgramFiles(x86)").unwrap_or_default();
        let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
        let chrome = vec![
            format!("{}\\Google\\Chrome\\Application\\chrome.exe", pf),
            format!("{}\\Google\\Chrome\\Application\\chrome.exe", pf86),
            format!("{}\\Google\\Chrome\\Application\\chrome.exe", local),
        ];
        let edge = vec![
            format!("{}\\Microsoft\\Edge\\Application\\msedge.exe", pf86),
            format!("{}\\Microsoft\\Edge\\Application\\msedge.exe", pf),
        ];
        let want: Vec<String> = match qual {
            "chrome" => chrome,
            "edge" => edge,
            _ => chrome.into_iter().chain(edge.into_iter()).collect(),
        };
        for exe in want {
            if std::path::Path::new(&exe).exists() {
                if std::process::Command::new(&exe).arg(url).spawn().is_ok() {
                    return Ok(exe);
                }
            }
        }
        let alvo = if qual == "edge" { "msedge" } else { "chrome" };
        if std::process::Command::new("cmd").args(["/C", "start", "", alvo, url]).spawn().is_ok() {
            return Ok(alvo.into());
        }
        Err("não encontrei o Chrome nem o Edge instalados".into())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let prog = if qual == "edge" { "microsoft-edge" } else { "google-chrome" };
        std::process::Command::new(prog).arg(url).spawn().map_err(|e| e.to_string())?;
        Ok(prog.into())
    }
}

#[tauri::command]
fn abrir_navegador(qual: Option<String>, url: Option<String>) -> Result<String, String> {
    launch_browser(qual.as_deref().unwrap_or("auto"), url.as_deref().unwrap_or(""))
}

fn nav_cfg_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_config_dir().ok().map(|d| {
        let _ = std::fs::create_dir_all(&d);
        d.join("cerne_navegador.json")
    })
}

#[tauri::command]
fn configurar_navegador(app: AppHandle, base: String, token: String) -> Result<(), String> {
    let p = nav_cfg_path(&app).ok_or("sem diretório de configuração")?;
    let v = serde_json::json!({ "base": base.trim().trim_end_matches('/'), "token": token.trim() });
    std::fs::write(&p, v.to_string()).map_err(|e| e.to_string())
}

#[tauri::command]
fn status_navegador(app: AppHandle) -> serde_json::Value {
    if let Some(p) = nav_cfg_path(&app) {
        if let Ok(s) = std::fs::read_to_string(&p) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
                let tok = v.get("token").and_then(|x| x.as_str()).unwrap_or("");
                return serde_json::json!({ "base": v.get("base"), "pareado": !tok.is_empty() });
            }
        }
    }
    serde_json::json!({ "pareado": false })
}

// Laço que mantém presença no Cerne e, quando o agente pede, ABRE o navegador do usuário.
fn iniciar_poller_navegador(app: &AppHandle) {
    let h = app.clone();
    std::thread::spawn(move || loop {
        let cfg = nav_cfg_path(&h)
            .and_then(|p| std::fs::read_to_string(&p).ok())
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
        let (base, token) = match cfg {
            Some(v) => (
                v.get("base").and_then(|x| x.as_str()).unwrap_or(APP_URL).to_string(),
                v.get("token").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            ),
            None => (APP_URL.to_string(), String::new()),
        };
        if token.is_empty() {
            std::thread::sleep(Duration::from_secs(6));
            continue;
        }
        let _ = ureq::post(&format!("{}/desktop/hello", base))
            .set("X-Ext-Token", &token)
            .timeout(Duration::from_secs(15))
            .send_json(serde_json::json!({ "versao": env!("CARGO_PKG_VERSION") }));
        let resp = ureq::post(&format!("{}/desktop/poll", base))
            .set("X-Ext-Token", &token)
            .timeout(Duration::from_secs(35))
            .send_json(serde_json::json!({}));
        match resp {
            Ok(r) => {
                if let Ok(v) = r.into_json::<serde_json::Value>() {
                    if let Some(acao) = v.get("acao") {
                        let a = acao.get("acao").and_then(|x| x.as_str()).unwrap_or("");
                        if a == "abrir_navegador" {
                            let url = acao.get("url").and_then(|x| x.as_str()).unwrap_or("");
                            let _ = launch_browser("auto", url);
                        } else if a == "computador" {
                            // roda em outra thread: o diálogo de permissão pode esperar o usuário sem travar o long-poll
                            let (h2, acao2, base2, tok2) = (h.clone(), acao.clone(), base.clone(), token.clone());
                            std::thread::spawn(move || {
                                let res = computador::executar(&h2, &acao2);
                                let id = acao2.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                                let _ = ureq::post(&format!("{}/desktop/result", base2))
                                    .set("X-Ext-Token", &tok2)
                                    .timeout(Duration::from_secs(30))
                                    .send_json(serde_json::json!({ "id": id, "resultado": res }));
                            });
                        }
                    }
                }
            }
            Err(ureq::Error::Status(401, _)) => std::thread::sleep(Duration::from_secs(10)),
            Err(_) => std::thread::sleep(Duration::from_secs(4)),
        }
    });
}

// ===================== Janelas / bandeja =====================

fn abrir_janela(app: &AppHandle, label: &str, arquivo: &str, titulo: &str, w: f64, h: f64) {
    if let Some(win) = app.get_webview_window(label) {
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(app, label, WebviewUrl::App(arquivo.into()))
        .title(titulo)
        .inner_size(w, h)
        .min_inner_size(460.0, 520.0)
        .resizable(true)
        .center()
        .build();
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState::default())
        .manage(projeto::Liberadas::default())
        .manage(computador::Controle::default())
        .invoke_handler(tauri::generate_handler![
            conectar, escolher_pasta, ingerir_pasta,
            listar_dir, ler_arquivo, escrever_arquivo, abrir_caminho, rodar_comando,
            digitar_texto, pressionar_tecla, verificar_atualizacao,
            abrir_navegador, configurar_navegador, status_navegador,
            projeto::ler_pasta_projeto, projeto::gravar_arquivos_projeto, computador::computador_chat
        ])
        .setup(|app| {
            let show = MenuItem::with_id(app, "show", "Abrir Cerne", true, None::<&str>)?;
            let ingest = MenuItem::with_id(app, "ingest", "Ingerir pasta local…", true, None::<&str>)?;
            let agente = MenuItem::with_id(app, "agente", "Agente local…", true, None::<&str>)?;
            let navcfg = MenuItem::with_id(app, "navcfg", "Conectar navegador…", true, None::<&str>)?;
            let navabrir = MenuItem::with_id(app, "navabrir", "Abrir navegador p/ o Cerne", true, None::<&str>)?;
            let update = MenuItem::with_id(app, "update", "Verificar atualização", true, None::<&str>)?;
            let site = MenuItem::with_id(app, "site", "Abrir no navegador", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &ingest, &agente, &navcfg, &navabrir, &update, &site, &quit])?;
            let _tray = TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Cerne")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.unminimize();
                            let _ = w.set_focus();
                        }
                    }
                    "ingest" => abrir_janela(app, "ingest", "ingest.html", "Cerne — Ingerir pasta local", 580.0, 660.0),
                    "agente" => abrir_janela(app, "agente", "agente.html", "Cerne — Agente local", 720.0, 720.0),
                    "navcfg" => abrir_janela(app, "navegador", "navegador.html", "Cerne — Conectar navegador", 560.0, 620.0),
                    "navabrir" => { let _ = launch_browser("auto", ""); }
                    "update" => checar_update_bandeja(app),
                    "site" => {
                        use tauri_plugin_shell::ShellExt;
                        let _ = app.shell().open(APP_URL, None);
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            computador::criar_moldura(app.handle());  // oculta; mostrada quando um agente controla um app
            iniciar_poller_navegador(app.handle());   // abre o navegador quando o agente pedir (se pareado)
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o Cerne Desktop");
}

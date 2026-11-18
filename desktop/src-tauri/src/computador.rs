// Ferramenta "computador" dos agentes do Cerne: o servidor pede, o DESKTOP decide.
//
//  * pastas/arquivos: só dentro de pastas liberadas pelo usuário (mesmo diálogo nativo do chat);
//  * gravar: diálogo nativo Sobrescrever / Criar novo / Cancelar + backup (reaproveita projeto.rs);
//  * rodar / digitar / tecla / abrir: diálogo nativo "Permitir uma vez / por 15 min / Negar";
//  * enquanto controla um app, uma MOLDURA esfumaçada envolve a janela controlada.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind, MessageDialogResult};

use crate::projeto::{self, ArquivoNovo, Liberadas};

#[derive(Default)]
pub struct Controle {
    ate: Mutex<Option<Instant>>, // "permitir por 15 min"
}

static GERACAO_MOLDURA: AtomicU64 = AtomicU64::new(0);

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

/// Permissão para AGIR (rodar/digitar/tecla/abrir). Uma vez, ou por 15 minutos.
fn permitir_acao(app: &AppHandle, ctl: &Controle, descricao: &str) -> Result<(), String> {
    if let Some(ate) = *ctl.ate.lock().unwrap() {
        if Instant::now() < ate {
            return Ok(());
        }
    }
    let r = app
        .dialog()
        .message(format!(
            "Um agente do Cerne quer CONTROLAR seu computador:\n\n{}\n\n\"Permitir por 15 min\" libera as próximas ações do agente \
sem perguntar de novo (digitar, teclas, abrir e executar). Durante o controle, a janela usada fica com uma moldura azul.",
            descricao
        ))
        .title("Cerne — permitir controle do computador?")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::YesNoCancelCustom(
            "Permitir uma vez".into(),
            "Permitir por 15 min".into(),
            "Negar".into(),
        ))
        .blocking_show_with_result();
    match r {
        MessageDialogResult::Yes => Ok(()),
        MessageDialogResult::Custom(t) if t == "Permitir uma vez" => Ok(()),
        MessageDialogResult::No => {
            *ctl.ate.lock().unwrap() = Some(Instant::now() + Duration::from_secs(15 * 60));
            Ok(())
        }
        MessageDialogResult::Custom(t) if t == "Permitir por 15 min" => {
            *ctl.ate.lock().unwrap() = Some(Instant::now() + Duration::from_secs(15 * 60));
            Ok(())
        }
        _ => Err("o usuário negou a ação".into()),
    }
}

fn pasta_de(caminho: &str) -> Result<(PathBuf, PathBuf), String> {
    let p = PathBuf::from(caminho.trim());
    if !p.is_absolute() {
        return Err("use um caminho absoluto (ex.: C:/Users/voce/Documentos/arquivo.txt)".into());
    }
    let alvo = if p.exists() { p.canonicalize().map_err(|e| e.to_string())? } else { p.clone() };
    let pasta = if alvo.is_dir() { alvo.clone() } else { alvo.parent().map(|x| x.to_path_buf()).ok_or("caminho sem pasta")? };
    let pasta = pasta.canonicalize().map_err(|_| format!("pasta não encontrada: {}", pasta.display()))?;
    Ok((alvo, pasta))
}

/// Executa uma ação pedida pelo servidor. Sempre retorna JSON (ok ou erro).
pub fn executar(app: &AppHandle, cmd: &Value) -> Value {
    match executar_inner(app, cmd) {
        Ok(v) => v,
        Err(e) => json!({ "erro": e }),
    }
}

fn executar_inner(app: &AppHandle, cmd: &Value) -> Result<Value, String> {
    let lib = app.state::<Liberadas>();
    let ctl = app.state::<Controle>();
    let op = s(cmd, "op");
    let quem = "Um agente do Cerne";
    match op.as_str() {
        "listar" => {
            let (_, pasta) = pasta_de(&s(cmd, "caminho"))?;
            projeto::garantir_leitura(app, &lib, &pasta, quem)?;
            let mut itens = Vec::new();
            for e in std::fs::read_dir(&pasta).map_err(|e| e.to_string())?.flatten().take(500) {
                let md = e.metadata().ok();
                itens.push(json!({
                    "nome": e.file_name().to_string_lossy(),
                    "tipo": if md.as_ref().map(|m| m.is_dir()).unwrap_or(false) { "pasta" } else { "arquivo" },
                    "bytes": md.map(|m| m.len()).unwrap_or(0)
                }));
            }
            Ok(json!({ "pasta": pasta.to_string_lossy().trim_start_matches(r"\\?\"), "itens": itens }))
        }
        "ler" => {
            let (alvo, pasta) = pasta_de(&s(cmd, "caminho"))?;
            projeto::garantir_leitura(app, &lib, &pasta, quem)?;
            if alvo.is_dir() {
                return Err("é uma pasta — use a ação listar".into());
            }
            let b = std::fs::read(&alvo).map_err(|e| e.to_string())?;
            let txt = String::from_utf8_lossy(&b[..b.len().min(100_000)]).to_string();
            Ok(json!({ "caminho": alvo.to_string_lossy().trim_start_matches(r"\\?\"), "bytes": b.len(), "conteudo": txt }))
        }
        "gravar" => {
            let caminho = s(cmd, "caminho");
            let p = PathBuf::from(caminho.trim());
            let pai = p.parent().ok_or("caminho inválido")?;
            let pasta = pai.canonicalize().map_err(|_| format!("a pasta {} não existe", pai.display()))?;
            let nome = p.file_name().and_then(|n| n.to_str()).ok_or("nome de arquivo inválido")?.to_string();
            projeto::garantir_leitura(app, &lib, &pasta, quem)?;
            let conteudo = cmd.get("conteudo").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let g = projeto::gravar_core(app, &lib, &pasta.to_string_lossy(), vec![ArquivoNovo { caminho: nome, conteudo }], quem)?;
            Ok(json!({ "gravados": g.gravados, "backup": g.backup, "pasta": pasta.to_string_lossy().trim_start_matches(r"\\?\") }))
        }
        "rodar" => {
            let programa = s(cmd, "programa");
            if programa.is_empty() {
                return Err("faltou o programa".into());
            }
            let args: Vec<String> = cmd
                .get("args")
                .and_then(|a| a.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let pasta = s(cmd, "pasta");
            permitir_acao(app, &ctl, &format!("EXECUTAR: {} {}{}", programa, args.join(" "),
                if pasta.is_empty() { String::new() } else { format!("\n(na pasta {})", pasta) }))?;
            rodar(app, &programa, &args, &pasta)
        }
        "digitar" => {
            let texto = s(cmd, "texto");
            let resumo: String = texto.chars().take(120).collect();
            permitir_acao(app, &ctl, &format!("DIGITAR na janela em foco:\n\"{}\"", resumo))?;
            moldura(app, true);
            std::thread::sleep(Duration::from_millis(350));
            let mut e = enigo::Enigo::new(&enigo::Settings::default()).map_err(|x| x.to_string())?;
            use enigo::Keyboard;
            e.text(&texto).map_err(|x| x.to_string())?;
            moldura_fim(app);
            Ok(json!({ "ok": true, "digitado": texto.chars().count() }))
        }
        "tecla" => {
            let tecla = s(cmd, "tecla");
            permitir_acao(app, &ctl, &format!("PRESSIONAR a tecla '{}' na janela em foco", tecla))?;
            use enigo::{Direction::Click, Key, Keyboard};
            let k = match tecla.to_lowercase().as_str() {
                "enter" | "return" => Key::Return,
                "tab" => Key::Tab,
                "esc" | "escape" => Key::Escape,
                "space" | "espaco" | "espaço" => Key::Space,
                "backspace" => Key::Backspace,
                "delete" => Key::Delete,
                "up" | "cima" | "arrowup" => Key::UpArrow,
                "down" | "baixo" | "arrowdown" => Key::DownArrow,
                "left" | "esquerda" | "arrowleft" => Key::LeftArrow,
                "right" | "direita" | "arrowright" => Key::RightArrow,
                "home" => Key::Home,
                "end" => Key::End,
                _ => return Err("tecla não suportada (enter, tab, esc, space, backspace, delete, setas, home, end)".into()),
            };
            moldura(app, true);
            std::thread::sleep(Duration::from_millis(250));
            let mut e = enigo::Enigo::new(&enigo::Settings::default()).map_err(|x| x.to_string())?;
            e.key(k, Click).map_err(|x| x.to_string())?;
            moldura_fim(app);
            Ok(json!({ "ok": true, "tecla": tecla }))
        }
        "abrir" => {
            let alvo = s(cmd, "caminho");
            if alvo.is_empty() {
                return Err("faltou o caminho (arquivo, pasta, programa ou URL)".into());
            }
            permitir_acao(app, &ctl, &format!("ABRIR: {}", alvo))?;
            use tauri_plugin_shell::ShellExt;
            #[allow(deprecated)]
            app.shell().open(&alvo, None).map_err(|e| e.to_string())?;
            // a janela nova demora a aparecer: contorna-a quando ela vier para o primeiro plano
            let h = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(1800));
                moldura(&h, true);
                moldura_fim(&h);
            });
            Ok(json!({ "ok": true, "aberto": alvo }))
        }
        _ => Err(format!("ação desconhecida: {}", op)),
    }
}

fn rodar(app: &AppHandle, programa: &str, args: &[String], pasta: &str) -> Result<Value, String> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let mut c = Command::new(programa);
    c.args(args).stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());
    if !pasta.is_empty() {
        c.current_dir(pasta);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW (apps com janela abrem normalmente)
    }
    let mut ch = c.spawn().map_err(|e| format!("não consegui executar '{}': {}", programa, e))?;
    let mut so = ch.stdout.take().unwrap();
    let mut se = ch.stderr.take().unwrap();
    let t1 = std::thread::spawn(move || { let mut b = Vec::new(); let _ = so.read_to_end(&mut b); b });
    let t2 = std::thread::spawn(move || { let mut b = Vec::new(); let _ = se.read_to_end(&mut b); b });
    let fim = Instant::now() + Duration::from_secs(120);
    let mut mostrou = false;
    loop {
        if let Some(st) = ch.try_wait().map_err(|e| e.to_string())? {
            let mut saida = String::from_utf8_lossy(&t1.join().unwrap_or_default()).to_string();
            let err = String::from_utf8_lossy(&t2.join().unwrap_or_default()).to_string();
            if !err.is_empty() {
                saida.push_str("\n[stderr]\n");
                saida.push_str(&err);
            }
            if mostrou {
                moldura_fim(app);
            }
            let saida: String = if saida.chars().count() > 8000 { saida.chars().skip(saida.chars().count() - 8000).collect() } else { saida };
            return Ok(json!({ "codigo": st.code().unwrap_or(-1), "saida": saida }));
        }
        if !mostrou && Instant::now() + Duration::from_secs(118) < fim {
            // programa com janela (não terminou rápido): contorna a janela em foco
            moldura(app, true);
            mostrou = true;
        }
        if Instant::now() > fim {
            moldura_fim(app);
            return Ok(json!({ "ok": true, "em_execucao": true, "pid": ch.id(),
                              "nota": "o programa continua rodando (passou de 120s) — provavelmente é um app com janela" }));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

// ===================== Moldura esfumaçada ao redor da janela controlada =====================

#[cfg(windows)]
fn janela_em_foco() -> Option<(i32, i32, i32, i32)> {
    use windows_sys::Win32::Foundation::{HWND, RECT};
    use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowRect, IsIconic};
    unsafe {
        let h: HWND = GetForegroundWindow();
        if h.is_null() || IsIconic(h) != 0 {
            return None;
        }
        let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        let ok = DwmGetWindowAttribute(h, DWMWA_EXTENDED_FRAME_BOUNDS as u32, &mut r as *mut RECT as *mut _, std::mem::size_of::<RECT>() as u32) == 0;
        if !ok && GetWindowRect(h, &mut r) == 0 {
            return None;
        }
        let (w, hh) = (r.right - r.left, r.bottom - r.top);
        if w < 80 || hh < 60 {
            return None;
        }
        Some((r.left, r.top, w, hh))
    }
}

#[cfg(not(windows))]
fn janela_em_foco() -> Option<(i32, i32, i32, i32)> {
    None
}

/// Cria a janela da moldura OCULTA — chamar no setup (thread principal); criar janelas
/// fora da thread principal no Windows é frágil.
pub fn criar_moldura(app: &AppHandle) {
    if app.get_webview_window("moldura").is_some() {
        return;
    }
    match WebviewWindowBuilder::new(app, "moldura", WebviewUrl::App("moldura.html".into()))
        .title("Cerne — controlando")
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .focusable(false)
        .shadow(false)
        .resizable(false)
        .visible(false)
        .inner_size(400.0, 300.0)
        .build()
    {
        Ok(m) => {
            let _ = m.set_ignore_cursor_events(true);
        }
        Err(e) => eprintln!("[moldura] erro ao criar: {}", e),
    }
}

/// Mostra (ou reposiciona) a moldura em volta da janela em primeiro plano. Não rouba o foco
/// (não-focável) e deixa os cliques passarem (ignore cursor events).
pub fn moldura(app: &AppHandle, _ativa: bool) {
    GERACAO_MOLDURA.fetch_add(1, Ordering::SeqCst);
    let Some((x, y, w, h)) = janela_em_foco() else { return };
    // não contorna a própria moldura nem janelas do Cerne
    if let Some(m) = app.get_webview_window("moldura") {
        if let (Ok(p), Ok(sz)) = (m.outer_position(), m.outer_size()) {
            if p.x == x - 14 && p.y == y - 14 && sz.width as i32 == w + 28 && sz.height as i32 == h + 28 && m.is_visible().unwrap_or(false) {
                return;
            }
        }
    }
    let Some(m) = app.get_webview_window("moldura") else { eprintln!("[moldura] janela não criada"); return };
    let _ = m.set_ignore_cursor_events(true);
    let _ = m.set_position(PhysicalPosition::new(x - 14, y - 14));
    let _ = m.set_size(PhysicalSize::new((w + 28) as u32, (h + 28) as u32));
    let _ = m.show();
}

/// Esconde a moldura alguns segundos depois da última ação (ações seguidas mantêm a moldura).
pub fn moldura_fim(app: &AppHandle) {
    let g = GERACAO_MOLDURA.load(Ordering::SeqCst);
    let h = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(5));
        if GERACAO_MOLDURA.load(Ordering::SeqCst) == g {
            if let Some(m) = h.get_webview_window("moldura") {
                let _ = m.hide();
            }
        }
    });
}

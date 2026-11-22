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
    /// cópias de trabalho do agente de implementação: (cópia, original). Gravar DENTRO delas não pede
    /// diálogo (o usuário autorizou a cópia inteira); o original só muda via "aplicar", com diálogo + backup.
    copias: Mutex<Vec<(PathBuf, PathBuf)>>,
}

/// Comando chamado direto pelo CHAT do Cerne (página remota, via IPC): mesmas operações da ferramenta
/// "computador", sempre com os diálogos nativos. Assíncrono: os diálogos bloqueiam fora da thread principal.
#[tauri::command]
pub async fn computador_chat(app: AppHandle, op: String, args: Value) -> Value {
    let mut cmd = if args.is_object() { args } else { json!({}) };
    cmd["op"] = json!(op);
    executar(&app, &cmd)
}

fn copia_de(ctl: &Controle, p: &std::path::Path) -> Option<(PathBuf, PathBuf)> {
    // o arquivo (ou a subpasta) pode ainda não existir: sobe até o primeiro ancestral existente
    let c = p.ancestors().find_map(|a| a.canonicalize().ok())?;
    ctl.copias.lock().unwrap().iter().find(|(cp, _)| c.starts_with(cp)).cloned()
}

fn ignorado(nome: &str) -> bool {
    projeto::IGNORAR_DIRS.contains(&nome) || nome == ".git"
}

fn copiar_dir(origem: &std::path::Path, destino: &std::path::Path, n: &mut usize) -> Result<(), String> {
    std::fs::create_dir_all(destino).map_err(|e| e.to_string())?;
    for e in std::fs::read_dir(origem).map_err(|e| e.to_string())?.flatten() {
        let nome = e.file_name().to_string_lossy().to_string();
        let ft = match e.file_type() { Ok(f) => f, Err(_) => continue };
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            if !ignorado(&nome) {
                copiar_dir(&e.path(), &destino.join(&nome), n)?;
            }
        } else if std::fs::copy(e.path(), destino.join(&nome)).is_ok() {
            *n += 1;
        }
    }
    Ok(())
}

/// Arquivos da cópia que diferem do original: (relativo, novo?).
fn diferencas(copia: &std::path::Path, original: &std::path::Path) -> Vec<(String, bool)> {
    let mut v = Vec::new();
    for e in walkdir::WalkDir::new(copia).into_iter()
        .filter_entry(|e| !(e.file_type().is_dir() && ignorado(e.file_name().to_str().unwrap_or(""))))
        .filter_map(|e| e.ok()).filter(|e| e.file_type().is_file())
    {
        let rel = match e.path().strip_prefix(copia) { Ok(r) => r.to_path_buf(), Err(_) => continue };
        let orig = original.join(&rel);
        let rels = rel.to_string_lossy().replace('\\', "/");
        if !orig.exists() {
            v.push((rels, true));
        } else if std::fs::read(e.path()).ok() != std::fs::read(&orig).ok() {
            v.push((rels, false));
        }
    }
    v
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
            let txt = String::from_utf8_lossy(&b).to_string();
            let total = txt.lines().count();
            let ini = cmd.get("linha_inicio").and_then(|x| x.as_u64()).unwrap_or(1).max(1) as usize;
            let fim = cmd.get("linha_fim").and_then(|x| x.as_u64()).map(|x| x as usize).unwrap_or(total).min(total);
            let trecho: String = if ini > 1 || fim < total {
                txt.lines().skip(ini - 1).take(fim.saturating_sub(ini - 1)).collect::<Vec<_>>().join("\n")
            } else {
                txt.clone()
            };
            let trecho: String = trecho.chars().take(100_000).collect();
            Ok(json!({ "caminho": alvo.to_string_lossy().trim_start_matches(r"\\?\"), "linhas_total": total,
                       "linhas": format!("{}-{}", ini, fim), "conteudo": trecho }))
        }
        "buscar" => {
            let (_, pasta) = pasta_de(&s(cmd, "caminho"))?;
            projeto::garantir_leitura(app, &lib, &pasta, quem)?;
            let termo = s(cmd, "termo").to_lowercase();
            if termo.trim().len() < 2 {
                return Err("faltou o termo da busca".into());
            }
            let max = cmd.get("max").and_then(|x| x.as_u64()).unwrap_or(60).min(200) as usize;
            let mut achados = Vec::new();
            'arq: for e in walkdir::WalkDir::new(&pasta).max_depth(12).into_iter()
                .filter_entry(|e| !(e.file_type().is_dir() && ignorado(e.file_name().to_str().unwrap_or(""))))
                .filter_map(|e| e.ok()).filter(|e| e.file_type().is_file() && projeto::eh_texto(e.path()))
            {
                let Ok(b) = std::fs::read(e.path()) else { continue };
                if b.len() > 2_000_000 { continue; }
                let txt = String::from_utf8_lossy(&b);
                let rel = e.path().strip_prefix(&pasta).unwrap_or(e.path()).to_string_lossy().replace('\\', "/");
                for (i, l) in txt.lines().enumerate() {
                    if l.to_lowercase().contains(&termo) {
                        achados.push(format!("{}:{}: {}", rel, i + 1, l.trim().chars().take(160).collect::<String>()));
                        if achados.len() >= max { break 'arq; }
                    }
                }
            }
            Ok(json!({ "pasta": projeto::exibir(&pasta), "termo": termo, "achados": achados }))
        }
        "preparar_copia" => {
            let (_, original) = pasta_de(&s(cmd, "pasta"))?;
            if let Some((cp, _)) = ctl.copias.lock().unwrap().iter().find(|(_, o)| o == &original) {
                return Ok(json!({ "copia": projeto::exibir(cp), "original": projeto::exibir(&original), "reaproveitada": true }));
            }
            let pedido: String = s(cmd, "pedido").chars().take(240).collect();
            let nome = original.file_name().and_then(|n| n.to_str()).unwrap_or("projeto");
            let carimbo = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let destino = original.parent().ok_or("pasta sem pai")?.join(format!("{}-cerne-{}", nome, carimbo));
            let ok = app.dialog()
                .message(format!(
                    "O chat do Cerne vai IMPLEMENTAR uma alteração no seu projeto:\n\"{}\"\n\nEle vai trabalhar numa CÓPIA:\n{}\n\n\
O ORIGINAL ({}) NÃO será alterado. Na cópia o agente lê, grava arquivos e roda comandos (build/testes — cada comando \
pede permissão). No fim você vê o que mudou e decide se aplica no original.\n\nCriar a cópia?",
                    pedido, projeto::exibir(&destino), projeto::exibir(&original)))
                .title("Cerne — trabalhar numa cópia do projeto?")
                .kind(MessageDialogKind::Info)
                .buttons(MessageDialogButtons::OkCancelCustom("Criar cópia e implementar".into(), "Cancelar".into()))
                .blocking_show();
            if !ok {
                return Err("o usuário não autorizou a cópia de trabalho".into());
            }
            let mut n = 0usize;
            copiar_dir(&original, &destino, &mut n)?;
            let destino = destino.canonicalize().map_err(|e| e.to_string())?;
            lib.0.lock().unwrap().insert(destino.clone());
            lib.0.lock().unwrap().insert(original.clone());
            ctl.copias.lock().unwrap().push((destino.clone(), original.clone()));
            Ok(json!({ "copia": projeto::exibir(&destino), "original": projeto::exibir(&original), "arquivos": n,
                       "nota": "node_modules, .git, dist e build não são copiados" }))
        }
        "editar" => {
            // troca um trecho EXATO e ÚNICO de um arquivo da cópia — o resto do arquivo fica intacto
            let p = PathBuf::from(s(cmd, "caminho").trim());
            if copia_de(&ctl, &p).is_none() {
                return Err("editar só funciona dentro da cópia de trabalho".into());
            }
            let trocar = s(cmd, "trocar");
            let por = s(cmd, "por");
            if trocar.is_empty() {
                return Err("faltou o trecho 'trocar' (copie-o exatamente do arquivo)".into());
            }
            let txt = std::fs::read_to_string(&p).map_err(|e| format!("não consegui ler: {}", e))?;
            // tolera diferença de quebra de linha (CRLF × LF) entre o que o modelo copiou e o arquivo
            let (txt_n, trocar_n) = (txt.replace("\r\n", "\n"), trocar.replace("\r\n", "\n"));
            let n = txt_n.matches(&trocar_n).count();
            if n == 0 {
                return Err("o trecho 'trocar' não foi encontrado exatamente no arquivo — leia o arquivo de novo e copie o trecho literal".into());
            }
            if n > 1 {
                return Err(format!("o trecho 'trocar' aparece {} vezes — inclua mais linhas de contexto para ficar único", n));
            }
            let mut novo = txt_n.replacen(&trocar_n, &por.replace("\r\n", "\n"), 1);
            if txt.contains("\r\n") {
                novo = novo.replace('\n', "\r\n");
            }
            std::fs::write(&p, &novo).map_err(|e| e.to_string())?;
            Ok(json!({ "editado": projeto::exibir(&p), "linhas": novo.lines().count() }))
        }
        "diff" => {
            let alvo = s(cmd, "copia");
            let (cp, orig) = copia_de(&ctl, &PathBuf::from(alvo.trim())).ok_or("isso não é uma cópia de trabalho do agente")?;
            let d = diferencas(&cp, &orig);
            Ok(json!({ "copia": projeto::exibir(&cp), "original": projeto::exibir(&orig),
                       "alterados": d.iter().filter(|x| !x.1).map(|x| &x.0).collect::<Vec<_>>(),
                       "novos": d.iter().filter(|x| x.1).map(|x| &x.0).collect::<Vec<_>>() }))
        }
        "aplicar" => {
            let alvo = s(cmd, "copia");
            let (cp, orig) = copia_de(&ctl, &PathBuf::from(alvo.trim())).ok_or("isso não é uma cópia de trabalho do agente")?;
            let d = diferencas(&cp, &orig);
            if d.is_empty() {
                return Ok(json!({ "gravados": [], "nota": "nenhuma diferença entre a cópia e o original" }));
            }
            let arquivos: Vec<ArquivoNovo> = d.iter()
                .filter_map(|(rel, _)| std::fs::read_to_string(cp.join(rel)).ok().map(|c| ArquivoNovo { caminho: rel.clone(), conteudo: c }))
                .collect();
            let g = projeto::gravar_core(app, &lib, &orig.to_string_lossy(), arquivos, "O chat do Cerne (aplicar a implementação)")?;
            Ok(json!({ "gravados": g.gravados, "backup": g.backup, "original": projeto::exibir(&orig) }))
        }
        "gravar" => {
            let caminho = s(cmd, "caminho");
            let p = PathBuf::from(caminho.trim());
            if let Some((cp, _)) = copia_de(&ctl, &p) {
                // dentro da cópia de trabalho: grava direto (o usuário autorizou a cópia), sem sair dela
                let alvo = if p.is_absolute() { p.clone() } else { cp.join(&p) };
                if alvo.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                    return Err("caminho com '..' não é permitido".into());
                }
                if let Some(pai) = alvo.parent() {
                    std::fs::create_dir_all(pai).map_err(|e| e.to_string())?;
                    if !pai.canonicalize().map(|x| x.starts_with(&cp)).unwrap_or(false) {
                        return Err("fora da cópia de trabalho".into());
                    }
                }
                let conteudo = cmd.get("conteudo").and_then(|x| x.as_str()).unwrap_or("");
                // rede de segurança: reescrever um arquivo EXISTENTE "de memória" costuma apagar partes dele
                if let Ok(antigo) = std::fs::read_to_string(&alvo) {
                    let (a, n) = (antigo.lines().count(), conteudo.lines().count());
                    if a >= 15 && n * 10 < a * 7 {
                        return Err(format!(
                            "recusado: o arquivo já existe com {} linhas e o novo conteúdo tem {} — isso apagaria partes dele. \
Para alterar arquivo existente use a ferramenta editar (trocar um trecho exato por outro).", a, n));
                    }
                }
                std::fs::write(&alvo, conteudo).map_err(|e| e.to_string())?;
                return Ok(json!({ "gravado": projeto::exibir(&alvo), "bytes": conteudo.len(), "na_copia": true }));
            }
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
            let segundos = cmd.get("timeout").and_then(|x| x.as_u64()).unwrap_or(120).clamp(5, 900);
            permitir_acao(app, &ctl, &format!("EXECUTAR: {} {}{}", programa, args.join(" "),
                if pasta.is_empty() { String::new() } else { format!("\n(na pasta {})", pasta) }))?;
            rodar(app, &programa, &args, &pasta, segundos)
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

fn rodar(_app: &AppHandle, programa: &str, args: &[String], pasta: &str, segundos: u64) -> Result<Value, String> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let montar = |prog: &str, a: &[String]| {
        let mut c = Command::new(prog);
        c.args(a).stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());
        if !pasta.is_empty() {
            c.current_dir(pasta);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW (apps com janela abrem normalmente)
        }
        c
    };
    let mut ch = match montar(programa, args).spawn() {
        Ok(ch) => ch,
        // no Windows, npm/npx/yarn/pnpm são .cmd — só rodam via cmd /C
        Err(e) if cfg!(windows) && e.kind() == std::io::ErrorKind::NotFound => {
            let mut a = vec!["/C".to_string(), programa.to_string()];
            a.extend(args.iter().cloned());
            montar("cmd", &a).spawn().map_err(|e| format!("não consegui executar '{}': {}", programa, e))?
        }
        Err(e) => return Err(format!("não consegui executar '{}': {}", programa, e)),
    };
    let mut so = ch.stdout.take().unwrap();
    let mut se = ch.stderr.take().unwrap();
    let t1 = std::thread::spawn(move || { let mut b = Vec::new(); let _ = so.read_to_end(&mut b); b });
    let t2 = std::thread::spawn(move || { let mut b = Vec::new(); let _ = se.read_to_end(&mut b); b });
    let fim = Instant::now() + Duration::from_secs(segundos);
    loop {
        if let Some(st) = ch.try_wait().map_err(|e| e.to_string())? {
            let mut saida = String::from_utf8_lossy(&t1.join().unwrap_or_default()).to_string();
            let err = String::from_utf8_lossy(&t2.join().unwrap_or_default()).to_string();
            if !err.is_empty() {
                saida.push_str("\n[stderr]\n");
                saida.push_str(&err);
            }
            let n = saida.chars().count();
            let saida: String = if n > 10000 {
                // começo + fim: erros de build/teste costumam aparecer no fim
                let ini: String = saida.chars().take(2500).collect();
                let fim_: String = saida.chars().skip(n - 7000).collect();
                format!("{}\n…[saída cortada]…\n{}", ini, fim_)
            } else { saida };
            return Ok(json!({ "codigo": st.code().unwrap_or(-1), "saida": saida }));
        }
        if Instant::now() > fim {
            let _ = ch.kill();
            return Ok(json!({ "codigo": -1, "tempo_esgotado": true,
                              "nota": format!("passou de {}s e foi interrompido — use um timeout maior ou um comando mais específico", segundos) }));
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

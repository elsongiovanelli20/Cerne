// Acesso a pastas locais pedido pelo chat do Cerne (página remota).
//
// Regras de segurança (a página remota NUNCA decide sozinha):
//  * toda leitura de uma pasta nova passa por um diálogo NATIVO de permissão;
//  * toda gravação passa por OUTRO diálogo nativo, listando os arquivos;
//  * gravação só dentro de uma pasta já liberada, sem `..`/caminho absoluto;
//  * o original é copiado para `<pasta>/.cerne-backup/<timestamp>/` antes de sobrescrever.

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind, MessageDialogResult};

#[derive(Default)]
pub struct Liberadas(pub(crate) Mutex<HashSet<PathBuf>>);

pub(crate) const IGNORAR_DIRS: &[&str] = &[
    ".git", "node_modules", "target", "dist", "build", "out", "__pycache__", ".venv", "venv",
    "env", ".next", ".nuxt", "bin", "obj", ".idea", ".vscode", ".cerne-backup", "coverage",
    ".gradle", ".dart_tool", "Pods", "vendor",
];
const EXT_TEXTO: &[&str] = &[
    "py", "js", "jsx", "ts", "tsx", "mjs", "cjs", "json", "md", "txt", "html", "htm", "css",
    "scss", "sass", "less", "vue", "svelte", "java", "kt", "kts", "cs", "go", "rs", "rb", "php",
    "c", "h", "cpp", "hpp", "cc", "swift", "dart", "sql", "sh", "ps1", "bat", "cmd", "yml",
    "yaml", "toml", "ini", "cfg", "conf", "xml", "csv", "env.example", "gradle", "properties",
    "r", "m", "lua", "pl", "ex", "exs", "erl", "scala", "graphql", "proto", "tf", "dockerfile",
];
const NOMES_TEXTO: &[&str] = &["Dockerfile", "Makefile", "Procfile", "README", "LICENSE", ".gitignore", ".env.example"];
const MAX_POR_ARQUIVO: usize = 20_000;
const MAX_TOTAL: usize = 350_000;
const MAX_ARVORE: usize = 300;

fn sem_acento(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            x => x,
        })
        .collect()
}

/// Termos úteis da pergunta (sem acento, ≥4 letras, sem palavras genéricas), cortados em 8 letras
/// para casar variações ("classificação" ~ "classificacao.ts", "regras" ~ "regra").
fn termos_da_consulta(q: &str) -> Vec<String> {
    let stop = ["para", "com", "que", "uma", "por", "como", "mais", "sobre", "esse", "essa", "este", "esta", "isso",
                "qual", "quais", "onde", "quando", "sendo", "seja", "analise", "analisar", "codigo", "pasta", "programa",
                "projeto", "fale", "diga", "desenvolvido", "arquitetura", "users", "user", "documents", "chatgpt"];
    let mut v: Vec<String> = sem_acento(q)
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|w| w.chars().count() >= 4 && !stop.contains(w))
        .map(|w| w.chars().take(8).collect::<String>())
        .collect();
    v.sort();
    v.dedup();
    v
}

/// Importância de um arquivo para ENTENDER o projeto (menor = antes). Espelha `_pasta_prioridade` do servidor.
fn prioridade(rel: &str) -> u32 {
    let c = rel.to_lowercase();
    let nome = c.rsplit('/').next().unwrap_or("").to_string();
    let prof = c.matches('/').count() as u32;
    let locks = ["package-lock.json", "pnpm-lock.yaml", "yarn.lock", "poetry.lock", "cargo.lock", "composer.lock", "gemfile.lock"];
    if locks.contains(&nome.as_str()) || nome.ends_with(".min.js") || nome.ends_with(".map") || nome.ends_with(".svg") {
        return 999;
    }
    if nome.contains(".spec.") || nome.contains(".test.") || c.contains("/test/") || c.contains("/tests/")
        || c.contains("/e2e/") || c.contains("__tests__") || c.contains("__golden__") || c.starts_with("e2e/") {
        return 700 + prof.min(9);
    }
    if nome.starts_with("readme") || nome.starts_with("leia-me") || nome.contains("arquitetura") || nome.contains("architecture") {
        return prof.min(9);
    }
    let manifestos = ["package.json", "pyproject.toml", "requirements.txt", "cargo.toml", "go.mod", "pom.xml",
                      "build.gradle", "composer.json", "gemfile", "pnpm-workspace.yaml", "turbo.json", "setup.py"];
    if manifestos.contains(&nome.as_str()) || nome.ends_with(".csproj") {
        return 100 + prof.min(9);
    }
    let exts = [".ts", ".tsx", ".js", ".jsx", ".py", ".java", ".kt", ".cs", ".go", ".rs", ".rb", ".php", ".sql", ".vue", ".dart"];
    let codigo = exts.iter().any(|e| nome.ends_with(e));
    let dominio = ["regra", "rule", "valid", "classific", "elegib", "habilit", "polic", "calc", "status", "fluxo",
                   "workflow", "prazo", "edital", "service", "controller", "entity", "entidade", "model", "domain",
                   "dominio", "usecase"];
    let entrada = ["main.", "app.", "index.", "server.", "settings.", "urls.", "routes."];
    if codigo && !nome.ends_with(".sql") && (dominio.iter().any(|d| nome.contains(d)) || entrada.iter().any(|e| nome.starts_with(e))) {
        return 200 + prof.min(9);
    }
    if c.starts_with("docs/") || c.contains("/docs/") || (nome.ends_with(".md") && prof <= 1) {
        return 250 + prof.min(9);
    }
    if codigo {
        return 350 + prof.min(9);
    }
    if nome.starts_with("dockerfile") || nome.starts_with("docker-compose") || nome.starts_with("cloudbuild")
        || c.contains("/workflows/") || nome.ends_with(".tf") || nome.ends_with(".yml") || nome.ends_with(".yaml") {
        return 400 + prof.min(9);
    }
    500 + prof.min(9)
}

/// Acha o caminho real dentro de um trecho de texto com espaços
/// ("C:\Users\X\M-Tech mas retornou…" → "C:\Users\X\M-Tech").
fn resolver(candidato: &str) -> Option<PathBuf> {
    let limpo = candidato
        .trim()
        .trim_matches(|c: char| c == '"' || c == '\'' || c == '`' || c == '“' || c == '”');
    let palavras: Vec<&str> = limpo.split(' ').collect();
    for n in (1..=palavras.len()).rev() {
        let s = palavras[..n]
            .join(" ")
            .trim_end_matches(|c: char| ",.;:!?)]}\"'`”".contains(c))
            .to_string();
        if s.len() < 3 {
            continue;
        }
        let p = PathBuf::from(&s);
        if p.exists() {
            return p.canonicalize().ok();
        }
    }
    None
}

pub(crate) fn eh_texto(p: &Path) -> bool {
    let nome = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if NOMES_TEXTO.iter().any(|n| nome.eq_ignore_ascii_case(n)) {
        return true;
    }
    match p.extension().and_then(|e| e.to_str()) {
        Some(e) => EXT_TEXTO.contains(&e.to_lowercase().as_str()),
        None => false,
    }
}

pub(crate) fn exibir(p: &Path) -> String {
    // canonicalize no Windows gera "\\?\C:\..." — esconde o prefixo nas mensagens.
    p.to_string_lossy().trim_start_matches(r"\\?\").to_string()
}

#[derive(Serialize)]
pub struct Pacote {
    pasta: String,
    nome: String,
    total_arquivos: usize,
    arquivos_lidos: usize,
    truncado: bool,
    texto: String,
}

/// Pede (uma vez por sessão) a liberação NATIVA de leitura de uma pasta. `quem` aparece no diálogo.
pub fn garantir_leitura(app: &AppHandle, lib: &Liberadas, raiz: &Path, quem: &str) -> Result<(), String> {
    if lib.0.lock().unwrap().iter().any(|l| raiz.starts_with(l)) {
        return Ok(());
    }
    let ok = app
        .dialog()
        .message(format!(
            "{} pediu para LER:

{}

Somente leitura de arquivos (ignora node_modules, .git, binários). O conteúdo vai para o modelo de IA escolhido.

Permitir?",
            quem,
            exibir(raiz)
        ))
        .title("Cerne — permitir acesso à pasta?")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom("Permitir leitura".into(), "Negar".into()))
        .blocking_show();
    if !ok {
        return Err("acesso negado pelo usuário".into());
    }
    lib.0.lock().unwrap().insert(raiz.to_path_buf());
    Ok(())
}

/// Lê (somente leitura) uma pasta ou arquivo citado no chat, após permissão nativa.
#[tauri::command]
pub async fn ler_pasta_projeto(
    app: AppHandle,
    lib: State<'_, Liberadas>,
    candidato: String,
    consulta: Option<String>,
) -> Result<Pacote, String> {
    let termos = termos_da_consulta(consulta.as_deref().unwrap_or(""));
    let raiz = resolver(&candidato).ok_or_else(|| format!("não encontrei no seu computador o caminho \"{}\"", candidato.trim()))?;
    garantir_leitura(&app, &lib, &raiz, "O chat do Cerne")?;

    let nome = raiz.file_name().and_then(|n| n.to_str()).unwrap_or("pasta").to_string();
    let mut arvore: Vec<String> = Vec::new();
    let mut corpo = String::new();
    let (mut total, mut lidos, mut truncado) = (0usize, 0usize, false);

    let arquivos: Vec<PathBuf> = if raiz.is_file() {
        vec![raiz.clone()]
    } else {
        walkdir::WalkDir::new(&raiz)
            .max_depth(10)
            .sort_by_file_name()
            .into_iter()
            .filter_entry(|e| !(e.file_type().is_dir() && IGNORAR_DIRS.contains(&e.file_name().to_str().unwrap_or(""))))
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .map(|e| e.into_path())
            .collect()
    };
    let base = if raiz.is_file() { raiz.parent().unwrap_or(&raiz).to_path_buf() } else { raiz.clone() };

    // 1ª passada: árvore + estrutura por diretório + candidatos de texto com PRIORIDADE (o que explica o projeto
    // e suas REGRAS vem antes; ordem alfabética enchia o limite com .github/ e o primeiro app)
    let mut dirs: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let mut candidatos: Vec<(u32, String, &PathBuf)> = Vec::new();
    for p in &arquivos {
        total += 1;
        let rel = p.strip_prefix(&base).unwrap_or(p).to_string_lossy().replace('\\', "/");
        let partes: Vec<&str> = rel.split('/').collect();
        let d = if partes.len() >= 3 { format!("{}/{}", partes[0], partes[1]) } else if partes.len() == 2 { partes[0].to_string() } else { "(raiz)".into() };
        *dirs.entry(d).or_insert(0) += 1;
        if arvore.len() < MAX_ARVORE {
            arvore.push(rel.clone());
        }
        if eh_texto(p) {
            let pr = prioridade(&rel);
            if pr < 990 {
                candidatos.push((pr, rel, p));
            }
        }
    }
    // relevância à PERGUNTA dentro do código (domínio 2xx e demais fontes 3xx): termos no caminho e no início do
    // arquivo puxam para cima — projetos grandes têm centenas de controllers/services e o pacote não cabe todos
    let mut relev: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    if !termos.is_empty() {
        for (pr, rel, p) in &candidatos {
            if (200..400).contains(pr) {
                let mut alvo = sem_acento(rel);
                if let Ok(b) = std::fs::read(p) {
                    alvo.push(' ');
                    alvo.push_str(&sem_acento(&String::from_utf8_lossy(&b[..b.len().min(4096)])));
                }
                let h = termos.iter().filter(|t| alvo.contains(t.as_str())).count().min(4) as u32;
                relev.insert(rel.clone(), h);
            }
        }
    }
    let catg = |pr: u32| -> u32 { if (250..300).contains(&pr) { 3 } else if pr >= 300 { pr / 100 + 1 } else { pr / 100 } };
    candidatos.sort_by(|a, b| {
        catg(a.0).cmp(&catg(b.0))
            .then_with(|| relev.get(&b.1).unwrap_or(&0).cmp(relev.get(&a.1).unwrap_or(&0)))
            .then_with(|| a.0.cmp(&b.0))
            .then_with(|| a.1.cmp(&b.1))
    });
    // diversidade: intercala o código por MÓDULO (ex.: apps/auth, apps/solicitacoes…) — o 1º de cada módulo,
    // depois o 2º… assim uma pergunta geral vê o sistema todo, não só o primeiro app em ordem alfabética
    let grupo = |rel: &str| rel.split('/').take(3).collect::<Vec<_>>().join("/");
    let mut cont: std::collections::HashMap<(u32, String), u32> = std::collections::HashMap::new();
    let mut ordem_grupo: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for (pr, rel, _) in &candidatos {
        let c = catg(*pr);
        let r = cont.entry((c, grupo(rel))).or_insert(0);
        ordem_grupo.insert(rel.clone(), *r);
        *r += 1;
    }
    candidatos.sort_by(|a, b| {
        catg(a.0).cmp(&catg(b.0))
            .then_with(|| relev.get(&b.1).unwrap_or(&0).cmp(relev.get(&a.1).unwrap_or(&0)))
            .then_with(|| ordem_grupo.get(&a.1).unwrap_or(&0).cmp(ordem_grupo.get(&b.1).unwrap_or(&0)))
            .then_with(|| a.0.cmp(&b.0))
            .then_with(|| a.1.cmp(&b.1))
    });
    let estrutura = dirs.iter().map(|(d, n)| format!("{}: {}", d, n)).collect::<Vec<_>>().join(", ");

    // cota por categoria (READMEs de subpasta são dezenas e engoliam o pacote); o resto fica para o CÓDIGO
    let cota = |cat: u32| -> usize {
        match cat { 0 => 50_000, 1 => 20_000, 2 => usize::MAX, 3 => usize::MAX, 4 => 15_000, 5 => 10_000, 7 => 10_000, _ => 40_000 }
    };
    let mut usado_cat: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    for (pr, rel, p) in candidatos {
        if corpo.len() >= MAX_TOTAL {
            truncado = true;
            continue;
        }
        let cat = if pr >= 250 && pr < 300 { 6 } else { pr / 100 };   // docs (250+) têm cota própria
        let ja = *usado_cat.get(&cat).unwrap_or(&0);
        if ja >= cota(cat) {
            truncado = true;
            continue;
        }
        let bytes = match std::fs::read(p) {
            Ok(b) => b,
            Err(_) => continue,
        };
        if bytes.iter().take(4096).any(|b| *b == 0) {
            continue; // binário disfarçado
        }
        let mut txt = String::from_utf8_lossy(&bytes).to_string();
        // README/docs de subpasta: trecho menor; README da raiz e código: até MAX_POR_ARQUIVO
        let max_arq = if (cat == 0 && pr > 1) || cat == 6 { 6_000 } else { MAX_POR_ARQUIVO };
        if txt.len() > max_arq {
            let mut corte = max_arq;
            while !txt.is_char_boundary(corte) {
                corte -= 1;
            }
            txt.truncate(corte);
            txt.push_str("\n…[arquivo truncado]");
            truncado = true;
        }
        *usado_cat.entry(cat).or_insert(0) += txt.len();
        corpo.push_str(&format!("\n\n===== ARQUIVO: {} =====\n{}", rel, txt));
        lidos += 1;
    }

    let texto = format!(
        "[PASTA LOCAL LIDA PELO CERNE DESKTOP]\nPasta: {pasta}\nArquivos: {total} no total · {lidos} de texto/código incluídos{trunc}\n\n\
INSTRUÇÕES: este é o código-fonte real do projeto do usuário; analise-o diretamente (não peça para enviar o código). \
Se o usuário pedir alterações, além de explicar, devolva cada arquivo alterado COMPLETO neste formato exato, \
com caminho relativo à pasta:\n<<<ARQUIVO: caminho/relativo.ext>>>\n(conteúdo completo do arquivo)\n<<<FIM>>>\n\
O Cerne Desktop oferecerá ao usuário um botão para gravar (com backup automático).\n\n\
ESTRUTURA COMPLETA (diretório: nº de arquivos): {estrutura}\n\n\
Os arquivos abaixo vêm em ORDEM DE IMPORTÂNCIA (README, manifestos, código de domínio com as regras, docs, infra; testes por último).\n\n\
ÁRVORE DE ARQUIVOS:\n{arvore}{mais}{corpo}",
        estrutura = estrutura,
        pasta = exibir(&base),
        total = total,
        lidos = lidos,
        trunc = if truncado { " (parte do conteúdo foi truncada por tamanho)" } else { "" },
        arvore = arvore.join("\n"),
        mais = if total > MAX_ARVORE { format!("\n… e mais {} arquivos", total - MAX_ARVORE) } else { String::new() },
        corpo = corpo,
    );
    Ok(Pacote { pasta: exibir(&base), nome, total_arquivos: total, arquivos_lidos: lidos, truncado, texto })
}

#[derive(Deserialize)]
pub struct ArquivoNovo {
    pub caminho: String,
    pub conteudo: String,
}

#[derive(Serialize)]
pub struct Gravacao {
    pub gravados: Vec<String>,
    pub backup: Option<String>,
}

fn rel_seguro(rel: &str) -> Option<PathBuf> {
    let p = PathBuf::from(rel.trim().replace('/', std::path::MAIN_SEPARATOR_STR));
    if p.as_os_str().is_empty() || p.is_absolute() {
        return None;
    }
    if p.components().any(|c| !matches!(c, Component::Normal(_))) {
        return None; // bloqueia "..", "C:", raiz
    }
    Some(p)
}

/// "app.py" → "app (cerne).py", "app (cerne 2).py", … (primeiro nome livre).
fn nome_livre(abs: &Path, raiz: &Path) -> (PathBuf, String) {
    let pai = abs.parent().unwrap_or(raiz);
    let stem = abs.file_stem().and_then(|s| s.to_str()).unwrap_or("arquivo");
    let ext = abs.extension().and_then(|e| e.to_str()).map(|e| format!(".{}", e)).unwrap_or_default();
    let mut n = 1;
    loop {
        let nome = if n == 1 { format!("{} (cerne){}", stem, ext) } else { format!("{} (cerne {}){}", stem, n, ext) };
        let cand = pai.join(nome);
        if !cand.exists() {
            let rel = cand.strip_prefix(raiz).unwrap_or(&cand).to_string_lossy().to_string();
            return (cand, rel);
        }
        n += 1;
    }
}

/// Grava arquivos propostos pelo chat dentro de uma pasta JÁ liberada para leitura,
/// após confirmação nativa e com backup dos originais.
#[tauri::command]
pub async fn gravar_arquivos_projeto(
    app: AppHandle,
    lib: State<'_, Liberadas>,
    pasta: String,
    arquivos: Vec<ArquivoNovo>,
) -> Result<Gravacao, String> {
    gravar_core(&app, &lib, &pasta, arquivos, "O chat do Cerne")
}

pub fn gravar_core(app: &AppHandle, lib: &Liberadas, pasta: &str, arquivos: Vec<ArquivoNovo>, quem: &str) -> Result<Gravacao, String> {
    if arquivos.is_empty() {
        return Err("nenhum arquivo para gravar".into());
    }
    let raiz = PathBuf::from(pasta.trim()).canonicalize().map_err(|_| "pasta não encontrada".to_string())?;
    if !raiz.is_dir() || !lib.0.lock().unwrap().iter().any(|l| raiz.starts_with(l) || l == &raiz) {
        return Err("essa pasta não foi liberada nesta sessão — peça a leitura dela no chat primeiro".into());
    }
    let mut alvos: Vec<(PathBuf, String, &str)> = Vec::new();
    for a in &arquivos {
        let rel = rel_seguro(&a.caminho).ok_or_else(|| format!("caminho inválido: {}", a.caminho))?;
        alvos.push((raiz.join(&rel), rel.to_string_lossy().to_string(), a.conteudo.as_str()));
    }
    let lista: Vec<String> = alvos
        .iter()
        .take(20)
        .map(|(abs, rel, _)| format!("{} {}", if abs.exists() { "✎" } else { "+" }, rel))
        .collect();
    let existentes = alvos.iter().filter(|(abs, _, _)| abs.exists()).count();
    let msg = format!(
        "{} quer GRAVAR {} arquivo(s) em:
{}

{}{}

(✎ = já existe · + = novo)",
        quem,
        alvos.len(),
        exibir(&raiz),
        lista.join("
"),
        if alvos.len() > 20 { format!("
… e mais {}", alvos.len() - 20) } else { String::new() }
    );
    // novo_ao_lado = true → arquivos existentes NÃO são tocados; grava "nome (cerne).ext"
    let novo_ao_lado = if existentes > 0 {
        let r = app
            .dialog()
            .message(format!(
                "{}

{} arquivo(s) já existe(m). O que fazer?

• Sobrescrever — substitui (o original vai para .cerne-backup)
• Criar novo — mantém o original e grava uma cópia \"nome (cerne).ext\" ao lado",
                msg, existentes
            ))
            .title("Cerne — sobrescrever ou criar novo?")
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::YesNoCancelCustom("Sobrescrever".into(), "Criar novo".into(), "Cancelar".into()))
            .blocking_show_with_result();
        match r {
            MessageDialogResult::Yes => false,
            MessageDialogResult::No => true,
            MessageDialogResult::Custom(t) if t == "Sobrescrever" => false,
            MessageDialogResult::Custom(t) if t == "Criar novo" => true,
            _ => return Err("gravação cancelada por você".into()),
        }
    } else {
        let ok = app
            .dialog()
            .message(format!("{}

Permitir a gravação?", msg))
            .title("Cerne — permitir gravação?")
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom("Gravar".into(), "Cancelar".into()))
            .blocking_show();
        if !ok {
            return Err("gravação cancelada por você".into());
        }
        false
    };

    let carimbo = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let dir_bk = raiz.join(".cerne-backup").join(carimbo.to_string());
    let mut usou_bk = false;
    let mut gravados = Vec::new();
    for (abs0, rel0, conteudo) in alvos {
        let (abs, rel) = if novo_ao_lado && abs0.exists() { nome_livre(&abs0, &raiz) } else { (abs0, rel0) };
        if let Some(pai) = abs.parent() {
            std::fs::create_dir_all(pai).map_err(|e| format!("{}: {}", rel, e))?;
            // defesa extra contra links simbólicos/junctions apontando para fora
            let pai_c = pai.canonicalize().map_err(|e| format!("{}: {}", rel, e))?;
            if !pai_c.starts_with(&raiz) {
                return Err(format!("{}: fora da pasta liberada", rel));
            }
        }
        if abs.exists() {
            let bk = dir_bk.join(&rel);
            if let Some(p) = bk.parent() {
                let _ = std::fs::create_dir_all(p);
            }
            std::fs::copy(&abs, &bk).map_err(|e| format!("backup de {} falhou: {}", rel, e))?;
            usou_bk = true;
        }
        std::fs::write(&abs, conteudo).map_err(|e| format!("{}: {}", rel, e))?;
        gravados.push(rel.replace('\\', "/"));
    }
    Ok(Gravacao { gravados, backup: if usou_bk { Some(exibir(&dir_bk)) } else { None } })
}

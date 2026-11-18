// Manifesto dos comandos do app: cada comando vira uma permissão "allow-<nome>".
// Assim a página REMOTA do Cerne só recebe os comandos listados em capabilities/remote.json
// (ler/gravar pasta com diálogo nativo) — nunca rodar_comando, escrever_arquivo etc.
const COMANDOS: &[&str] = &[
    "conectar", "escolher_pasta", "ingerir_pasta",
    "listar_dir", "ler_arquivo", "escrever_arquivo", "abrir_caminho", "rodar_comando",
    "digitar_texto", "pressionar_tecla", "verificar_atualizacao",
    "abrir_navegador", "configurar_navegador", "status_navegador",
    "ler_pasta_projeto", "gravar_arquivos_projeto", "computador_chat",
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(COMANDOS)),
    )
    .expect("falha no build do Tauri");
}

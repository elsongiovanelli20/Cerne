# Cerne Desktop

App soberano do Cerne para Windows (Tauri v2 · Rust + WebView2, **sem Chromium empacotado** → instalador ~3 MB).

## O que faz
- **Workspace nativo**: janela que carrega `https://cerne.orqion-ia.com.br` com login persistente + bandeja (tray) + instância única.
- **Ingerir pasta local** (bandeja → *Ingerir pasta local…*): percorre uma pasta do PC e envia os documentos (PDF/Word/texto/HTML/CSV/imagens, até 6 níveis) direto para o acervo do Cerne (mascarados/indexados no servidor).
- **Agente local** (bandeja → *Agente local…*): acessa arquivos (listar/ler/escrever), abre apps/URLs, roda comandos e simula teclado — capacidades que só o desktop tem.
- **Abrir o navegador quando fechado** (bandeja → *Conectar navegador…*): cole o mesmo código de pareamento gerado no Cerne (Agentes → Navegador do usuário). O desktop mantém contato com o servidor (`/desktop/hello` + long-poll `/desktop/poll`) e, quando um agente pede para operar o navegador com ele **fechado**, o Cerne Desktop abre o Chrome/Edge automaticamente — e a extensão assume o controle (com a moldura). A extensão sozinha só age com o navegador já aberto; o desktop cobre o caso do navegador fechado.
- **Auto-update**: verifica `https://cerne.orqion-ia.com.br/download/desktop-manifest`, baixa e instala versões novas (verificadas por assinatura).

## Requisitos de build
- Rust (cargo), Node + npm, WebView2 (nativo no Windows 10/11).

## Build
```bash
cd desktop
npm install
# assina o artifact de auto-update (obrigatório porque createUpdaterArtifacts=true)
TAURI_SIGNING_PRIVATE_KEY="$(cat cerne_updater.key)" TAURI_SIGNING_PRIVATE_KEY_PASSWORD="" npx tauri build
```
Saídas em `src-tauri/target/release/bundle/nsis/`:
- `Cerne Desktop_1.0.0_x64-setup.exe` — instalador
- `...setup.exe.sig` — assinatura do updater (vai no `latest.json`)

> **Guarde `cerne_updater.key` (e a senha) em segredo.** Sem ela não é possível assinar atualizações e o auto-update para de funcionar.

## Publicar uma nova versão (release)
1. Suba a versão em `src-tauri/tauri.conf.json` (`"version"`) e em `Cargo.toml`.
2. `npx tauri build` (com as env de assinatura acima).
3. Envie o novo `...setup.exe` para o servidor em `/data/app/CerneDesktop-Setup-<versão>.exe` (o `/download/cerne-desktop` serve esse arquivo).
4. Gere o `latest.json` com a nova versão + conteúdo do `.sig` + `url` do instalador e coloque em `/data/app/latest.json` (servido por `/download/desktop-manifest`).
5. Atualize `_DESKTOP_EXE`/`_DESKTOP_VER`/`_DESKTOP_SHA` no `app/main.py` se mudar o nome/versão.
Os apps instalados detectam a nova versão e se atualizam sozinhos.

## Assinatura de código (Authenticode → remover o aviso do SmartScreen)
O instalador **não é assinado com Authenticode** hoje, então o Windows SmartScreen avisa (“editor desconhecido”). Isso **exige um certificado de assinatura de código de uma Autoridade Certificadora (OV ou EV, pago)** — a organização precisa adquiri-lo. Assinatura self-signed **não** remove o aviso.

Quando tiver o certificado (instalado no repositório de certificados do Windows), o pipeline fica pronto com **uma linha**: em `tauri.conf.json` → `bundle.windows`:
```json
"certificateThumbprint": "<THUMBPRINT_DO_CERTIFICADO>",
"digestAlgorithm": "sha256",
"timestampUrl": "http://timestamp.digicert.com"
```
e rebuild. O Tauri assina o `.exe` **antes** de gerar o `.sig` do updater, então tudo permanece coerente.
Para gerar o thumbprint de um certificado já instalado: `Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert`.

## Segurança / soberania
- A ingestão e o auto-update usam a sessão da sua organização (login por e-mail/senha).
- O agente local só age quando **você** aciona (abrir pasta, rodar comando, digitar). É uma ferramenta de automação assistida, instalada e governada pela sua organização.

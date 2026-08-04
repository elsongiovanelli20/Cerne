# Cerne — Copiloto do Navegador (extensão Chrome / Edge)

Deixa o seu **agente Cerne** operar, com a sua permissão, o navegador que você **já usa** — abrir páginas, ler
conteúdo, preencher campos, clicar e baixar arquivos. Enquanto o Cerne atua, uma **moldura esfumaçada nas cores do
Cerne** aparece em volta da página, para você sempre saber quando o agente está no controle.

Tudo é **soberano**: a extensão fala **apenas com o seu servidor Cerne** (ex.: `https://cerne.orqion-ia.com.br`),
por long‑poll. Não há servidores de terceiros no meio.

## Como instalar (modo desenvolvedor — Chrome e Edge)

1. Baixe e descompacte a extensão (botão **Baixar extensão** na tela **Baixar App** do Cerne, ou o `.zip` gerado).
2. Abra `chrome://extensions` (ou `edge://extensions`).
3. Ative o **Modo do desenvolvedor** (canto superior).
4. Clique em **Carregar sem compactação** (*Load unpacked*) e selecione a pasta `browser/` descompactada.
5. Fixe a extensão na barra (ícone do Cerne).

> Em organizações, o admin pode publicar a extensão na **Chrome Web Store / Edge Add‑ons** ou forçar a instalação
> por política (`ExtensionInstallForcelist`) para todos os computadores — sem modo desenvolvedor.

## Como parear

1. No Cerne, vá em **Agentes → Navegador do usuário** e clique em **Gerar código**.
2. Clique no ícone da extensão, confirme o **Servidor Cerne**, cole o **código de pareamento** e clique em
   **Parear e conectar**. O ponto fica **ciano** quando conectado.

## Como usar

Peça a um agente que tenha a ferramenta **Navegador** (ex.: o agente pronto *Operador de Navegador*). Exemplos:

- "Abra `portal.exemplo.com/relatorios`, encontre o link do relatório de julho e **baixe**."
- "Entre na página X, leia a tabela e me traga os valores."
- "Preencha o campo de busca com 'contrato 2025' e clique em pesquisar."

Enquanto o agente opera, a **moldura do Cerne** fica visível na aba. A moldura some sozinha alguns segundos após a
última ação (rede de segurança), e você pode **pausar** o controle a qualquer momento pelo popup.

## Permissões e por quê

- `tabs`, `activeTab`, `scripting` — abrir/ler/interagir com a aba ativa que você autorizou.
- `downloads` — baixar arquivos que o agente localizar.
- `storage` — guardar o servidor e o código de pareamento localmente.
- `<all_urls>` — para poder operar a página que você indicar (a extensão só age quando o Cerne envia um comando
  seu; fora disso, fica ociosa).

## Privacidade

A extensão só executa comandos vindos do **seu** Cerne, autenticados pelo **código de pareamento** do seu usuário.
Nada é enviado a terceiros. Pause ou despareie quando quiser pelo popup.

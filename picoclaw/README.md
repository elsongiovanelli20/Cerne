# PicoClaw no Cerne (fase A — servidor)

[PicoClaw](https://github.com/sipeed/picoclaw) (MIT, Go) roda como **porta de entrada por mensagem** do Cerne
(Telegram hoje; WhatsApp depois) e usa as ferramentas do Cerne pelo **servidor MCP** em `POST /mcp`.

```
Telegram ──► picoclaw (container) ──► app:/mask-proxy/v1/messages ──► gateway (LiteLLM) ──► NVIDIA/Groq
                      │                    (PII mascarada/de-mascarada)
                      └──► app:/mcp  (buscar_acervo · gerar_documento · listar/executar_agente ·
                                      solicitar/consultar_aprovacao · lembrar · data_hora)
```

- **Modelos:** Nemotron 3 Ultra (principal) com reserva Mistral Large 4 → GPT-OSS 120B; mensagens simples vão
  para o Nemotron 3.5 Lightning (`routing.light_model`). Tudo via proxy mascarador (PII não sai).
- **Segurança:** no servidor o shell (`exec`) fica **desligado** e a escrita é restrita ao workspace do PicoClaw.
  O servidor MCP usa um token próprio (`CERNE_MCP_TOKEN`) e roda como `picoclaw@runtime` com perfil
  `colaborador`. Ações destrutivas passam por `solicitar_aprovacao` → **Workspace → Fila** no Cerne.
  Links de arquivo são assinados e expiram em 24h.
- **Auditoria:** cada chamada de ferramenta gera evento `mcp · ferramenta` no registro do Cerne.

## Ligar o Telegram

1. No Telegram, fale com **@BotFather** → `/newbot` → copie o token.
2. Fale com **@userinfobot** para descobrir o seu ID numérico.
3. No `.env` do servidor:
   ```
   TELEGRAM_BOT_TOKEN=123456:ABC...
   TELEGRAM_ALLOW_FROM=111111111,222222222   # só esses IDs podem usar o bot
   ```
4. `python3 picoclaw/render_config.py && docker compose up -d picoclaw`

Sem `TELEGRAM_ALLOW_FROM` o canal **não liga** (lista vazia = qualquer pessoa poderia usar o bot).

## Arquivos

- `config.template.json` — config sem segredos (versionada).
- `render_config.py` — gera `data/config.json` a partir do `.env` (fora do git, permissão 600).
- `workspace/AGENT.md` — instruções do assistente (PT-BR, regra de aprovação).
- `data/` — estado do PicoClaw (sessões, memória, config gerada) — **não versionar**.

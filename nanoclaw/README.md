# Cerne · Camada de Agentes (NanoClaw)

Runtime de **agentes de IA** do Cerne, baseado no [NanoClaw](https://github.com/nanocoai/nanoclaw)
(MIT). Cada agente roda em container Docker isolado; a inferência é **soberana**, roteada
pelo gateway do Cerne (LiteLLM) para o Ollama local — nada sai da máquina.

## A ponte de modelos (Fase 1 — PROVADA)

O gateway do Cerne já expõe um endpoint **compatível com Anthropic** (`/v1/messages`),
que é o que o Claude Agent SDK (base do NanoClaw) consome. Apontando o NanoClaw para o
gateway, os agentes usam modelos **locais** sem nenhuma chave da Anthropic.

Verificado em produção (2026-07-17):

```bash
curl http://localhost:4000/v1/messages \
  -H "x-api-key: $LITELLM_MASTER_KEY" -H "anthropic-version: 2023-06-01" \
  -H "Content-Type: application/json" \
  -d '{"model":"agent","max_tokens":25,"messages":[{"role":"user","content":"responda só: ok soberano"}]}'
# → {"type":"message","model":"agent","content":[{"type":"text","text":"ok soberano"}], ...}
```

Aliases soberanos registrados no gateway (`gateway/litellm-config.yaml`):
`agent`, `claude-3-5-sonnet-20241022`, `claude-3-5-haiku-20241022` → todos para o Ollama local.
Assim o NanoClaw funciona apontado ao gateway mesmo que o Claude Agent SDK peça um nome
de modelo "claude-*" por padrão.

## Como conectar o NanoClaw ao gateway

O Claude Agent SDK honra `ANTHROPIC_BASE_URL` e `ANTHROPIC_API_KEY`. Configure o NanoClaw
com o `.env` deste diretório (veja `.env.example`):

```
ANTHROPIC_BASE_URL=http://cerne-gateway:4000   # gateway do Cerne na rede interna do compose
ANTHROPIC_API_KEY=${LITELLM_MASTER_KEY}        # a chave mestra do gateway (não é chave da Anthropic)
ANTHROPIC_MODEL=agent                          # alias soberano → Ollama local
```

- **100% soberano:** troque só o `ANTHROPIC_MODEL` por `chat` ou outro alias local.
- **Agente mais forte (híbrido):** aponte o alias `agent` no `litellm-config.yaml` para um
  provedor externo — a PII já é mascarada pelo pipeline do Cerne e o consumo entra na
  observabilidade e na governança. Nada muda no NanoClaw.

## Instalação (runtime dedicado — recomendado)

> ⚠️ **Não instalar no servidor de produção atual sem antes liberar recursos.**
> Em 2026-07-17 o servidor `orqion-arm` estava com **~330 MB de RAM livre** (23 GB usados
> de 24) e **disco a 78%**, e **sem Node/pnpm**. O NanoClaw exige Node 20+, pnpm 10+ e roda
> um container Docker por grupo de agentes — isso pressionaria a stack do Cerne (risco de OOM).
> Rode o runtime de agentes em uma **instância dedicada** (ou uma VM ARM adicional na Oracle),
> na mesma VPC, com acesso ao gateway do Cerne pela rede interna.

Passos (na instância de agentes):

```bash
# 1. dependências
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash - && sudo apt-get install -y nodejs
sudo npm install -g pnpm
# Docker já deve estar instalado

# 2. NanoClaw
git clone https://github.com/nanocoai/nanoclaw && cd nanoclaw
cp /caminho/deste/repo/nanoclaw/.env.example .env    # e ajuste ANTHROPIC_BASE_URL/KEY
bash nanoclaw.sh                                       # instalador oficial

# 3. apontar para o gateway do Cerne
#    - se na mesma rede docker do Cerne: ANTHROPIC_BASE_URL=http://cerne-gateway:4000
#    - se em host separado: ANTHROPIC_BASE_URL=https://litellm.orqion-ia.com.br  (atrás do Caddy)
```

## Segurança

- Agentes só enxergam o que for explicitamente montado nos containers.
- Bash roda **dentro** do container, nunca no host.
- Credenciais dos canais (WhatsApp/Telegram/…) ficam no vault do NanoClaw, fora dos containers.
- A governança do Cerne (RBAC, cotas, auditoria) é aplicada na **orquestração** (Fase 2):
  o Cerne Web dispara execuções no NanoClaw e registra tudo no ledger.

## Próximas fases

- **Fase 2:** Cerne Web evolui as Skills para "Agentes" (ferramentas + permissões + coleção)
  e orquestra o NanoClaw, com auditoria e cota por usuário.
- **Fase 3:** Cerne Desktop (app Tauri) com o módulo #work + agente local.
- **Fase 4:** canais (WhatsApp/Telegram) e tarefas agendadas, sob governança.

# Cerne — Mapa consolidado de IA (o que dá pra colocar no app)

Consolidação de 3 mapas do mercado contra o que o Cerne já tem:
**(1)** Corrida global de IA (USA/China/Índia) · **(2)** Microsoft Copilot Operations · **(3)** Plataforma Claude.
**Legenda:** ✅ já tem · 🟡 parcial/viável já · ⬜ roadmap · ⛔ fora do escopo soberano

---

## 1. LLMs → acervo externo mascarado
> O masking-proxy do Cerne aceita **qualquer LLM compatível com OpenAI**: a PII é mascarada antes de sair. Então "colocar no acervo" = adicionar um alias no `litellm-config.yaml` + colar a chave.

| LLM (da corrida global) | País | API grátis? | Status no Cerne |
|---|---|---|---|
| **GLM** (Zhipu) | 🇨🇳 | ✅ GLM-Flash grátis | ✅ `ext-glm` |
| **Qwen** (Alibaba) | 🇨🇳 | ✅ DashScope trial / Groq | ✅ `ext-qwen`, `ext-qwen-max` |
| **Gemini** (Google) | 🇺🇸 | ✅ 1500 req/dia | ✅ `ext-gemini` |
| **Llama** (Meta) via Groq/Cerebras | 🇺🇸 | ✅ grátis | ✅ `ext-groq`, `ext-cerebras` |
| **Mistral** | 🇪🇺 | ✅ free tier | ✅ `ext-mistral` |
| **DeepSeek** (inclui R1 "reasoning") | 🇨🇳 | ✅ grátis via OpenRouter | 🟡 **adicionar `ext-deepseek`** |
| **Kimi** (Moonshot, contexto gigante) | 🇨🇳 | 🟡 créditos grátis | 🟡 **adicionar `ext-kimi`** |
| **OpenRouter** (agregador) | 🇺🇸 | ✅ dezenas de modelos `:free` | ⭐ **adicionar `ext-openrouter`** — 1 chave, muitos grátis |
| **Hugging Face** (modelos abertos) | 🇺🇸 | ✅ Inference API grátis | 🟡 opcional `ext-hf` |
| **ERNIE** (Baidu) | 🇨🇳 | ✅ ERNIE-Speed/Lite grátis | 🟡 opcional |
| MiniMax, Yi (01.AI) | 🇨🇳 | 🟡 | 🟡 opcional |
| Grok (xAI), Perplexity, ChatGPT, Copilot | 🇺🇸 | ❌ pago | ⛔ (pago / não soberano) |
| Doubao, Baichuan, Hailuo, ERNIE Bot (chat) | 🇨🇳 | apps, sem API livre | ⛔ |
| Sarvam, Krutrim, BharatGPT, Hanooman | 🇮🇳 | 🟡 regionais | ⛔ (foco PT-BR) |

**Aliases prontos para colar** (grátis, alto valor):
```yaml
  - model_name: ext-openrouter      # agregador: dezenas de modelos :free com 1 chave
    litellm_params:
      model: openai/deepseek/deepseek-r1:free
      api_base: https://openrouter.ai/api/v1
      api_key: os.environ/OPENROUTER_API_KEY
  - model_name: ext-deepseek        # DeepSeek-V3 / R1 (reasoning) direto
    litellm_params:
      model: deepseek/deepseek-chat
      api_key: os.environ/DEEPSEEK_API_KEY
  - model_name: ext-kimi            # Moonshot Kimi (contexto gigante)
    litellm_params:
      model: openai/moonshot-v1-128k
      api_base: https://api.moonshot.cn/v1
      api_key: os.environ/MOONSHOT_API_KEY
```

**Ferramentas não-LLM da imagem → equivalente Cerne:**
- Zapier/Make → **n8n já é o equivalente** (camada de Hooks) ✅
- LangChain/LlamaIndex/HuggingFace/W&B/Scale → frameworks; Cerne já usa equivalentes (LiteLLM, Qdrant, Presidio) ✅
- Midjourney/Runway/Pika/Sora/Suno/ElevenLabs/HeyGen/Synthesia → geração de mídia ⛔ (Cerne é texto/RAG)
- Notion/Jasper/Grammarly/Cursor/Replit/Vercel → apps de ponta ⛔

---

## 2. Capacidades (plataforma Claude) → features do Cerne
| Capacidade | Cerne hoje | Viável já? |
|---|---|---|
| Chat | ✅ painel /agent | — |
| **1M Token context** | 🟡 RAG resolve o mesmo (chunk+retrieve) | ✅ ok |
| **Thinking** (raciocínio) | 🟡 loop Agentic RAG | 🟡 add modelo *reasoning* (DeepSeek-R1/QwQ) no acervo |
| **Skills** (SOPs/voz/regras) | ⬜ | ✅ **viável já** — biblioteca de skills/prompts salvos |
| **Prompt Templates** | ⬜ | ✅ **viável já** — templates reutilizáveis |
| **Global Instructions** | 🟡 SYSTEM_PROMPT fixo | ✅ tornar editável (Settings) |
| **AskUserQuestion** | ⬜ | 🟡 agente pedir esclarecimento antes de responder |
| **Artifacts** (saída interativa) | ⬜ | 🟡 gerar tabela/HTML do acervo |
| **Connectors** (Slack/Drive/Notion/50+) | 🟡 **n8n Hooks** já conecta | ✅ ativar fluxos |
| **Projects** (pastas persistentes) | 🟡 acervo único | 🟡 múltiplas coleções por projeto |
| Excel / Chrome / Plugins | ⬜ | ⛔ (client-side) |
| Team / Enterprise / SSO | ⬜ | ⬜ roadmap (RBAC) |

---

## 3. Operações enterprise (Copilot Ops) → governança/casos de uso
| Caso de uso Copilot | Cerne |
|---|---|
| Knowledge mgmt & retrieval | ✅ núcleo (RAG) |
| Document automation & summarization | ✅ ingest + /agent |
| Data extraction (PDF/imagens) | ✅ OCR + parsing |
| Process/workflow automation | 🟡 n8n Hooks (ativar) |
| **Department copilots** (RH/Financeiro/Compras) | 🟡 **viável** = coleção + system-prompt por depto (= Skills) |
| Governance (Purview / data safety / audit) | ✅ Audit Ledger + PII + Guardrails |
| **Prompt library / best practices** | ⬜ → ✅ **viável já** (Skills + Templates) |
| Adoption measurement / analytics | 🟡 Observability tem tokens/custo; falta "tarefas/tempo economizado" |
| Meeting intelligence & reporting | ⬜ (transcrição → ingest; futuro) |
| Email/communication ops | 🟡 via conectores n8n |
| Audit framework (readiness/access/depth) | 🟡 Preflight + Audit cobrem parte |

**Hábitos de times de alta performance (Copilot):** escrever tarefas como workflows · dar contexto+restrições · padronizar prompts como templates · medir impacto → tudo isso vira **biblioteca de Skills/Templates + métricas** no Cerne. ✅ viável.

---

## 4. CONSOLIDADO — "viável já" (sem GPU, quase tudo frontend/config)
1. ⭐ **Expandir acervo de LLMs grátis**: +OpenRouter (agregador `:free`), +DeepSeek (R1/reasoning), +Kimi. *(config + chave, mascarado)*
2. **Skills / Prompt Templates** — telas + persistência (= biblioteca de prompts = department copilots).
3. **Global Instructions editável** (system prompt por tenant/coleção).
4. **Ativar n8n Hooks** = Connectors (Slack/Drive/Notion).
5. **Projects/Coleções** — múltiplas coleções (por depto/projeto).
6. **AskUserQuestion no agente** — pedir esclarecimento quando ambíguo.
7. **Modo "Thinking"** — modelo de raciocínio (DeepSeek-R1/QwQ) no acervo.
8. **Métricas de adoção** — tarefas automatizadas / tempo economizado na Observability.

## 5. Roadmap (precisa de mais)
- Artifacts (saídas interativas geradas) · Meeting intelligence (transcrição) · RBAC/SSO + multi-tenancy · Deployment Runs · GPU tier (vLLM/NIM) para modelos grandes/multi-agente.

## 6. Fora do escopo (soberania/foco)
Geração de mídia (imagem/vídeo/voz), apps de ponta (Cursor/Notion/Grammarly), integrações client-side (Excel/Chrome), LLMs pagos de fronteira (Grok/GPT/Copilot) e modelos regionais indianos.

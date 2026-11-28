# Cerne — Backlog & Status

Revisão completa do plano inicial (`plano-desenvolvimento-pink.md`) + tudo construído nas sessões.
**Legenda:** ✅ feito · 🟡 parcial · ⬜ falta

---

## 0. Resumo executivo (o que o Cerne é hoje)
Plataforma de **IA soberana** rodando 100% na própria infra: **RAG privado com PII mascarada + grafo + reranker + agente (Agentic RAG) + guardrails + auditoria imutável**, com **Admin Center** (11 telas reais), servindo em **cerne.orqion-ia.com.br** (HTTPS + login) no **Oracle ARM (grátis)** e no laptop. Falta o **miolo enterprise** (GPU, multi-tenancy/RBAC, orquestração de deploy, multi-agente).

---

## 1. Infra & Deploy
| Item | Status |
|---|---|
| Stack Docker (8 serviços: ollama, qdrant, pii, gateway, rerank, neo4j, litellm-db, app) | ✅ |
| Deploy laptop (dev) + Oracle ARM (staging soberano, Always Free) | ✅ |
| Domínio `cerne.orqion-ia.com.br` + HTTPS (Caddy/Let's Encrypt) + Basic Auth | ✅ |
| Acesso por túnel SSH (alternativo) | ✅ |
| Persistência em volumes (D:\ laptop / data ARM) | ✅ |
| n8n como camada de Hooks (rede ligada + fluxo pronto) | 🟡 falta importar/ativar |
| Hardening OCI (só gateway/painel públicos; serviços em localhost) | 🟡 |
| IaC (Terraform) · CI/CD · Secrets manager (Vault) | ⬜ |

## 2. Camadas da arquitetura (L1–L7)
| Camada | Itens | Status |
|---|---|---|
| **L1 Core Infra** | Docker ✅ · GPU ⬜ · K8s/Terraform ⬜ | 🟡 |
| **L2 Integration** | PII masking Presidio pt ✅ · CPF/CNPJ (checksum)/RG/título/CNH/PIS/CEP/cartão ✅ · OCR (Tesseract) ✅ · Unstructured/Airbyte/Prefect ⬜ | 🟡 |
| **L3 Context** | Qdrant (vetor) ✅ · Neo4j (grafo) ✅ · RAG híbrido ✅ · Reranker BGE+lexical ✅ · memória de sessão ✅ · mem0/Cognee ⬜ | 🟡 |
| **L4 Intelligence** | Ollama (qwen3.5:4b texto+visão, CPU) ✅ · embeddings ✅ · vLLM/NIM ⬜ · fine-tuning (Axolotl) ⬜ · registry ⬜ | 🟡 |
| **L5 Action/Gateway** | LiteLLM OpenAI-compat ✅ · provedores externos mascarados ✅ · roteamento por sensibilidade ⬜ · MCP ⬜ | 🟡 |
| **L6 Agentic** | Agente (tools+memória+Agentic RAG) ✅ · Guardrails ✅ · multi-agente/CrewAI/A2A ⬜ | 🟡 |
| **L7 Management** | Admin Center ✅ · Observability (LiteLLM+PG) ✅ · Audit Ledger ✅ · Preflight ✅ · RBAC ⬜ · FinOps ⬜ · Policy/Blueprint/Troubleshooting ⬜ | 🟡 |

## 3. Escadas de maturidade
**LLM → Agentic AI:** 1 LLM ✅ · 2 RAG ✅ · 3 Tool Calling ✅ · 4 Memory 🟡 (sessão ✅, longo prazo ⬜) · 5 **AI Agent ✅** · 6 Agentic/multi-agente ⬜ · 7 Skills&Hooks 🟡 (Skills ✅ + AskUserQuestion ✅; Hooks/n8n parcial — falta ativar) · 8 Governance&Observability 🟡 (observ.+guardrails+audit+PII ✅; FinOps/policy/RBAC ⬜)

**RAG → Agentic RAG:** ✅ **completo** (reescrever consulta → recuperar → gerar → checar relevância → re-tentar)

## 4. Admin Center (telas do mockup)
**Reais (dados vivos):** Overview ✅ · Environments 🟡 (2 ambientes; multi-ambiente central ⬜) · **Projects & Collections ✅** · Components ✅ · Preflight Checks ✅ · Model Gateway ✅ (11 provedores) · **Skills & Instructions ✅** · Observability ✅ (+ adoção) · Events ✅ · Guardrails ✅ · Audit Ledger ✅ · Admin Users 🟡 (acesso real; RBAC ⬜) · Settings 🟡 (instrução global editável; resto leitura) · Navigation map ✅

**Roadmap (menu presente, honesto):** Deployment Runs ⬜ · Setup Wizard ⬜ · Inference Manager ⬜ · Policy Center ⬜ · Blueprint Manager ⬜ · Context Manager ⬜

## 5. Temas transversais
| Tema | Status |
|---|---|
| Segurança: TLS ✅ · Basic Auth ✅ · secrets (Vault) ⬜ · mTLS ⬜ · hardening 🟡 · scan de vuln ⬜ | 🟡 |
| LGPD: PII masking ✅ · audit imutável ✅ · residência (soberano) ✅ · direito de eliminação 🟡 (limpar acervo; por-documento ⬜) · base legal/consentimento ⬜ | 🟡 |
| Confiabilidade: restart policies ✅ · SLOs ⬜ · HA ⬜ · backup/DR ⬜ · rollback ⬜ | 🟡 |
| Observabilidade interna: LLMOps (LiteLLM+PG) ✅ · Langfuse ⬜ · Prometheus/infra ⬜ | 🟡 |
| Docs/DX: README, COMO-USAR, PROVEDORES-EXTERNOS, BACKLOG ✅ · runbooks ⬜ | 🟡 |

## 6. Fase 0 — Fundações (invisível, majoritariamente ⬜)
IaC/Terraform ⬜ · Secrets (Vault) ⬜ · CI/CD (plataforma + modelos) ⬜ · Multi-tenancy/isolamento ⬜ · Zero-trust/mTLS ⬜ · Telemetria base (Prometheus) ⬜

---

## 7. BACKLOG PRIORIZADO (próximos saltos)

### A. Rápido / sem GPU (laptop ou ARM)

**✅ Concluído em 2026-07-02 — validado no laptop E no ARM:**
- [x] ⭐ **Acervo de LLMs grátis expandido** — +OpenRouter (agregador `:free`), +DeepSeek, +Kimi/Moonshot, +raciocínio (11 provedores `ext-*` registrados)
- [x] **Modo Thinking** — `ext-reason` = GPT-OSS 120B (raciocínio) **LIVE via Groq**, pela camada de mascaramento
- [x] **Skills / Prompt Templates** — CRUD persistido (`/data/app/appstate.json`) + tela "Skills & Instructions" + seletor no agente = **department copilots**
- [x] **Global Instructions editável** — system prompt persistido, editável no painel (reflete no Settings)
- [x] **Projects/Coleções** — coleções isoladas por projeto/departamento (ingest + busca escopadas) + tela "Projects"
- [x] **AskUserQuestion** — agente pede esclarecimento quando o acervo não responde (em vez de inventar)
- [x] **Métricas de adoção** — tarefas de IA / documentos / PII protegidas / tempo economizado (est.) na Observability

**⬜ Pendente (ficou para depois):**
- [ ] **Ativar o fluxo n8n Hooks** (`cerne-hook.json`) — exige login no n8n (sem credencial minha); importar pela UI. Rede já ligada.
- [ ] **Colar as chaves** de OpenRouter/DeepSeek/Moonshot no `.env` do ARM para ativar esses provedores (slots já prontos; Groq/Thinking já funciona)
- [ ] **Roteamento automático por sensibilidade** no gateway (sensível → local; resto → externo mascarado)
- [ ] **mem0** — memória de longo prazo (hoje só memória de sessão)
- [ ] **Langfuse** — observabilidade completa (hoje LiteLLM+Postgres)
- [ ] **Direito de eliminação por documento** (LGPD) — delete por `source`/coleção
- [ ] **Settings totalmente editável** (hoje só a Instrução global persiste; resto é leitura)
- [ ] Filtrar ruído de NER no grafo (CLÁUSULA/LGPD viram entidade)
- [ ] Rotacionar/gerir a senha do painel via variável

### B. Requer GPU (o salto de capacidade)
- [ ] **Inference Manager** — vLLM / NVIDIA NIM + gestão de GPU
- [ ] Modelos grandes self-hosted (Qwen3/GLM) — chat fluido
- [ ] **Fine-tuning** (Axolotl)
- [ ] **Multi-agente / A2A** (CrewAI) — estágio 6
- [ ] Agente rápido (hoje ~1–2 min por resposta na CPU)

### C. Enterprise / cluster
- [ ] **RBAC/SSO** (Keycloak) → Admin Users de verdade
- [ ] **Deployment Manager/Runs** — instalar/atualizar/rollback com logs ao vivo
- [ ] **Multi-tenancy** + IaC + CI/CD (Fase 0)
- [ ] **FinOps** (quotas de tokens por tenant) · **Policy Center** · **Blueprint Manager** · **Troubleshooting Engine**
- [ ] Backup/DR + SLOs + HA

---

## 8. O que NÃO existe (evitar planejar sobre incógnita)
- "NemoClaw" (do diagrama original) — não é produto reconhecido; tratar como placeholder.
- Agente rápido em CPU — inviável; depende de GPU.
- Deployment orchestration / K8s wizard — Cerne roda em Docker; K8s é decisão de infra futura.

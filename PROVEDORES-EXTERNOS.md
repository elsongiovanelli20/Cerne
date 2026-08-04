# Provedores externos com mascaramento (masking-proxy)

O Cerne pode usar um modelo **externo gratuito e mais forte** (ex.: GLM-4.7-Flash)
para tarefas difíceis — **mascarando a PII antes de a requisição sair da máquina** e
**reconstituindo** o dado real na resposta. O provedor nunca vê o dado pessoal.

## Como funciona

```
prompt (com CPF, nome...)
   │  [app mascara — tokens ÚNICOS + mapa LOCAL]
   ▼
"...[PERSON_1], CPF [BR_CPF_1]..."   ──►  provedor externo (GLM/Gemini/Groq...)
   ▼                                        (só vê tokens, nunca a PII)
resposta com [PERSON_1]...
   │  [app de-mascara pelo mapa local]
   ▼
"...João da Silva, CPF 111.444.777-35..."   ◄─ você vê o dado real
```

O mascaramento acontece **no app, antes do gateway**. Por isso ele é
**agnóstico de provedor**: qualquer modelo que você adicionar (um alias + uma chave)
já herda a proteção de PII automaticamente. Dá até para **alternar entre provedores**
para esticar as cotas gratuitas — todos atrás da mesma camada de privacidade.

## ⚠️ A garantia e o limite (leia)

**Protege:** a PII direta reconhecida (CPF, CNPJ, nome, e-mail, telefone, CEP).

**NÃO protege / continua saindo da máquina:**
- PII que o mascarador **erra** (nenhum detector é perfeito);
- **conteúdo confidencial que não é PII** — código do cliente, valores, segredos
  comerciais, nome do projeto;
- **quase-identificadores** combinados (data + valor + cidade) podem re-identificar;
- **residência de dados**: o texto (mascarado) vai para um provedor **no exterior**
  (a Z.ai é na China) — pode violar contrato/LGPD independentemente do PII.

**Regra de ouro:** isto é **redução de risco**, não **soberania**. Use para tarefas
**difíceis e de baixa sensibilidade**. Para dado de cliente regulado, continue no
**modelo local** (`/ask`), que não sai da máquina.

## Provedores gratuitos suportados (jul/2026)

Todos configurados no gateway como aliases `ext-*`, todos passam pelo mascaramento.

| Alias | Provedor | Modelo padrão | Free tier | Destaque | vs GLM-4.7-Flash |
|---|---|---|---|---|---|
| `ext-gemini` | Google Gemini | gemini-2.5-flash | **1.500 req/dia, sem cartão** | contexto 1M, multimodal | **mais forte** (geral) |
| `ext-cerebras` | Cerebras | gpt-oss-120b | 1M tokens/dia | throughput altíssimo (~2000 tok/s); tem `zai-glm-4.7` | **mais forte** (120B) |
| `ext-groq` | Groq | llama-3.3-70b-versatile | 30 req/min | mais rápido; tem gpt-oss-120b, qwen3-32b | **mais forte** (70B) |
| `ext-glm` | Z.ai | glm-4.7-flash | grátis, 1 concorrência | ótimo em código/agêntico p/ o tamanho | referência |
| `ext-mistral` | Mistral | codestral-latest | free tier | Codestral/Devstral p/ código | comparável |

**Resumo:** para **melhor qualidade grátis**, `ext-gemini` (cota enorme) ou
`ext-cerebras`/`ext-groq` (modelos de 70–120B) superam o GLM-4.7-Flash. Para
**código**, GLM-4.7-Flash e Codestral são fortes. `ext-glm` é um bom padrão.

## Como ativar um provedor

1. Pegue uma chave gratuita:
   - **Gemini:** https://aistudio.google.com → *Get API key* (sem cartão)
   - **Groq:** https://console.groq.com → *API Keys*
   - **Cerebras:** https://cloud.cerebras.ai → *API Keys*
   - **Z.ai (GLM):** https://z.ai → API (tier Flash grátis)
   - **Mistral:** https://console.mistral.ai → *API Keys*
2. Cole no `.env` (só a que for usar):
   ```
   GEMINI_API_KEY=xxxxx
   ```
3. Recarregue o gateway:
   ```powershell
   cd D:\cerne; docker compose up -d gateway
   ```

## Como usar

```powershell
# usa RAG nos seus docs + modelo externo, com PII mascarada no caminho
$body = @{ question = "Resuma a obrigação do contratante"; provider = "ext-gemini" } | ConvertTo-Json
$bytes = [System.Text.Encoding]::UTF8.GetBytes($body)
$r = Invoke-RestMethod http://localhost:8000/ask-externo -Method Post -ContentType "application/json; charset=utf-8" -Body $bytes
$r.answer                 # resposta com o dado real reconstituído
$r.enviado_ao_provedor    # PROVA: exatamente o texto (mascarado) que saiu
$r.pii_protegida          # tokens que foram mascarados
```

Parâmetros: `provider` (`ext-glm`|`ext-gemini`|`ext-groq`|`ext-cerebras`|`ext-mistral`),
`use_rag` (true = usa seus documentos; false = pergunta direta), `top_k`.

Compare com o local: mesma pergunta em `/ask` (100% soberano, `qwen2.5:3b`) vs.
`/ask-externo` (mais forte, mascarado). Roteie por sensibilidade da tarefa.

## Fontes da pesquisa
- Free LLM API tiers 2026 (Groq/Cerebras/Mistral/Gemini): https://ianlpaterson.com/blog/free-llm-api-2026/
- Free LLM APIs comparados — OpenRouter: https://openrouter.ai/blog/tutorials/free-llm-apis-compared/
- Lista curada de APIs gratuitas: https://github.com/amardeeplakshkar/awesome-free-llm-apis
- GLM-4.7-Flash (benchmarks/free): https://tokenmix.ai/blog/glm-4-7-flash-review-pricing-benchmark-2026

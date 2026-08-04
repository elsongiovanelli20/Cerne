# Como usar o Cerne

O Cerne é um **RAG privado com mascaramento de PII**, rodando 100% na sua máquina:
você envia documentos → os dados pessoais são mascarados → tudo é indexado →
você faz perguntas e recebe respostas baseadas no seu acervo. **Nada sai da máquina.**

Este guia é operacional: como ligar, usar, integrar e resolver problemas.
Para arquitetura e migração para a nuvem, veja o [README.md](README.md).

---

## 1. O que está rodando

| Serviço | Porta (host) | Para que serve |
|---|---|---|
| **Painel + RAG** (`cerne-app`) | http://localhost:8000 | interface e API de ingestão/pergunta |
| **Gateway** (`cerne-gateway`) | http://localhost:4000 | API compatível com OpenAI (aponte ferramentas aqui) |
| **Ollama** (`cerne-ollama`) | 11434 | modelos: `qwen2.5:3b` (chat) + `nomic-embed-text` (embeddings) |
| **PII** (`cerne-pii`) | interno | mascara CPF, CNPJ, CEP, nome, e-mail, telefone (Presidio pt) |
| **Qdrant** (`cerne-qdrant`) | 6333 | banco vetorial (onde os documentos ficam indexados) |

Todos os dados persistem em **`D:\cerne\data\`** (modelos, vetores, uploads).

---

## 2. Ligar e desligar

```powershell
cd D:\cerne

# LIGAR tudo (build na primeira vez + baixa modelos)
.\scripts\up.ps1
#   ou, se já estiver construído:
docker compose up -d

# ver status
docker compose ps

# DESLIGAR (libera a RAM; os dados ficam salvos)
docker compose stop

# ligar de novo depois
docker compose start

# DERRUBAR os containers (dados continuam em D:\cerne\data)
docker compose down
```

### Convivendo com o Orqion/BPM/n8n
Os 16 GB são compartilhados. Se a máquina ficar lenta com tudo ligado, pause a
stack do BPM/n8n enquanto usa o Cerne:

```powershell
# parar BPM + n8n (libera ~2 GB dentro da WSL)
docker stop $(docker ps --format "{{.Names}}" | Where-Object { $_ -match 'BPM|n8n|orqion' })

# trazer de volta quando precisar (ou use o botão Start no Docker Desktop)
docker start $(docker ps -a --format "{{.Names}}" | Where-Object { $_ -match 'BPM|n8n|orqion' -and $_ -notmatch 'edge_runtime' })
```

---

## 3. Usar pelo painel (jeito mais fácil)

Abra **http://localhost:8000** no navegador.

1. **Ingerir** — cole um texto (ou escolha um `.txt`/`.pdf`) e clique em *Ingerir*.
   O painel mostra quantos trechos foram indexados e **quais tipos de PII foram
   mascarados** (`BR_CPF`, `PERSON`, `EMAIL_ADDRESS`...).
2. **Perguntar** — digite uma pergunta e clique em *Perguntar*. A resposta vem
   fundamentada no que você ingeriu. Se a informação não estiver lá, ele diz que
   não encontrou (não inventa).

> A primeira pergunta após ligar pode demorar (o modelo carrega na RAM).
> As seguintes ficam bem mais rápidas enquanto o modelo está "quente".

---

## 4. Usar pela API

### 4a. RAG (ingestão e perguntas) — `cerne-app` na porta 8000

**PowerShell** (use este padrão para não quebrar acentos):

```powershell
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

function Post($path, $obj) {
  $bytes = [System.Text.Encoding]::UTF8.GetBytes(($obj | ConvertTo-Json -Depth 5))
  Invoke-RestMethod -Uri "http://localhost:8000$path" -Method Post `
    -ContentType "application/json; charset=utf-8" -Body $bytes
}

# ingerir um texto
Post "/ingest" @{ text = "João da Silva, CPF 111.444.777-35, deve R$ 500."; source = "nota.txt" }

# ingerir um arquivo (.txt ou .pdf)
$form = @{ file = Get-Item "C:\caminho\documento.pdf" }
Invoke-RestMethod -Uri "http://localhost:8000/ingest-file" -Method Post -Form $form

# perguntar
(Post "/ask" @{ question = "Quanto o João deve?"; top_k = 4 }).answer
```

Endpoints:
- `POST /ingest` — `{ "text": "...", "source": "..." }`
- `POST /ingest-file` — multipart, campo `file` (`.txt`/`.pdf`)
- `POST /ask` — `{ "question": "...", "top_k": 4 }` → `{ answer, sources, chunks_used }`

### 4b. Gateway OpenAI-compatível — `cerne-gateway` na porta 4000

Este é o ponto que deixa **qualquer ferramenta que fala "OpenAI" usar o modelo
local**. Basta apontar a URL base para `http://localhost:4000` e usar a chave do
`.env` (`LITELLM_MASTER_KEY`). Modelos disponíveis: `chat` e `embed`.

**curl / PowerShell:**
```powershell
$headers = @{ Authorization = "Bearer SUA_MASTER_KEY_DO_ENV" }
$body = @{ model = "chat"; messages = @(@{ role="user"; content="Explique soberania de dados em 1 frase." }) } | ConvertTo-Json
Invoke-RestMethod -Uri "http://localhost:4000/v1/chat/completions" -Method Post -Headers $headers `
  -ContentType "application/json" -Body $body
```

**Python (SDK oficial da OpenAI, apontado para o Cerne):**
```python
from openai import OpenAI
client = OpenAI(base_url="http://localhost:4000", api_key="SUA_MASTER_KEY_DO_ENV")

resp = client.chat.completions.create(
    model="chat",
    messages=[{"role": "user", "content": "Resuma este chamado: ..."}],
)
print(resp.choices[0].message.content)
```

> É exatamente assim que um produto (ex.: o Orqion Flow) integraria o Cerne:
> troca `api.openai.com` por `localhost:4000` e passa a rodar no modelo local.
> Depois, no `litellm-config.yaml`, dá para rotear tarefas difíceis para um
> modelo frontier como fallback — sem mudar o código do produto.

---

## 5. Conferir o mascaramento de PII

O serviço de PII é interno, mas dá para testá-lo pelo container do app:

```powershell
Get-Content D:\cerne\scripts\test_pii.py -Raw | docker exec -i cerne-app python -
```

Ele detecta: **CPF e CNPJ validados por dígito verificador** (um CPF inválido não
é tratado como CPF), CEP, nome (`PERSON`), e-mail, telefone. Um CPF válido vira
`<BR_CPF>`, um nome vira `<PERSON>`, e assim por diante.

---

## 6. Trocar o modelo

Os nomes `chat` e `embed` são fixos para o app; o modelo real por trás deles fica
em **`gateway/litellm-config.yaml`**. Para trocar o modelo de chat:

```powershell
# 1) baixe o modelo desejado
docker exec cerne-ollama ollama pull qwen2.5:3b        # bom em português (padrão atual)
# docker exec cerne-ollama ollama pull llama3.2:1b     # mais leve/rápido
# docker exec cerne-ollama ollama pull llama3.1:8b     # melhor, mas exige mais RAM

# 2) edite gateway/litellm-config.yaml, no model_name "chat":
#      model: ollama/qwen2.5:3b

# 3) recarregue só o gateway
cd D:\cerne; docker compose up -d gateway
```

Referência de peso na CPU: `1b` (mais rápido) < `3b` (equilíbrio) < `8b` (melhor,
precisa de RAM livre — pause o BPM/n8n antes).

---

## 7. Gerenciar os documentos indexados

Os vetores vivem em `D:\cerne\data\qdrant`. Para **zerar** o acervo e começar limpo:

```powershell
cd D:\cerne
docker compose stop app qdrant
Remove-Item -Recurse -Force D:\cerne\data\qdrant\*
docker compose up -d qdrant app     # o app recria a coleção vazia ao subir
```

Para inspecionar o Qdrant (opcional): http://localhost:6333/dashboard

---

## 8. Desempenho e memória (a verdade honesta)

- A inferência roda em **CPU** — respostas levam de segundos a ~1 min conforme o
  modelo e se ele está "quente". É esperado nesta máquina.
- **Manter o modelo quente** acelera muito. Ele descarrega após alguns minutos
  ocioso; a pergunta seguinte recarrega.
- Com o BPM/n8n parados sobra RAM e some o swap — a diferença medida aqui foi de
  **~326s para ~88s** numa mesma pergunta (partindo do modelo frio).
- Limites de RAM por container estão no `docker-compose.yml` (`mem_limit`).

---

## 9. Problemas comuns

| Sintoma | Causa provável | O que fazer |
|---|---|---|
| Primeira pergunta demora muito | modelo carregando na RAM | normal; espere; mantenha ligado p/ ficar quente |
| Máquina travando | RAM disputada com BPM/n8n | `docker stop` da stack BPM/n8n (seção 2) |
| `/ask` retorna 500 | erro no app | `docker logs --tail 30 cerne-app` |
| Acentos embaralhados (`Ã©`) no terminal | encoding do console | use o painel no navegador; nos scripts já está corrigido |
| `cerne-pii` não fica *healthy* | falha ao carregar modelo pt | `docker logs cerne-pii` |
| Porta 8000/4000 ocupada | outro serviço na porta | mude `APP_PORT`/`GATEWAY_PORT` no `.env` e `docker compose up -d` |
| Ver logs de qualquer serviço | — | `docker logs --tail 50 cerne-<serviço>` |

---

## 10. Levar para a nuvem (Oracle ARM)

Nada aqui é nativo do Windows: copie a pasta `D:\cerne` para a instância e rode
`docker compose up -d --build`. Os ajustes de segurança (expor só o gateway/painel
com TLS, subir os `mem_limit`, bloco de GPU/fallback frontier) estão no
[README.md](README.md), seção "Preparado para a nuvem".

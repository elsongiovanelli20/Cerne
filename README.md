# Cerne — RAG privado com PII masking

O núcleo de uma plataforma de IA soberana, em escala de protótipo: um documento
entra, tem os **dados pessoais mascarados**, é indexado, e uma pergunta é
respondida **com base no seu próprio acervo** — tudo rodando **100% na máquina**,
sem nada sair para APIs de terceiros.

Este é o "demo matador": num único fluxo, exercita quatro camadas do produto.

| Camada | Papel | Serviço aqui |
|---|---|---|
| **L4** Intelligence | inferência + embeddings | `ollama` (llama3.2:3b, nomic-embed-text) |
| **L5** Action / API Bus | gateway universal (API OpenAI-compat) | `gateway` (LiteLLM, porta 4000) |
| **L2** Integration | ingestão + **PII masking** | `pii` (Presidio + CPF/CNPJ/CEP em pt) |
| **L3** Context | memória / busca vetorial + RAG | `qdrant` + `app` (porta 8000) |

---

## Pré-requisitos
- Docker Desktop com backend WSL2 (já instalado).
- `C:\Users\<você>\.wslconfig` limitando a RAM do WSL (este repositório assume `memory=10GB`).
- Todo o dado persistente vive em `D:\cerne\data\` (modelos, vetores, uploads).

## Subir (Windows / PowerShell)
```powershell
cd D:\cerne
.\scripts\up.ps1          # build + up + baixa os modelos
```
Ou manualmente:
```powershell
docker compose up -d --build
.\scripts\pull-models.ps1
```

## Usar
- **Painel:** http://localhost:8000 — ingerir texto/arquivo e perguntar.
- **Teste ponta a ponta:** `.\scripts\smoke-test.ps1`
- **Gateway (OpenAI-compat):** http://localhost:4000 — qualquer ferramenta que fala "OpenAI" aponta pra cá.

### Endpoints do app
- `POST /ingest` `{ "text": "...", "source": "..." }`
- `POST /ingest-file` (multipart, campo `file`; aceita `.txt` e `.pdf`)
- `POST /ask` `{ "question": "...", "top_k": 4 }`

## Operar sem travar a máquina (16 GB)
```powershell
docker stats                       # ver consumo de RAM ao vivo
docker compose stop                # desligar tudo
docker compose down                # derrubar (dados ficam em D:\cerne\data)
```
A soma dos limites de memória dos containers fica em ~8 GB, dentro do teto de 10 GB do WSL.
Inferência roda em **CPU** — respostas vêm lentas (segundos a dezenas de segundos). É esperado.

---

## Preparado para a nuvem (Oracle Ampere A1, 24 GB ARM)
O stack foi desenhado para migrar **sem reescrever nada**:

1. **Tudo é container e todas as imagens têm build arm64** (ollama, qdrant, litellm,
   python-slim). Copie a pasta `cerne/` para a instância e rode `docker compose up -d --build`.
2. **Sem dependência nativa** (nada de Ollama instalado no host): o mesmo compose sobe
   igual no Windows e no Linux ARM.
3. **Caminhos relativos** (`./data/...`): funcionam em qualquer host.

### Ajustes ao migrar
- **RAM:** com 24 GB dá para subir tudo junto **+ Neo4j (grafo)** e **+ Langfuse
  (observabilidade)**, e usar um modelo maior (ex.: `llama3.1:8b`). Suba os `mem_limit`.
- **Segurança (agora é internet-facing):**
  - exponha **apenas o gateway** (4000) e o painel (8000) atrás de um reverse proxy
    com TLS (Caddy/nginx). **Nunca** exponha `qdrant`/`pii` publicamente — remova os
    `ports:` deles e deixe só na rede interna do compose.
  - troque `LITELLM_MASTER_KEY` no `.env` por uma chave forte.
  - feche tudo nos *Security Lists* da Oracle + firewall do host.

### Preparado para GPU (passo seguinte)
Quando houver GPU, o serviço `ollama` ganha aceleração adicionando ao compose:
```yaml
    deploy:
      resources:
        reservations:
          devices:
            - {driver: nvidia, count: all, capabilities: [gpu]}
```
E o `gateway` pode rotear tarefas difíceis para um modelo frontier (ex.: `gpt-4o-mini`)
— basta descomentar o bloco de exemplo em `gateway/litellm-config.yaml`.

## Estrutura
```
D:\cerne\
├─ docker-compose.yml        # orquestra os 5 serviços
├─ .env                      # modelos, chave do gateway, portas
├─ gateway/litellm-config.yaml
├─ pii/                      # microserviço de PII masking (Presidio + BR)
├─ app/                      # RAG: ingestão + pergunta + painel
├─ samples/contrato-exemplo.txt
├─ scripts/                  # up.ps1, pull-models.ps1, smoke-test.ps1
└─ data/                     # PERSISTÊNCIA (ollama, qdrant, uploads)
```

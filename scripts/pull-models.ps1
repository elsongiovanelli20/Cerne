# Baixa os modelos que o Cerne usa para dentro do container ollama.
# Rode depois que o stack estiver de pé (docker compose up -d).
Write-Host "Baixando modelos no container cerne-ollama (pode demorar)..." -ForegroundColor Cyan
docker exec cerne-ollama ollama pull llama3.2:3b
docker exec cerne-ollama ollama pull nomic-embed-text
Write-Host "`nModelos disponiveis:" -ForegroundColor Green
docker exec cerne-ollama ollama list

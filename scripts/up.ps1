# Sobe o stack Cerne inteiro e baixa os modelos.
$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    Write-Host "Construindo e subindo containers..." -ForegroundColor Cyan
    docker compose up -d --build

    Write-Host "Aguardando o ollama ficar saudavel..." -ForegroundColor Cyan
    for ($i = 0; $i -lt 30; $i++) {
        $st = docker inspect --format '{{.State.Health.Status}}' cerne-ollama 2>$null
        if ($st -eq 'healthy') { break }
        Start-Sleep -Seconds 5
    }
    & "$PSScriptRoot\pull-models.ps1"

    Write-Host "`nPronto. Abra o painel em: http://localhost:8000" -ForegroundColor Green
    Write-Host "Gateway (OpenAI-compat) em:  http://localhost:4000"
} finally {
    Pop-Location
}

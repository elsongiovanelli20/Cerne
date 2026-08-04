# Teste ponta a ponta: ingere o contrato de exemplo e faz duas perguntas.
# Uso: .\scripts\smoke-test.ps1
$ErrorActionPreference = "Stop"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8   # acentos corretos no terminal
$app = "http://localhost:8000"
$root = Split-Path $PSScriptRoot -Parent

function Post($path, $obj) {
    $bytes = [System.Text.Encoding]::UTF8.GetBytes(($obj | ConvertTo-Json -Depth 5))
    return Invoke-RestMethod -Uri "$app$path" -Method Post -ContentType "application/json; charset=utf-8" -Body $bytes
}

Write-Host "== 1. Ingerindo contrato-exemplo.txt (PII sera mascarada) ==" -ForegroundColor Cyan
# ReadAllText devolve string limpa (Get-Content -Raw anexa metadados e quebra o JSON)
$text = [System.IO.File]::ReadAllText("$root\samples\contrato-exemplo.txt", [System.Text.Encoding]::UTF8)
Post "/ingest" @{ text = $text; source = "contrato-exemplo.txt" } | ConvertTo-Json -Depth 5

Write-Host "`n== 2. Pergunta respondivel pelo contexto (pode demorar em CPU) ==" -ForegroundColor Cyan
(Post "/ask" @{ question = "Qual o prazo de pagamento previsto no contrato?" }).answer

Write-Host "`n== 3. Pergunta sobre PII (deve estar protegida) ==" -ForegroundColor Cyan
(Post "/ask" @{ question = "Qual o CPF do contratante?" }).answer

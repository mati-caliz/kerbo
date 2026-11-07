<#
Abre sesiones interactivas de Claude Code, una detrás de la otra, sobre la próxima etapa abierta
del ROADMAP. Cada sesión arranca limpia con el pedido de pedido.md; cuando termina su avance,
escribís /exit y se abre la siguiente. Un archivo .parity\autopilot\STOP corta todo.

  .\scripts\autopilot\sesion.ps1                      # carril principal, en la carpeta kerbo
  .\scripts\autopilot\sesion.ps1 -Carril b            # otro carril, en ..\kerbo-b
  .\scripts\autopilot\sesion.ps1 -Etapa 14            # una etapa fija
  .\scripts\autopilot\sesion.ps1 -Modelo sonnet       # otro modelo que el de la tabla
#>
param(
    [string]$Carril = 'principal',
    [string]$Etapa = '',
    [string]$Modelo = '',
    [string]$Permisos = 'bypassPermissions'
)

$ErrorActionPreference = 'Stop'
$Principal = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Paridad = Join-Path $Principal '.parity'
$Tomas = Join-Path $Paridad 'autopilot'
$Parada = Join-Path $Tomas 'STOP'
$EtapasExcluidas = @('10')
$ModeloPorDefecto = 'opus'
$ModelosPorEtapa = @{ '17' = 'sonnet' }

New-Item -ItemType Directory -Force $Tomas | Out-Null
# Los scripts de paridad buscan .parity en esta variable: los carriles no la copian ni la enlazan,
# porque borrar un worktree con un enlace adentro borra también lo enlazado.
$env:KERBO_PARITY = $Paridad

function Carpeta-Del-Carril {
    if ($Carril -eq 'principal') { return $Principal }
    $Carpeta = Join-Path (Split-Path $Principal -Parent) "kerbo-$Carril"
    if (-not (Test-Path $Carpeta)) {
        git -C $Principal fetch -q origin
        git -C $Principal worktree add -q --detach $Carpeta origin/main
    }
    return $Carpeta
}

function Al-Dia([string]$Carpeta) {
    git -C $Carpeta fetch -q origin
    if (git -C $Carpeta status --porcelain) {
        Write-Host "La carpeta $Carpeta tiene cambios sin commitear: sigo desde ahí." -ForegroundColor Yellow
        return
    }
    if ($Carril -eq 'principal') { git -C $Carpeta pull -q --ff-only }
    else { git -C $Carpeta checkout -q --detach origin/main }
}

function Etapas-En-Orden([string]$Carpeta) {
    Get-Content (Join-Path $Carpeta 'ROADMAP.md') -Encoding UTF8 |
        Where-Object { $_ -match '^### (\d+)\. (.*)$' } |
        ForEach-Object { [pscustomobject]@{ Numero = $Matches[1]; Titulo = $Matches[2] } }
}

function Tomada-Por-Otro([string]$Numero) {
    $Toma = Join-Path $Tomas "etapa-$Numero.carril"
    (Test-Path $Toma) -and ((Get-Content $Toma) -ne $Carril)
}

function Proxima-Etapa([string]$Carpeta) {
    $Etapas = Etapas-En-Orden $Carpeta
    if ($Etapa) { return $Etapas | Where-Object { $_.Numero -eq $Etapa } | Select-Object -First 1 }
    $Etapas | Where-Object { $EtapasExcluidas -notcontains $_.Numero -and -not (Tomada-Por-Otro $_.Numero) } |
        Select-Object -First 1
}

$Carpeta = Carpeta-Del-Carril
while (-not (Test-Path $Parada)) {
    Al-Dia $Carpeta
    $Elegida = Proxima-Etapa $Carpeta
    if (-not $Elegida) { Write-Host 'No queda ninguna etapa libre.' -ForegroundColor Yellow; break }
    $Toma = Join-Path $Tomas "etapa-$($Elegida.Numero).carril"
    Set-Content -Path $Toma -Value $Carril -Encoding ASCII
    $ElModelo = $Modelo
    if (-not $ElModelo) { $ElModelo = $ModelosPorEtapa[$Elegida.Numero] }
    if (-not $ElModelo) { $ElModelo = $ModeloPorDefecto }
    $Mensaje = "Seguí la etapa $($Elegida.Numero) del ROADMAP ($($Elegida.Titulo)), carril $Carril. Leé scripts/autopilot/pedido.md y hacé lo que dice."
    Write-Host "== Etapa $($Elegida.Numero): $($Elegida.Titulo) | carril $Carril | $ElModelo" -ForegroundColor Cyan
    Push-Location $Carpeta
    try { & claude --model $ElModelo --permission-mode $Permisos -n "kerbo $($Elegida.Numero) ($Carril)" $Mensaje }
    finally { Pop-Location; Remove-Item $Toma -ErrorAction SilentlyContinue }
    $Respuesta = Read-Host 'Enter: otra sesión limpia | q: salir'
    if ($Respuesta -eq 'q') { break }
}

if ($Carril -ne 'principal' -and -not (git -C $Carpeta status --porcelain)) {
    git -C $Principal worktree remove --force $Carpeta
    Write-Host "Carpeta $Carpeta borrada (no tenía cambios)."
}

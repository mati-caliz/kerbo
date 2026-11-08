<#
Corre sesiones de Claude Code sin intervención (claude -p), una detrás de la otra, sobre la
próxima etapa abierta del ROADMAP, y muestra en vivo lo que dice cada una y qué herramientas usa. Cada sesión arranca limpia con el pedido de pedido.md y, cuando
termina su avance, se cierra sola y arranca la siguiente. Una etapa que termina sin commits se
saltea en esta corrida; tres sesiones seguidas sin avanzar cortan el carril. El resumen de cada
sesión queda en .parity\autopilot\registro\. Un archivo .parity\autopilot\STOP corta todo.

  .\scripts\autopilot\sesion.ps1                      # carril principal, en la carpeta kerbo
  .\scripts\autopilot\sesion.ps1 -Carril b            # otro carril, en ..\kerbo-b
  .\scripts\autopilot\sesion.ps1 -Etapa 14            # una etapa fija
  .\scripts\autopilot\sesion.ps1 -Modelo sonnet       # otro modelo que el de la tabla
  .\scripts\autopilot\sesion.ps1 -Esfuerzo high       # otro esfuerzo que el de la tabla
#>
param(
    [string]$Carril = 'principal',
    [string]$Etapa = '',
    [string]$Modelo = '',
    [string]$Esfuerzo = '',
    [string]$Permisos = 'bypassPermissions',
    [string]$Compactar = '150k'
)

$ErrorActionPreference = 'Stop'
$Principal = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Paridad = Join-Path $Principal '.parity'
$Tomas = Join-Path $Paridad 'autopilot'
$Parada = Join-Path $Tomas 'STOP'
$Registro = Join-Path $Tomas 'registro'
$EtapasExcluidas = [System.Collections.Generic.List[string]]@('10')
$MaximoSesionesSinAvance = 3
$SegundosParaCortarConCtrlC = 10
$LargoMaximoDeHerramienta = 160
# Mecánico con arnés (versiones, tandas de reglas): Sonnet. Diseño con paridad: Opus en medio.
# Investigación difícil (tipos, tests inteligentes): Opus en alto.
$ModeloPorDefecto = 'opus'
$EsfuerzoPorDefecto = 'medium'
$ModelosPorEtapa = @{ '17' = 'sonnet' }
$EsfuerzosPorEtapa = @{ '16' = 'high'; '11' = 'high' }

New-Item -ItemType Directory -Force $Tomas, $Registro | Out-Null
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

function Commiteo-Desde([string]$Carpeta, [datetime]$Inicio) {
    $Desde = $Inicio.ToString('yyyy-MM-ddTHH:mm:ss')
    [bool](git -C $Carpeta log -g --format=%gs --since=$Desde HEAD | Where-Object { $_ -like 'commit*' })
}

function Recortar([string]$Texto) {
    $Linea = ($Texto -replace '\s+', ' ').Trim()
    if ($Linea.Length -le $LargoMaximoDeHerramienta) { return $Linea }
    $Linea.Substring(0, $LargoMaximoDeHerramienta) + '…'
}

function Detalle-De-Herramienta($Entrada) {
    foreach ($Campo in 'command', 'file_path', 'pattern', 'description', 'prompt') {
        if ($Entrada.$Campo) { return Recortar $Entrada.$Campo }
    }
    ''
}

function Mostrar-Evento([string]$Linea, [string]$Resumen) {
    try { $Evento = $Linea | ConvertFrom-Json } catch { Write-Host $Linea; return }
    if ($Evento.type -eq 'assistant') {
        foreach ($Bloque in $Evento.message.content) {
            if ($Bloque.type -eq 'text') { Write-Host "`n$($Bloque.text)" }
            elseif ($Bloque.type -eq 'tool_use') {
                Write-Host "  > $($Bloque.name) $(Detalle-De-Herramienta $Bloque.input)" -ForegroundColor DarkGray
            }
        }
    }
    elseif ($Evento.type -eq 'result') {
        Set-Content -Path $Resumen -Value $Evento.result -Encoding UTF8
        $Minutos = [math]::Round($Evento.duration_ms / 60000)
        Write-Host "`n== Sesión terminada en $Minutos min (resumen en $Resumen)" -ForegroundColor Cyan
    }
}

$Carpeta = Carpeta-Del-Carril
$SesionesSinAvance = 0
while (-not (Test-Path $Parada)) {
    Al-Dia $Carpeta
    $Elegida = Proxima-Etapa $Carpeta
    if (-not $Elegida) { Write-Host 'No queda ninguna etapa libre.' -ForegroundColor Yellow; break }
    $Toma = Join-Path $Tomas "etapa-$($Elegida.Numero).carril"
    Set-Content -Path $Toma -Value $Carril -Encoding ASCII
    $ElModelo = $Modelo
    if (-not $ElModelo) { $ElModelo = $ModelosPorEtapa[$Elegida.Numero] }
    if (-not $ElModelo) { $ElModelo = $ModeloPorDefecto }
    $ElEsfuerzo = $Esfuerzo
    if (-not $ElEsfuerzo) { $ElEsfuerzo = $EsfuerzosPorEtapa[$Elegida.Numero] }
    if (-not $ElEsfuerzo) { $ElEsfuerzo = $EsfuerzoPorDefecto }
    $Mensaje = "Seguí la etapa $($Elegida.Numero) del ROADMAP ($($Elegida.Titulo)), carril $Carril. Leé scripts/autopilot/pedido.md y hacé lo que dice."
    Write-Host "== Etapa $($Elegida.Numero): $($Elegida.Titulo) | carril $Carril | $ElModelo, esfuerzo $ElEsfuerzo" -ForegroundColor Cyan
    $Inicio = Get-Date
    $Resumen = Join-Path $Registro ("{0:yyyy-MM-dd_HHmm}-etapa-{1}-{2}.txt" -f $Inicio, $Elegida.Numero, $Carril)
    Push-Location $Carpeta
    try {
        & claude -p --output-format stream-json --verbose --model $ElModelo --effort $ElEsfuerzo --permission-mode $Permisos --autocompact $Compactar -n "kerbo $($Elegida.Numero) ($Carril)" $Mensaje |
            ForEach-Object { Mostrar-Evento $_ $Resumen }
    }
    finally { Pop-Location; Remove-Item $Toma -ErrorAction SilentlyContinue }

    if (Commiteo-Desde $Carpeta $Inicio) { $SesionesSinAvance = 0 }
    else {
        $SesionesSinAvance++
        if (-not $Etapa) { $EtapasExcluidas.Add($Elegida.Numero) }
        Write-Host "La etapa $($Elegida.Numero) terminó sin commits: no la retomo en esta corrida." -ForegroundColor Yellow
    }
    if ($SesionesSinAvance -ge $MaximoSesionesSinAvance) {
        Write-Host "$MaximoSesionesSinAvance sesiones seguidas sin avanzar: corto el carril." -ForegroundColor Red
        break
    }
    Write-Host "Otra sesión limpia en $SegundosParaCortarConCtrlC segundos (Ctrl+C para cortar)." -ForegroundColor DarkGray
    Start-Sleep -Seconds $SegundosParaCortarConCtrlC
}

if ($Carril -ne 'principal' -and -not (git -C $Carpeta status --porcelain)) {
    git -C $Principal worktree remove --force $Carpeta
    Write-Host "Carpeta $Carpeta borrada (no tenía cambios)."
}

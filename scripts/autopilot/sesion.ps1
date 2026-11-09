<#
Abre sesiones interactivas de Claude Code, una detrás de la otra, sobre la próxima etapa abierta
del ROADMAP. Cada sesión arranca limpia con el pedido de pedido.md; cuando termina su avance,
escribís /exit y se abre la siguiente. Un archivo .parity\autopilot\STOP corta todo.

Con -Continuo no hay que tocar nada: cada sesión corre sola (claude -p), al terminar arranca la
siguiente, ante un error (límite de uso, red) espera y reintenta, una etapa que no avanza dos veces
seguidas se saltea, y la PC no se suspende mientras corre. El registro completo de cada sesión queda
en .parity\autopilot\bucle\.

  .\scripts\autopilot\sesion.ps1                      # carril principal, en la carpeta kerbo
  .\scripts\autopilot\sesion.ps1 -Carril b            # otro carril, en ..\kerbo-b
  .\scripts\autopilot\sesion.ps1 -Etapa 14            # una etapa fija
  .\scripts\autopilot\sesion.ps1 -Modelo sonnet       # otro modelo que el de la tabla
  .\scripts\autopilot\sesion.ps1 -Esfuerzo high       # otro esfuerzo que el de la tabla
  .\scripts\autopilot\sesion.ps1 -Continuo            # sin intervención
#>
param(
    [string]$Carril = 'principal',
    [string]$Etapa = '',
    [string]$Modelo = '',
    [string]$Esfuerzo = '',
    [string]$Permisos = 'bypassPermissions',
    [string]$Compactar = '150k',
    [switch]$Continuo
)

$ErrorActionPreference = 'Stop'
$Principal = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Paridad = Join-Path $Principal '.parity'
$Tomas = Join-Path $Paridad 'autopilot'
$Parada = Join-Path $Tomas 'STOP'
$EtapasExcluidas = @('10')
# Mecánico con arnés (versiones, tandas de reglas): Sonnet. Diseño con paridad: Opus en medio.
# Investigación difícil (tipos, tests inteligentes): Opus en alto.
$ModeloPorDefecto = 'opus'
$EsfuerzoPorDefecto = 'medium'
$ModelosPorEtapa = @{ '17' = 'sonnet' }
$EsfuerzosPorEtapa = @{ '16' = 'high'; '11' = 'high' }
$Registros = Join-Path $Tomas 'bucle'
$SesionesSinAvanceParaSaltear = 2
$EsperaInicialTrasErrorEnMinutos = 5
$EsperaMaximaTrasErrorEnMinutos = 60
$SesionesSinAvance = @{}
$EtapasSalteadas = [System.Collections.Generic.List[string]]::new()

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

# La toma guarda el proceso dueño: si esa ventana se cerró de golpe, la toma quedó huérfana y se ignora.
function Tomada-Por-Otro([string]$Numero) {
    $Toma = Join-Path $Tomas "etapa-$Numero.carril"
    if (-not (Test-Path $Toma)) { return $false }
    $CarrilDueno, $ProcesoDueno = (Get-Content $Toma -Raw).Trim() -split ' '
    if ($CarrilDueno -eq $Carril) { return $false }
    if (-not $ProcesoDueno) { return $true }
    [bool](Get-Process -Id $ProcesoDueno -ErrorAction SilentlyContinue)
}

function Proxima-Etapa([string]$Carpeta) {
    $Etapas = Etapas-En-Orden $Carpeta | Where-Object { $EtapasSalteadas -notcontains $_.Numero }
    if ($Etapa) { return $Etapas | Where-Object { $_.Numero -eq $Etapa } | Select-Object -First 1 }
    $Etapas | Where-Object { $EtapasExcluidas -notcontains $_.Numero -and -not (Tomada-Por-Otro $_.Numero) } |
        Select-Object -First 1
}

function Mantener-Despierta {
    Add-Type -Namespace Kerbo -Name Energia -MemberDefinition @'
[DllImport("kernel32.dll")] public static extern uint SetThreadExecutionState(uint estado);
'@
    $EstadoPersistente = [uint32]2147483648
    $SistemaRequerido = [uint32]1
    [Kerbo.Energia]::SetThreadExecutionState([uint32]($EstadoPersistente -bor $SistemaRequerido)) | Out-Null
}

function Estado-Del-Trabajo([string]$Carpeta) {
    "$(git -C $Carpeta rev-parse HEAD) $(git -C $Carpeta status --porcelain)"
}

function Resumen-De-Herramienta($Entrada) {
    foreach ($Campo in 'description', 'file_path', 'pattern', 'command') {
        if ($Entrada.$Campo) { return ([string]$Entrada.$Campo).Split("`n")[0] }
    }
    ''
}

function Mostrar-Evento([string]$Linea) {
    try { $Evento = $Linea | ConvertFrom-Json } catch { return }
    if ($Evento.type -eq 'assistant') {
        foreach ($Bloque in $Evento.message.content) {
            if ($Bloque.type -eq 'text') { Write-Host $Bloque.text }
            elseif ($Bloque.type -eq 'tool_use') {
                Write-Host "  > $($Bloque.name) $(Resumen-De-Herramienta $Bloque.input)" -ForegroundColor DarkGray
            }
        }
    }
    elseif ($Evento.type -eq 'result') {
        $Minutos = [math]::Round($Evento.duration_ms / 60000)
        Write-Host "== Sesión terminada ($($Evento.subtype), $Minutos min)" -ForegroundColor Cyan
    }
}

function Iniciar-Desatendida([string[]]$Argumentos, [string]$Numero) {
    New-Item -ItemType Directory -Force $Registros | Out-Null
    $Registro = Join-Path $Registros ("{0}-etapa-{1}-{2}.jsonl" -f (Get-Date -Format 'yyyy-MM-dd_HHmm'), $Numero, $Carril)
    $Escritor = [System.IO.StreamWriter]::new($Registro, $false, [System.Text.UTF8Encoding]::new($false))
    try {
        & claude -p --output-format stream-json --verbose @Argumentos | ForEach-Object {
            $Escritor.WriteLine($_)
            $Escritor.Flush()
            Mostrar-Evento $_
        }
        return $LASTEXITCODE
    }
    finally { $Escritor.Dispose() }
}

function Esperar-Tras-Error([int]$ErroresSeguidos) {
    $Minutos = [math]::Min($EsperaMaximaTrasErrorEnMinutos, $EsperaInicialTrasErrorEnMinutos * [math]::Pow(2, $ErroresSeguidos - 1))
    Write-Host "La sesión terminó con error (límite de uso o red): reintento en $Minutos min." -ForegroundColor Yellow
    for ($Minuto = 0; $Minuto -lt $Minutos -and -not (Test-Path $Parada); $Minuto++) { Start-Sleep -Seconds 60 }
}

function Anotar-Avance([string]$Numero, [bool]$Avanzo) {
    if ($Avanzo) { $SesionesSinAvance[$Numero] = 0; return }
    $SesionesSinAvance[$Numero] = 1 + [int]$SesionesSinAvance[$Numero]
    if ($SesionesSinAvance[$Numero] -ge $SesionesSinAvanceParaSaltear) {
        $EtapasSalteadas.Add($Numero)
        Write-Host "La etapa $Numero no avanzó en $SesionesSinAvanceParaSaltear sesiones seguidas (¿espera una decisión tuya?): la salteo." -ForegroundColor Yellow
    }
}

if ($Continuo) {
    [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
    Mantener-Despierta
}
$ErroresSeguidos = 0
$Carpeta = Carpeta-Del-Carril
while (-not (Test-Path $Parada)) {
    Al-Dia $Carpeta
    $Elegida = Proxima-Etapa $Carpeta
    if (-not $Elegida) { Write-Host 'No queda ninguna etapa libre.' -ForegroundColor Yellow; break }
    $Toma = Join-Path $Tomas "etapa-$($Elegida.Numero).carril"
    Set-Content -Path $Toma -Value "$Carril $PID" -Encoding ASCII
    $ElModelo = $Modelo
    if (-not $ElModelo) { $ElModelo = $ModelosPorEtapa[$Elegida.Numero] }
    if (-not $ElModelo) { $ElModelo = $ModeloPorDefecto }
    $ElEsfuerzo = $Esfuerzo
    if (-not $ElEsfuerzo) { $ElEsfuerzo = $EsfuerzosPorEtapa[$Elegida.Numero] }
    if (-not $ElEsfuerzo) { $ElEsfuerzo = $EsfuerzoPorDefecto }
    $Mensaje = "Seguí la etapa $($Elegida.Numero) del ROADMAP ($($Elegida.Titulo)), carril $Carril. Leé scripts/autopilot/pedido.md y hacé lo que dice."
    Write-Host "== Etapa $($Elegida.Numero): $($Elegida.Titulo) | carril $Carril | $ElModelo, esfuerzo $ElEsfuerzo" -ForegroundColor Cyan
    $Argumentos = @('--model', $ElModelo, '--effort', $ElEsfuerzo, '--permission-mode', $Permisos,
        '--autocompact', $Compactar, '-n', "kerbo $($Elegida.Numero) ($Carril)", $Mensaje)
    $EstadoAntes = Estado-Del-Trabajo $Carpeta
    Push-Location $Carpeta
    try {
        if ($Continuo) { $CodigoDeSalida = Iniciar-Desatendida $Argumentos $Elegida.Numero }
        else { & claude @Argumentos }
    }
    finally { Pop-Location; Remove-Item $Toma -ErrorAction SilentlyContinue }
    if (-not $Continuo) {
        $Respuesta = Read-Host 'Enter: otra sesión limpia | q: salir'
        if ($Respuesta -eq 'q') { break }
        continue
    }
    if ($CodigoDeSalida -ne 0) {
        $ErroresSeguidos++
        Esperar-Tras-Error $ErroresSeguidos
        continue
    }
    $ErroresSeguidos = 0
    Anotar-Avance $Elegida.Numero ($EstadoAntes -ne (Estado-Del-Trabajo $Carpeta))
}

if ($Carril -ne 'principal' -and -not (git -C $Carpeta status --porcelain)) {
    git -C $Principal worktree remove --force $Carpeta
    Write-Host "Carpeta $Carpeta borrada (no tenía cambios)."
}

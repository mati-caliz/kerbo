# Instalador de kerbo para Windows (x86_64 y ARM) sin el bash de Git: baja la última versión de los
# releases de GitHub, verifica el sha256, deja kerbo.exe en %LOCALAPPDATA%\Programs\kerbo (o en
# KERBO_INSTALL_DIR) y suma esa carpeta al PATH del usuario. Corre en Windows PowerShell 5.1 y en
# PowerShell 7.
#
#   irm https://kerbo.matiascaliz.com.ar/install.ps1 | iex

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$Repository = 'mati-caliz/kerbo'
$X86Target = 'x86_64-pc-windows-msvc'
$ArmTarget = 'aarch64-pc-windows-msvc'
$Binary = 'kerbo.exe'
$PathSeparator = ';'

function Stop-Install([string]$Message) {
    throw "kerbo: $Message"
}

function Get-Target {
    $architecture = $env:PROCESSOR_ARCHITECTURE
    if ($env:PROCESSOR_ARCHITEW6432) {
        $architecture = $env:PROCESSOR_ARCHITEW6432
    }
    switch ($architecture) {
        'AMD64' { return $X86Target }
        'ARM64' { return $ArmTarget }
        default {
            Stop-Install "todavía no hay binario para Windows $architecture. Con Rust instalado: cargo install kerbo --locked"
        }
    }
}

function Get-LatestVersion {
    if ($env:KERBO_VERSION) {
        return $env:KERBO_VERSION
    }
    $release = Invoke-RestMethod -UseBasicParsing "https://api.github.com/repos/$Repository/releases/latest"
    if (-not $release.tag_name) {
        Stop-Install 'no se pudo averiguar la última versión'
    }
    return $release.tag_name.TrimStart('v')
}

function Test-Checksum([string]$Archive, [string]$ChecksumFile) {
    $expected = ((Get-Content $ChecksumFile -Raw).Trim() -split '\s+')[0].ToLowerInvariant()
    $actual = (Get-FileHash -Algorithm SHA256 $Archive).Hash.ToLowerInvariant()
    if ($expected -ne $actual) {
        Stop-Install 'el sha256 no coincide'
    }
}

function Add-ToUserPath([string]$Directory) {
    $current = [Environment]::GetEnvironmentVariable('Path', 'User')
    $entries = @($current -split $PathSeparator | Where-Object { $_ })
    if ($entries -contains $Directory) {
        return $false
    }
    [Environment]::SetEnvironmentVariable('Path', (($entries + $Directory) -join $PathSeparator), 'User')
    $env:Path = "$env:Path$PathSeparator$Directory"
    return $true
}

[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
$target = Get-Target
$version = Get-LatestVersion
$installDir = if ($env:KERBO_INSTALL_DIR) { $env:KERBO_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'Programs\kerbo' }
$archive = "kerbo-$version-$target.zip"
$baseUrl = "https://github.com/$Repository/releases/download/v$version"
$workDir = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $workDir | Out-Null
try {
    $archivePath = Join-Path $workDir $archive
    Invoke-WebRequest -UseBasicParsing "$baseUrl/$archive" -OutFile $archivePath
    Invoke-WebRequest -UseBasicParsing "$baseUrl/$archive.sha256" -OutFile "$archivePath.sha256"
    Test-Checksum $archivePath "$archivePath.sha256"
    Expand-Archive -Path $archivePath -DestinationPath $workDir -Force
    New-Item -ItemType Directory -Force -Path $installDir | Out-Null
    Copy-Item (Join-Path $workDir $Binary) (Join-Path $installDir $Binary) -Force
}
finally {
    Remove-Item -Recurse -Force $workDir -ErrorAction SilentlyContinue
}

Write-Host "kerbo $version instalado en $installDir\$Binary"
if (Add-ToUserPath $installDir) {
    Write-Host "Se sumó $installDir al PATH del usuario: abrí otra terminal para usarlo."
}

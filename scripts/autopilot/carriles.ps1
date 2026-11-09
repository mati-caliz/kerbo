<#
Abre una pestaña de Windows Terminal por carril, cada una con sesion.ps1: el primero en la carpeta
kerbo y los demás en su propia carpeta (..\kerbo-b, ...). Más de dos no rinde: la cola de procesos
pesados deja dos a la vez.

  .\scripts\autopilot\carriles.ps1            # dos carriles
  .\scripts\autopilot\carriles.ps1 -Cantidad 1
  .\scripts\autopilot\carriles.ps1 -Continuo   # sin intervención: ver sesion.ps1
#>
param([int]$Cantidad = 2, [switch]$Continuo)

$Sesion = Join-Path $PSScriptRoot 'sesion.ps1'
$Nombres = @('principal', 'b', 'c')
$Modo = if ($Continuo) { ' -Continuo' } else { '' }
$Pestanas = for ($Indice = 0; $Indice -lt $Cantidad; $Indice++) {
    $Nombre = $Nombres[$Indice]
    "new-tab --title `"kerbo $Nombre`" powershell -NoExit -ExecutionPolicy Bypass -File `"$Sesion`" -Carril $Nombre$Modo"
}
Start-Process wt.exe -ArgumentList ('-w kerbo ' + ($Pestanas -join ' ; '))

# scripts/download_soundfonts.ps1
# Script de aprovisionamiento de SoundFonts oficiales para CI/CD y despliegues automáticos
param (
    [string]$TargetDir = "soundfonts",
    [string]$BaseUrl = "https://github.com/Vladivostok214/ImpactSamplerStudio/releases/download/soundfonts-v1.0.0"
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path $TargetDir)) {
    New-Item -ItemType Directory -Path $TargetDir -Force | Out-Null
}

$soundfonts = @(
    @{
        RemoteName = "Mellotron.sf2"
        LocalName  = "Mellotron.sf2"
        Hash       = "F697AC18509B51061FE341CFA2D825DD221285882A4401A6A7121093081283F8"
    },
    @{
        RemoteName = "Piano.-.Realtime.sf2"
        LocalName  = "Piano - Realtime.sf2"
        Hash       = "3CC74C758618D58DCD67DB0A3BB51E73A6D7F78C7F312082645C695D388E66EE"
    },
    @{
        RemoteName = "Rhodes.from.Dream.Piano.sf2"
        LocalName  = "Rhodes (from Dream Piano).sf2"
        Hash       = "EA2733C334E382D60F5B02288302536ABDCF046EDE2B02907328E8BAB2F83B05"
    },
    @{
        RemoteName = "Yamaha_RX7_Drums.sf2"
        LocalName  = "Yamaha_RX7_Drums.sf2"
        Hash       = "E25B9E383FA8EDED889268446CA8F2D20C4E9D643503BAE0404C60CFA0C2EE11"
    }
)

function Get-Sha256Hex([string]$filePath) {
    $sha = [System.Security.Cryptography.SHA256]::Create()
    $fs = [System.IO.File]::OpenRead($filePath)
    $hash = [System.BitConverter]::ToString($sha.ComputeHash($fs)).Replace("-", "")
    $fs.Close()
    return $hash
}

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host " ImpactSampler Studio - Aprovisionamiento de SoundFonts " -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan

$allValid = $true

foreach ($sf in $soundfonts) {
    $dest = Join-Path $TargetDir $sf.LocalName
    $needsDownload = $true

    if (Test-Path $dest) {
        Write-Host "Verificando archivo local existente: $($sf.LocalName)..." -NoNewline
        $currentHash = Get-Sha256Hex $dest
        if ($currentHash -ieq $sf.Hash) {
            Write-Host " [OK - Checksum Verificado]" -ForegroundColor Green
            $needsDownload = $false
        } else {
            Write-Host " [Hash mismatch, re-descargando]" -ForegroundColor Yellow
            Remove-Item $dest -Force
        }
    }

    if ($needsDownload) {
        $url = "$BaseUrl/$($sf.RemoteName)"
        Write-Host "Descargando $($sf.LocalName) desde $url..." -ForegroundColor Magenta

        if (Get-Command curl.exe -ErrorAction SilentlyContinue) {
            & curl.exe -L --fail --retry 3 --retry-delay 2 -o $dest $url
            if ($LASTEXITCODE -ne 0) {
                Write-Error "Fallo al descargar $($sf.LocalName) con curl (exit code: $LASTEXITCODE)"
            }
        } else {
            $client = New-Object System.Net.WebClient
            $client.DownloadFile($url, $dest)
        }

        $computedHash = Get-Sha256Hex $dest
        if ($computedHash -ieq $sf.Hash) {
            Write-Host " -> $($sf.LocalName) verificado exitosamente [SHA256 Match]" -ForegroundColor Green
        } else {
            Write-Error "ERROR: Checksum invalido para $($sf.LocalName)! Esperado: $($sf.Hash), Obtenido: $computedHash"
            $allValid = $false
        }
    }
}

if (-not $allValid) {
    exit 1
}

Write-Host "Todos los SoundFonts de fabrica verificados e instalados correctamente en '$TargetDir'." -ForegroundColor Green

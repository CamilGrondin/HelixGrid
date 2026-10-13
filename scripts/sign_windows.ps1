<#
.SYNOPSIS
    Script de Signature de Code Windows pour HelixGrid (SmartScreen & Authenticode).

.DESCRIPTION
    Signe récursivement les binaires auxiliaires (sidecars AutoDock Vina), l'exécutable
    principal HelixGrid.exe et les installateurs MSI/NSIS avec signtool.exe en SHA-256
    avec horodatage RFC 3161 (DigiCert Timestamp).

.PARAMETER CertPath
    Chemin vers le fichier de certificat .pfx (ou variable SMARTSCREEN_CERT_PATH).

.PARAMETER CertPassword
    Mot de passe du certificat .pfx (ou variable SMARTSCREEN_CERT_PASSWORD).

.PARAMETER CertThumbprint
    Empreinte SHA-1 du certificat installé dans le magasin Windows (ou SMARTSCREEN_CERT_THUMBPRINT).
#>

[CmdletBinding()]
param(
    [string]$CertPath = $env:SMARTSCREEN_CERT_PATH,
    [string]$CertPassword = $env:SMARTSCREEN_CERT_PASSWORD,
    [string]$CertThumbprint = $env:SMARTSCREEN_CERT_THUMBPRINT,
    [string]$TimestampUrl = "http://timestamp.digicert.com"
)

$ErrorActionPreference = "Stop"

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "  HelixGrid - Signature Windows Authenticode & SmartScreen" -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan

# 1. Localisation de signtool.exe
$SignTool = $null
$WindowsKits = "C:\Program Files (x86)\Windows Kits\10\bin"

if (Get-Command "signtool.exe" -ErrorAction SilentlyContinue) {
    $SignTool = "signtool.exe"
} elseif (Test-Path $WindowsKits) {
    $SignTool = Get-ChildItem -Path $WindowsKits -Filter "signtool.exe" -Recurse |
        Where-Object { $_.FullName -match "x64" } |
        Select-Object -First 1 -ExpandProperty FullName
}

if (-not $SignTool -or -not (Test-Path $SignTool)) {
    Write-Error "[!] signtool.exe introuvable. Veuillez installer le Windows 10/11 SDK."
    exit 1
}

Write-Host "[+] SignTool détecté : $SignTool" -ForegroundColor Green

# 2. Validation du certificat
$SignArgs = @("sign", "/fd", "SHA256", "/tr", $TimestampUrl, "/td", "SHA256")

if ($CertPath -and (Test-Path $CertPath)) {
    Write-Host "[+] Utilisation du fichier PFX : $CertPath" -ForegroundColor Green
    $SignArgs += @("/f", $CertPath)
    if ($CertPassword) {
        $SignArgs += @("/p", $CertPassword)
    }
} elseif ($CertThumbprint) {
    Write-Host "[+] Utilisation du certificat par empreinte (Thumbprint) : $CertThumbprint" -ForegroundColor Green
    $SignArgs += @("/sha1", $CertThumbprint, "/sm")
} else {
    Write-Warning "[!] Aucun certificat spécifié (CertPath ou CertThumbprint absent)."
    Write-Host "    Pour tester sans certificat officiel, vous pouvez générer un certificat auto-signé :"
    Write-Host "    New-SelfSignedCertificate -Type CodeSigningCert -Subject 'CN=HelixGrid Development' -CertStoreLocation Cert:\CurrentUser\My"
    Write-Host "    Pour la production, consultez le guide SIGNING_WINDOWS_SMARTSCREEN.md."
    exit 1
}

# 3. Répertoire racine des artefacts
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectDir = Split-Path -Parent $ScriptDir
$TauriDir = Join-Path $ProjectDir "src-tauri"
$TargetDir = Join-Path $TauriDir "target\release"

# 4. Signature des binaires auxiliaires (sidecars AutoDock Vina)
Write-Host "`n[*] Étape 1 : Signature des binaires Vina sidecars..." -ForegroundColor Yellow
$VinaBins = @(
    (Join-Path $TauriDir "bin\vina-x86_64-pc-windows-msvc.exe"),
    (Join-Path $TauriDir "bin\vina.exe")
)

foreach ($bin in $VinaBins) {
    if (Test-Path $bin) {
        Write-Host "    -> Signature de $(Split-Path -Leaf $bin)..."
        & $SignTool @SignArgs $bin
    }
}

# 5. Signature des exécutables et packages d'installation générés par Tauri
Write-Host "`n[*] Étape 2 : Signature des artefacts finaux (HelixGrid.exe, MSI, NSIS)..." -ForegroundColor Yellow

$Artifacts = Get-ChildItem -Path $TargetDir -Recurse -Include "HelixGrid.exe", "*.msi", "*.exe" |
    Where-Object { $_.FullName -notmatch "build|deps|incremental" }

if ($Artifacts.Count -eq 0) {
    Write-Warning "[-] Aucun artefact trouvé dans $TargetDir. Avez-vous lancé 'tauri build' ?"
} else {
    foreach ($art in $Artifacts) {
        Write-Host "    -> Signature de $($art.Name)..."
        & $SignTool @SignArgs $art.FullName
    }
}

# 6. Vérification de la signature
Write-Host "`n[*] Étape 3 : Vérification de la chaîne de signature..." -ForegroundColor Yellow
foreach ($art in $Artifacts) {
    & $SignTool verify /pa /v $art.FullName
}

Write-Host "`n==========================================================" -ForegroundColor Green
Write-Host "  [SUCCÈS] Tous les binaires Windows ont été signés et horodatés !" -ForegroundColor Green
Write-Host "==========================================================" -ForegroundColor Green

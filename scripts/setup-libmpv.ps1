# Prépare libmpv/ à la racine du dépôt : libmpv-2.dll, en-têtes (include/mpv) et mpv.lib.
# Idempotent : ne fait rien si libmpv/libmpv-2.dll et libmpv/mpv.lib existent déjà.
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$dest = Join-Path $root 'libmpv'
if ((Test-Path "$dest\libmpv-2.dll") -and (Test-Path "$dest\mpv.lib")) {
    Write-Host "libmpv/ déjà prêt."
    return
}

$tmp = Join-Path ([IO.Path]::GetTempPath()) ("epikodi-libmpv-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
    # Dernière release shinchiro, variante x86_64 de base (pas v3, pas i686, pas gcc).
    $rel = Invoke-RestMethod 'https://api.github.com/repos/shinchiro/mpv-winbuild-cmake/releases/latest' -Headers @{ 'User-Agent' = 'epikodi-setup' }
    $asset = $rel.assets | Where-Object { $_.name -match '^mpv-dev-x86_64-\d.*\.7z$' } | Select-Object -First 1
    if (-not $asset) { throw "Aucun asset mpv-dev-x86_64-*.7z dans la release $($rel.tag_name)" }
    Write-Host "Téléchargement de $($asset.name)"
    $archive = Join-Path $tmp $asset.name
    Invoke-WebRequest $asset.browser_download_url -OutFile $archive

    $ex = Join-Path $tmp 'extract'
    New-Item -ItemType Directory -Path $ex | Out-Null
    # tar.exe de Windows (bsdtar) explicitement : un tar GNU du PATH ne lit pas le 7z.
    & "$env:SystemRoot\System32\tar.exe" -xf $archive -C $ex
    if ($LASTEXITCODE -ne 0) { throw "Extraction échouée" }

    $dll = Get-ChildItem $ex -Recurse -Filter 'libmpv-2.dll' | Select-Object -First 1
    $inc = Get-ChildItem $ex -Recurse -Directory -Filter 'mpv' | Where-Object { Test-Path "$($_.FullName)\client.h" } | Select-Object -First 1
    if (-not $dll -or -not $inc) { throw "libmpv-2.dll ou en-têtes introuvables dans l'archive" }

    New-Item -ItemType Directory -Force -Path "$dest\include" | Out-Null
    Copy-Item $dll.FullName "$dest\libmpv-2.dll" -Force
    Copy-Item $inc.FullName "$dest\include" -Recurse -Force

    # mpv.lib : .def généré depuis les exports de la DLL, puis lib.exe (MSVC via vswhere).
    $vswhere = 'C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe'
    $vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $vs) { throw "Outils MSVC introuvables (vswhere)" }
    $msvcVer = Get-Content "$vs\VC\Auxiliary\Build\Microsoft.VCToolsVersion.default.txt" -TotalCount 1
    $bin = "$vs\VC\Tools\MSVC\$($msvcVer.Trim())\bin\Hostx64\x64"

    $exports = & "$bin\dumpbin.exe" /exports "$dest\libmpv-2.dll"
    $names = foreach ($l in $exports) {
        if ($l -match '^\s+\d+\s+[0-9A-F]+\s+[0-9A-F]{8}\s+(\S+)') { $Matches[1] }
    }
    if (-not $names) { throw "Aucun export trouvé dans libmpv-2.dll" }
    Set-Content "$dest\mpv.def" (@('LIBRARY libmpv-2.dll', 'EXPORTS') + $names) -Encoding ascii

    & "$bin\lib.exe" /nologo "/def:$dest\mpv.def" /name:libmpv-2.dll "/out:$dest\mpv.lib" /MACHINE:x64
    if ($LASTEXITCODE -ne 0) { throw "lib.exe a échoué" }
    Write-Host "libmpv/ prêt ($($asset.name))."
} finally {
    Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
}

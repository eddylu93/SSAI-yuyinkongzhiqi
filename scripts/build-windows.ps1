param([switch]$StageOnly)
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
. ./scripts/dev-toolchain.ps1
function Run-Checked([string]$Program, [string[]]$Arguments) {
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Program exited with $LASTEXITCODE" }
}
if (!$StageOnly) {
    Run-Checked npm.cmd @('ci')
    Run-Checked npm.cmd @('test')
    Run-Checked npm.cmd @('run','build')
    Run-Checked cargo @('test','--locked','--features','custom-protocol','--manifest-path','src-tauri/Cargo.toml')
    Run-Checked cargo @('build','--locked','--release','--features','custom-protocol','--manifest-path','src-tauri/Cargo.toml')
}
$stage = 'release/windows-0.8.2'
$buildDir = 'src-tauri/target/release'
New-Item -Force -ItemType Directory $stage | Out-Null
foreach ($pair in @(@("$buildDir/au05-desk.exe","$stage/AU05Desk.exe"),@("$buildDir/WebView2Loader.dll","$stage/WebView2Loader.dll"))) {
    if (!(Test-Path $pair[1]) -or (Get-FileHash $pair[0]).Hash -ne (Get-FileHash $pair[1]).Hash) {
        Copy-Item $pair[0] $pair[1] -Force
    }
}
$noticePaths = 'vendor/README.md','vendor/AU05-Keys-LICENSE.txt','vendor/OpenVibeKey-Lighting-LICENSE.txt','vendor/Open-VibeKey-LICENSE.txt','node_modules/@phosphor-icons/vue/LICENSE','node_modules/vue/LICENSE','node_modules/three/LICENSE','node_modules/@tauri-apps/api/LICENSE-MIT'
$notice = foreach ($path in $noticePaths) {
    if (!(Test-Path $path)) { throw "Missing license: $path" }
    $path
    Get-Content -Raw -Encoding utf8 $path
}
$notice | Set-Content "$stage/ThirdPartyNotices.txt" -Encoding utf8
Run-Checked python @('scripts/source-archive.py',"$stage/AU05-Desk-source.zip")
Run-Checked makensis @('/INPUTCHARSET','UTF8','packaging/windows.nsi')

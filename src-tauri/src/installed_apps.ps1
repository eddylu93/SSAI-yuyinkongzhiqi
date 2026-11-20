$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$apps = @{}

function Add-App([string]$name, [string]$target) {
    if (!$name -or !$target -or $name -match '^(卸载|移除|Uninstall\b|Remove\b)') { return }
    if ($target -notlike 'shell:AppsFolder\*') {
        if (![IO.Path]::IsPathRooted($target) -or [IO.Path]::GetExtension($target) -ne '.exe' -or ![IO.File]::Exists($target)) { return }
    }
    if ($target -like 'shell:AppsFolder\OpenAI.Codex_*!*' -and $name -eq 'ChatGPT') { $name = 'Codex（ChatGPT）' }
    if ($name -in 'Weixin','WeChat') { $name = '微信' }
    if (!$apps.ContainsKey($target)) { $apps[$target] = [ordered]@{name=$name; app=$target} }
}

$shell = New-Object -ComObject Shell.Application
foreach ($entry in $shell.NameSpace('shell:AppsFolder').Items()) {
    $id = [string]$entry.Path
    if ($id -match '^[A-Za-z0-9.-]+_[A-Za-z0-9]+![A-Za-z0-9.-]+$') {
        Add-App $entry.Name ('shell:AppsFolder\' + $id)
    } elseif (!$entry.ExtendedProperty('System.Link.Arguments')) {
        Add-App $entry.Name ([string]$entry.ExtendedProperty('System.Link.TargetParsingPath'))
    }
}

# App Paths also covers desktop clients installed without a Start menu shortcut.
foreach ($root in 'HKCU:\Software','HKLM:\Software','HKLM:\Software\WOW6432Node') {
    foreach ($key in Get-ChildItem "$root\Microsoft\Windows\CurrentVersion\App Paths" -ErrorAction SilentlyContinue) {
        $path = [Environment]::ExpandEnvironmentVariables(([string]$key.GetValue('')).Trim('"'))
        if (![IO.File]::Exists($path)) { continue }
        try { $name = [Diagnostics.FileVersionInfo]::GetVersionInfo($path).FileDescription }
        catch { $name = $null }
        if (!$name) { $name = [IO.Path]::GetFileNameWithoutExtension($path) }
        Add-App $name $path
    }
}

# Portable clients may only be discoverable while their desktop window is open.
foreach ($process in Get-Process | Where-Object { $_.MainWindowHandle -ne 0 -and $_.Name -in 'Codex','ChatGPT','Claude','DeepSeek','DeepSeek Harness','WorkBuddy','Weixin','WeChat','Doubao' }) {
    if ($process.Path -and $process.Path -notlike '*\WindowsApps\*') { Add-App $process.Name $process.Path }
}
[Console]::Write((ConvertTo-Json -InputObject @($apps.Values | Sort-Object { $_.name },{ $_.app }) -Compress))

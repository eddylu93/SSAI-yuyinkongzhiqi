$ErrorActionPreference = 'Stop'
$target = $env:AU05_FOCUS_TARGET | ConvertFrom-Json
Add-Type -AssemblyName UIAutomationClient
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class ComposerWindow {
    [DllImport("oleacc.dll")] public static extern int AccessibleObjectFromWindow(
        IntPtr window, uint id, ref Guid iid, [MarshalAs(UnmanagedType.Interface)] out object result);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool ShowWindowAsync(IntPtr h, int cmd);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, IntPtr pid);
    [DllImport("user32.dll")] static extern bool AttachThreadInput(uint from, uint to, bool attach);
    [DllImport("user32.dll")] static extern bool BringWindowToTop(IntPtr h);
    public static void Activate(IntPtr target) {
        uint self = GetCurrentThreadId();
        uint front = GetWindowThreadProcessId(GetForegroundWindow(), IntPtr.Zero);
        bool attached = front != 0 && front != self && AttachThreadInput(self, front, true);
        try {
            BringWindowToTop(target);
            SetForegroundWindow(target);
        } finally {
            if (attached) AttachThreadInput(self, front, false);
        }
    }
}
"@
function Find-ChatWindow {
    $candidates = @(Get-Process -Name $target.processes -ErrorAction SilentlyContinue | Where-Object {
        $_.MainWindowHandle -ne 0 -and (!$target.package -or
          (($_.Path -split '\\') -like $target.package.Split('!')[0]) -or
          ($_.ProcessName -eq $target.id -and $_.MainModule.FileVersionInfo.ProductName -like ('*'+$target.name+'*')))
    })
    $front = [ComposerWindow]::GetForegroundWindow()
    $chosen = $candidates | Where-Object MainWindowHandle -eq $front | Select-Object -First 1
    if ($chosen) {return $chosen}
    if ($candidates.Count -eq 1) {return $candidates[0]}
    if ($candidates.Count -gt 1) {exit 2}
}
$clock = [Diagnostics.Stopwatch]::StartNew()
$app = Find-ChatWindow
if (!$app) {
    $installed = @(Get-StartApps | Where-Object {
        ($target.package -and $_.AppID -like $target.package) -or $_.Name -in $target.startNames
    })
    if ($installed.Count -ne 1) {exit 2}
    Start-Process explorer.exe -ArgumentList ('shell:AppsFolder\'+$installed[0].AppID)
    while (!$app -and $clock.Elapsed.TotalSeconds -lt 9) {
        Start-Sleep -Milliseconds 140
        $app = Find-ChatWindow
    }
    if (!$app) {exit 2}
}
$window = $app.MainWindowHandle
if ($target.id -eq 'wechat') {
    # WeChat uses the same process for chat, media viewers and other windows.
    # Process.MainWindowHandle can refer to whichever was last raised.
    $processCondition = [Windows.Automation.PropertyCondition]::new(
        [Windows.Automation.AutomationElement]::ProcessIdProperty,$app.Id)
    $windows = @([Windows.Automation.AutomationElement]::RootElement.FindAll(
        [Windows.Automation.TreeScope]::Children,$processCondition))
    $front = [ComposerWindow]::GetForegroundWindow()
    $active = @($windows | Where-Object { [IntPtr]$_.Current.NativeWindowHandle -eq $front })
    $main = @($windows | Where-Object { $_.Current.ClassName -in 'WeChatMainWndForPC','mmui::MainWindow' -or
        $_.Current.Name -match '^(微信|Weixin|WeChat)(\s*[（(]\d+[）)])?$' })
    if ($active.Count -eq 1) { $window = $front }
    elseif ($main.Count -eq 1) { $window = [IntPtr]$main[0].Current.NativeWindowHandle }
    elseif ($windows.Count -gt 1) { exit 2 }
}
if ([ComposerWindow]::IsIconic($window)) {[void][ComposerWindow]::ShowWindowAsync($window,9)}
if ([ComposerWindow]::GetForegroundWindow() -ne $window) {
    [void](New-Object -ComObject WScript.Shell).AppActivate($app.Id)
    [void][ComposerWindow]::SetForegroundWindow($window)
    if ([ComposerWindow]::GetForegroundWindow() -ne $window) {
        [ComposerWindow]::Activate($window)
    }
}
$condition = [Windows.Automation.PropertyCondition]::new(
    [Windows.Automation.AutomationElement]::ControlTypeProperty,[Windows.Automation.ControlType]::Edit)
$root = [Windows.Automation.AutomationElement]::FromHandle($window)
# Older Electron renderers expose the composer via MSAA. Require a Send button
# in the edit's own group so drafts still match after the placeholder disappears.
function Find-LegacyRole($node,$role,$depth) {
    if (!$node -or $depth -le 0 -or $clock.Elapsed.TotalSeconds -gt 15) {return}
    try {if ($node.accRole(0) -eq $role) {,$node; return}} catch {}
    for ($i=1; $i -le $node.accChildCount; $i++) {
        try {$child = $node.accChild($i)} catch {continue}
        Find-LegacyRole $child $role ($depth-1)
    }
}
function Focus-LegacyEditor {
    $class = [Windows.Automation.PropertyCondition]::new(
        [Windows.Automation.AutomationElement]::ClassNameProperty,'Chrome_RenderWidgetHostHWND')
    $matches = @(
        foreach ($renderer in $root.FindAll([Windows.Automation.TreeScope]::Descendants,$class)) {
            $iid = [Guid]'618736e0-3c3d-11cf-810c-00aa00389b71'
            $doc = $null
            if ([ComposerWindow]::AccessibleObjectFromWindow([IntPtr]$renderer.Current.NativeWindowHandle,
                [uint32]4294967292,[ref]$iid,[ref]$doc) -ne 0) {continue}
            foreach ($edit in @(Find-LegacyRole $doc 42 42)) {
                try {
                    $state = $edit.accState(0)
                    if (!($state -band 0x100000) -or ($state -band 0x20018041)) {continue}
                    $group = $edit.accParent.accParent
                    $send = @(Find-LegacyRole $group 43 7 | Where-Object {
                        try {$_.accName(0) -match '^(发送|发送消息|Send|Send message)$'} catch {$false}
                    })
                    if ($send.Count -eq 1) {,$edit}
                } catch {}
            }
        }
    )
    if ($clock.Elapsed.TotalSeconds -gt 15 -or $matches.Count -ne 1 -or [ComposerWindow]::GetForegroundWindow() -ne $window) {return $false}
    $matches[0].accSelect(1,0)
    Start-Sleep -Milliseconds 120
    return (($matches[0].accState(0) -band 4) -ne 0)
}
$limit = $clock.Elapsed.TotalSeconds + 3
do {
    if ([ComposerWindow]::GetForegroundWindow() -eq $window) {
        $edits = @($root.FindAll([Windows.Automation.TreeScope]::Descendants,$condition) | Where-Object {
            $x = $_.Current
            !$x.IsOffscreen -and !$x.IsPassword -and $x.IsEnabled -and $x.IsKeyboardFocusable -and
            ($x.Name -notmatch '搜索|search|password|密码') -and
            (($target.editorClass -and $x.ClassName -match $target.editorClass) -or
             ($x.Name -match $target.editorName) -or
             ($target.id -eq 'wechat' -and $x.AutomationId -match '(?:^|[.])chat_input_field$'))
        })
        $named = @($edits | Where-Object {$_.Current.Name -match $target.editorName})
        if ($named.Count -eq 1) {$edits = $named}
        if ($edits.Count -eq 1) {
            $edits[0].SetFocus()
            $focused = [Windows.Automation.AutomationElement]::FocusedElement
            if ([ComposerWindow]::GetForegroundWindow() -eq $window -and
                [Windows.Automation.Automation]::Compare($edits[0],$focused)) {
                [Console]::WriteLine($window.ToInt64()); exit 0
            }
        } elseif (!$edits.Count -and $target.id -eq 'wechat' -and $root.Current.ClassName -like 'Qt*QWindowIcon') {
            $renderClass = [Windows.Automation.PropertyCondition]::new(
                [Windows.Automation.AutomationElement]::ClassNameProperty,'MMUIRenderSubWindowHW')
            if ($root.FindFirst([Windows.Automation.TreeScope]::Descendants,$renderClass)) {
                Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition $env:AU05_WECHAT_HELPER
                # Wait for the restored window to finish its desktop animation.
                Start-Sleep -Milliseconds 450
                if ([WeChatComposer]::Focus($window)) {
                    [Console]::WriteLine($window.ToInt64()); exit 0
                }
                exit 4
            }
        } elseif (!$edits.Count -and $target.legacy -and (Focus-LegacyEditor)) {
            if ([ComposerWindow]::GetForegroundWindow() -eq $window) {
                [Console]::WriteLine($window.ToInt64()); exit 0
            }
        }
    }
    Start-Sleep -Milliseconds 90
} while ($clock.Elapsed.TotalSeconds -lt $limit)
if ([ComposerWindow]::GetForegroundWindow() -ne $window) {exit 3}
exit 4

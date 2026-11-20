$ErrorActionPreference='Stop'
[Console]::InputEncoding=[Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
Add-Type -AssemblyName UIAutomationClient
$request=[Console]::In.ReadToEnd() | ConvertFrom-Json
$e=[Windows.Automation.AutomationElement]::FocusedElement
if(!$e -or $e.Current.IsPassword -or !$e.Current.IsEnabled){exit 2}
$pattern=$null
if(!$e.TryGetCurrentPattern([Windows.Automation.TextPattern]::Pattern,[ref]$pattern)){exit 2}
$value=$null
if($e.TryGetCurrentPattern([Windows.Automation.ValuePattern]::Pattern,[ref]$value) -and $value.Current.IsReadOnly){exit 2}
if($e.Current.ControlType -ne [Windows.Automation.ControlType]::Edit -and $e.Current.ControlType -ne [Windows.Automation.ControlType]::Document){exit 2}
$id=($e.GetRuntimeId() -join '.')+'@'+$e.Current.ProcessId
if($request.id -and $id -ne $request.id){exit 2}
if($request.text){
    $sel=$pattern.GetSelection()
    if($sel.Count -ne 1){exit 2}
    $range=$sel[0].Clone()
    $range.MoveEndpointByRange([Windows.Automation.Text.TextPatternRangeEndpoint]::Start,$range,[Windows.Automation.Text.TextPatternRangeEndpoint]::End)
    $length=[string]$request.text
    [void]$range.MoveEndpointByUnit([Windows.Automation.Text.TextPatternRangeEndpoint]::Start,[Windows.Automation.Text.TextUnit]::Character,(-$length.Length))
    if($range.GetText($length.Length) -cne $length){exit 2}
}
[Console]::Write($id)

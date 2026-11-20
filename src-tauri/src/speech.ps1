$ErrorActionPreference='Stop'
[Console]::InputEncoding = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
Add-Type -AssemblyName System.Speech
$bytes=[Convert]::FromBase64String([Console]::In.ReadToEnd())
$stream=New-Object IO.MemoryStream(,$bytes)
try {
    $recognizers=[System.Speech.Recognition.SpeechRecognitionEngine]::InstalledRecognizers()
    $choice=$recognizers | Where-Object { $_.Culture.Name -eq 'zh-CN' } | Select-Object -First 1
    if (!$choice){$choice=$recognizers | Select-Object -First 1}
    if (!$choice){throw 'No system recognizer'}
    $engine=[System.Speech.Recognition.SpeechRecognitionEngine]::new($choice)
    $engine.LoadGrammar([System.Speech.Recognition.DictationGrammar]::new())
    $engine.SetInputToWaveStream($stream)
    $parts=@()
    while ($parts.Count -lt 128) {
        try { $result=$engine.Recognize() }
        catch [InvalidOperationException] {
            if ($stream.Position -eq $stream.Length -and $parts.Count -gt 0){break}
            throw
        }
        if (!$result){break}
        if ($result.Text){$parts += $result.Text}
    }
    [Console]::Write(($parts -join ' '))
} finally { if($engine){$engine.Dispose()};$stream.Dispose() }

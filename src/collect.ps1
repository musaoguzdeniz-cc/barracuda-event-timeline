$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)

function Limit-Text([string]$text, [int]$length) {
    if ($text.Length -le $length) { return $text }
    if ([char]::IsHighSurrogate($text[$length - 1])) { $length-- }
    return $text.Substring(0, $length)
}

$sources = @()
$paths = ConvertFrom-Json -InputObject $env:BARRACUDA_TIMELINE_FILES
if ($paths.Count -gt 0) {
    foreach ($path in $paths) {
        $fullPath = [IO.Path]::GetFullPath($path)
        $sources += @{ path = $fullPath; name = [IO.Path]::GetFileName($fullPath); offline = $true }
    }
} else {
    foreach ($channel in @('System', 'Application', 'Microsoft-Windows-WindowsUpdateClient/Operational')) {
        $sources += @{ path = $channel; name = $channel; offline = $false }
    }
}

$interesting = "Level=1 or Level=2 or Level=3" +
    " or (Provider[@Name='Microsoft-Windows-Kernel-General'] and (EventID=12 or EventID=13))" +
    " or (Provider[@Name='Microsoft-Windows-Kernel-Power'] and EventID=41)" +
    " or (Provider[@Name='EventLog'] and (EventID=6005 or EventID=6006 or EventID=6008))" +
    " or (Provider[@Name='User32'] and EventID=1074)" +
    " or (Provider[@Name='Service Control Manager'] and (EventID=7040 or EventID=7045))" +
    " or (Provider[@Name='Windows Error Reporting'] and EventID=1001)" +
    " or (Provider[@Name='Microsoft-Windows-WindowsUpdateClient'] and (EventID=19 or EventID=20 or EventID=43 or EventID=44))"

$events = New-Object 'System.Collections.Generic.List[object]'
$warnings = New-Object 'System.Collections.Generic.List[string]'
$opened = 0
$limit = 850
foreach ($source in $sources) {
    $reader = $null
    try {
        $condition = '(' + $interesting + ')'
        $pathType = [System.Diagnostics.Eventing.Reader.PathType]::FilePath
        if (-not $source.offline) {
            $condition += ' and TimeCreated[timediff(@SystemTime) <= 604800000]'
            $pathType = [System.Diagnostics.Eventing.Reader.PathType]::LogName
        }
        $query = [System.Diagnostics.Eventing.Reader.EventLogQuery]::new(
            $source.path, $pathType, ('*[System[' + $condition + ']]'))
        $query.ReverseDirection = $true
        $reader = [System.Diagnostics.Eventing.Reader.EventLogReader]::new($query)
        $opened++
        for ($count = 0; $count -le $limit; $count++) {
            $item = $reader.ReadEvent()
            if ($null -eq $item) { break }
            try {
                if ($count -eq $limit) {
                    $warnings.Add(('{0}: yalnızca en yeni {1} eşleşme gösteriliyor.' -f $source.name, $limit))
                    break
                }
                if ($null -eq $item.TimeCreated) { continue }
                $message = ''
                $xml = ''
                try { $message = [string]$item.FormatDescription() } catch {}
                try { $xml = [string]$item.ToXml() } catch {}
                $clipped = $message.Length -gt 4000 -or $xml.Length -gt 9000
                if ($message.Length -gt 4000) { $message = Limit-Text $message 4000 }
                if ($xml.Length -gt 9000) { $xml = Limit-Text $xml 9000 }
                $events.Add(@{
                    time = $item.TimeCreated.ToUniversalTime().ToString('o')
                    computer = [string]$item.MachineName; channel = [string]$item.LogName
                    provider = [string]$item.ProviderName; id = [uint32]$item.Id
                    record = $item.RecordId; level = $item.Level
                    source = $source.name; message = $message; xml = $xml; clipped = $clipped
                })
            } finally {
                $item.Dispose()
            }
        }
    } catch {
        $warnings.Add(('{0}: {1}' -f $source.name, $_.Exception.Message))
    } finally {
        if ($null -ne $reader) { $reader.Dispose() }
    }
}
[Console]::Write((ConvertTo-Json -InputObject @{ events = $events; warnings = $warnings; opened = $opened } -Depth 5 -Compress))

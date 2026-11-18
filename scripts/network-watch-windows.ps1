<#
  Network test, Windows (master prompt, sections 2 and 20; ADR 0028): while
  the end-to-end suite runs, samples the TCP connections and UDP endpoints of
  the app's process tree (project-write.exe and everything it starts, such as
  msedgewebview2.exe), and afterwards reads the DNS queries of that tree from
  the DNS client log. scripts/network-check.mjs evaluates the output.

  Sampling misses connections shorter than the interval, and UDP endpoints
  show no remote address, so this part only reports (see docs/sicherheit.md).

  Usage:
    pwsh -File scripts/network-watch-windows.ps1 -Out <file> -StopFile <file>
  Ends when the stop file exists. Writes one JSON object per line.
#>
param(
  [Parameter(Mandatory = $true)][string]$Out,
  [Parameter(Mandatory = $true)][string]$StopFile,
  [string]$AppName = 'project-write.exe',
  [int]$IntervalMs = 1000
)

$ErrorActionPreference = 'Stop'
$dnsLog = 'Microsoft-Windows-DNS-Client/Operational'

# The DNS client log is off by default. Its event 3006 names each query and
# is written in the process that asks (dnsapi.dll), so it carries its id.
try {
  wevtutil sl $dnsLog /e:true /q:true
} catch {
  Write-Warning "The DNS client log could not be enabled: $_"
}
$started = Get-Date

$folder = Split-Path -Parent ([System.IO.Path]::GetFullPath($Out))
New-Item -ItemType Directory -Force -Path $folder | Out-Null
$writer = [System.IO.StreamWriter]::new($Out, $false, [System.Text.UTF8Encoding]::new($false))
# Every process of the tree seen during the run: pid -> name.
$seen = @{}

function Write-Entry($entry) {
  $writer.WriteLine((ConvertTo-Json -Compress -Depth 5 $entry))
  $writer.Flush()
}

try {
  while (-not (Test-Path $StopFile)) {
    $all = @(Get-CimInstance -ClassName Win32_Process -Property ProcessId, ParentProcessId, Name)
    $children = @{}
    foreach ($process in $all) {
      $parent = [int]$process.ParentProcessId
      if (-not $children.ContainsKey($parent)) {
        $children[$parent] = [System.Collections.Generic.List[object]]::new()
      }
      $children[$parent].Add($process)
    }
    # The app and all its descendants.
    $queue = [System.Collections.Generic.Queue[object]]::new()
    foreach ($process in $all) {
      if ($process.Name -ieq $AppName) {
        $queue.Enqueue($process)
      }
    }
    $ids = [System.Collections.Generic.HashSet[int]]::new()
    $tree = [System.Collections.Generic.List[object]]::new()
    while ($queue.Count -gt 0) {
      $process = $queue.Dequeue()
      $id = [int]$process.ProcessId
      if (-not $ids.Add($id)) {
        continue
      }
      $tree.Add([ordered]@{ pid = $id; ppid = [int]$process.ParentProcessId; name = [string]$process.Name })
      $seen[$id] = [string]$process.Name
      if ($children.ContainsKey($id)) {
        foreach ($child in $children[$id]) {
          $queue.Enqueue($child)
        }
      }
    }

    $tcp = @()
    $udp = @()
    if ($ids.Count -gt 0) {
      $tcp = @(Get-NetTCPConnection -ErrorAction SilentlyContinue |
          Where-Object { $ids.Contains([int]$_.OwningProcess) } |
          ForEach-Object {
            [ordered]@{
              pid = [int]$_.OwningProcess
              localAddress = [string]$_.LocalAddress
              localPort = [int]$_.LocalPort
              remoteAddress = [string]$_.RemoteAddress
              remotePort = [int]$_.RemotePort
              state = [string]$_.State
            }
          })
      $udp = @(Get-NetUDPEndpoint -ErrorAction SilentlyContinue |
          Where-Object { $ids.Contains([int]$_.OwningProcess) } |
          ForEach-Object {
            [ordered]@{
              pid = [int]$_.OwningProcess
              localAddress = [string]$_.LocalAddress
              localPort = [int]$_.LocalPort
            }
          })
    }
    Write-Entry ([ordered]@{
        type = 'sample'
        time = (Get-Date).ToString('o')
        processes = $tree.ToArray()
        tcp = $tcp
        udp = $udp
      })
    Start-Sleep -Milliseconds $IntervalMs
  }

  # DNS queries of the tree since the start.
  $events = @(Get-WinEvent -FilterHashtable @{ LogName = $dnsLog; Id = 3006; StartTime = $started } -ErrorAction SilentlyContinue)
  foreach ($record in $events) {
    $id = [int]$record.ProcessId
    if ($seen.ContainsKey($id)) {
      Write-Entry ([ordered]@{
          type = 'dns'
          time = $record.TimeCreated.ToString('o')
          pid = $id
          process = $seen[$id]
          name = [string]$record.Properties[0].Value
        })
    }
  }
} finally {
  $writer.Dispose()
}

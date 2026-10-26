# Puts the FDA's drug label files (openFDA) on a Cindrix drive, in <Target>\drug-labels\raw.
# Then open Cindrix > Intel > Drug labels and press "Get the labels ready" (it makes them searchable, once).
#
# Usage:
#   powershell -File scripts\get-druglabels.ps1 -Target D:\Cindrix
#   powershell -File scripts\get-druglabels.ps1 -Target D:\Cindrix -Yes     (do not ask first)
#
# Needs internet and several gigabytes. Finished files are skipped, so run it again to finish after an
# interruption. This runs on its own, outside the program: the program itself never goes to the internet.
# Data: openFDA drug labels, U.S. Food and Drug Administration, public information (https://open.fda.gov/terms/).
# The labels are shown to READ. They are not advice, and the assistant still never gives drug doses.
param(
  [Parameter(Mandatory=$true)][string]$Target,
  [switch]$Yes
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$raw = Join-Path $Target 'drug-labels\raw'
New-Item -ItemType Directory -Force -Path $raw | Out-Null

Write-Host "Looking up the list of FDA label files..."
$index = Invoke-RestMethod -Uri 'https://api.fda.gov/download.json' -UseBasicParsing
$parts = @($index.results.drug.label.partitions)
if ($parts.Count -eq 0) { Write-Host "The FDA list did not have drug labels in it. Try again later."; exit 1 }

$todo = @($parts | Where-Object { -not (Test-Path (Join-Path $raw (([IO.Path]::GetFileName($_.file)) -replace '\.zip$',''))) })
$mb = ($todo | ForEach-Object { [double]($_.size_mb) } | Measure-Object -Sum).Sum
Write-Host ("{0} files in all, {1} already on the drive, {2} to download (about {3:N1} GB zipped, more once opened)." -f $parts.Count, ($parts.Count - $todo.Count), $todo.Count, ($mb/1024))
if ($todo.Count -eq 0) { Write-Host "Nothing to do."; exit 0 }
$free = (Get-Item $Target).PSDrive.Free
if ($free -and ($mb * 1MB * 4) -gt $free) { Write-Host ("There may not be enough room on that drive ({0:N1} GB free). Opened, the files are about four times bigger." -f ($free/1GB)); exit 1 }
if (-not $Yes) { if ((Read-Host "Download them now? (y/n)") -notmatch '^[yY]') { exit 0 } }

foreach ($p in $todo) {
  $zipName = [IO.Path]::GetFileName($p.file)
  $zip = Join-Path $raw "$zipName.part"
  Write-Host ("Downloading {0}..." -f $zipName)
  try {
    Invoke-WebRequest -Uri $p.file -OutFile $zip -UseBasicParsing
    Expand-Archive -Path $zip -DestinationPath $raw -Force
    Remove-Item $zip -Force
  } catch {
    Write-Host ("  Could not get {0}. Run this again later to finish it. ({1})" -f $zipName, $_.Exception.Message)
    if (Test-Path $zip) { Remove-Item $zip -Force }
  }
}
Write-Host "Done. Open Cindrix > Intel > Drug labels and press 'Get the labels ready'."

# Downloads the street map for a Cindrix drive: a US extract of the Protomaps OpenStreetMap
# build, written to <Target>\maps\us\street\*.pmtiles.
#
# Usage:
#   powershell -File scripts\get-maps.ps1 -Target D:\Cindrix
#   powershell -File scripts\get-maps.ps1 -Target D:\Cindrix -Regions conus,alaska,hawaii -MaxZoom 14
#   powershell -File scripts\get-maps.ps1 -Target D:\Cindrix -Source https://example.com/20260101.pmtiles
#
# Needs internet and a lot of time: the program reads only the map pieces inside the area you
# ask for, not the whole planet file (about 120 GB), but a whole-US extract is still many
# gigabytes. Each extra zoom level roughly doubles the size, so -MaxZoom 14 or 13 makes a
# much smaller map. Nothing here has been run by a person yet; the build address pattern
# below is from memory and may have changed.
#
# License: OpenStreetMap data, ODbL. "© OpenStreetMap contributors" must stay visible on the map.
param(
  [Parameter(Mandatory=$true)][string]$Target,
  # A Protomaps daily build (.pmtiles). If left empty, recent dates on build.protomaps.com are tried.
  # The list of builds is at https://maps.protomaps.com/builds
  [string]$Source = '',
  [ValidateSet('conus','alaska','hawaii')][string[]]$Regions = @('conus'),
  [ValidateRange(0,15)][int]$MaxZoom = 15,
  [switch]$Force
)
$ErrorActionPreference = 'Stop'

# min lon, min lat, max lon, max lat
$boxes = @{
  conus  = '-125.0,24.3,-66.8,49.6'
  alaska = '-179.5,51.0,-129.0,71.6'
  hawaii = '-160.7,18.8,-154.7,22.3'
}

$street = Join-Path $Target 'maps\us\street'
New-Item -ItemType Directory -Force -Path $street | Out-Null

# 1. The pmtiles tool (one program, no installer).
$pm = (Get-Command pmtiles -ErrorAction SilentlyContinue).Source
if (-not $pm) {
  $tools = Join-Path $Target 'engines\pmtiles'
  $pm = Join-Path $tools 'pmtiles.exe'
  if (-not (Test-Path $pm)) {
    Write-Host 'Fetching the pmtiles tool from github.com/protomaps/go-pmtiles ...'
    $rel = Invoke-RestMethod 'https://api.github.com/repos/protomaps/go-pmtiles/releases/latest' -Headers @{ 'User-Agent' = 'cindrix-ops' }
    $asset = $rel.assets | Where-Object { $_.name -match 'Windows' -and $_.name -match '(x86_64|amd64)' -and $_.name -match '\.zip$' } | Select-Object -First 1
    if (-not $asset) { throw 'Could not find a Windows build of pmtiles on its releases page. Download it by hand and put pmtiles.exe on your PATH.' }
    New-Item -ItemType Directory -Force -Path $tools | Out-Null
    $zip = Join-Path $tools $asset.name
    Invoke-WebRequest -UseBasicParsing $asset.browser_download_url -OutFile $zip
    Expand-Archive -Path $zip -DestinationPath $tools -Force
    Remove-Item $zip
    if (-not (Test-Path $pm)) { throw "pmtiles.exe was not in the download (looked in $tools)." }
  }
}
Write-Host "Using $pm"

# 2. Which build to read from.
if (-not $Source) {
  foreach ($back in 1..10) {
    $day = (Get-Date).AddDays(-$back).ToString('yyyyMMdd')
    $url = "https://build.protomaps.com/$day.pmtiles"
    try {
      $r = Invoke-WebRequest -UseBasicParsing -Method Head $url
      if ($r.StatusCode -eq 200) { $Source = $url; break }
    } catch { }
  }
  if (-not $Source) { throw 'Could not find a recent build. Open https://maps.protomaps.com/builds, copy the address of a build, and pass it as -Source.' }
}
Write-Host "Reading from $Source"

# 3. One extract per region.
foreach ($region in $Regions) {
  $out = Join-Path $street "$region.pmtiles"
  if ((Test-Path $out) -and -not $Force) { Write-Host "Already have $region.pmtiles (use -Force to redo it)"; continue }
  $part = "$out.part"
  if (Test-Path $part) { Remove-Item $part }
  Write-Host "Extracting $region (box $($boxes[$region]), zoom 0 to $MaxZoom). This can take a long time ..."
  & $pm extract $Source $part "--bbox=$($boxes[$region])" "--maxzoom=$MaxZoom"
  if ($LASTEXITCODE -ne 0) { throw "pmtiles extract failed for $region (exit $LASTEXITCODE)." }
  # A real PMTiles v3 file starts with the letters PMTiles and the number 3.
  $head = New-Object byte[] 8
  $fs = [System.IO.File]::OpenRead($part)
  try { [void]$fs.Read($head, 0, 8) } finally { $fs.Dispose() }
  if ([System.Text.Encoding]::ASCII.GetString($head, 0, 7) -ne 'PMTiles' -or $head[7] -ne 3) { throw "$region.pmtiles is not a PMTiles v3 file; it was not kept." }
  Move-Item -Force $part $out
  $gb = [math]::Round((Get-Item $out).Length / 1GB, 2)
  Write-Host "  $region.pmtiles: $gb GB"
}

# 4. Record where it came from (every source is listed, doc 3).
$note = @(
  'Street map: OpenStreetMap data via the Protomaps basemap build.',
  "Source:   $Source",
  "Regions:  $($Regions -join ', ')  (max zoom $MaxZoom)",
  "Fetched:  $((Get-Date).ToString('yyyy-MM-dd'))",
  'License:  Open Database License (ODbL). Show: (c) OpenStreetMap contributors.'
)
Set-Content -Path (Join-Path $street 'SOURCE.txt') -Value $note -Encoding UTF8

Write-Host 'Done. Check the drive with: cd core; cargo run --release --example check_drive -- <drive folder>'

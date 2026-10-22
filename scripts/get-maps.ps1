# Downloads the street map for a Cindrix drive: a US extract of the Protomaps OpenStreetMap
# build, written to <Target>\maps\us\street\*.pmtiles.
#
# Usage:
#   powershell -File scripts\get-maps.ps1 -Target D:\Cindrix
#   powershell -File scripts\get-maps.ps1 -Target D:\Cindrix -Regions conus,alaska,hawaii -MaxZoom 14
#   powershell -File scripts\get-maps.ps1 -Target D:\Cindrix -Source https://example.com/20260101.pmtiles
#
# A SMALL map of just your own area (a few MB to a few hundred MB, fine for a first test):
#   powershell -File scripts\get-maps.ps1 -Target D:\Cindrix -Bbox:"-105.10,39.62,-104.80,39.82" -Name my-area -MaxZoom 14
# -Bbox is "west,south,east,north" in decimal degrees. In the US the two longitudes (west and
# east) are negative numbers. Write it with the colon as shown, so the leading minus sign is
# not mistaken for another option. To find the numbers, open https://www.openstreetmap.org,
# zoom to your area, press Export, and read the four values it shows (left, bottom, right, top).
#
# Needs internet and some time: the program reads only the map pieces inside the area you
# ask for, not the whole planet file (about 120 GB), but a whole-US extract is still many
# gigabytes. Each extra zoom level roughly doubles the size, so -MaxZoom 14 or 13 makes a
# much smaller map.
#
# FINDING PLACES: the Maps screen can look up a town, ZIP code, county, park, lake, mountain, airport or
# hospital by name with no internet. That needs a places list, made from the free GeoNames data:
#   powershell -File scripts\get-maps.ps1 -Target D:\Cindrix -PlacesOnly     (just the list, about 100 MB download)
#   powershell -File scripts\get-maps.ps1 -Target D:\Cindrix -Places         (the map and the list)
# It is written to <Target>\maps\places\places.tsv. Street addresses are not in it.
#
# License: OpenStreetMap data, ODbL. "© OpenStreetMap contributors" must stay visible on the map.
param(
  [Parameter(Mandatory=$true)][string]$Target,
  # A Protomaps daily build (.pmtiles). If left empty, the newest build is looked up.
  # The list of builds is at https://maps.protomaps.com/builds
  [string]$Source = '',
  [ValidateSet('conus','alaska','hawaii')][string[]]$Regions = @('conus'),
  # Your own area instead of the regions above: "west,south,east,north" in decimal degrees.
  [string]$Bbox = '',
  # The file name (without .pmtiles) used for -Bbox.
  [ValidatePattern('^[A-Za-z0-9_-]{1,40}$')][string]$Name = 'my-area',
  [ValidateRange(0,15)][int]$MaxZoom = 15,
  [switch]$Force,
  # Also make the list of places the Maps screen searches (needs internet once).
  [switch]$Places,
  # Only the list of places, not the map.
  [switch]$PlacesOnly
)
$ErrorActionPreference = 'Stop'

function Build-Places([string]$root) {
  $dir = Join-Path $root 'maps\places'
  New-Item -ItemType Directory -Force -Path $dir | Out-Null
  $out = Join-Path $dir 'places.tsv'
  $tmp = Join-Path $env:TEMP ('cindrix-places-' + [Guid]::NewGuid().ToString('N'))
  New-Item -ItemType Directory -Force -Path $tmp | Out-Null
  try {
    $rows = New-Object System.Collections.Generic.List[string]
    $inv = [Globalization.CultureInfo]::InvariantCulture
    # Towns, counties, parks, lakes, peaks, airports and hospitals (GeoNames, United States).
    Write-Host 'Fetching the US places from download.geonames.org (about 100 MB) ...'
    $zip = Join-Path $tmp 'dump.zip'
    Invoke-WebRequest -UseBasicParsing 'https://download.geonames.org/export/dump/US.zip' -OutFile $zip
    Expand-Archive -Path $zip -DestinationPath $tmp -Force
    $reader = New-Object System.IO.StreamReader((Join-Path $tmp 'US.txt'), [System.Text.Encoding]::UTF8)
    try {
      while ($null -ne ($line = $reader.ReadLine())) {
        $f = $line.Split("`t")
        if ($f.Count -lt 15) { continue }
        $class = $f[6]; $code = $f[7]; $kind = $null
        if ($class -eq 'P') { $kind = 'city' }
        elseif ($code -eq 'ADM2') { $kind = 'county' }
        elseif ($code -eq 'PRK' -or $code -eq 'PRKN') { $kind = 'park' }
        elseif ($code -eq 'LK' -or $code -eq 'RSV') { $kind = 'lake' }
        elseif ($code -eq 'MT' -or $code -eq 'PK') { $kind = 'peak' }
        elseif ($code -eq 'AIRP') { $kind = 'airport' }
        elseif ($code -eq 'HSP') { $kind = 'hospital' }
        if (-not $kind) { continue }
        $rows.Add(($f[1], $f[10], $kind, $f[4], $f[5], $f[14] -join "`t"))
      }
    } finally { $reader.Dispose() }
    # ZIP codes.
    Write-Host 'Fetching the ZIP codes ...'
    $zz = Join-Path $tmp 'zip.zip'
    $zd = Join-Path $tmp 'zipdir'
    Invoke-WebRequest -UseBasicParsing 'https://download.geonames.org/export/zip/US.zip' -OutFile $zz
    Expand-Archive -Path $zz -DestinationPath $zd -Force
    foreach ($line in [System.IO.File]::ReadLines((Join-Path $zd 'US.txt'))) {
      $f = $line.Split("`t")
      if ($f.Count -lt 11) { continue }
      $rows.Add(($f[1], $f[4], 'zip', $f[9], $f[10], '0' -join "`t"))
    }
    $part = "$out.part"
    [System.IO.File]::WriteAllLines($part, $rows, (New-Object System.Text.UTF8Encoding($false)))
    Move-Item -Force $part $out
    Write-Host ("Wrote {0:N0} places to {1}" -f $rows.Count, $out)
    $note = @('', 'Places: GeoNames (geonames.org), license Creative Commons Attribution 4.0.', "Fetched: $((Get-Date).ToString('yyyy-MM-dd'))")
    Add-Content -Path (Join-Path $dir 'SOURCE.txt') -Value $note -Encoding UTF8
  } finally { Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue }
}
if ($PlacesOnly) { Build-Places $Target; return }

# min lon, min lat, max lon, max lat
$boxes = @{
  conus  = '-125.0,24.3,-66.8,49.6'
  alaska = '-179.5,51.0,-129.0,71.6'
  hawaii = '-160.7,18.8,-154.7,22.3'
}

# What to cut out: your own box if given, else the named regions.
$jobs = @()
if ($Bbox) {
  $p = $Bbox -split ','
  $n = @()
  foreach ($x in $p) { $v = 0.0; if ([double]::TryParse($x.Trim(), [Globalization.NumberStyles]::Float, [Globalization.CultureInfo]::InvariantCulture, [ref]$v)) { $n += $v } }
  if ($p.Count -ne 4 -or $n.Count -ne 4) { throw "-Bbox must be four numbers: west,south,east,north (for example -105.10,39.62,-104.80,39.82). Got: $Bbox" }
  if ($n[0] -lt -180 -or $n[2] -gt 180 -or $n[1] -lt -90 -or $n[3] -gt 90 -or $n[0] -ge $n[2] -or $n[1] -ge $n[3]) { throw "-Bbox is out of range. West must be less than east and south less than north (west/east -180 to 180, south/north -90 to 90). Got: $Bbox" }
  $jobs += [pscustomobject]@{ Name = $Name; Box = (($n | ForEach-Object { $_.ToString([Globalization.CultureInfo]::InvariantCulture) }) -join ',') }
} else {
  foreach ($r in $Regions) { $jobs += [pscustomobject]@{ Name = $r; Box = $boxes[$r] } }
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
function Test-Build([string]$url) {
  try { return ((Invoke-WebRequest -UseBasicParsing -Method Head $url -TimeoutSec 30).StatusCode -eq 200) } catch { return $false }
}
if (-not $Source) {
  # First choice: the list the builds page (maps.protomaps.com/builds) itself reads, newest first.
  try {
    $list = Invoke-RestMethod 'https://build-metadata.protomaps.dev/builds.json' -TimeoutSec 30
    $cands = @($list | Where-Object { $_.key -match '^\d{8}\.pmtiles$' } | Sort-Object key -Descending | Select-Object -First 3)
    foreach ($c in $cands) {
      $url = "https://build.protomaps.com/$($c.key)"
      if (Test-Build $url) { $Source = $url; break }
    }
  } catch { Write-Host "(The list of builds was not available: $($_.Exception.Message). Trying recent dates instead.)" }
  # Otherwise guess: builds are named by date, and the last week or so is kept.
  if (-not $Source) {
    foreach ($back in 1..10) {
      $url = "https://build.protomaps.com/$((Get-Date).AddDays(-$back).ToString('yyyyMMdd')).pmtiles"
      if (Test-Build $url) { $Source = $url; break }
    }
  }
  if (-not $Source) { throw 'Could not find a recent build. Open https://maps.protomaps.com/builds, copy the address of a build, and pass it as -Source.' }
}
Write-Host "Reading from $Source"

# 3. One extract per job.
foreach ($job in $jobs) {
  $out = Join-Path $street "$($job.Name).pmtiles"
  if ((Test-Path $out) -and -not $Force) { Write-Host "Already have $($job.Name).pmtiles (use -Force to redo it)"; continue }
  $part = "$out.part"
  if (Test-Path $part) { Remove-Item $part }
  Write-Host "Extracting $($job.Name) (box $($job.Box), zoom 0 to $MaxZoom). A big area can take a long time ..."
  & $pm extract $Source $part "--bbox=$($job.Box)" "--maxzoom=$MaxZoom"
  if ($LASTEXITCODE -ne 0) { throw "pmtiles extract failed for $($job.Name) (exit $LASTEXITCODE)." }
  # A real PMTiles v3 file starts with the letters PMTiles and the number 3.
  $head = New-Object byte[] 8
  $fs = [System.IO.File]::OpenRead($part)
  try { [void]$fs.Read($head, 0, 8) } finally { $fs.Dispose() }
  if ([System.Text.Encoding]::ASCII.GetString($head, 0, 7) -ne 'PMTiles' -or $head[7] -ne 3) { throw "$($job.Name).pmtiles is not a PMTiles v3 file; it was not kept." }
  Move-Item -Force $part $out
  $bytes = (Get-Item $out).Length
  if ($bytes -ge 1GB) { Write-Host ("  {0}.pmtiles: {1:N2} GB" -f $job.Name, ($bytes / 1GB)) } else { Write-Host ("  {0}.pmtiles: {1:N1} MB" -f $job.Name, ($bytes / 1MB)) }
}

# 4. Record where it came from (every source is listed, doc 3). Each run adds a section.
$areas = @()
foreach ($job in $jobs) { $areas += ('{0} [{1}]' -f $job.Name, $job.Box) }
$note = @(
  '',
  'Street map: OpenStreetMap data via the Protomaps basemap build.',
  "Source:   $Source",
  ('Areas:    {0}  (max zoom {1})' -f ($areas -join '; '), $MaxZoom),
  "Fetched:  $((Get-Date).ToString('yyyy-MM-dd'))",
  'License:  Open Database License (ODbL). Show: (c) OpenStreetMap contributors.'
)
Add-Content -Path (Join-Path $street 'SOURCE.txt') -Value $note -Encoding UTF8

if ($Places) { Build-Places $Target }

Write-Host 'Done. Check the drive with: cd core; cargo run --release --example check_drive -- <drive folder>'

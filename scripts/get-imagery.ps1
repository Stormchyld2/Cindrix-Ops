# Puts USGS aerial photos on a Cindrix drive, for the Maps screen's Satellite button (no internet needed afterwards).
# They are the free, public-domain USGS "The National Map" picture tiles (aerial photos mostly from the NAIP program).
#
# Usage:
#   powershell -File scripts\get-imagery.ps1 -Target D:\ -Layer aerial               (the whole US at a wide view, zoom 0 to 13)
#   powershell -File scripts\get-imagery.ps1 -Target D:\ -Layer aerial -Bbox:"-80.30,39.40,-80.00,39.60" -Name fairmont -MaxZoom 16
# A SMALL area in full sharpness is how you get house-level detail: -Bbox is "west,south,east,north" like get-maps.ps1.
# Every file is used together in Maps: the wide one underneath, the sharp one over it.
#
# HOW BIG: each zoom level is four times the tiles of the one before. Rough size for the lower 48:
#   zoom 13 about 15 GB (towns, fields, roads)   zoom 14 about 55 GB   zoom 15 about 220 GB   zoom 16 about 1,000 GB.
# So the whole country goes to a wide view and your own areas go sharp. A county at zoom 16 is a few GB.
# The tiles come from USGS one at a time over the internet (a few thousand a minute), so a big area takes hours or days.
# If it stops, run it again: tiles already fetched are kept.
#
# Output: <Target>\maps\satellite\us\<Name>.pmtiles
# Needs internet. Uses DuckDB and pmtiles (free; fetched once to <Target>\engines).
param(
  [Parameter(Mandatory=$true)][string]$Target,
  [Parameter(Mandatory=$true)][ValidateSet('aerial')][string]$Layer,
  [string]$Bbox = '',
  [ValidatePattern('^[A-Za-z0-9_-]{1,40}$')][string]$Name = '',
  [ValidateRange(0,17)][int]$MinZoom = 0,
  [ValidateRange(0,17)][int]$MaxZoom = 13,
  [ValidateRange(1,16)][int]$Parallel = 6,
  # Allow more than 1,500,000 tiles in one run.
  [switch]$Yes,
  [switch]$Force,
  # Use the packed tile file from an earlier run instead of packing the pictures again (the tiles must be fully fetched).
  [switch]$UseExistingPack,
  [string]$Duckdb = '',
  [string]$DuckdbVersion = '1.5.6',
  # Test only: use this pmtiles program, and this address instead of the USGS one.
  [string]$Pmtiles = '',
  [string]$TileBase = ''
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
Add-Type -AssemblyName System.Net.Http
# Windows PowerShell allows only 2 connections at a time unless told otherwise, and needs TLS 1.2 asked for.
[System.Net.ServicePointManager]::DefaultConnectionLimit = 64
try { [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.SecurityProtocolType]::Tls12 } catch { }

try {
  $logDir = Join-Path $Target 'maps'
  New-Item -ItemType Directory -Force -Path $logDir | Out-Null
  Start-Transcript -Path (Join-Path $logDir 'get-imagery-log.txt') -Append | Out-Null
} catch { }

if ($MaxZoom -lt $MinZoom) { throw '-MaxZoom must not be below -MinZoom.' }
if (-not $Name) { $Name = if ($Bbox) { 'my-area' } else { 'us-wide' } }
$service = 'USGSImageryOnly'  # the topo map (USGSTopo) was tried and taken out: see docs/29
if (-not $TileBase) { $TileBase = "https://basemap.nationalmap.gov/arcgis/rest/services/$service/MapServer/tile" }
$onWindows = ($PSVersionTable.PSVersion.Major -lt 6) -or $IsWindows

# ---- areas ----
if ($Bbox) {
  $p = $Bbox.Split(',') | ForEach-Object { [double]::Parse($_.Trim(), [System.Globalization.CultureInfo]::InvariantCulture) }
  if ($p.Count -ne 4 -or $p[0] -ge $p[2] -or $p[1] -ge $p[3]) { throw 'Bbox must be "west,south,east,north", for example -80.30,39.40,-80.00,39.60' }
  $boxes = @(,$p)
} else {
  $boxes = @(@(-125.0,24.3,-66.8,49.5), @(-161.0,18.8,-154.7,22.4), @(-170.0,51.0,-129.0,71.6))
}
function Tile-X([double]$lon, [int]$z) { [int][math]::Floor(($lon + 180.0) / 360.0 * [math]::Pow(2, $z)) }
function Tile-Y([double]$lat, [int]$z) {
  $r = $lat * [math]::PI / 180.0
  [int][math]::Floor((1.0 - [math]::Log([math]::Tan($r) + 1.0 / [math]::Cos($r)) / [math]::PI) / 2.0 * [math]::Pow(2, $z))
}
# the tiles of one zoom: a list of x/y ranges, one per area (areas that overlap are not counted twice)
function Tile-Ranges([int]$z) {
  $max = [int]([math]::Pow(2, $z) - 1)
  foreach ($b in $boxes) {
    $x0 = [math]::Max(0, (Tile-X $b[0] $z)); $x1 = [math]::Min($max, (Tile-X $b[2] $z))
    $y0 = [math]::Max(0, (Tile-Y $b[3] $z)); $y1 = [math]::Min($max, (Tile-Y $b[1] $z))
    [pscustomobject]@{ x0 = $x0; x1 = $x1; y0 = $y0; y1 = $y1 }
  }
}
$total = 0
foreach ($z in $MinZoom..$MaxZoom) { foreach ($r in (Tile-Ranges $z)) { $total += ($r.x1 - $r.x0 + 1) * ($r.y1 - $r.y0 + 1) } }
$gb = [math]::Round($total * 25KB / 1GB, 1)
Write-Host ("{0:N0} tiles to fetch, roughly {1} GB. At about 30 tiles a second that is about {2:N1} hours." -f $total, $gb, ($total / 30 / 3600))
if ($total -gt 1500000 -and -not $Yes) {
  throw "That is more than 1,500,000 tiles. For the whole country use -MaxZoom 13; for sharper detail use -Bbox for just your area. (Add -Yes to do it anyway.)"
}

$outDir = Join-Path $Target 'maps/satellite/us'
$final = Join-Path $outDir "$Name.pmtiles"
if ((Test-Path $final) -and -not $Force) { Write-Host "Already have $Name.pmtiles (use -Force to make it again)."; return }
$work = Join-Path $Target "maps/_imagery-work/$Layer-$Name"
New-Item -ItemType Directory -Force -Path $outDir, $work | Out-Null

# ---- tools ----
if (-not $Duckdb) {
  $dir = Join-Path $Target 'engines/duckdb'
  $exe = Join-Path $dir ($(if ($onWindows) { 'duckdb.exe' } else { 'duckdb' }))
  if (-not (Test-Path $exe)) {
    Write-Host 'Fetching DuckDB...'
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    $os = if ($onWindows) { 'windows' } else { 'linux' }
    $url = "https://install.duckdb.org/v$DuckdbVersion/duckdb_cli-$os-amd64.zip"
    $zip = Join-Path $dir 'duckdb.zip'
    try { Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $zip } catch { throw "Could not fetch DuckDB from $url : $_" }
    Expand-Archive -Force -Path $zip -DestinationPath $dir
    Remove-Item $zip -Force
    if (-not $onWindows) { & chmod +x $exe }
  }
  $Duckdb = $exe
}
$pm = $Pmtiles
if (-not $pm) { $pm = (Get-Command pmtiles -ErrorAction SilentlyContinue).Source }
if (-not $pm) {
  $tools = Join-Path $Target 'engines/pmtiles'
  $pm = Join-Path $tools 'pmtiles.exe'
  if (-not (Test-Path $pm)) {
    Write-Host 'Fetching the pmtiles tool...'
    $rel = Invoke-RestMethod 'https://api.github.com/repos/protomaps/go-pmtiles/releases/latest' -Headers @{ 'User-Agent' = 'cindrix-ops' }
    $asset = $rel.assets | Where-Object { $_.name -match 'Windows' -and $_.name -match '(x86_64|amd64)' -and $_.name -match '\.zip$' } | Select-Object -First 1
    if (-not $asset) { throw 'Could not find a Windows build of pmtiles. Download it by hand and put pmtiles.exe on your PATH.' }
    New-Item -ItemType Directory -Force -Path $tools | Out-Null
    $zip = Join-Path $tools $asset.name
    Invoke-WebRequest -UseBasicParsing $asset.browser_download_url -OutFile $zip
    Expand-Archive -Path $zip -DestinationPath $tools -Force
    Remove-Item $zip
  }
}
function Run-Sql([string]$sql) {
  $f = [System.IO.Path]::GetTempFileName()
  try {
    Set-Content -LiteralPath $f -Value $sql -Encoding UTF8
    Get-Content -Raw -LiteralPath $f | & $Duckdb -bail
    if ($LASTEXITCODE -ne 0) { throw "DuckDB stopped with an error (code $LASTEXITCODE)." }
  } finally { Remove-Item $f -Force -ErrorAction SilentlyContinue }
}

# ---- step 1: fetch the tiles, a few at a time, politely ----
$client = New-Object System.Net.Http.HttpClient
$client.Timeout = [TimeSpan]::FromSeconds(60)
$client.DefaultRequestHeaders.UserAgent.ParseAdd('cindrix-ops-offline-library/1.0 (personal offline copy)')
$format = $null
$lost = 0
$script:failed = 0
foreach ($z in $MinZoom..$MaxZoom) {
  if (Test-Path (Join-Path $work "z$z.fetched")) { Write-Host "Zoom $z already done."; continue }
  $todo = New-Object System.Collections.Generic.List[object]
  $seen = New-Object 'System.Collections.Generic.HashSet[string]'
  foreach ($r in (Tile-Ranges $z)) {
    for ($x = $r.x0; $x -le $r.x1; $x++) {
      for ($y = $r.y0; $y -le $r.y1; $y++) {
        if (-not $seen.Add("$x/$y")) { continue }
        $base = Join-Path (Join-Path (Join-Path $work "$z") "$x") "$y"
        if ((Test-Path "$base.jpg") -or (Test-Path "$base.png") -or (Test-Path "$base.none")) { continue }
        $todo.Add(@($x, $y, $base))
      }
    }
  }
  $failedAtStart = $script:failed
  Write-Host ("Zoom {0}: {1:N0} tiles to fetch..." -f $z, $todo.Count)
  $running = New-Object System.Collections.Generic.List[object]
  $done = 0
  $drain = {
    param($wait)
    while ($running.Count -ge $wait -and $running.Count -gt 0) {
      $idx = [System.Threading.Tasks.Task]::WaitAny(@($running | ForEach-Object { $_.task }))
      $item = $running[$idx]; $running.RemoveAt($idx)
      $bytes = $null
      try {
        $resp = $item.task.Result
        if ($resp.IsSuccessStatusCode) { $bytes = $resp.Content.ReadAsByteArrayAsync().Result }
        elseif ([int]$resp.StatusCode -ne 404) { $item.fail = $true }
        $resp.Dispose()
      } catch { $item.fail = $true }
      if ($item.fail -and $item.tries -lt 4) {
        $item.tries++
        Start-Sleep -Milliseconds (1500 * $item.tries)
        $item.fail = $false
        $item.task = $client.GetAsync($item.url)
        $running.Add($item)
        continue
      }
      if ($item.fail) {
        # leave this one for the next run; the rest carries on (a stray tile should not stop hours of work)
        $script:failed++
        if ($script:failed -gt 200) { throw "USGS stopped answering (200 tiles missed). Run this again when the internet is steady; finished tiles are kept." }
        continue
      }
      [void][System.IO.Directory]::CreateDirectory((Split-Path $item.base))
      if ($bytes -and $bytes.Length -gt 100) {
        $ext = $null
        if ($bytes[0] -eq 0xFF -and $bytes[1] -eq 0xD8) { $ext = 'jpg' } elseif ($bytes[0] -eq 0x89 -and $bytes[1] -eq 0x50) { $ext = 'png' }
        if ($ext -and -not $script:format) { $script:format = $ext }
        if ($ext -and $ext -eq $script:format) { [System.IO.File]::WriteAllBytes("$($item.base).$ext", $bytes) } else { $script:lost++; [System.IO.File]::WriteAllBytes("$($item.base).none", [byte[]]@()) }
      } else {
        [System.IO.File]::WriteAllBytes("$($item.base).none", [byte[]]@())
      }
      $script:done++
      if ($script:done % 2000 -eq 0) { Write-Host ("  zoom {0}: {1:N0} of {2:N0}" -f $z, $script:done, $todo.Count) }
    }
  }
  foreach ($t in $todo) {
    $url = "$TileBase/$z/$($t[1])/$($t[0])"
    $running.Add([pscustomobject]@{ task = $client.GetAsync($url); url = $url; base = $t[2]; tries = 0; fail = $false })
    if ($running.Count -ge $Parallel) { & $drain $Parallel }
  }
  & $drain 1
  if ($failedAtStart -eq $script:failed) { Set-Content -LiteralPath (Join-Path $work "z$z.fetched") -Value (Get-Date -Format s) }
}
if ($script:failed) { Write-Host "$($script:failed) tiles did not answer and were left out. Run the same command again later to fill them in." }
if ($lost) { Write-Host "$lost tiles were in a different picture format and were left out." }

# ---- step 2: pack the files into one MBTiles file, then one PMTiles file ----
$mb = Join-Path $work "$Name.mbtiles"
$packedMark = Join-Path $work 'packed.ok'

# The pmtiles tool looks every tile up by its zoom, column and row. Without an index on those three, each lookup reads the
# whole file, and a country-sized map takes days instead of minutes. (The MBTiles format asks for this index.)
function Add-TileIndex([string]$file) {
  $sql = 'CREATE INDEX IF NOT EXISTS tile_index ON tiles (zoom_level, tile_column, tile_row);'
  Write-Host 'Making the tile index...'
  if ($onWindows) {
    # Windows has SQLite built in (winsqlite3.dll); it is called directly so nothing else has to be installed.
    if (-not ([System.Management.Automation.PSTypeName]'CxSqlite').Type) {
      Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class CxSqlite {
  const string Dll = "winsqlite3.dll";
  [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)] static extern int sqlite3_open(byte[] file, out IntPtr db);
  [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)] static extern int sqlite3_exec(IntPtr db, byte[] sql, IntPtr cb, IntPtr arg, out IntPtr err);
  [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)] static extern IntPtr sqlite3_errmsg(IntPtr db);
  [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)] static extern int sqlite3_close(IntPtr db);
  static byte[] Z(string s) { byte[] b = Encoding.UTF8.GetBytes(s); byte[] r = new byte[b.Length + 1]; Array.Copy(b, r, b.Length); return r; }
  public static string Exec(string file, string sql) {
    IntPtr db;
    if (sqlite3_open(Z(file), out db) != 0) { return "the file could not be opened"; }
    IntPtr err;
    int rc = sqlite3_exec(db, Z(sql), IntPtr.Zero, IntPtr.Zero, out err);
    string msg = rc == 0 ? "" : Marshal.PtrToStringAnsi(sqlite3_errmsg(db));
    sqlite3_close(db);
    return rc == 0 ? "" : (msg == "" ? "error " + rc : msg);
  }
}
"@
    }
    $e = [CxSqlite]::Exec($file, $sql)
    if ($e) { throw "The tile index could not be made ($e)." }
  } else {
    $sq = Get-Command sqlite3 -ErrorAction SilentlyContinue
    if (-not $sq) { throw 'The sqlite3 program is needed to make the tile index. Install it (sudo apt install sqlite3) and run this again.' }
    & $sq.Source $file $sql
    if ($LASTEXITCODE -ne 0) { throw "The tile index could not be made (sqlite3 code $LASTEXITCODE)." }
  }
}

$fmt = $format
if (-not $fmt) {
  $any = Get-ChildItem -Recurse -File -Path $work -Include *.jpg,*.png | Select-Object -First 1
  if ($any) { $fmt = $any.Extension.TrimStart('.') }
}
if (-not $fmt) { throw 'No tiles came back for this area (USGS may not have pictures there). Nothing was made.' }
$reuse = (Test-Path $mb) -and ($UseExistingPack -or (Test-Path $packedMark))
if ($reuse) { Write-Host 'Using the tile file that is already packed.' } else {
Write-Host 'Packing the tiles into one file...'
if (Test-Path $mb) { Remove-Item $mb -Force }
$mbp = $mb.Replace('\','/')
$bounds = ($boxes | ForEach-Object { $_ })
$west = ($boxes | ForEach-Object { $_[0] } | Measure-Object -Minimum).Minimum
$south = ($boxes | ForEach-Object { $_[1] } | Measure-Object -Minimum).Minimum
$east = ($boxes | ForEach-Object { $_[2] } | Measure-Object -Maximum).Maximum
$north = ($boxes | ForEach-Object { $_[3] } | Measure-Object -Maximum).Maximum
$inv = [System.Globalization.CultureInfo]::InvariantCulture
$boundsText = ($west.ToString($inv)) + ',' + ($south.ToString($inv)) + ',' + ($east.ToString($inv)) + ',' + ($north.ToString($inv))
Run-Sql @"
INSTALL sqlite; LOAD sqlite;
ATTACH '$mbp' AS mb (TYPE sqlite);
CREATE TABLE mb.metadata (name TEXT, value TEXT);
CREATE TABLE mb.tiles (zoom_level INTEGER, tile_column INTEGER, tile_row INTEGER, tile_data BLOB);
INSERT INTO mb.metadata VALUES ('name','$Name'),('format','$fmt'),('type','baselayer'),('minzoom','$MinZoom'),('maxzoom','$MaxZoom'),('bounds','$boundsText'),('version','1.0'),('description','USGS The National Map ($service), made by scripts/get-imagery.ps1');
"@
foreach ($z in $MinZoom..$MaxZoom) {
  $zdir = Join-Path $work "$z"
  if (-not (Test-Path $zdir)) { continue }
  $cols = @(Get-ChildItem -Directory -Path $zdir | Sort-Object { [int]$_.Name })
  for ($i = 0; $i -lt $cols.Count; $i += 150) {
    $take = $cols[$i..([math]::Min($i + 149, $cols.Count - 1))]
    $globs = ($take | ForEach-Object { "'" + (Join-Path $_.FullName "*.$fmt").Replace('\','/') + "'" }) -join ','
    Run-Sql @"
INSTALL sqlite; LOAD sqlite;
ATTACH '$mbp' AS mb (TYPE sqlite);
INSERT INTO mb.tiles
SELECT $z AS z,
       CAST(regexp_extract(replace(filename,'\','/'), '/(\d+)/(\d+)\.$fmt', 1) AS INTEGER) AS x,
       ((1 << $z) - 1) - CAST(regexp_extract(replace(filename,'\','/'), '/(\d+)/(\d+)\.$fmt', 2) AS INTEGER) AS y,
       content
FROM read_blob([$globs]);
"@
  }
  Write-Host "  zoom $z packed"
}
Set-Content -LiteralPath $packedMark -Value (Get-Date -Format s)
}
Add-TileIndex $mb
Write-Host 'Making the PMTiles file...'
$tmp = $final + '.part'
if (Test-Path $tmp) { Remove-Item $tmp -Force }
& $pm convert $mb $tmp
if ($LASTEXITCODE -ne 0) { throw "pmtiles convert failed (exit $LASTEXITCODE)." }
# only the first 8 bytes are read: a country-sized file is far too big to load whole (Windows PowerShell stops at 2 GB)
$fs = [System.IO.File]::OpenRead($tmp)
try { $head = New-Object byte[] 8; [void]$fs.Read($head, 0, 8) } finally { $fs.Dispose() }
if ([System.Text.Encoding]::ASCII.GetString($head, 0, 7) -ne 'PMTiles' -or $head[7] -ne 3) { throw "$Name.pmtiles is not a PMTiles v3 file; it was not kept." }
Move-Item -Force -LiteralPath $tmp -Destination $final
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
$sz = (Get-Item $final).Length
Set-Content -LiteralPath (Join-Path $outDir "$Name.SOURCE.txt") -Value @"
$Layer pictures from USGS The National Map ($service), https://www.usgs.gov/programs/national-geospatial-program/national-map
Public domain (US government work). Made by scripts/get-imagery.ps1 on $(Get-Date -Format yyyy-MM-dd). Zoom $MinZoom to $MaxZoom.
Aerial photos come mostly from the NAIP program (about 1 meter); Alaska has coarser pictures.
"@
Write-Host ("Done: {0} ({1:N1} MB). Open Maps and press {2}." -f $final, ($sz / 1MB), 'Satellite')

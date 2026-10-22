# Puts street addresses on a Cindrix drive so the Maps screen can find "123 Main St, Denver CO" with no internet.
# They come from the Overture Maps addresses data (free; many sources, see licenses\README.md). The result is plain
# text files, <Target>\maps\addresses\<Name>\A.tsv ... Z.tsv, 0.tsv ... 9.tsv, _.tsv. See core\src\addresses.rs.
#
# Usage:
#   powershell -File scripts\get-addresses.ps1 -Target D:\
#   powershell -File scripts\get-addresses.ps1 -Target D:\ -Bbox:"-105.10,39.62,-104.80,39.82" -Name my-area
# -Bbox is "west,south,east,north" like get-maps.ps1. Without it: the lower 48, Alaska and Hawaii (Name "us").
#
# Not every house is in the data. Some places have most of their addresses, some have few or none.
# If it stops (the internet drops), run it again: finished letters are kept and the rest carries on.
#
# Needs internet. Uses DuckDB (free, MIT license), fetched once to <Target>\engines\duckdb.
param(
  [Parameter(Mandatory=$true)][string]$Target,
  [string]$Bbox = '',
  [ValidatePattern('^[A-Za-z0-9_-]{1,40}$')][string]$Name = 'us',
  # An Overture release such as 2026-10-21.0. Left empty, the newest is looked up.
  [string]$Release = '',
  # Use this duckdb program instead of fetching one.
  [string]$Duckdb = '',
  [string]$DuckdbVersion = '1.5.6',
  # Start again even if a stage or letters are already there.
  [switch]$Force,
  # Only check that the street-name rules give the answers in this file (tests\address_keys.tsv), then stop.
  [string]$SelfTest = ''
)
$ErrorActionPreference = 'Stop'

# Short forms. KEEP IN STEP WITH SHORT_WORDS in core\src\addresses.rs (the build checks both against tests\address_keys.tsv).
$short = [ordered]@{
  STREET='ST'; AVENUE='AVE'; AV='AVE'; ROAD='RD'; DRIVE='DR'; LANE='LN'; COURT='CT'; CIRCLE='CIR'; BOULEVARD='BLVD'; PLACE='PL';
  HIGHWAY='HWY'; PARKWAY='PKWY'; TRAIL='TRL'; TERRACE='TER'; SQUARE='SQ'; ALLEY='ALY'; EXPRESSWAY='EXPY'; FREEWAY='FWY'; TURNPIKE='TPKE';
  CROSSING='XING'; MOUNTAIN='MTN'; HEIGHTS='HTS'; POINT='PT'; RIDGE='RDG'; VALLEY='VLY'; CREEK='CRK'; NORTH='N'; SOUTH='S'; EAST='E'; WEST='W';
  NORTHEAST='NE'; NORTHWEST='NW'; SOUTHEAST='SE'; SOUTHWEST='SW'; SAINT='ST'; FORT='FT'; MOUNT='MT'
}
function Key-Sql([string]$col) {
  $cases = ($short.GetEnumerator() | ForEach-Object { "WHEN '$($_.Key)' THEN '$($_.Value)'" }) -join ' '
  return "array_to_string(list_transform(string_split(trim(regexp_replace(upper($col),'[^A-Z0-9]+',' ','g')),' '), w -> CASE w $cases ELSE w END),' ')"
}

try {
  $logDir = Join-Path $Target 'maps'
  New-Item -ItemType Directory -Force -Path $logDir | Out-Null
  Start-Transcript -Path (Join-Path $logDir 'get-addresses-log.txt') -Append | Out-Null
} catch { }

# ---- DuckDB ----
$onWindows = ($PSVersionTable.PSVersion.Major -lt 6) -or $IsWindows
if (-not $Duckdb) {
  $dir = Join-Path $Target 'engines\duckdb'
  $exe = Join-Path $dir ($(if ($onWindows) { 'duckdb.exe' } else { 'duckdb' }))
  if (-not (Test-Path $exe)) {
    Write-Host 'Fetching DuckDB (the free program that reads the address data)...'
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    $os = if ($onWindows) { 'windows' } else { 'linux' }
    $ProgressPreference = 'SilentlyContinue'
    # A fixed, tested version (the newest is not always the one this script was checked with).
    $url = "https://install.duckdb.org/v$DuckdbVersion/duckdb_cli-$os-amd64.zip"
    $zip = Join-Path $dir 'duckdb.zip'
    try { Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $zip } catch { throw "Could not fetch DuckDB from $url : $_" }
    Expand-Archive -Force -Path $zip -DestinationPath $dir
    Remove-Item $zip -Force
    if (-not $onWindows) { & chmod +x $exe }
  }
  $Duckdb = $exe
}

function Run-Sql([string]$sql) {
  $f = [System.IO.Path]::GetTempFileName()
  try {
    Set-Content -LiteralPath $f -Value $sql -Encoding UTF8
    Get-Content -Raw -LiteralPath $f | & $Duckdb -bail
    if ($LASTEXITCODE -ne 0) { throw "DuckDB stopped with an error (code $LASTEXITCODE)." }
  } finally { Remove-Item $f -Force -ErrorAction SilentlyContinue }
}

# ---- check of the street-name rules ----
if ($SelfTest) {
  $bad = 0; $n = 0
  foreach ($line in Get-Content -LiteralPath $SelfTest) {
    if (-not $line.Trim() -or $line.StartsWith('#')) { continue }
    $parts = $line -split "`t", 2
    $in = $parts[0].Replace("'", "''")
    $f = [System.IO.Path]::GetTempFileName()
    Set-Content -LiteralPath $f -Value ("COPY (SELECT " + (Key-Sql "'$in'") + " AS k) TO '" + ($f.Replace('\','/')) + ".out' (FORMAT CSV, HEADER false);") -Encoding UTF8
    Get-Content -Raw -LiteralPath $f | & $Duckdb -bail
    $got = (Get-Content -LiteralPath ($f + '.out') -Raw).Trim()
    Remove-Item $f, ($f + '.out') -Force
    $n++
    if ($got -ne $parts[1]) { Write-Host "WRONG: '$($parts[0])' gave '$got', wanted '$($parts[1])'"; $bad++ }
  }
  if ($bad) { throw "$bad of $n street names came out different from core\src\addresses.rs." }
  Write-Host "All $n street names agree with the program."
  return
}

# ---- what to fetch ----
if ($Bbox) {
  $p = $Bbox.Split(',') | ForEach-Object { [double]$_.Trim() }
  if ($p.Count -ne 4 -or $p[0] -ge $p[2] -or $p[1] -ge $p[3]) { throw 'Bbox must be "west,south,east,north", for example -105.10,39.62,-104.80,39.82' }
  $boxes = @(,$p)
} else {
  # lower 48, Alaska (east of the date line), Hawaii
  $boxes = @(@(-125.0,24.3,-66.8,49.5), @(-180.0,51.0,-129.0,71.6), @(-161.0,18.8,-154.7,22.4))
}
$inv = [System.Globalization.CultureInfo]::InvariantCulture
$where = '(' + (($boxes | ForEach-Object { "bbox.xmin >= $($_[0].ToString($inv)) AND bbox.xmax <= $($_[2].ToString($inv)) AND bbox.ymin >= $($_[1].ToString($inv)) AND bbox.ymax <= $($_[3].ToString($inv))" }) -join ') OR (') + ")"

if (-not $Release) {
  try {
    $cat = Invoke-RestMethod -UseBasicParsing -Uri 'https://stac.overturemaps.org/catalog.json'
    $Release = [string]$cat.latest
  } catch { }
  if (-not $Release) { $Release = '2026-10-21.0'; Write-Host "Could not look up the newest release; using $Release." }
}
Write-Host "Overture release $Release"

$out = Join-Path $Target "maps\addresses\$Name"
$work = Join-Path $out '_work'
New-Item -ItemType Directory -Force -Path $out, $work | Out-Null
$stage = (Join-Path $work 'stage.parquet').Replace('\','/')
$s3 = "s3://overturemaps-us-west-2/release/$Release/theme=addresses/type=address/*"
$key = Key-Sql 'street'

# ---- step 1: the US rows, one file on this drive ----
if ($Force -or -not (Test-Path (Join-Path $work 'stage.done'))) {
  Write-Host 'Step 1 of 2: reading the addresses from the internet. This is the long part.'
  $tries = 0
  while ($true) {
    $tries++
    try {
      Run-Sql @"
INSTALL httpfs; LOAD httpfs;
SET s3_region='us-west-2';
SET temp_directory='$($work.Replace('\','/'))';
COPY (
  SELECT $key AS k,
         trim(number) AS num,
         COALESCE(left(trim(postcode),5),'') AS zip,
         COALESCE(CASE WHEN length(trim(address_levels[1].value)) = 2 THEN upper(trim(address_levels[1].value)) END,'') AS st,
         round((bbox.ymin + bbox.ymax) / 2, 6) AS lat,
         round((bbox.xmin + bbox.xmax) / 2, 6) AS lon
  FROM read_parquet('$s3', hive_partitioning=1)
  WHERE country = 'US' AND ($where)
    AND street IS NOT NULL AND number IS NOT NULL
) TO '$stage' (FORMAT PARQUET);
"@
      break
    } catch {
      if ($tries -ge 5) { throw "Could not read the addresses after 5 tries: $_" }
      Write-Host "That did not work ($_). Trying again in 30 seconds ($tries of 5)..."
      Start-Sleep -Seconds 30
    }
  }
  Set-Content -LiteralPath (Join-Path $work 'stage.done') -Value (Get-Date -Format s)
}

# ---- step 2: one file per first letter, sorted so the program can jump straight to a street ----
Write-Host 'Step 2 of 2: sorting into files.'
# (not 'A'..'Z': Windows PowerShell cannot make a range of letters)
$letters = @([char[]](65..90) + [char[]](48..57) | ForEach-Object { [string]$_ }) + @('_')
foreach ($l in $letters) {
  $dest = Join-Path $out "$l.tsv"
  if ((Test-Path $dest) -and -not $Force) { Write-Host "  $l already done"; continue }
  $part = ($dest + '.part').Replace('\','/')
  $cond = if ($l -eq '_') { "NOT regexp_matches(substr(k,1,1), '[A-Z0-9]')" } else { "substr(k,1,1) = '$l'" }
  Run-Sql @"
SET temp_directory='$($work.Replace('\','/'))';
SET memory_limit='3GB';
COPY (
  SELECT DISTINCT k, num, zip, st, lat, lon
  FROM read_parquet('$stage')
  WHERE k <> '' AND num <> '' AND $cond
    AND NOT regexp_matches(k || num, '[\t\r\n"]')
  ORDER BY k, TRY_CAST(regexp_extract(num, '^[0-9]+', 0) AS HUGEINT) NULLS FIRST, num
) TO '$part' (FORMAT CSV, DELIMITER '\t', HEADER false);
"@
  Move-Item -Force -LiteralPath ($dest + '.part') -Destination $dest
  Write-Host "  $l done ($([math]::Round((Get-Item $dest).Length / 1MB)) MB)"
}
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
Set-Content -LiteralPath (Join-Path $out 'SOURCE.txt') -Value @"
Street addresses from Overture Maps (release $Release), https://overturemaps.org
Made by scripts/get-addresses.ps1 on $(Get-Date -Format yyyy-MM-dd).
Overture's addresses combine many open sources, each with its own license; see licenses/README.md and https://docs.overturemaps.org/attribution
Not every house is in this data.
"@
Write-Host "Done. Street addresses are in $out"

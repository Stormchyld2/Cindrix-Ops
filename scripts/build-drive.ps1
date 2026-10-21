# Fills a Cindrix drive or folder with the content for one edition, from manifest\content.json.
#
# Usage:
#   powershell -File scripts\build-drive.ps1 -Target D:\ -Edition go
#   powershell -File scripts\build-drive.ps1 -Target E:\ -Edition vault -Optional
#   powershell -File scripts\build-drive.ps1 -Target D:\ -Edition go -DryRun      (asks the servers for real sizes, downloads nothing)
#   powershell -File scripts\build-drive.ps1 -Target C:\Cindrix -Edition go -Only qwen35-2b,fema-guides,wikem    (a small first test, about 2.5 GB)
#
# What it does: makes the folders and the cindrix-drive.json marker, works out each item's real
# download address and size, refuses to start if the total would pass the edition's size limit,
# then downloads each file (it can be stopped and run again: unfinished files are continued).
# Kiwix files are checked against the .sha256 file Kiwix publishes; model files against their
# known size. Items marked script/bundled/manual in the manifest are listed but not downloaded here.
#
# Limits (doc 7, same numbers as core/src/product.rs): go 100 GB, vault 3400 GB, hub none.
# Needs internet and curl.exe (comes with Windows 10 and 11). Large downloads take hours.
param(
  [Parameter(Mandatory=$true)][string]$Target,
  [ValidateSet('go','vault','hub')][string]$Edition = 'go',
  [string]$Name = '',
  [switch]$Optional,     # also fetch items the manifest marks as not required
  [string[]]$Only = @(), # fetch just these item ids, for example: -Only qwen35-2b,fema-guides,wikem
  [switch]$DryRun,       # only work out sizes and print the plan
  [string]$Manifest = ''
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
if (-not $Manifest) { $Manifest = Join-Path $root 'manifest\content.json' }
$m = Get-Content -Raw -Path $Manifest | ConvertFrom-Json
$budgetGb = switch ($Edition) { 'go' { 100 } 'vault' { 3400 } default { $null } }

# 1. Folders and marker (skipped in a dry run).
if (-not $DryRun) {
  foreach ($d in 'library','models','engines','music','user') { New-Item -ItemType Directory -Force -Path (Join-Path $Target $d) | Out-Null }
  $music = Join-Path $Target 'music\README.txt'
  if (-not (Test-Path $music)) { Set-Content -Path $music -Value 'Copy your music here (folders like Artist\Album\01 - Song.mp3). It is never changed or counted in the size limit.' -Encoding UTF8 }
  if (-not $Name) { $Name = switch ($Edition) { 'vault' { 'Cindrix Ops Vault' } 'hub' { 'Cindrix Ops Hub' } default { 'Cindrix Ops Go' } } }
  Set-Content -Path (Join-Path $Target 'cindrix-drive.json') -Value ('{ "name": "' + $Name + '", "edition": "' + $Edition + '" }') -Encoding UTF8
}

# 2. Which items belong to this edition. Go takes tier A (Wikipedia without pictures); Vault takes tier B and V.
function Wanted($i) {
  if ($i.editions -notcontains $Edition) { return $false }
  if ($Edition -eq 'go' -and $i.tier -in 'B','V') { return $false }
  if ($Edition -eq 'vault' -and $i.tier -eq 'A') { return $false }
  if (-not $i.required -and -not $Optional -and -not ($Edition -eq 'vault' -and $i.tier -in 'B','V')) { return $false }
  return $true
}
$items = @($m.items | Where-Object { Wanted $_ })
if ($Only.Count) {
  # A short list for a first test: just the named items, whatever their tier or whether they are required.
  $ids = @($Only | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ })
  $unknown = @($ids | Where-Object { @($m.items | ForEach-Object { $_.id }) -notcontains $_ })
  if ($unknown.Count) { throw "Unknown item id(s): $($unknown -join ', '). Valid ids: $((@($m.items | ForEach-Object { $_.id })) -join ', ')" }
  $items = @($m.items | Where-Object { $ids -contains $_.id -and $_.editions -contains $Edition })
  $skipped = @($ids | Where-Object { @($items | ForEach-Object { $_.id }) -notcontains $_ })
  if ($skipped.Count) { Write-Warning "Not part of the $Edition edition, skipped: $($skipped -join ', ')" }
}

# 3. Real address and size of each downloadable item.
function Head($url) {
  $out = & curl.exe -sIL --fail --max-time 60 $url 2>$null
  if ($LASTEXITCODE -ne 0) { return $null }
  $len = ($out | Select-String -Pattern '^(?i)content-length:\s*(\d+)' | Select-Object -Last 1)
  if ($len) { return [int64]$len.Matches[0].Groups[1].Value } else { return -1 }
}
$kiwix = 'https://download.kiwix.org/zim'
$plan = @()
foreach ($i in $items) {
  $s = $i.source
  $row = [ordered]@{ id = $i.id; name = $i.name; kind = $s.kind; url = $null; file = $null; dest = $s.dest; bytes = [int64]0; note = '' }
  if ($s.kind -eq 'kiwix') {
    $listing = (& curl.exe -sL --fail --max-time 120 "$kiwix/$($s.dir)/") -join "`n"
    $names = [regex]::Matches($listing, [regex]::Escape($s.prefix) + '[0-9-]+\.zim(?![.\w])') | ForEach-Object { $_.Value } | Sort-Object -Unique
    if (-not $names) { $row.note = "NOT FOUND on the server ($($s.dir)/$($s.prefix)*)" } else {
      $row.file = @($names)[-1]; $row.url = "$kiwix/$($s.dir)/$($row.file)"
      $b = Head $row.url
      if ($null -eq $b) { $row.note = 'server did not answer for this file' } else { $row.bytes = [math]::Max($b, 0) }
    }
  } elseif ($s.kind -eq 'huggingface') {
    $row.file = $s.file; $row.url = "https://huggingface.co/$($s.repo)/resolve/main/$($s.file)"; $row.bytes = [int64]$s.bytes
    $b = Head $row.url
    if ($null -eq $b) { $row.note = 'server did not answer for this file' } elseif ($b -gt 0 -and $b -ne $row.bytes) { $row.note = "size on server is $b, manifest says $($row.bytes)"; $row.bytes = $b }
  } else { $row.note = "not downloaded here: $($s.how)" }
  $plan += [pscustomobject]$row
}

# 4. Print the plan and check the size limit.
Write-Host ""; Write-Host "Plan for $Edition ($Target)"; Write-Host "----------------------------------------"
foreach ($p in $plan) {
  $gb = if ($p.bytes) { '{0,8:N2} GB' -f ($p.bytes / 1GB) } else { '         -  ' }
  Write-Host ("{0} {1,-22} {2}" -f $gb, $p.id, $p.file) -NoNewline
  if ($p.note) { Write-Host "   [$($p.note)]" } else { Write-Host "" }
}
$planned = ($plan | Measure-Object -Property bytes -Sum).Sum; if (-not $planned) { $planned = 0 }
$have = 0
foreach ($d in 'library','models','maps','manuals') {
  $dir = Join-Path $Target $d
  if (Test-Path $dir) { $x = (Get-ChildItem $dir -Recurse -File -ErrorAction SilentlyContinue | Where-Object { $_.Name -notlike '*.part' } | Measure-Object Length -Sum).Sum; if ($x) { $have += $x } }
}
$totalGb = [math]::Round(($planned + $have) / 1GB, 1)
Write-Host "----------------------------------------"
Write-Host "Download total: $([math]::Round($planned / 1GB, 1)) GB, already on the drive: $([math]::Round($have / 1GB, 1)) GB, together: $totalGb GB"
$missing = @($plan | Where-Object { $_.kind -in 'kiwix','huggingface' -and (-not $_.url -or $_.note -like 'NOT FOUND*' -or $_.note -like 'server did not*') })
if ($missing.Count) { Write-Warning "$($missing.Count) item(s) could not be found: $((($missing | ForEach-Object { $_.id }) -join ', '))" }
if ($budgetGb -and $totalGb -gt $budgetGb) { throw "Cindrix $Edition content would be $totalGb GB, over the $budgetGb GB limit. Remove optional items or choose a smaller tier." }
if ($DryRun) { Write-Host "Dry run: nothing was downloaded."; if ($missing.Count) { exit 2 } else { exit 0 } }

# 5. Download, continuing unfinished files, then check them.
function Fetch($url, $dest) {
  $part = "$dest.part"
  & curl.exe -L --fail --retry 5 --retry-delay 10 -C - -o $part $url
  if ($LASTEXITCODE -ne 0) { throw "download failed: $url" }
  Move-Item -Force $part $dest
}
foreach ($p in $plan | Where-Object { $_.url }) {
  $dir = Join-Path $Target $p.dest
  $dest = Join-Path $dir $p.file
  if ((Test-Path $dest) -and (Get-Item $dest).Length -eq $p.bytes) { Write-Host "Already have $($p.file)"; continue }
  Write-Host "Downloading $($p.file) ($([math]::Round($p.bytes / 1GB, 2)) GB) ..."
  Fetch $p.url $dest
  if ($p.bytes -gt 0 -and (Get-Item $dest).Length -ne $p.bytes) { throw "$($p.file) has the wrong size; delete it and run again." }
  if ($p.kind -eq 'kiwix') {
    $sumText = (& curl.exe -sL --fail --max-time 120 "$($p.url).sha256") -join ' '
    if ($sumText -match '^\s*([0-9a-fA-F]{64})') {
      $got = (Get-FileHash -Algorithm SHA256 -Path $dest).Hash
      if ($got -ne $Matches[1].ToUpper()) { throw "$($p.file) failed its checksum; delete it and run again." }
      Write-Host "  checksum ok"
    } else { Write-Warning "  no checksum published for $($p.file); size matched" }
  }
}
Write-Host ""
Write-Host "Done. Items not downloaded here (maps, manuals, bundled content) are listed above."
Write-Host "Next: scripts\get-maps.ps1 for the street map, then run cindrix-ops.exe from $Target."

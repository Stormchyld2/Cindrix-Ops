# Puts offline archives (.zim files: Wikipedia, medical books, how-to sites and more) on a Cindrix drive,
# in <Target>\library, from the list in content\library-catalog.json.
#
# Usage:
#   powershell -File scripts\get-library.ps1 -Target D:\Cindrix -List
#   powershell -File scripts\get-library.ps1 -Target D:\Cindrix -Category medicine,survival -Tier essential
#   powershell -File scripts\get-library.ps1 -Target D:\Cindrix -Category all -Tier standard -Yes
#   powershell -File scripts\get-library.ps1 -Target D:\Cindrix -Url https://download.kiwix.org/zim/.../name.zim   (any one archive by its address)
#
# Tiers add up: "standard" also gets "essential"; "comprehensive" gets all three. The total size is shown
# first and you are asked before anything is downloaded (use -Yes to skip the question). Files already on
# the drive are skipped, so you can run it again to finish after an interruption.
#
# This runs on its own, outside the program: the program itself never goes to the internet.
# Each archive keeps its own license (see the Kiwix library page for the archive). The list is adapted from
# Project NOMAD (Apache-2.0), see licenses\README.md.
param(
  [Parameter(Mandatory=$true)][string]$Target,
  [string[]]$Category = @('all'),
  [ValidateSet('essential','standard','comprehensive')][string]$Tier = 'essential',
  [switch]$List,
  [switch]$Yes,
  [string]$Catalog = '',
  # One archive by its address (for example a Khan Academy style course archive from the Kiwix library).
  [string]$Url = ''
)
$ErrorActionPreference = 'Stop'
if (-not $Catalog) { $Catalog = Join-Path $PSScriptRoot '..\content\library-catalog.json' }
if (-not (Test-Path $Catalog)) { $Catalog = Join-Path $Target 'content\library-catalog.json' }
if (-not (Test-Path $Catalog)) { Write-Host "Could not find library-catalog.json. Use -Catalog <path>."; exit 1 }
$cat = Get-Content -Raw -Encoding UTF8 $Catalog | ConvertFrom-Json
if ($Url) {
  $cat = [pscustomobject]@{ categories = @([pscustomobject]@{ slug='extra'; name='Extra'; description=''; tiers=@([pscustomobject]@{ tier='essential'; description=''; resources=@([pscustomobject]@{ title=[IO.Path]::GetFileName($Url); description=''; url=$Url; size_mb=0 }) }) }) }
}
$rank = @{ essential = 1; standard = 2; comprehensive = 3 }

if ($List) {
  foreach ($c in $cat.categories) {
    Write-Host ""; Write-Host ("{0} ({1}): {2}" -f $c.name, $c.slug, $c.description)
    foreach ($t in $c.tiers) {
      $mb = ($t.resources | Measure-Object size_mb -Sum).Sum
      Write-Host ("  {0}: {1} archives, about {2:N1} GB. {3}" -f $t.tier, @($t.resources).Count, ($mb/1024), $t.description)
      foreach ($r in $t.resources) { Write-Host ("      {0} ({1:N0} MB): {2}" -f $r.title, $r.size_mb, $r.description) }
    }
  }
  if ($cat.to_find) {
    Write-Host ""; Write-Host "Not on the list yet (no checked address). Search these at https://browse.library.kiwix.org and use -Url:"
    foreach ($i in $cat.to_find.items) { Write-Host ("  {0} (search: {1}): {2}" -f $i.title, $i.search, $i.why) }
  }
  exit 0
}

$want = @()
foreach ($c in $cat.categories) {
  if ($Category -notcontains 'all' -and $Category -notcontains $c.slug) { continue }
  foreach ($t in $c.tiers) {
    if ($rank[$t.tier] -le $rank[$Tier]) { $want += $t.resources }
  }
}
if ($want.Count -eq 0) { Write-Host "Nothing matches. Use -List to see the category names."; exit 1 }

$dir = Join-Path $Target 'library'
New-Item -ItemType Directory -Force -Path $dir | Out-Null
$todo = @($want | Where-Object { -not (Test-Path (Join-Path $dir ([IO.Path]::GetFileName($_.url)))) })
$mb = ($todo | Measure-Object size_mb -Sum).Sum
Write-Host ("{0} archives chosen, {1} already on the drive, {2} to download (about {3:N1} GB)." -f $want.Count, ($want.Count - $todo.Count), $todo.Count, ($mb/1024))
if ($todo.Count -eq 0) { exit 0 }
$free = (Get-Item $Target).PSDrive.Free
if ($free -and ($mb * 1MB) -gt $free) { Write-Host ("There is not enough room on that drive ({0:N1} GB free). Pick fewer categories or a lower tier." -f ($free/1GB)); exit 1 }
if (-not $Yes) { if ((Read-Host "Download them now? (y/n)") -notmatch '^[yY]') { exit 0 } }

foreach ($r in $todo) {
  $name = [IO.Path]::GetFileName($r.url)
  $final = Join-Path $dir $name
  $part = "$final.part"
  Write-Host ("Downloading {0} ({1:N0} MB)..." -f $r.title, $r.size_mb)
  try {
    Invoke-WebRequest -Uri $r.url -OutFile $part -UseBasicParsing
    Move-Item -Force $part $final
  } catch {
    Write-Host ("  Could not get {0}. Run this again later to finish it. ({1})" -f $r.title, $_.Exception.Message)
  }
}
Write-Host "Done. Open Cindrix and the new archives appear in the library."

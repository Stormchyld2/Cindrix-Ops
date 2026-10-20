# Fills the library folder of a Cindrix folder/drive with Kiwix archives.
# Usage:  powershell -File scripts\build-drive.ps1 -Target D:\Cindrix [-Edition go|vault|hub] [-Name 'My drive']
# Budgets (doc 7): go content must stay at or under 100 GB, vault at or under 3400 GB, hub has no limit.
# Needs internet. Large downloads (Wikipedia is several GB). Nothing here is tested by a person yet.
param(
  [Parameter(Mandatory=$true)][string]$Target,
  [ValidateSet('go','vault','hub')][string]$Edition = 'go',
  [string]$Name = ''
)
$ErrorActionPreference = 'Stop'
$lib = Join-Path $Target 'library'
New-Item -ItemType Directory -Force -Path $lib | Out-Null
# The owner's own music goes here. It is never counted in the size budget.
$music = Join-Path $Target 'music'
New-Item -ItemType Directory -Force -Path $music | Out-Null
if (-not (Test-Path (Join-Path $music 'README.txt'))) { Set-Content -Path (Join-Path $music 'README.txt') -Value 'Copy your music here (folders like Artist\Album\01 - Song.mp3). It is never changed or counted in the size limit.' -Encoding UTF8 }
# The marker file is how the program finds this drive on any computer, whatever drive letter it gets.
if (-not $Name) { $Name = switch ($Edition) { 'vault' { 'Cindrix Ops Vault' } 'hub' { 'Cindrix Ops Hub' } default { 'Cindrix Ops Go' } } }
$marker = '{ "name": "' + $Name + '", "edition": "' + $Edition + '" }'
Set-Content -Path (Join-Path $Target 'cindrix-drive.json') -Value $marker -Encoding UTF8
$base = 'https://download.kiwix.org/zim'
# project folder on download.kiwix.org, filename prefix (latest dated file is picked)
$wanted = @(
  @{ dir='wikipedia'; prefix='wikipedia_en_all_nopic_' },
  @{ dir='wikipedia'; prefix='wikipedia_en_medicine_maxi_' },
  @{ dir='other';     prefix='ready.gov_en_' }
)
foreach ($w in $wanted) {
  $listing = (Invoke-WebRequest -UseBasicParsing "$base/$($w.dir)/").Content
  $names = [regex]::Matches($listing, "$([regex]::Escape($w.prefix))[0-9-]+\.zim") | ForEach-Object { $_.Value } | Sort-Object -Unique
  if (-not $names) { Write-Warning "Not found on server: $($w.prefix)"; continue }
  $latest = $names[-1]
  $dest = Join-Path $lib $latest
  if (Test-Path $dest) { Write-Host "Already have $latest"; continue }
  Write-Host "Downloading $latest ..."
  Invoke-WebRequest -UseBasicParsing "$base/$($w.dir)/$latest" -OutFile $dest
}
# Size budget: add up the real size of the library folder and fail when the edition's limit is passed.
# Keep these numbers the same as core/src/product.rs.
$budgetGb = switch ($Edition) { 'go' { 100 } 'vault' { 3400 } default { $null } }
$bytes = (Get-ChildItem -Path $lib -Recurse -File -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
if (-not $bytes) { $bytes = 0 }
$gb = [math]::Round($bytes / 1GB, 1)
Write-Host "Library content: $gb GB"
if ($budgetGb -and $gb -gt $budgetGb) { throw "Cindrix $Edition content is $gb GB, over the $budgetGb GB limit." }
Write-Host "Done. Put your model .gguf file in $(Join-Path $Target 'models')."

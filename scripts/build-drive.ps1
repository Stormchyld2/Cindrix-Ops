# Fills the library folder of a Cindrix folder/drive with Kiwix archives.
# Usage:  powershell -File scripts\build-drive.ps1 -Target D:\Cindrix [-Edition go|vault] [-Name 'My drive']
# Needs internet. Large downloads (Wikipedia is several GB). Nothing here is tested by a person yet.
param(
  [Parameter(Mandatory=$true)][string]$Target,
  [ValidateSet('go','vault')][string]$Edition = 'go',
  [string]$Name = ''
)
$ErrorActionPreference = 'Stop'
$lib = Join-Path $Target 'library'
New-Item -ItemType Directory -Force -Path $lib | Out-Null
# The marker file is how the program finds this drive on any computer, whatever drive letter it gets.
if (-not $Name) { $Name = if ($Edition -eq 'vault') { 'Cindrix Vault' } else { 'Cindrix Go' } }
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
Write-Host "Done. Put your model .gguf file in $(Join-Path $Target 'models')."

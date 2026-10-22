@echo off
rem Double-click to put USGS aerial photos and the topo map on this drive for the whole United States at a wide view.
rem Needs internet; each takes several hours and about 15 GB. For one area in full detail (house level) run, for example:
rem   Get-Imagery.bat -Layer aerial -Bbox:"-80.30,39.40,-80.00,39.60" -Name my-county -MaxZoom 16
if not "%~1"=="" (
  powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0get-imagery.ps1" -Target "%~dp0.." %*
  goto end
)
echo Putting the whole-US aerial photos on this drive (zoom 0 to 13). Leave this window open. If it stops, run it again.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0get-imagery.ps1" -Target "%~dp0.." -Layer aerial
echo.
echo Now the topo map.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0get-imagery.ps1" -Target "%~dp0.." -Layer topo
:end
echo.
pause

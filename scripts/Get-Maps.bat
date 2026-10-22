@echo off
rem Double-click to put the street map of the whole United States on this drive (lower 48, Alaska and Hawaii).
rem It needs internet and a lot of disk space (tens of gigabytes) and can take hours.
rem For a smaller map add:  -MaxZoom 13     For one area only:  -Bbox:"west,south,east,north" -Name my-area
echo Putting the US street map, place names and the places list on this drive. Leave this window open.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0get-maps.ps1" -Target "%~dp0.." -Places %*
echo.
pause

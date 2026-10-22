@echo off
rem Double-click to put street addresses for the whole United States (lower 48, Alaska and Hawaii) on this drive.
rem It needs internet, about 15 GB of free space while it works (about 6 GB stays) and can take a few hours.
rem For one area only:  -Bbox:"west,south,east,north" -Name my-area
echo Putting street addresses on this drive. Leave this window open. If it stops, run it again: it carries on.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0get-addresses.ps1" -Target "%~dp0.." %*
echo.
pause

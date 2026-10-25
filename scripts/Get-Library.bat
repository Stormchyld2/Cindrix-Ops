@echo off
rem Double-click to put the essential offline archives (medicine, survival, farming and more) on this drive.
rem It needs internet and some disk space and shows the total size before it downloads anything.
rem For more:  -Tier standard     For some topics only:  -Category medicine,survival     To see the list:  -List
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0get-library.ps1" -Target "%~dp0.." %*
echo.
pause

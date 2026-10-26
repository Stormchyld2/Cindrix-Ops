@echo off
rem Double-click to put the FDA's drug label files on this drive (several gigabytes, needs internet).
rem Afterwards open Cindrix > Intel > Drug labels and press "Get the labels ready".
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0get-druglabels.ps1" -Target "%~dp0.." %*
echo.
pause

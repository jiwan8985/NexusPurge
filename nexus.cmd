@echo off
rem NexusPurge CLI shell shim for cmd.exe / PATH. Delegates to nexus.ps1.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0nexus.ps1" %*

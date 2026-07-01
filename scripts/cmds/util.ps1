[CmdletBinding()]
param(
  [Parameter(Position = 0)] [string]$Sub = "help"
)

# [유틸리티] open-data / log

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$ErrorActionPreference = "Stop"
$Root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
. (Join-Path $PSScriptRoot "..\lib\output.ps1")

Push-Location $Root
try {
  switch ($Sub) {
    "open-data" {
      $dataDir = Join-Path $env:APPDATA "com.nexuspurge.app"
      if (-not (Test-Path $dataDir)) {
        Write-Host "   데이터 폴더 없음: $dataDir" -ForegroundColor Yellow
        Write-Host "   (앱을 한 번 실행하면 생성됩니다)"
      } else {
        step "데이터 폴더 열기: $dataDir"
        Start-Process explorer.exe $dataDir
      }
    }

    "log" {
      step "git log (최근 20)"
      git log --oneline --graph --decorate -20
    }

    default {
      Write-Error "util.ps1: unknown sub-command '$Sub' (open-data|log)"
      exit 1
    }
  }
} finally {
  Pop-Location
}

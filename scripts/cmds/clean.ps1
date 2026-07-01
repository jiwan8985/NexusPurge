[CmdletBinding()]
param(
  [Parameter(Position = 0)] [string]$Sub = "help"
)

# [정리] all / cache

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$ErrorActionPreference = "Stop"
$Root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
. (Join-Path $PSScriptRoot "..\lib\output.ps1")

Push-Location $Root
try {
  switch ($Sub) {
    "all" {
      step "dist/ + .vite/ + src-tauri/target/ 삭제"
      foreach ($path in @("dist", ".vite", "src-tauri\target")) {
        $full = Join-Path $Root $path
        if (Test-Path $full) {
          Remove-Item -Recurse -Force $full
          ok "삭제: $path"
        }
      }
      ok "clean 완료"
    }

    "cache" {
      step "dist/ + .vite/ 삭제  (target 보존)"
      foreach ($path in @("dist", ".vite")) {
        $full = Join-Path $Root $path
        if (Test-Path $full) {
          Remove-Item -Recurse -Force $full
          ok "삭제: $path"
        }
      }
      ok "cache clean 완료"
    }

    default {
      Write-Error "clean.ps1: unknown sub-command '$Sub' (all|cache)"
      exit 1
    }
  }
} finally {
  Pop-Location
}

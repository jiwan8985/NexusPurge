[CmdletBinding()]
param(
  [Parameter(Position = 0)] [string]$Sub = "help"
)

# [빌드] fe / debug / release

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$ErrorActionPreference = "Stop"
$Root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
. (Join-Path $PSScriptRoot "..\lib\output.ps1")

Push-Location $Root
try {
  switch ($Sub) {
    "fe" {
      step "프론트엔드 빌드  (tsc + vite build)"
      pnpm build
      ok "dist/ 생성 완료"
    }

    "debug" {
      step "Tauri 디버그 빌드"
      pnpm tauri build --debug
      ok "src-tauri/target/debug/bundle/ 확인"
    }

    "release" {
      step "Tauri 릴리즈 빌드"
      pnpm tauri build
      ok "src-tauri/target/release/bundle/ 확인"
    }

    default {
      Write-Error "build.ps1: unknown sub-command '$Sub' (fe|debug|release)"
      exit 1
    }
  }
} finally {
  Pop-Location
}

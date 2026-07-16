[CmdletBinding()]
param(
  [Parameter(Position = 0)] [string]$Sub = "help"
)

# [테스트] run / watch / rust / all

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$ErrorActionPreference = "Stop"
$Root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
. (Join-Path $PSScriptRoot "..\lib\output.ps1")

Push-Location $Root
try {
  switch ($Sub) {
    "run" {
      step "Vitest"
      pnpm test
    }

    "watch" {
      step "Vitest (감시 모드)"
      pnpm test:watch
    }

    "rust" {
      step "cargo test"
      cargo test --manifest-path src-tauri/Cargo.toml
    }

    "all" {
      step "Vitest"
      pnpm test
      step "cargo test"
      cargo test --manifest-path src-tauri/Cargo.toml
      ok "전체 테스트 완료"
    }

    default {
      Write-Error "test.ps1: unknown sub-command '$Sub' (run|watch|rust|all)"
      exit 1
    }
  }
} finally {
  Pop-Location
}

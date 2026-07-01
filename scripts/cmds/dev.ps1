[CmdletBinding()]
param(
  [Parameter(Position = 0)] [string]$Sub = "help"
)

# [개발] install / dev / check / fmt / clippy

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$ErrorActionPreference = "Stop"
$Root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
. (Join-Path $PSScriptRoot "..\lib\output.ps1")

Push-Location $Root
try {
  switch ($Sub) {
    "install" {
      step "pnpm install"
      pnpm install
    }

    "dev" {
      step "pnpm tauri dev"
      pnpm tauri dev
    }

    "check" {
      step "TypeScript typecheck"
      pnpm exec tsc --noEmit
      step "cargo check"
      cargo check --manifest-path src-tauri/Cargo.toml
      ok "check 완료"
    }

    "fmt" {
      step "cargo fmt"
      cargo fmt --manifest-path src-tauri/Cargo.toml
      ok "포맷 완료"
    }

    "clippy" {
      step "cargo clippy"
      cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
    }

    default {
      Write-Error "dev.ps1: unknown sub-command '$Sub' (install|dev|check|fmt|clippy)"
      exit 1
    }
  }
} finally {
  Pop-Location
}

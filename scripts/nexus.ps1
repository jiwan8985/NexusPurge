[CmdletBinding()]
param(
  [Parameter(Position = 0)] [string]$Command = "help",
  [Parameter(Position = 1, ValueFromRemainingArguments)] [string[]]$Rest
)

$ErrorActionPreference = "Stop"
$Root = Resolve-Path (Join-Path $PSScriptRoot "..")

function usage {
  Write-Host @"
nexus.ps1 <command>

  install       pnpm install
  dev           pnpm tauri dev
  build         pnpm tauri build
  check         tsc --noEmit  +  cargo check
  test          pnpm test
"@
}

Push-Location $Root
try {
  switch ($Command) {
    "install" { pnpm install }
    "dev"     { pnpm tauri dev }
    "build"   { pnpm tauri build }
    "check"   {
      pnpm exec tsc --noEmit
      cargo check --manifest-path src-tauri/Cargo.toml
    }
    "test"    { pnpm test }
    { $_ -in "help","-h","--help" } { usage }
    default   { Write-Error "unknown: $Command"; usage; exit 1 }
  }
} finally {
  Pop-Location
}

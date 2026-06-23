[CmdletBinding()]
param(
  [Parameter(Position = 0)] [string]$Command = "help",
  [Parameter(Position = 1, ValueFromRemainingArguments)] [string[]]$Rest
)

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$ErrorActionPreference = "Stop"
$Root = Resolve-Path (Join-Path $PSScriptRoot "..")

function usage {
  Write-Host @"
nexus.ps1 <command>

  [개발]
  install       pnpm install
  dev           pnpm tauri dev
  check         tsc --noEmit  +  cargo check
  fmt           cargo fmt  (Rust 코드 포맷)
  clippy        cargo clippy  (Rust 린트)

  [테스트]
  test          pnpm test  (Vitest)
  test:watch    pnpm test:watch  (감시 모드)
  test:rust     cargo test
  test:all      pnpm test  +  cargo test

  [빌드]
  build:fe      tsc + vite build  (프론트엔드만)
  build:debug   pnpm tauri build --debug
  build         pnpm tauri build  (릴리즈)

  [정리]
  clean         dist/ + .vite/ + src-tauri/target/ 삭제
  clean:cache   dist/ + .vite/ 만 삭제  (target 보존)

  [유틸리티]
  open:data     앱 데이터 폴더 열기  (profiles.json 위치)
  log           git log --oneline -20
"@
}

function step([string]$msg) {
  Write-Host "`n>> $msg" -ForegroundColor Cyan
}

function ok([string]$msg) {
  Write-Host "   $msg" -ForegroundColor Green
}

Push-Location $Root
try {
  switch ($Command) {

    # ── 개발 ──────────────────────────────────────────────────────────────────

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

    # ── 테스트 ────────────────────────────────────────────────────────────────

    "test" {
      step "Vitest"
      pnpm test
    }

    "test:watch" {
      step "Vitest (감시 모드)"
      pnpm test:watch
    }

    "test:rust" {
      step "cargo test"
      cargo test --manifest-path src-tauri/Cargo.toml
    }

    "test:all" {
      step "Vitest"
      pnpm test
      step "cargo test"
      cargo test --manifest-path src-tauri/Cargo.toml
      ok "전체 테스트 완료"
    }

    # ── 빌드 ──────────────────────────────────────────────────────────────────

    "build:fe" {
      step "프론트엔드 빌드  (tsc + vite build)"
      pnpm build
      ok "dist/ 생성 완료"
    }

    "build:debug" {
      step "Tauri 디버그 빌드"
      pnpm tauri build --debug
      ok "src-tauri/target/debug/bundle/ 확인"
    }

    "build" {
      step "Tauri 릴리즈 빌드"
      pnpm tauri build
      ok "src-tauri/target/release/bundle/ 확인"
    }

    # ── 정리 ──────────────────────────────────────────────────────────────────

    "clean" {
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

    "clean:cache" {
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

    # ── 유틸리티 ──────────────────────────────────────────────────────────────

    "open:data" {
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

    { $_ -in "help", "-h", "--help" } { usage }

    default {
      Write-Error "unknown command: $Command"
      usage
      exit 1
    }
  }
} finally {
  Pop-Location
}


[CmdletBinding()]
param(
  [Parameter(Position = 0)] [string]$Command = "help",
  [Parameter(Position = 1, ValueFromRemainingArguments)] [string[]]$Rest
)

# (루트) nexus.ps1 이 사용하는 Windows용 명령어 디스패처.
# 실제 실행 로직은 scripts/cmds/*.ps1 에 그룹별로 분리되어 있음.
# macOS/Linux 대응 버전: nexus (bash) -> scripts/nexus-core.sh -> scripts/cmds/*.sh

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$ErrorActionPreference = "Stop"

$CmdsDir = Join-Path $PSScriptRoot "cmds"

function usage {
  Write-Host @"
사용법: .\nexus.ps1 <command>   (예: .\nexus.ps1 dev, .\nexus.ps1 test:watch)
       nexus <command>         (PATH 등록 시, cmd.exe 포함)

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

# 명령어 이름 -> (그룹 스크립트, 서브커맨드)
$dispatch = @{
  "install"     = @{ file = "dev.ps1";   sub = "install" }
  "dev"         = @{ file = "dev.ps1";   sub = "dev" }
  "check"       = @{ file = "dev.ps1";   sub = "check" }
  "fmt"         = @{ file = "dev.ps1";   sub = "fmt" }
  "clippy"      = @{ file = "dev.ps1";   sub = "clippy" }

  "test"        = @{ file = "test.ps1";  sub = "run" }
  "test:watch"  = @{ file = "test.ps1";  sub = "watch" }
  "test:rust"   = @{ file = "test.ps1";  sub = "rust" }
  "test:all"    = @{ file = "test.ps1";  sub = "all" }

  "build:fe"    = @{ file = "build.ps1"; sub = "fe" }
  "build:debug" = @{ file = "build.ps1"; sub = "debug" }
  "build"       = @{ file = "build.ps1"; sub = "release" }

  "clean"       = @{ file = "clean.ps1"; sub = "all" }
  "clean:cache" = @{ file = "clean.ps1"; sub = "cache" }

  "open:data"   = @{ file = "util.ps1";  sub = "open-data" }
  "log"         = @{ file = "util.ps1";  sub = "log" }
}

if ($Command -in @("help", "-h", "--help")) {
  usage
  exit 0
}

if (-not $dispatch.ContainsKey($Command)) {
  Write-Error "unknown command: $Command"
  usage
  exit 1
}

$target = $dispatch[$Command]
if ($Rest) {
  & (Join-Path $CmdsDir $target.file) $target.sub @Rest
} else {
  & (Join-Path $CmdsDir $target.file) $target.sub
}
exit $LASTEXITCODE

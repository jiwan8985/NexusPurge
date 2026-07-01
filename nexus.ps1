[CmdletBinding()]
param(
  [Parameter(Position = 0)] [string]$Command = "help",
  [Parameter(Position = 1, ValueFromRemainingArguments)] [string[]]$Rest
)

# NexusPurge 프로젝트 전용 CLI (Windows).
# 실행 로직: scripts/nexus-core.ps1 + scripts/cmds/*.ps1
# macOS/Linux 대응 버전: 루트 nexus (bash)
# PATH에 저장소 루트를 추가하면 어디서든 `nexus <command>` 로 사용 가능 (cmd.exe는 nexus.cmd 경유).

if ($Command -in @("-v", "--version", "version")) {
  $pkg = Get-Content (Join-Path $PSScriptRoot "package.json") -Raw | ConvertFrom-Json
  Write-Host "NexusPurge CLI v$($pkg.version)"
  exit 0
}

if ($Rest) {
  & (Join-Path $PSScriptRoot "scripts\nexus-core.ps1") $Command @Rest
} else {
  & (Join-Path $PSScriptRoot "scripts\nexus-core.ps1") $Command
}
exit $LASTEXITCODE

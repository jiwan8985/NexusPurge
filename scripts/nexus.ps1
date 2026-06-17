[CmdletBinding()]
param(
  [Parameter(Position = 0)]
  [string]$Command = "help",

  [Parameter(Position = 1, ValueFromRemainingArguments = $true)]
  [string[]]$Args
)

$ErrorActionPreference = "Stop"
$RootDir = Resolve-Path (Join-Path $PSScriptRoot "..")
$LogDir = Join-Path $RootDir ".logs"

function Show-Usage {
  @"
NexusPurge helper

Usage:
  .\scripts\nexus.ps1 <command> [args]

Commands:
  install             Install frontend dependencies with pnpm
  dev                 Start the Vite dev server in the background
  tauri               Start the full Tauri desktop app in the background
  preview             Start the Vite preview server in the background
  stop [name|all]     Stop dev, tauri, preview, or all managed processes
  restart <name>      Restart dev, tauri, or preview
  status              Show managed process status
  logs [name] [-f]    Show logs for dev, tauri, preview, or latest log
  build               Run pnpm build
  tauri-build         Run pnpm tauri build
  test                Run pnpm test
  check               Run pnpm build and cargo test
  cargo-check         Run cargo check for the Tauri backend
  cargo-test          Run cargo test for the Tauri backend
  localstack          Run the LocalStack integration script
  clean-logs          Remove managed log and pid files
  help                Show this help

Examples:
  .\scripts\nexus.ps1 install
  .\scripts\nexus.ps1 tauri
  .\scripts\nexus.ps1 logs tauri -f
  .\scripts\nexus.ps1 stop all
"@
}

function Ensure-LogDir {
  if (-not (Test-Path -LiteralPath $LogDir)) {
    New-Item -ItemType Directory -Path $LogDir | Out-Null
  }
}

function Require-Pnpm {
  if (Get-Command pnpm -ErrorAction SilentlyContinue) {
    return
  }

  if (Get-Command corepack -ErrorAction SilentlyContinue) {
    & corepack enable pnpm
  }

  if (-not (Get-Command pnpm -ErrorAction SilentlyContinue)) {
    throw "pnpm is required. Install pnpm or enable it with corepack."
  }
}

function Get-PidFile([string]$Name) {
  Join-Path $LogDir "nexus-$Name.pid"
}

function Get-LogFile([string]$Name) {
  Join-Path $LogDir "nexus-$Name.log"
}

function Get-ErrorLogFile([string]$Name) {
  Join-Path $LogDir "nexus-$Name-error.log"
}

function Get-RunnerFile([string]$Name) {
  Join-Path $LogDir "nexus-$Name.cmd"
}

function Read-ManagedPid([string]$Name) {
  $file = Get-PidFile $Name
  if (Test-Path -LiteralPath $file) {
    $value = (Get-Content -LiteralPath $file -Raw).Trim()
    if ($value -match "^\d+$") {
      return [int]$value
    }
  }
  return $null
}

function Test-Running([Nullable[int]]$PidValue) {
  if (-not $PidValue) {
    return $false
  }
  return [bool](Get-Process -Id $PidValue -ErrorAction SilentlyContinue)
}

function Get-ChildProcessIds([int]$ParentPid) {
  try {
    $children = Get-CimInstance Win32_Process -Filter "ParentProcessId = $ParentPid" -ErrorAction Stop
  } catch {
    return
  }

  foreach ($child in $children) {
    Get-ChildProcessIds -ParentPid ([int]$child.ProcessId)
    [int]$child.ProcessId
  }
}

function Stop-ProcessTree([int]$PidValue) {
  $ids = @(Get-ChildProcessIds -ParentPid $PidValue) + $PidValue
  foreach ($id in ($ids | Select-Object -Unique)) {
    $process = Get-Process -Id $id -ErrorAction SilentlyContinue
    if ($process) {
      Stop-Process -Id $id -Force -ErrorAction SilentlyContinue
    }
  }
}

function Resolve-Executable([string]$Name) {
  $cmdCommand = Get-Command "$Name.cmd" -ErrorAction SilentlyContinue
  if ($cmdCommand) {
    return $cmdCommand.Source
  }

  $command = Get-Command $Name -ErrorAction Stop
  return $command.Source
}

function Quote-CmdArgument([string]$Value) {
  '"' + ($Value -replace '"', '\"') + '"'
}

function Start-Managed([string]$Name, [string]$Executable, [string[]]$Arguments) {
  Ensure-LogDir
  Require-Pnpm

  $existingPid = Read-ManagedPid $Name
  if (Test-Running $existingPid) {
    Write-Host "$Name is already running: pid=$existingPid"
    Write-Host "log: $(Get-LogFile $Name)"
    return
  }

  $logFile = Get-LogFile $Name
  $runnerFile = Get-RunnerFile $Name
  $exePath = Resolve-Executable $Executable
  $commandLine = (@((Quote-CmdArgument $exePath)) + ($Arguments | ForEach-Object { Quote-CmdArgument $_ })) -join " "
  $runner = @(
    "@echo off"
    "cd /d $(Quote-CmdArgument $RootDir)"
    "call $commandLine > $(Quote-CmdArgument $logFile) 2>&1"
  )
  Set-Content -LiteralPath $runnerFile -Value $runner -Encoding ASCII

  $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
  $startInfo.FileName = $runnerFile
  $startInfo.WorkingDirectory = $RootDir
  $startInfo.UseShellExecute = $true
  $startInfo.WindowStyle = [System.Diagnostics.ProcessWindowStyle]::Hidden

  $process = [System.Diagnostics.Process]::Start($startInfo)

  Set-Content -LiteralPath (Get-PidFile $Name) -Value $process.Id
  Write-Host "started ${Name}: pid=$($process.Id)"
  Write-Host "log: $logFile"
}

function Stop-One([string]$Name) {
  Ensure-LogDir
  $pidValue = Read-ManagedPid $Name
  if (-not (Test-Running $pidValue)) {
    Remove-Item -LiteralPath (Get-PidFile $Name) -Force -ErrorAction SilentlyContinue
    Write-Host "$Name is not running"
    return
  }

  Stop-ProcessTree -PidValue $pidValue
  Remove-Item -LiteralPath (Get-PidFile $Name) -Force -ErrorAction SilentlyContinue
  Write-Host "stopped $Name"
}

function Stop-Target([string]$Target) {
  switch ($Target) {
    "all" {
      Stop-One "dev"
      Stop-One "tauri"
      Stop-One "preview"
    }
    "dev" { Stop-One "dev" }
    "tauri" { Stop-One "tauri" }
    "preview" { Stop-One "preview" }
    default { throw "unknown process: $Target" }
  }
}

function Show-Status {
  Ensure-LogDir
  foreach ($name in @("dev", "tauri", "preview")) {
    $pidValue = Read-ManagedPid $name
    if (Test-Running $pidValue) {
      Write-Host ("{0,-8} running pid={1} log={2}" -f $name, $pidValue, (Get-LogFile $name))
    } else {
      Write-Host ("{0,-8} stopped" -f $name)
    }
  }
}

function Show-Logs([string[]]$LogArgs) {
  Ensure-LogDir
  $name = $null
  $follow = $false

  foreach ($arg in $LogArgs) {
    if ($arg -eq "-f" -or $arg -eq "--follow") {
      $follow = $true
    } elseif (-not $name) {
      $name = $arg
    }
  }

  if ($name) {
    $file = Get-LogFile $name
    $errorFile = $null
  } else {
    $latest = Get-ChildItem -LiteralPath $LogDir -Filter "nexus-*.log" -File -ErrorAction SilentlyContinue |
      Sort-Object LastWriteTime -Descending |
      Select-Object -First 1
    $file = if ($latest) { $latest.FullName } else { $null }
    $errorFile = $null
  }

  $existingFiles = @()
  if ($file -and (Test-Path -LiteralPath $file)) {
    $existingFiles += $file
  }
  if ($errorFile -and (Test-Path -LiteralPath $errorFile)) {
    $existingFiles += $errorFile
  }

  if ($existingFiles.Count -eq 0) {
    throw "no log file found"
  }

  Write-Host "log: $($existingFiles -join ', ')"
  if ($follow) {
    Get-Content -LiteralPath $existingFiles -Tail 120 -Wait
  } else {
    Get-Content -LiteralPath $existingFiles -Tail 120
  }
}

function Invoke-InRoot([string]$Executable, [string[]]$Arguments) {
  Push-Location $RootDir
  try {
    & $Executable @Arguments
  } finally {
    Pop-Location
  }
}

switch ($Command) {
  "install" {
    Require-Pnpm
    Invoke-InRoot "pnpm" @("install")
  }
  "dev" {
    Start-Managed "dev" "pnpm" @("run", "dev")
  }
  "tauri" {
    Start-Managed "tauri" "pnpm" @("tauri", "dev")
  }
  "preview" {
    Start-Managed "preview" "pnpm" @("run", "preview")
  }
  "stop" {
    $target = if ($Args.Count -gt 0) { $Args[0] } else { "all" }
    Stop-Target $target
  }
  "restart" {
    if ($Args.Count -eq 0 -or $Args[0] -notin @("dev", "tauri", "preview")) {
      throw "restart requires dev, tauri, or preview"
    }
    Stop-One $Args[0]
    $restartArgs = if ($Args[0] -eq "dev") {
      @("run", "dev")
    } elseif ($Args[0] -eq "tauri") {
      @("tauri", "dev")
    } else {
      @("run", "preview")
    }
    Start-Managed $Args[0] "pnpm" $restartArgs
  }
  "status" {
    Show-Status
  }
  "logs" {
    Show-Logs $Args
  }
  "build" {
    Require-Pnpm
    Invoke-InRoot "pnpm" @("run", "build")
  }
  "tauri-build" {
    Require-Pnpm
    Invoke-InRoot "pnpm" @("tauri", "build")
  }
  "test" {
    Require-Pnpm
    Invoke-InRoot "pnpm" @("test")
  }
  "check" {
    Require-Pnpm
    Invoke-InRoot "pnpm" @("run", "build")
    Invoke-InRoot "cargo" @("test", "--manifest-path", "src-tauri/Cargo.toml")
  }
  "cargo-check" {
    Invoke-InRoot "cargo" @("check", "--manifest-path", "src-tauri/Cargo.toml")
  }
  "cargo-test" {
    Invoke-InRoot "cargo" @("test", "--manifest-path", "src-tauri/Cargo.toml")
  }
  "localstack" {
    Invoke-InRoot "bash" @("scripts/localstack-integration.sh")
  }
  "clean-logs" {
    Remove-Item -Path (Join-Path $LogDir "nexus-*.log") -Force -ErrorAction SilentlyContinue
    Remove-Item -Path (Join-Path $LogDir "nexus-*.pid") -Force -ErrorAction SilentlyContinue
    Remove-Item -Path (Join-Path $LogDir "nexus-*.cmd") -Force -ErrorAction SilentlyContinue
    Write-Host "removed managed logs"
  }
  "help" {
    Show-Usage
  }
  "-h" {
    Show-Usage
  }
  "--help" {
    Show-Usage
  }
  default {
    Write-Error "unknown command: $Command"
    Show-Usage
    exit 1
  }
}

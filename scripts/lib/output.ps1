function step([string]$msg) {
  Write-Host "`n>> $msg" -ForegroundColor Cyan
}

function ok([string]$msg) {
  Write-Host "   $msg" -ForegroundColor Green
}

[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)]
  [string]$Bucket,

  [string]$Region = "",
  [string]$Profile = "",
  [string]$EndpointUrl = "",
  [string]$Prefix = "",
  [switch]$WriteProbe,
  [string]$CloudFrontDistributionId = "",
  [switch]$CreateInvalidationProbe,
  [string]$InvalidationPath = "/__nexuspurge-permission-check__"
)

$ErrorActionPreference = "Stop"
$Failures = 0

function Write-Step([string]$Status, [string]$Name, [string]$Detail = "") {
  $line = "[{0}] {1}" -f $Status, $Name
  if ($Detail) {
    $line = "$line - $Detail"
  }
  Write-Host $line
}

function Require-AwsCli {
  if (-not (Get-Command aws -ErrorAction SilentlyContinue)) {
    throw "AWS CLI is required. Install AWS CLI v2 and configure credentials first."
  }
}

function Invoke-AwsChecked([string]$Name, [string[]]$AwsArgs) {
  $output = & aws @AwsArgs 2>&1
  $code = $LASTEXITCODE
  if ($code -eq 0) {
    Write-Step "OK" $Name
    return $output
  }

  $script:Failures += 1
  $message = ($output | Out-String).Trim()
  if (-not $message) {
    $message = "aws exited with code $code"
  }
  Write-Step "FAIL" $Name $message
  return $null
}

function Add-CommonArgs([string[]]$AwsArgs, [switch]$UseEndpoint) {
  $result = @($AwsArgs)
  if ($Profile) {
    $result += @("--profile", $Profile)
  }
  if ($Region) {
    $result += @("--region", $Region)
  }
  if ($UseEndpoint -and $EndpointUrl) {
    $result += @("--endpoint-url", $EndpointUrl)
  }
  return $result
}

Require-AwsCli

Write-Host "NexusPurge AWS/S3 permission check"
Write-Host "bucket: $Bucket"
if ($Region) { Write-Host "region: $Region" }
if ($Profile) { Write-Host "profile: $Profile" }
if ($EndpointUrl) { Write-Host "endpoint: $EndpointUrl" }
if ($Prefix) { Write-Host "prefix: $Prefix" }
Write-Host ""

Invoke-AwsChecked "AWS identity (sts:GetCallerIdentity)" `
  (Add-CommonArgs @("sts", "get-caller-identity", "--output", "json"))

Invoke-AwsChecked "S3 bucket exists/access (s3api:HeadBucket)" `
  (Add-CommonArgs @("s3api", "head-bucket", "--bucket", $Bucket) -UseEndpoint)

Invoke-AwsChecked "S3 bucket location (s3api:GetBucketLocation)" `
  (Add-CommonArgs @("s3api", "get-bucket-location", "--bucket", $Bucket, "--output", "json") -UseEndpoint)

Invoke-AwsChecked "S3 list prefix (s3api:ListBucket)" `
  (Add-CommonArgs @("s3api", "list-objects-v2", "--bucket", $Bucket, "--prefix", $Prefix, "--max-keys", "1", "--output", "json") -UseEndpoint)

if ($WriteProbe) {
  $safePrefix = $Prefix.Trim("/")
  $probeKey = if ($safePrefix) {
    "$safePrefix/.nexuspurge-permission-check-$([Guid]::NewGuid().ToString("N")).txt"
  } else {
    ".nexuspurge-permission-check-$([Guid]::NewGuid().ToString("N")).txt"
  }
  $probeFile = Join-Path ([System.IO.Path]::GetTempPath()) "nexuspurge-permission-check.txt"
  Set-Content -LiteralPath $probeFile -Value "nexuspurge permission check" -Encoding ASCII

  Invoke-AwsChecked "S3 write object (s3api:PutObject)" `
    (Add-CommonArgs @("s3api", "put-object", "--bucket", $Bucket, "--key", $probeKey, "--body", $probeFile, "--content-type", "text/plain", "--output", "json") -UseEndpoint)

  Invoke-AwsChecked "S3 read object metadata (s3api:HeadObject)" `
    (Add-CommonArgs @("s3api", "head-object", "--bucket", $Bucket, "--key", $probeKey, "--output", "json") -UseEndpoint)

  Invoke-AwsChecked "S3 delete probe object (s3api:DeleteObject)" `
    (Add-CommonArgs @("s3api", "delete-object", "--bucket", $Bucket, "--key", $probeKey, "--output", "json") -UseEndpoint)

  Remove-Item -LiteralPath $probeFile -Force -ErrorAction SilentlyContinue
} else {
  Write-Step "SKIP" "S3 write/delete probe" "pass -WriteProbe to test PutObject/HeadObject/DeleteObject"
}

if ($CloudFrontDistributionId) {
  Invoke-AwsChecked "CloudFront distribution access (cloudfront:GetDistribution)" `
    (Add-CommonArgs @("cloudfront", "get-distribution", "--id", $CloudFrontDistributionId, "--output", "json"))

  if ($CreateInvalidationProbe) {
    Invoke-AwsChecked "CloudFront invalidation probe (cloudfront:CreateInvalidation)" `
      (Add-CommonArgs @("cloudfront", "create-invalidation", "--distribution-id", $CloudFrontDistributionId, "--paths", $InvalidationPath, "--output", "json"))
  } else {
    Write-Step "SKIP" "CloudFront invalidation probe" "pass -CreateInvalidationProbe to create a real invalidation"
  }
}

Write-Host ""
if ($Failures -gt 0) {
  Write-Step "FAIL" "permission check completed" "$Failures check(s) failed"
  exit 1
}

Write-Step "OK" "permission check completed"

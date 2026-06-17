#!/usr/bin/env bash
set -euo pipefail

bucket=""
region=""
profile=""
endpoint_url=""
prefix=""
write_probe=0
cloudfront_distribution_id=""
create_invalidation_probe=0
invalidation_path="/__nexuspurge-permission-check__"
failures=0

usage() {
  cat <<'EOF'
NexusPurge AWS/S3 permission check

Usage:
  ./scripts/aws-permission-check.sh --bucket <bucket> [options]

Options:
  --bucket <name>                 S3 bucket name. Required.
  --region <region>               AWS region.
  --profile <profile>             AWS CLI profile.
  --endpoint-url <url>            Custom S3 endpoint, for LocalStack or S3-compatible storage.
  --prefix <prefix>               Prefix to test ListBucket and optional write probe.
  --write-probe                   Test PutObject, HeadObject, and DeleteObject with a temporary object.
  --cloudfront-distribution-id <id>
                                  Test CloudFront GetDistribution permission.
  --create-invalidation-probe     Create a real CloudFront invalidation for the probe path.
  --invalidation-path <path>      CloudFront path for invalidation probe.
  -h, --help                      Show this help.

Examples:
  ./scripts/aws-permission-check.sh --bucket my-bucket --region ap-northeast-2
  ./scripts/aws-permission-check.sh --bucket my-bucket --profile dev --prefix static --write-probe
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bucket)
      bucket="${2:-}"
      shift 2
      ;;
    --region)
      region="${2:-}"
      shift 2
      ;;
    --profile)
      profile="${2:-}"
      shift 2
      ;;
    --endpoint-url)
      endpoint_url="${2:-}"
      shift 2
      ;;
    --prefix)
      prefix="${2:-}"
      shift 2
      ;;
    --write-probe)
      write_probe=1
      shift
      ;;
    --cloudfront-distribution-id)
      cloudfront_distribution_id="${2:-}"
      shift 2
      ;;
    --create-invalidation-probe)
      create_invalidation_probe=1
      shift
      ;;
    --invalidation-path)
      invalidation_path="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -z "$bucket" ]]; then
  echo "--bucket is required" >&2
  usage >&2
  exit 1
fi

if ! command -v aws >/dev/null 2>&1; then
  echo "AWS CLI is required. Install AWS CLI v2 and configure credentials first." >&2
  exit 1
fi

step() {
  local status="$1"
  local name="$2"
  local detail="${3:-}"
  if [[ -n "$detail" ]]; then
    printf '[%s] %s - %s\n' "$status" "$name" "$detail"
  else
    printf '[%s] %s\n' "$status" "$name"
  fi
}

run_check() {
  local name="$1"
  shift
  local output
  if output="$("$@" 2>&1)"; then
    step "OK" "$name"
  else
    failures=$((failures + 1))
    step "FAIL" "$name" "$output"
  fi
}

echo "NexusPurge AWS/S3 permission check"
echo "bucket: $bucket"
[[ -n "$region" ]] && echo "region: $region"
[[ -n "$profile" ]] && echo "profile: $profile"
[[ -n "$endpoint_url" ]] && echo "endpoint: $endpoint_url"
[[ -n "$prefix" ]] && echo "prefix: $prefix"
echo

base_common=()
if [[ -n "$profile" ]]; then
  base_common+=("--profile" "$profile")
fi
if [[ -n "$region" ]]; then
  base_common+=("--region" "$region")
fi

base_s3=("${base_common[@]}")
if [[ -n "$endpoint_url" ]]; then
  base_s3+=("--endpoint-url" "$endpoint_url")
fi

run_check "AWS identity (sts:GetCallerIdentity)" \
  aws "${base_common[@]}" sts get-caller-identity --output json

run_check "S3 bucket exists/access (s3api:HeadBucket)" \
  aws "${base_s3[@]}" s3api head-bucket --bucket "$bucket"

run_check "S3 bucket location (s3api:GetBucketLocation)" \
  aws "${base_s3[@]}" s3api get-bucket-location --bucket "$bucket" --output json

run_check "S3 list prefix (s3api:ListBucket)" \
  aws "${base_s3[@]}" s3api list-objects-v2 --bucket "$bucket" --prefix "$prefix" --max-keys 1 --output json

if [[ "$write_probe" -eq 1 ]]; then
  probe_file="$(mktemp "${TMPDIR:-/tmp}/nexuspurge-permission-check.XXXXXX")"
  printf 'nexuspurge permission check\n' > "$probe_file"
  safe_prefix="${prefix#/}"
  safe_prefix="${safe_prefix%/}"
  probe_key=".nexuspurge-permission-check-$(date +%s)-$$.txt"
  if [[ -n "$safe_prefix" ]]; then
    probe_key="$safe_prefix/$probe_key"
  fi

  run_check "S3 write object (s3api:PutObject)" \
    aws "${base_s3[@]}" s3api put-object --bucket "$bucket" --key "$probe_key" --body "$probe_file" --content-type "text/plain" --output json

  run_check "S3 read object metadata (s3api:HeadObject)" \
    aws "${base_s3[@]}" s3api head-object --bucket "$bucket" --key "$probe_key" --output json

  run_check "S3 delete probe object (s3api:DeleteObject)" \
    aws "${base_s3[@]}" s3api delete-object --bucket "$bucket" --key "$probe_key" --output json

  rm -f "$probe_file"
else
  step "SKIP" "S3 write/delete probe" "pass --write-probe to test PutObject/HeadObject/DeleteObject"
fi

if [[ -n "$cloudfront_distribution_id" ]]; then
  run_check "CloudFront distribution access (cloudfront:GetDistribution)" \
    aws "${base_common[@]}" cloudfront get-distribution --id "$cloudfront_distribution_id" --output json

  if [[ "$create_invalidation_probe" -eq 1 ]]; then
    run_check "CloudFront invalidation probe (cloudfront:CreateInvalidation)" \
      aws "${base_common[@]}" cloudfront create-invalidation --distribution-id "$cloudfront_distribution_id" --paths "$invalidation_path" --output json
  else
    step "SKIP" "CloudFront invalidation probe" "pass --create-invalidation-probe to create a real invalidation"
  fi
fi

echo
if [[ "$failures" -gt 0 ]]; then
  step "FAIL" "permission check completed" "$failures check(s) failed"
  exit 1
fi

step "OK" "permission check completed"

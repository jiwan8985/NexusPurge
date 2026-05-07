#!/usr/bin/env bash
set -euo pipefail

BUCKET="${BUCKET:-nexuspurge-it}"
ENDPOINT="${ENDPOINT:-http://localhost:4566}"
REGION="${REGION:-us-east-1}"
WORKDIR="${WORKDIR:-/tmp/nexuspurge-localstack-it}"

if ! command -v docker >/dev/null 2>&1; then
  echo "docker is required" >&2
  exit 1
fi

AWS_CMD=(aws --endpoint-url "$ENDPOINT" --region "$REGION")
if command -v awslocal >/dev/null 2>&1; then
  AWS_CMD=(awslocal)
fi

if ! docker ps --format '{{.Names}}' | grep -qx localstack; then
  docker run -d -p 4566:4566 -e SERVICES=s3 --name localstack localstack/localstack:4.4.0 >/dev/null
fi

mkdir -p "$WORKDIR"
printf 'hello nexuspurge\n' > "$WORKDIR/hello.txt"
printf 'changed nexuspurge\n' > "$WORKDIR/hello-changed.txt"

"${AWS_CMD[@]}" s3api create-bucket --bucket "$BUCKET" >/dev/null 2>&1 || true
"${AWS_CMD[@]}" s3 cp "$WORKDIR/hello.txt" "s3://$BUCKET/hello.txt" >/dev/null
"${AWS_CMD[@]}" s3api head-object --bucket "$BUCKET" --key hello.txt >/dev/null
"${AWS_CMD[@]}" s3 cp "s3://$BUCKET/hello.txt" "$WORKDIR/downloaded.txt" >/dev/null
cmp "$WORKDIR/hello.txt" "$WORKDIR/downloaded.txt"
"${AWS_CMD[@]}" s3 cp "$WORKDIR/hello-changed.txt" "s3://$BUCKET/hello.txt" >/dev/null
"${AWS_CMD[@]}" s3 rm "s3://$BUCKET/hello.txt" >/dev/null

echo "LocalStack S3 integration passed: bucket=$BUCKET endpoint=$ENDPOINT"

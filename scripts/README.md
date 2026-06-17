# NexusPurge helper scripts

Convenience wrappers for common development and release tasks. Managed background
process logs and pid files are written to `.logs/` in the repository root.

## macOS

```bash
chmod +x scripts/nexus.sh
./scripts/nexus.sh install
./scripts/nexus.sh tauri
./scripts/nexus.sh logs tauri -f
./scripts/nexus.sh stop all
```

## Windows PowerShell

```powershell
.\scripts\nexus.ps1 install
.\scripts\nexus.ps1 tauri
.\scripts\nexus.ps1 logs tauri -f
.\scripts\nexus.ps1 stop all
.\scripts\nexus.ps1 aws-check -Bucket my-bucket -Region ap-northeast-2 -WriteProbe
```

If script execution is blocked on Windows, run the script with a process-scoped
policy:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\nexus.ps1 help
```

## Commands

| Command | Description |
| --- | --- |
| `install` | Install frontend dependencies with `pnpm install`. |
| `dev` | Start the Vite dev server in the background. |
| `tauri` | Start the full Tauri desktop app in the background. |
| `preview` | Start the Vite preview server in the background. |
| `stop [name\|all]` | Stop `dev`, `tauri`, `preview`, or all managed processes. |
| `restart <name>` | Restart `dev`, `tauri`, or `preview`. |
| `status` | Show managed process status. |
| `logs [name] [-f]` | Show logs for `dev`, `tauri`, `preview`, or the latest log. |
| `build` | Run `pnpm run build`. |
| `tauri-build` | Run `pnpm tauri build`. |
| `test` | Run `pnpm test`. |
| `check` | Run `pnpm run build` and Rust backend tests. |
| `cargo-check` | Run Rust backend compilation checks. |
| `cargo-test` | Run Rust backend tests. |
| `aws-check` | Validate AWS identity and S3/CloudFront permissions. |
| `localstack` | Run the LocalStack integration script. |
| `clean-logs` | Remove managed helper logs and pid files. |

## AWS/S3 Permission Check

The AWS check scripts use the AWS CLI and do not read app profiles or stored
secrets. Configure credentials through the normal AWS CLI mechanisms first:
environment variables, `aws configure`, SSO, or `--profile`.

PowerShell:

```powershell
.\scripts\nexus.ps1 aws-check -Bucket my-bucket -Region ap-northeast-2
.\scripts\nexus.ps1 aws-check -Bucket my-bucket -Profile dev -Prefix static -WriteProbe
.\scripts\nexus.ps1 aws-check -Bucket my-bucket -CloudFrontDistributionId E1234567890
```

Bash:

```bash
./scripts/nexus.sh aws-check --bucket my-bucket --region ap-northeast-2
./scripts/nexus.sh aws-check --bucket my-bucket --profile dev --prefix static --write-probe
./scripts/nexus.sh aws-check --bucket my-bucket --cloudfront-distribution-id E1234567890
```

Default checks are read-only: `sts:GetCallerIdentity`, `s3:HeadBucket`,
`s3:GetBucketLocation`, and `s3:ListBucket`. Add `-WriteProbe` or
`--write-probe` to verify `s3:PutObject`, `s3:GetObject` metadata access, and
`s3:DeleteObject` using a temporary object. CloudFront invalidation is skipped
by default because it creates a real invalidation; opt in with
`-CreateInvalidationProbe` or `--create-invalidation-probe`.

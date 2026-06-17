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
| `localstack` | Run the LocalStack integration script. |
| `clean-logs` | Remove managed helper logs and pid files. |

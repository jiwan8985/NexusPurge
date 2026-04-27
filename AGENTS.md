# Repository Guidelines

## Project Structure & Module Organization

NexusPurge is a Tauri 2 desktop app with a Vite React frontend and Rust backend. Frontend code lives in `src/`: `components/` for UI, `hooks/` for Tauri/API orchestration, `store/appStore.ts` for Zustand state, `types/index.ts` for shared TypeScript models, and `styles/` for global tokens. Backend code lives in `src-tauri/src/`: `commands/` exposes Tauri IPC handlers, `adapters/storage/` and `adapters/cdn/` contain provider implementations, and `utils/` holds hashing, config, and signing helpers. App icons and Tauri configuration are under `src-tauri/`.

## Build, Test, and Development Commands

- `npm install`: install frontend and Tauri CLI dependencies.
- `npm run dev`: run the Vite frontend only.
- `npm run build`: type-check TypeScript with `tsc`, then build the web assets.
- `npm run tauri dev`: run the full desktop app in development mode.
- `npm run tauri build`: create release bundles under `src-tauri/target/release/bundle/`.
- `cargo check --manifest-path src-tauri/Cargo.toml`: quickly validate Rust backend compilation.

## Coding Style & Naming Conventions

Use TypeScript, React function components, and CSS Modules (`*.module.css`) for component styling. Keep direct `invoke()` calls inside hooks instead of UI components. Centralize cross-boundary types in `src/types/index.ts` and keep them aligned with Rust `serde` structs. Use two-space indentation in TypeScript/CSS and standard `rustfmt` formatting for Rust. Prefer descriptive names such as `useTransfer`, `ProfileModal`, `S3Adapter`, and `build_sync_plan`.

## Testing Guidelines

There is no automated test script configured yet. Before opening a PR, run `npm run build` and `cargo check --manifest-path src-tauri/Cargo.toml`. For integration behavior, follow `TEST_GUIDE.md` with LocalStack or an AWS test bucket, covering profile connection, S3 browse/upload/download, smart sync decisions, and CDN purge where credentials allow. Name future tests after the unit under test, for example `hash_tests.rs` or `useTransfer.test.ts`.

## Commit & Pull Request Guidelines

Recent commits use short, outcome-focused summaries. Keep that style and describe the completed change rather than the implementation details. Pull requests should include a short description, commands run, manual test environment, linked issue if available, and screenshots or recordings for UI changes. Call out configuration or credential-handling changes explicitly.

## Security & Configuration Tips

Do not commit AWS credentials, bucket-specific secrets, or generated release artifacts. Profiles store metadata locally and secrets through the OS keyring; preserve that boundary when changing profile code. Keep custom endpoints, regions, and bucket names configurable for LocalStack and non-production testing.

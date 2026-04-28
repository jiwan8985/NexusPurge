# Repository Guidelines

## Project Structure & Module Organization

NexusPurge is a Tauri 2 desktop app with a Vite React frontend and Rust backend. Frontend code lives in `src/`: `components/` for UI, `hooks/` for Tauri/API orchestration, `store/appStore.ts` for Zustand state, `types/index.ts` for shared TypeScript models, and `styles/` for global tokens. Backend code lives in `src-tauri/src/`: `commands/` exposes Tauri IPC handlers, `adapters/storage/` and `adapters/cdn/` contain provider implementations, and `utils/` holds hashing, config, and signing helpers. App icons and Tauri configuration are under `src-tauri/`.

## Build, Test, and Development Commands

- `pnpm install`: install frontend and Tauri CLI dependencies.
- `pnpm run dev`: run the Vite frontend only.
- `pnpm run build`: type-check TypeScript with `tsc`, then build the web assets.
- `pnpm test`: run Vitest unit tests (jsdom environment, one-shot).
- `pnpm test:watch`: run Vitest in watch mode.
- `pnpm tauri dev`: run the full desktop app in development mode.
- `pnpm tauri build`: create release bundles under `src-tauri/target/release/bundle/`.
- `cargo check --manifest-path src-tauri/Cargo.toml`: quickly validate Rust backend compilation.
- `cargo test --manifest-path src-tauri/Cargo.toml`: run Rust unit tests.

## Coding Style & Naming Conventions

Use TypeScript, React function components, and CSS Modules (`*.module.css`) for component styling. Keep direct `invoke()` calls inside hooks instead of UI components. Centralize cross-boundary types in `src/types/index.ts` and keep them aligned with Rust `serde` structs. Use two-space indentation in TypeScript/CSS and standard `rustfmt` formatting for Rust. Prefer descriptive names such as `useTransfer`, `ProfileModal`, `S3Adapter`, and `build_sync_plan`.

## Testing Guidelines

Before opening a PR, run `pnpm test`, `pnpm build`, and `cargo test --manifest-path src-tauri/Cargo.toml`. These three commands are also enforced by the GitHub Actions CI workflow (`.github/workflows/ci.yml`).

Automated unit tests live in `src/test/` (Vitest + jsdom) and in `#[cfg(test)]` modules inside Rust source files. Name new frontend tests after the unit under test (e.g., `useTransfer.test.ts`) and place them in `src/test/`. Rust tests go in a `mod tests { ... }` block at the bottom of the relevant module.

For integration behavior that cannot be covered by unit tests, follow `TEST_GUIDE.md` with LocalStack or an AWS test bucket, covering profile connection, S3 browse/upload/download, smart sync decisions, sync preview dialog, and CDN purge where credentials allow.

## Commit & Pull Request Guidelines

Recent commits use short, outcome-focused summaries. Keep that style and describe the completed change rather than the implementation details. Pull requests should include a short description, commands run, manual test environment, linked issue if available, and screenshots or recordings for UI changes. Call out configuration or credential-handling changes explicitly.

## Security & Configuration Tips

Do not commit AWS credentials, bucket-specific secrets, or generated release artifacts. Profiles store metadata locally and secrets through the OS keyring; preserve that boundary when changing profile code. Keep custom endpoints, regions, and bucket names configurable for LocalStack and non-production testing.

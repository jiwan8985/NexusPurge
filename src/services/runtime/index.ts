import { desktopRuntime } from "./desktop-runtime";
import { webRuntime } from "./web-runtime";

export type { RuntimeBridge } from "./runtime-types";

const isTauriRuntime = "__TAURI_INTERNALS__" in window;

export const runtime = isTauriRuntime ? desktopRuntime : webRuntime;

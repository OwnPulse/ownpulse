/// <reference types="vite/client" />

// Injected at build time by Vite's `define` (vite.config.ts). Holds the git
// short SHA of the build, or "dev" when git is unavailable.
declare const __APP_VERSION__: string;

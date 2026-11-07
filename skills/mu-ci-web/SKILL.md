---
name: mu-ci-web
description: "Build and style the mu-ci-web Leptos WASM SPA frontend with dual-layer static caching and mobile viewport compliance."
---

# `mu-ci-web` Leptos WASM & Mobile Viewport Invariants

## 1. Dual-Layer Static Asset Caching

- HTML entry points (`index.html`) must be served with `Cache-Control: no-cache, no-store, must-revalidate`.
- WASM, JS, and CSS assets must use content-hash URL busting (`?v=<hash>`) paired with `Cache-Control: no-cache, must-revalidate`.

## 2. Mobile Viewport Compliance

- Segmented controls must remain contained with horizontal scrolling on narrow viewports.
- All form inputs (`<input>`, `<select>`, `<textarea>`) must specify `font-size: 16px` (`1rem`) to prevent iOS Safari auto-zoom on focus.

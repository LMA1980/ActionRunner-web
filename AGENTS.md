# ActionRunner-web Development & Deployment Guidelines

## 1. Branching & Pull Request Workflow (feat/**)

- **Feature Branch Invariant**: All changes intended for `main` must be developed on a dedicated `feat/**` branch (e.g., `feat/<feature-name>`) and merged via Pull Request.
- **No Direct Commits to `main`**: Never commit or push directly to the `main` branch.
- **Pull Request Creation**: Push the feature branch to `origin` and open a Pull Request using `gh pr create` with a descriptive summary of changes.

## 2. Mobile & iOS WebKit Viewport Guidelines

- **Contained Segmented Controls**: All tab bars, filter bars, and button groups must have `max-width: 100%`, `box-sizing: border-box`, and enable smooth touch scrolling (`overflow-x: auto; -webkit-overflow-scrolling: touch; scrollbar-width: none;`). They must never overflow container cards or viewports on narrow devices (e.g., 375px–402px wide).
- **Responsive Labels**: On mobile viewports (`@media (max-width: 640px)`), provide concise, shortened labels for segmented buttons (e.g., `"Skipped"` instead of `"Skipped / Coalesced"`) so options fit cleanly without clipping.
- **Prevent iOS Safari Auto-Zoom**: All form controls (`<input>`, `<select>`, `<textarea>`) must have `font-size: 16px` on mobile screens (`@media (max-width: 768px)`). Sub-16px font sizes trigger unwanted automatic zoom on iOS Safari upon focus.
- **Safe Area Inset Handling**: Pages must specify `<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover" />` and apply `env(safe-area-inset-top)` and `env(safe-area-inset-bottom)` to account for Dynamic Island, display cutouts, and home indicator bars.

## 3. WebAssembly & Static SPA Caching Architecture

- **Dual-Layer Cache Invariant**: WebAssembly applications must implement both server-side revalidation headers and build-time cache busting:
  1. **HTML Entry Point (`/` and `/index.html`)**: Must NEVER be cached by browsers:
     ```nginx
     add_header Cache-Control "no-cache, no-store, must-revalidate" always;
     add_header Pragma "no-cache" always;
     add_header Expires "0" always;
     ```
  2. **WASM, JS, & CSS Assets**: Must enforce conditional revalidation with ETags:
     ```nginx
     etag on;
     add_header Cache-Control "no-cache, must-revalidate" always;
     ```
  3. **Build-Time Content Hashing**: Build scripts (`scripts/build-dist.sh`) must compute a SHA-256 content hash of the compiled `.wasm` binary and inject versioned query parameters (`?v=<hash>`) into `dist/index.html` for CSS, JS, and WASM URLs. Never deploy unversioned asset URLs to production.

## 4. Build & Distribution Invariants

- **Distribution Rebuild**: Any changes to `src/`, `style.css`, or `index.html` require executing `scripts/build-dist.sh` to compile the release WASM binary, update `dist/`, and inject version hashes.
- **Verification**: Always verify local compilation with `cargo check --target wasm32-unknown-unknown` and unit tests with `cargo test` prior to opening a PR.

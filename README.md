# ActionRunner-web

<p align="center">
  <a href="https://hook.lma1980.duckdns.org/jobs">
    <img src="https://hook.lma1980.duckdns.org/cards/LMA1980/ActionRunner-web/card.svg" alt="ActionRunner-web Factory Scorecard" width="520"/>
  </a>
</p>

<p align="center">
  <a href="https://hook.lma1980.duckdns.org/jobs"><img src="https://hook.lma1980.duckdns.org/badges/LMA1980/ActionRunner-web/status.svg" alt="CI Status"/></a>
  <a href="https://hook.lma1980.duckdns.org/jobs"><img src="https://hook.lma1980.duckdns.org/badges/LMA1980/ActionRunner-web/score.svg" alt="Quality Score"/></a>
  <a href="https://hook.lma1980.duckdns.org/jobs"><img src="https://hook.lma1980.duckdns.org/badges/LMA1980/ActionRunner-web/coverage.svg" alt="Coverage"/></a>
  <a href="https://hook.lma1980.duckdns.org/jobs"><img src="https://hook.lma1980.duckdns.org/badges/LMA1980/ActionRunner-web/benchmark.svg" alt="Benchmark"/></a>
</p>

---

High-performance WebAssembly queue monitoring and job drill-down frontend built with **[Leptos](https://github.com/leptos-rs/leptos) (v0.8.22 CSR)** for the `ActionRunner-rs` CI execution sidecar on SpacemiT K3 RISC-V hardware.

---

## Key Features

- **Live Webhook Queue Monitor**:
  - Real-time queue depth and execution status (`BUSY (1/1)` / `IDLE (0/1)`).
  - High-visibility spotlight for the currently executing build with live elapsed seconds counter.
  - Ordered FIFO pending queue with wait timers and position indicators (`#1`, `#2`, ...).
  - Coalesced events tracking: indicates when older queued commits were superseded by newer commits for the same repository and ref to protect host compute resources.

- **Deep Job Drill-Down View**:
  - Direct URL navigation (`#/jobs/{id}`) and back navigation.
  - Real-time build status banner, duration, commit SHA, and trigger details.
  - Embedded live vector SVG scorecards (`/cards/{owner}/{repo}/card.svg`).
  - Four factory metric tiles: Tests pass-ratio, Line Coverage (`llvm-cov`), Cargo Micro-Benchmarks, Security & Linter hygiene.
  - Submodules breakdown grid: individual scorecards and metrics for all repository submodules (such as `k3-core`, `k3-engine`, `k3-manager`, dynamically scaling to any number of submodules).
  - Step-by-step log viewer: collapsible accordion view parsing each pipeline step (`cargo fmt`, `cargo clippy`, `cargo audit`, `cargo llvm-cov`, `cargo bench`, release build) with exit codes, timing, and search filter, alongside raw stdout/stderr output.

- **KPI Statistics**:
  - Queue depth, active worker utilization, passing/failing rates, and coalescing counters.

- **K3 Scorecard Theme**:
  - Dark mode aesthetic matching the GitHub native dark palette (`#0d1117`, `#161b22`, `#2ea44f`, `#da3633`, `#e3b341`, `#58a6ff`).

---

## Architecture & Integration with ActionRunner-rs

```
                       [ GitHub Webhook / Developer Browser ]
                                        │
                                        ▼
                             [ Nginx Reverse Proxy ]
                            (hook.lma1980.duckdns.org)
                                  │           │
                  Static SPA      │           │ API, Webhook, Cards & Badges
                  (/, /style.css, │           │ (/webhook, /api/*, /queue,
                   /pkg/*.wasm)   │           │  /cards/*, /badges/*)
                                  ▼           ▼
                         [ ActionRunner-web ] [ ActionRunner-rs ]
                           (Leptos 0.8.22)     (Rust CI Sidecar)
```

- Nginx reverse-proxies `/api/*`, `/queue`, `/webhook`, `/cards/*`, and `/badges/*` to `ActionRunner-rs` running on `127.0.0.1:8000`.
- Nginx serves the compiled Leptos WASM bundle from `/home/lma1980/repository/ActionRunner-web/dist`.
- When an API URL query parameter `?api=https://...` is passed, the frontend automatically points its requests to the specified endpoint.

---

## Building & Verification

### Local Development with Trunk
```bash
# Build and serve locally with hot reloading
trunk serve
```

### Production Release Build
```bash
# Compile optimized WebAssembly release bundle to ./dist
trunk build --release
```

### Native Tests & Linter Hygiene
```bash
# Verify formatting, linter hygiene, and unit tests
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

---

## License

Licensed under the GNU Affero General Public License v3.0 (AGPL-3.0-only). See [LICENSE](LICENSE) for details.

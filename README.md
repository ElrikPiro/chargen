# Chargen

Chargen is a local, offline-first prototype for deterministic population histories. The Rust executable serves its own HTML, CSS and JavaScript interface, stores yearly closes in SQLite and exposes a versioned JSON API. It listens on loopback only.

E0 defines the scenario, time and identity contracts. E1 adds annual demographic evolution, durable history, resumable jobs, branches and historical census/genealogy queries. E2 adds versioned routes, residence history, migration, cultural aggregates, institutional affiliations and organization histories. E3 adds grouped production and markets plus explicitly scheduled, historically recorded events. Current policy rates, economies and catalogs are fictional engineering fixtures; they are not calibrated demographic or economic claims. See the [E1 scope and acceptance criteria](docs/e1-scope.md), [E2 scope](docs/e2-scope.md), [E3 scope](docs/e3-scope.md), [API contract](docs/api-contract.md), and [architecture decisions](docs/architecture-decisions.md). Earlier Python files are retained under [legacy](legacy/); they are not part of the Rust workspace.

The workspace pins Rust 1.90.0. Run the local server with:

```sh
cargo run --locked --package chargen-app -- serve --database ./chargen.sqlite3
```

The default address is `127.0.0.1:7878`; `--bind` accepts loopback addresses only. The database is durable and is the source of truth after a restart. The UI includes E1 and E2 examples plus an E3 economy/event demo with fictional producers, goods and annual ledgers. To execute benchmark profiles, use `cargo run --release --locked --package chargen-app -- bench --profile E3-B1`; the CLI also accepts B0/B1/B2, E2-B0/E2-B1/E2-B2, E2 catalog profiles and E3-B0/E3-B1/E3-B2. E3 profiles use deterministic synthetic profession assignments, recipes, cash endowments and event schedules; their reports include transactions and historical market/event queries. These profiles are reproducible workload definitions, not performance guarantees.

Useful checks are `make check`, `make smoke`, and `make package`. `make package` creates a platform-named `tar.gz` with the executable, this README, the license and documentation, plus an embedded `MANIFEST.json` and a SHA-256 sidecar. GitHub Actions extracts that archive before running the API smoke test; release tags publish Linux and Windows packages, per-package manifests, a consolidated release manifest and `SHA256SUMS.txt`. The executable embeds the static web resources and needs no Node.js, Python or internet connection at runtime. Node.js is used only by repository checks and packaging tools.

The [E0/E1 validation report](docs/validation-e1.md) and [E2 validation report](docs/validation-e2.md) are historical evidence. See [E3 validation](docs/validation-e3.md) for the current economy/event acceptance results.

This project retains its root Coffeeware license in `LICENSE`.

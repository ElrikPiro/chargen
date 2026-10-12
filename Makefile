.PHONY: check fmt test clippy build smoke bench contracts-check web-check package clean

CARGO ?= cargo
NODE ?= node
PROFILE ?= B0
DATABASE ?=

check: fmt test clippy web-check contracts-check

fmt:
	$(CARGO) fmt --all -- --check

test:
	$(CARGO) test --workspace --locked

clippy:
	$(CARGO) clippy --workspace --all-targets --locked -- -D warnings

build:
	$(CARGO) build --release --package chargen-app --locked

web-check:
	$(NODE) --check web/app.js
	$(NODE) --check scripts/check-web.mjs
	$(NODE) --check scripts/smoke-api.mjs
	$(NODE) --check scripts/create-package-manifest.mjs
	$(NODE) --check scripts/check-package.mjs
	$(NODE) --check scripts/write-checksum.mjs
	$(NODE) --check scripts/assemble-release-manifest.mjs
	$(NODE) --check scripts/validate-e1.mjs
	$(NODE) --check scripts/validate-e2.mjs
	$(NODE) scripts/check-web.mjs

contracts-check:
	npx --yes --package=typescript@5.9.3 tsc --project web/tsconfig.json

smoke: build
	$(NODE) scripts/smoke-api.mjs target/release/chargen-app
	$(NODE) scripts/validate-e1.mjs target/release/chargen-app
	$(NODE) scripts/validate-e2.mjs target/release/chargen-app

bench:
	$(CARGO) run --release --package chargen-app --locked -- bench --profile $(PROFILE) $(if $(DATABASE),--database $(DATABASE),)

package: build
	sh scripts/package.sh

clean:
	$(CARGO) clean
	rm -rf dist

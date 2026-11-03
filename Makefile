.DEFAULT_GOAL := help

APP ?= desktop
ARGS ?=
DISK_WARN_GB ?= 25

DESKTOP_DIR := apps/desktop
MOBILE_DIR := apps/mobile
SERVER_ADMIN_DIR := apps/server-admin
WEBSITE_DIR := apps/website
PHOTO_ROOM_DIR := packages/photo-room
COMPANION_API_DIR := packages/companion-api
SHARED_DIR := packages/shared
VIEWER_DIR := packages/viewer
RUST_WORKSPACE := Cargo.toml

.PHONY: help doctor setup install dev build test check check-clean disk-usage disk-guard \
	desktop-install desktop-dev desktop-web-dev desktop-build desktop-test desktop-check desktop-bundle desktop-tauri \
	desktop-icons \
	mobile-install mobile-dev mobile-build mobile-test mobile-check mobile-ios-dev mobile-ios-build mobile-android-dev mobile-android-build mobile-tauri \
	server-admin-install server-admin-dev server-admin-build server-admin-test \
	website-install website-dev website-docs-dev website-build \
	photo-room-install photo-room-test companion-api-install companion-api-test shared-install shared-test viewer-install viewer-test \
	rust-check volumd-check volumd-run volumd-build \
	docker-build docker-up docker-down docker-logs compose-check \
	clean clean-build clean-rust clean-native clean-frontends clean-deps clean-all clean-docker-cache

help: ## Show the available repository commands.
	@echo "Volum repository"
	@echo ""
	@echo "  make setup                         Install every JavaScript package"
	@echo "  make dev APP=desktop               Run an app (desktop, mobile, server-admin, website)"
	@echo "  make build APP=desktop             Build an app frontend"
	@echo "  make test APP=desktop              Test desktop, mobile, photo-room, or all"
	@echo "  make check                         Run the complete local validation suite"
	@echo "  make check-clean                   Validate, then remove generated build output"
	@echo "  make disk-usage                    Report repository build/cache usage"
	@echo "  make clean                         Remove generated build output"
	@echo "  make clean-all                     Also remove reinstallable node_modules"
	@echo ""
	@echo "Desktop"
	@echo "  make desktop-dev                   Run the Tauri desktop app"
	@echo "  make desktop-web-dev               Run the browser-only desktop preview"
	@echo "  make desktop-bundle ARGS='...'     Build a platform Tauri bundle"
	@echo "  make desktop-tauri ARGS='...'      Pass arguments directly to Tauri"
	@echo "  make desktop-icons                 Regenerate platform icons"
	@echo ""
	@echo "Mobile"
	@echo "  make mobile-dev                    Run the browser preview"
	@echo "  make mobile-ios-dev ARGS='...'     Run the iOS app"
	@echo "  make mobile-ios-build ARGS='...'   Build the iOS app"
	@echo "  make mobile-android-dev ARGS='...' Run the Android app"
	@echo "  make mobile-android-build ARGS='...' Build the Android app"
	@echo ""
	@echo "Server and website"
	@echo "  make server-admin-dev              Run the browser administrator"
	@echo "  make volumd-run ARGS='...'         Run the headless server"
	@echo "  make website-dev                   Run the landing page and docs"
	@echo "  make website-docs-dev              Run only VitePress"
	@echo "  make docker-build|docker-up|docker-down|docker-logs"
	@echo ""
	@echo "Cleanup"
	@echo "  make disk-usage                    Show generated/cache usage by area"
	@echo "  make clean-rust                    Remove Cargo target directories"
	@echo "  make clean-native                  Remove native caches; preserve xcarchives"
	@echo "  make clean-frontends               Remove dist and test output"
	@echo "  make clean-deps                    Remove node_modules (run make setup later)"
	@echo "  make clean-all                     Remove build output and dependencies"
	@echo "  make clean-docker-cache            Interactively prune global Docker build cache"

doctor: ## Show required and optional development tools.
	@command -v node >/dev/null && node --version || { echo "node: missing"; exit 1; }
	@command -v npm >/dev/null && npm --version || { echo "npm: missing"; exit 1; }
	@command -v rustc >/dev/null && rustc --version || { echo "rustc: missing"; exit 1; }
	@command -v cargo >/dev/null && cargo --version || { echo "cargo: missing"; exit 1; }
	@command -v docker >/dev/null && docker --version || echo "docker: optional, not installed"

setup: companion-api-install shared-install viewer-install photo-room-install desktop-install mobile-install server-admin-install website-install ## Install all packages.

install: ## Install one package with APP=desktop|mobile|server-admin|website|photo-room|all.
	@if [ "$(APP)" = "all" ]; then $(MAKE) --no-print-directory setup; \
	else $(MAKE) --no-print-directory "$(APP)-install"; fi

dev: ## Run one app with APP=desktop|mobile|server-admin|website.
	@case "$(APP)" in desktop|mobile|server-admin|website) ;; *) echo "Unsupported APP for dev: $(APP)"; exit 2;; esac
	@$(MAKE) --no-print-directory "$(APP)-dev" ARGS="$(ARGS)"

build: ## Build one app with APP=desktop|mobile|server-admin|website.
	@case "$(APP)" in desktop|mobile|server-admin|website) ;; *) echo "Unsupported APP for build: $(APP)"; exit 2;; esac
	@$(MAKE) --no-print-directory "$(APP)-build" ARGS="$(ARGS)"

test: ## Test one package with APP=desktop|mobile|server-admin|photo-room|all.
	@if [ "$(APP)" = "all" ]; then \
		$(MAKE) --no-print-directory companion-api-test shared-test viewer-test desktop-test mobile-test server-admin-test photo-room-test; \
	else \
		case "$(APP)" in desktop|mobile|server-admin|photo-room|companion-api|shared|viewer) ;; *) echo "Unsupported APP for test: $(APP)"; exit 2;; esac; \
		$(MAKE) --no-print-directory "$(APP)-test"; \
	fi

check: companion-api-test shared-test viewer-test desktop-check mobile-check server-admin-test server-admin-build website-build photo-room-test rust-check volumd-check ## Validate all source packages.
	@$(MAKE) --no-print-directory disk-guard

check-clean: ## Validate all packages and always remove generated build output afterward.
	@status=0; \
	$(MAKE) --no-print-directory check || status=$$?; \
	clean_status=0; \
	$(MAKE) --no-print-directory clean-build || clean_status=$$?; \
	if [ $$status -ne 0 ]; then exit $$status; fi; \
	exit $$clean_status

desktop-install:
	npm ci --prefix $(DESKTOP_DIR)

desktop-dev:
	npm --prefix $(DESKTOP_DIR) run tauri -- dev $(ARGS)

desktop-web-dev:
	npm --prefix $(DESKTOP_DIR) run dev -- $(ARGS)

desktop-build:
	npm --prefix $(DESKTOP_DIR) run build
	npm --prefix $(DESKTOP_DIR) run budget

desktop-test:
	npm --prefix $(DESKTOP_DIR) test

desktop-check: desktop-build desktop-test

desktop-bundle:
	npm --prefix $(DESKTOP_DIR) run tauri -- build $(ARGS)

desktop-tauri:
	npm --prefix $(DESKTOP_DIR) run tauri -- $(ARGS)

desktop-icons:
	cd $(DESKTOP_DIR) && npx tauri icon ../../assets/brand/volum-appicon.png

mobile-install:
	npm ci --prefix $(MOBILE_DIR)

mobile-dev:
	npm --prefix $(MOBILE_DIR) run dev -- $(ARGS)

mobile-build:
	npm --prefix $(MOBILE_DIR) run build
	npm --prefix $(MOBILE_DIR) run budget

mobile-test:
	npm --prefix $(MOBILE_DIR) test

mobile-check: mobile-build mobile-test

mobile-ios-dev:
	npm --prefix $(MOBILE_DIR) run tauri -- ios dev $(ARGS)

mobile-ios-build:
	npm --prefix $(MOBILE_DIR) run tauri -- ios build $(ARGS)

mobile-android-dev:
	npm --prefix $(MOBILE_DIR) run tauri -- android dev $(ARGS)

mobile-android-build:
	npm --prefix $(MOBILE_DIR) run tauri -- android build $(ARGS)

mobile-tauri:
	npm --prefix $(MOBILE_DIR) run tauri -- $(ARGS)

server-admin-install:
	npm ci --prefix $(SERVER_ADMIN_DIR)

server-admin-dev:
	npm --prefix $(SERVER_ADMIN_DIR) run dev -- $(ARGS)

server-admin-build:
	npm --prefix $(SERVER_ADMIN_DIR) run build
	npm --prefix $(SERVER_ADMIN_DIR) run budget

server-admin-test:
	npm --prefix $(SERVER_ADMIN_DIR) test

website-install:
	npm ci --prefix $(WEBSITE_DIR)

website-dev:
	npm --prefix $(WEBSITE_DIR) run dev -- $(ARGS)

website-docs-dev:
	npm --prefix $(WEBSITE_DIR) run dev:docs -- $(ARGS)

website-build:
	npm --prefix $(WEBSITE_DIR) run build

photo-room-install:
	npm ci --prefix $(PHOTO_ROOM_DIR)

photo-room-test:
	npm --prefix $(PHOTO_ROOM_DIR) test

companion-api-install:
	npm ci --prefix $(COMPANION_API_DIR)

companion-api-test:
	npm --prefix $(COMPANION_API_DIR) run generate:check
	npm --prefix $(COMPANION_API_DIR) test

shared-install:
	npm ci --prefix $(SHARED_DIR)

shared-test:
	npm --prefix $(SHARED_DIR) test

viewer-install:
	npm ci --prefix $(VIEWER_DIR)

viewer-test:
	npm --prefix $(VIEWER_DIR) test

rust-check:
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --all-features -- -D warnings
	cargo test --workspace --all-features

volumd-check:
	cargo clippy -p volum-server -p volumd --all-targets --no-default-features -- -D warnings
	cargo test -p volum-server --no-default-features
	@if cargo tree -p volumd -e normal | grep -Ei '(tauri|wry|webkit|gtk|keyring|dbus|zbus|secret-service)'; then \
		echo "volumd unexpectedly depends on a desktop or credential-store runtime"; \
		exit 1; \
	fi

volumd-run:
	cargo run -p volumd -- $(ARGS)

volumd-build:
	cargo build -p volumd $(ARGS)

docker-build:
	docker build -t volum-server:local .

docker-up:
	docker compose up -d --build

docker-down:
	docker compose down

docker-logs:
	docker compose logs -f volumd

compose-check:
	VOLUM_PUBLIC_URL=http://100.64.0.1:43821 docker compose config -q

disk-usage: ## Report generated data that can make the checkout large.
	@echo "Volum generated data"
	@for path in \
		target \
		$(DESKTOP_DIR)/src-tauri/target \
		$(MOBILE_DIR)/src-tauri/target \
		$(MOBILE_DIR)/src-tauri/gen/android \
		$(MOBILE_DIR)/src-tauri/gen/apple \
		$(DESKTOP_DIR)/node_modules \
		$(MOBILE_DIR)/node_modules \
		$(SERVER_ADMIN_DIR)/node_modules \
		$(WEBSITE_DIR)/node_modules \
		$(PHOTO_ROOM_DIR)/node_modules \
		$(COMPANION_API_DIR)/node_modules \
		$(SHARED_DIR)/node_modules \
		$(VIEWER_DIR)/node_modules \
		$(DESKTOP_DIR)/dist \
		$(MOBILE_DIR)/dist \
		$(SERVER_ADMIN_DIR)/dist \
		$(WEBSITE_DIR)/dist; do \
		if [ -e "$$path" ]; then du -sh "$$path"; fi; \
	done | sort -h
	@total_kb=$$(du -sk \
		target \
		$(DESKTOP_DIR)/src-tauri/target \
		$(MOBILE_DIR)/src-tauri/target \
		$(MOBILE_DIR)/src-tauri/gen/android \
		$(MOBILE_DIR)/src-tauri/gen/apple \
		$(DESKTOP_DIR)/node_modules \
		$(MOBILE_DIR)/node_modules \
		$(SERVER_ADMIN_DIR)/node_modules \
		$(WEBSITE_DIR)/node_modules \
		$(PHOTO_ROOM_DIR)/node_modules \
		$(COMPANION_API_DIR)/node_modules \
		$(SHARED_DIR)/node_modules \
		$(VIEWER_DIR)/node_modules 2>/dev/null | awk '{ total += $$1 } END { print total + 0 }'); \
	awk -v kb="$$total_kb" 'BEGIN { printf "Total generated/cache data: %.1f GiB\n", kb / 1048576 }'

disk-guard: ## Warn when generated Rust/native output exceeds DISK_WARN_GB.
	@total_kb=$$(du -sk \
		target \
		$(DESKTOP_DIR)/src-tauri/target \
		$(MOBILE_DIR)/src-tauri/target \
		$(MOBILE_DIR)/src-tauri/gen/android \
		$(MOBILE_DIR)/src-tauri/gen/apple 2>/dev/null | awk '{ total += $$1 } END { print total + 0 }'); \
	limit_kb=$$(( $(DISK_WARN_GB) * 1024 * 1024 )); \
	if [ "$$total_kb" -gt "$$limit_kb" ]; then \
		total_gib=$$(awk -v kb="$$total_kb" 'BEGIN { printf "%.1f", kb / 1048576 }'); \
		echo "warning: generated Rust/native output is $${total_gib} GiB (limit: $(DISK_WARN_GB) GiB)"; \
		echo "run 'make disk-usage' to inspect it or 'make clean' to remove it"; \
	fi

clean: clean-build ## Remove generated Rust, native, and frontend build output.

clean-build: clean-rust clean-native clean-frontends ## Remove all regenerable build output.
	@$(MAKE) --no-print-directory disk-usage

clean-rust: ## Remove root and legacy app-local Cargo target directories.
	cargo clean --manifest-path $(RUST_WORKSPACE)
	rm -rf $(DESKTOP_DIR)/src-tauri/target $(MOBILE_DIR)/src-tauri/target

clean-native: ## Remove mobile native caches while preserving source and xcarchives.
	rm -rf \
		$(MOBILE_DIR)/src-tauri/gen/android/.gradle \
		$(MOBILE_DIR)/src-tauri/gen/android/.kotlin \
		$(MOBILE_DIR)/src-tauri/gen/android/.tauri \
		$(MOBILE_DIR)/src-tauri/gen/android/build \
		$(MOBILE_DIR)/src-tauri/gen/android/buildSrc/.gradle \
		$(MOBILE_DIR)/src-tauri/gen/android/buildSrc/build \
		$(MOBILE_DIR)/src-tauri/gen/android/app/build \
		$(MOBILE_DIR)/src-tauri/gen/android/app/.cxx \
		$(MOBILE_DIR)/src-tauri/gen/apple/.tauri \
		$(MOBILE_DIR)/src-tauri/gen/apple/Externals
	@if [ -d "$(MOBILE_DIR)/src-tauri/gen/apple/build" ]; then \
		find "$(MOBILE_DIR)/src-tauri/gen/apple/build" -mindepth 1 -maxdepth 1 ! -name '*.xcarchive' -exec rm -rf {} +; \
	fi

clean-frontends: ## Remove frontend builds and test reports.
	rm -rf \
		$(DESKTOP_DIR)/dist \
		$(MOBILE_DIR)/dist \
		$(SERVER_ADMIN_DIR)/dist \
		$(WEBSITE_DIR)/dist \
		$(MOBILE_DIR)/test-results \
		playwright-report \
		test-results

clean-deps: ## Remove reinstallable JavaScript dependencies; restore with make setup.
	rm -rf \
		$(DESKTOP_DIR)/node_modules \
		$(MOBILE_DIR)/node_modules \
		$(SERVER_ADMIN_DIR)/node_modules \
		$(WEBSITE_DIR)/node_modules \
		$(PHOTO_ROOM_DIR)/node_modules \
		$(COMPANION_API_DIR)/node_modules \
		$(SHARED_DIR)/node_modules \
		$(VIEWER_DIR)/node_modules

clean-all: clean-build clean-deps ## Remove build output and JavaScript dependencies, never private data.

clean-docker-cache: ## Interactively prune unused global Docker build cache.
	@echo "This affects unused Docker build cache from every local project."
	docker builder prune $(ARGS)

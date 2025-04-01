SHELL := /usr/bin/env bash
.SHELLFLAGS = -euo pipefail -c

ROOT_DIR := $(shell dirname $(realpath $(firstword $(MAKEFILE_LIST))))

PROFILE ?= release

TEST_FLAGS ?= -j4
LINT_FLAGS ?= -Dwarnings

PREFIX ?= /usr
BINDIR ?= $(PREFIX)/bin

GIT_SHA ?= $(shell git rev-parse HEAD)
export GIT_SHA

.PHONY: clean test install build lint all

ifeq ($(PROFILE),dev)
target_dir := $(ROOT_DIR)/target/debug
else
target_dir := $(ROOT_DIR)/target/$(PROFILE)
endif

test:
	@echo "Running tests with flags: ${TEST_FLAGS}"
	@cargo test ${TEST_FLAGS}

build:
	@echo "Building project with profile: ${PROFILE}"
	@cargo build --profile $(PROFILE)
	@echo "Build complete."

install: build
	@echo "Installing chatty to $(DESTDIR)$(BINDIR)"
	@install -m 0755 -d $(DESTDIR)$(BINDIR)
	@install -m 0755 $(target_dir)/chatty $(DESTDIR)$(BINDIR)

clean:
	@echo "Cleaning project..."
	@cargo clean
	@echo "Clean complete."

all: install

lint:
	@echo "Running linter with flags: ${LINT_FLAGS}"
	@cargo clippy -- ${LINT_FLAGS}

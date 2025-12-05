# =========================================================
#  Minimal Makefile for Docker Compose stack control
#  ---------------------------------------------------------
#  Usage:
#    make init-dc       # one-shot init + up (idempotent)
#    make status-dc     # show stack status
#  ---------------------------------------------------------
#  .env file (same directory as this Makefile) is auto-loaded
#  and not committed to git (.env ignored, example.env tracked)
# =========================================================

SHELL := /usr/bin/env bash
STACK := scripts/compose.sh

# --- Auto-load .env if exists ---
ifneq (,$(wildcard .env))
  include .env
  export $(shell sed 's/=.*//' .env)
endif

# --- Default values (can be overridden via .env) ---
PROJECT_NAME ?= vms
PROFILE      ?= compose
export PROJECT_NAME PROFILE

.PHONY: init-dc status-dc ensure-scripts-permission

ensure-scripts-permission:
	@if [ ! -x "$(STACK)" ]; then \
		echo "Granting execute permission to $(STACK)"; \
		chmod +x "$(STACK)"; \
	fi

init-dc: ensure-scripts-permission
	@$(STACK) init

status-dc: ensure-scripts-permission
	@$(STACK) status

launch:
	HLS_ROOT=./data/hls cargo run -p stream-node

test:
	cargo test
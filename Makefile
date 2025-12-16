# =========================================================
#  Quadrant VMS - Makefile
#  ---------------------------------------------------------
#  Development and deployment commands
# =========================================================

SHELL := /usr/bin/env bash

# --- Auto-load .env if exists ---
ifneq (,$(wildcard .env))
  include .env
  export $(shell sed 's/=.*//' .env)
endif

# --- Default values ---
PROJECT_NAME ?= quadrant-vms
COMPOSE_FILE ?= docker-compose.yml

.PHONY: help test build launch

# =========================================================
# Help
# =========================================================

help:
	@echo "Quadrant VMS - Available Commands"
	@echo ""
	@echo "Development:"
	@echo "  make test              - Run all tests"
	@echo "  make build             - Build all crates (release mode)"
	@echo "  make launch            - Launch stream-node locally"
	@echo ""
	@echo "Docker Deployment:"
	@echo "  make docker-build      - Build all Docker images"
	@echo "  make docker-up         - Start all services"
	@echo "  make docker-down       - Stop all services"
	@echo "  make docker-restart    - Restart all services"
	@echo "  make docker-logs       - View logs (all services)"
	@echo "  make docker-status     - Show service status"
	@echo "  make docker-clean      - Remove all containers and volumes"
	@echo ""
	@echo "Docker Service Management:"
	@echo "  make docker-ps         - List running containers"
	@echo "  make logs-coordinator  - View coordinator logs"
	@echo "  make logs-gateway      - View admin-gateway logs"
	@echo "  make logs-ui           - View operator-ui logs"
	@echo ""
	@echo "Utility:"
	@echo "  make docker-init       - Initialize .env from .env.docker"
	@echo "  make docker-shell SVC=<service> - Open shell in service container"
	@echo ""

# =========================================================
# Development Commands
# =========================================================

test:
	cargo test

build:
	cargo build --release

launch:
	HLS_ROOT=./data/hls cargo run -p stream-node

# =========================================================
# Docker Compose Commands
# =========================================================

docker-init:
	@if [ ! -f .env ]; then \
		echo "Creating .env from .env.docker..."; \
		cp .env.docker .env; \
		echo "✅ .env created. Please review and update configuration."; \
	else \
		echo "⚠️  .env already exists. Skipping initialization."; \
	fi

docker-build:
	docker compose -f $(COMPOSE_FILE) build

docker-up: docker-init
	docker compose -f $(COMPOSE_FILE) up -d
	@echo ""
	@echo "✅ All services started!"
	@echo "   Operator UI:    http://localhost:8090"
	@echo "   Admin Gateway:  http://localhost:8081"
	@echo "   MinIO Console:  http://localhost:9001"
	@echo ""
	@echo "Run 'make docker-logs' to view logs"

docker-down:
	docker compose -f $(COMPOSE_FILE) down

docker-restart:
	docker compose -f $(COMPOSE_FILE) restart

docker-logs:
	docker compose -f $(COMPOSE_FILE) logs -f

docker-status:
	docker compose -f $(COMPOSE_FILE) ps

docker-ps:
	docker compose -f $(COMPOSE_FILE) ps

docker-clean:
	@echo "⚠️  This will remove all containers, volumes, and data. Continue? [y/N]"
	@read -r response && [ "$$response" = "y" ] || (echo "Cancelled."; exit 1)
	docker compose -f $(COMPOSE_FILE) down -v
	@echo "✅ All containers and volumes removed"

# =========================================================
# Service-Specific Log Commands
# =========================================================

logs-coordinator:
	docker compose -f $(COMPOSE_FILE) logs -f coordinator

logs-gateway:
	docker compose -f $(COMPOSE_FILE) logs -f admin-gateway

logs-ui:
	docker compose -f $(COMPOSE_FILE) logs -f operator-ui

logs-stream:
	docker compose -f $(COMPOSE_FILE) logs -f stream-node

logs-recorder:
	docker compose -f $(COMPOSE_FILE) logs -f recorder-node

logs-ai:
	docker compose -f $(COMPOSE_FILE) logs -f ai-service

logs-auth:
	docker compose -f $(COMPOSE_FILE) logs -f auth-service

logs-device:
	docker compose -f $(COMPOSE_FILE) logs -f device-manager

logs-alert:
	docker compose -f $(COMPOSE_FILE) logs -f alert-service

logs-playback:
	docker compose -f $(COMPOSE_FILE) logs -f playback-service

# =========================================================
# Utility Commands
# =========================================================

docker-shell:
	@if [ -z "$(SVC)" ]; then \
		echo "Usage: make docker-shell SVC=<service-name>"; \
		echo "Available services: coordinator, admin-gateway, stream-node, recorder-node, etc."; \
		exit 1; \
	fi
	docker compose -f $(COMPOSE_FILE) exec $(SVC) /bin/sh
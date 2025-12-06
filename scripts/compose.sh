#!/usr/bin/env bash
set -euo pipefail

# ------------------------------------------------------------
# Docker Compose Stack Helper (idempotent)
# ------------------------------------------------------------
# Usage:
#   scripts/compose.sh help
#   scripts/compose.sh init              # one-shot init + up (idempotent)
#   scripts/compose.sh up                # start/update stack
#   scripts/compose.sh down              # stop stack (keep volumes)
#   scripts/compose.sh status            # show stack status
#   scripts/compose.sh logs [svc ...]    # follow logs (all or selected services)
#   scripts/compose.sh s3-init [bucket]  # create S3 bucket on MinIO if present
#   scripts/compose.sh db-init [sql]     # apply SQL file to Postgres if present
#   scripts/compose.sh clean             # remove local stamp/state files
#
# Environment:
#   .env at repo root (ignored by git) is auto-loaded if present.
#   PROJECT_NAME (default: vms)
#   PROFILE      (default: compose)  -> selects profiles/<PROFILE>/docker-compose.yml
#   COMPOSE_FILE (optional override)
#   GHCR_USER / GHCR_TOKEN (optional; auto-login to GHCR if set)
# ------------------------------------------------------------

# --- resolve repo root (script lives in scripts/) ---
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# --- auto-load root-level .env if present ---
ENV_FILE="${ENV_FILE:-${ROOT_DIR}/.env}"
if [[ -f "${ENV_FILE}" ]]; then
  # Load non-comment, non-empty lines as environment variables
  set -a
  # shellcheck disable=SC2046
  source <(grep -E '^[A-Za-z_][A-Za-z0-9_]*=.*' "${ENV_FILE}" || true)
  set +a
  echo "loaded environment from ${ENV_FILE}"
else
  echo "no .env found at ${ENV_FILE}; using current shell environment"
fi

# --- defaults (can be overridden via env/.env) ---
PROJECT_NAME="${PROJECT_NAME:-vms}"
PROFILE="${PROFILE:-compose}"
COMPOSE_FILE="${COMPOSE_FILE:-${ROOT_DIR}/profiles/${PROFILE}/docker-compose.yml}"
STATE_DIR="${STATE_DIR:-${ROOT_DIR}/.make}"
STAMP_INIT="${STATE_DIR}/compose.init.done"

# --- detect docker compose CLI (v2 or v1) ---
if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
  DC="docker compose"
elif command -v docker-compose >/dev/null 2>&1; then
  DC="docker-compose"
else
  echo "docker compose is not available"; exit 1
fi
COMPOSE="${DC} -f ${COMPOSE_FILE} --project-name ${PROJECT_NAME}"

# ------------------------------
# helpers
# ------------------------------
usage() {
  cat <<EOF
Docker Compose Stack Helper

Commands:
  help                 Show this help
  init                 One-shot init + up (idempotent)
  up                   Start or update stack
  down                 Stop stack (keep volumes)
  status               Show stack status
  logs [svc ...]       Follow logs (all or selected services)
  s3-init [bucket]     Create S3 bucket on MinIO if present (default: vms)
  db-init [sql]        Apply SQL file to Postgres if present
  clean                Remove local stamp/state files

Environment:
  .env (root)          Auto-loaded if present
  PROJECT_NAME         Default: vms
  PROFILE              Default: compose (selects profiles/<PROFILE>/docker-compose.yml)
  COMPOSE_FILE         Override compose file path
  GHCR_USER/TOKEN      If set, auto-login to ghcr.io before pull/build
EOF
  echo
  echo "Current:"
  echo "  PROJECT_NAME=${PROJECT_NAME}"
  echo "  PROFILE=${PROFILE}"
  echo "  COMPOSE_FILE=${COMPOSE_FILE}"
  echo "  ENV_FILE=${ENV_FILE}"
}

need_file() {
  [[ -f "$1" ]] || { echo "missing required file: $1" >&2; exit 1; }
}

ghcr_login_if_configured() {
  if [[ -n "${GHCR_USER:-}" && -n "${GHCR_TOKEN:-}" ]]; then
    echo "${GHCR_TOKEN}" | docker login ghcr.io -u "${GHCR_USER}" --password-stdin || true
  fi
}

pull_with_fallback_build() {
  # Try pulling images; if it fails (private/missing), fall back to local build
  if ! ${COMPOSE} pull; then
    echo "pull failed, falling back to local build"
    ${COMPOSE} build
  fi
}

wait_ready() {
  echo "waiting for services to become ready..."
  # MinIO readiness (if present)
  if ${COMPOSE} ps --services | grep -q "^minio$"; then
    for i in {1..30}; do
      if curl -fsS --max-time 2 http://localhost:9000/minio/health/ready >/dev/null 2>&1; then
        echo "MinIO ready"; break; fi; sleep 1; done
  fi
  # Postgres readiness (if present)
  if ${COMPOSE} ps --services | grep -q "^postgres$"; then
    for i in {1..30}; do
      docker exec "$(${COMPOSE} ps -q postgres)" pg_isready -U postgres >/dev/null 2>&1 && \
        { echo "Postgres ready"; break; } || true
      sleep 1
    done
  fi
  # NATS quick signal (if present)
  if ${COMPOSE} ps --services | grep -q "^nats$"; then
    echo "NATS assumed ready"
  fi
}

init_once() {
  mkdir -p "${STATE_DIR}"
  if [[ -f "${STAMP_INIT}" ]]; then
    echo "init already completed, skipping (idempotent)"
    return 0
  fi
  need_file "${COMPOSE_FILE}"
  ghcr_login_if_configured
  pull_with_fallback_build
  ${COMPOSE} up -d
  wait_ready
  touch "${STAMP_INIT}"
  echo "initialization completed"
}

# ------------------------------
# commands
# ------------------------------
cmd="${1:-help}"
shift || true

case "${cmd}" in
  help|-h|--help)
    usage
    ;;

  init)
    init_once
    ;;

  up)
    need_file "${COMPOSE_FILE}"
    ghcr_login_if_configured
    pull_with_fallback_build
    ${COMPOSE} up -d
    wait_ready
    ;;

  down)
    need_file "${COMPOSE_FILE}"
    ${COMPOSE} down
    ;;

  status)
    need_file "${COMPOSE_FILE}"
    ${COMPOSE} ps
    ;;

  logs)
    need_file "${COMPOSE_FILE}"
    # Follow logs for all services or only the ones provided as args
    ${COMPOSE} logs -f "$@"
    ;;

  s3-init)
    # Create bucket on MinIO (default: vms). No-op if MinIO service is absent.
    bucket="${1:-vms}"
    if ${COMPOSE} ps --services | grep -q "^minio$"; then
      cid="$(${COMPOSE} ps -q minio)"
      echo "creating bucket: ${bucket}"
      docker run --rm --network "container:${cid}" \
        -e MC_HOST_local="http://minio:minio123@127.0.0.1:9000" \
        minio/mc:latest sh -c "mc mb --ignore-existing local/${bucket} && mc ls local"
    else
      echo "MinIO service not found; skipping"
    fi
    ;;

  db-init)
    # Apply SQL file to Postgres (if service exists and file provided)
    sql="${1:-}"
    if [[ -n "${sql}" && -f "${sql}" ]]; then
      if ${COMPOSE} ps --services | grep -q "^postgres$"; then
        echo "applying SQL: ${sql}"
        docker exec -i "$(${COMPOSE} ps -q postgres)" \
          psql -U postgres -d "${POSTGRES_DB:-vms}" < "${sql}"
      else
        echo "Postgres service not found; skipping"
      fi
    else
      echo "missing or non-existent SQL file; nothing to do"
    fi
    ;;

  clean)
    rm -rf "${STATE_DIR}"
    echo "cleaned: ${STATE_DIR}"
    ;;

  *)
    echo "Unknown command: ${cmd}" >&2
    usage
    exit 1
    ;;
esac

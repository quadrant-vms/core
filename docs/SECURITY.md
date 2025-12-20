# Security Overview

## Authentication & Authorization

Quadrant VMS uses JWT-based auth with RBAC. The auth-service issues tokens,
and other services validate them via shared middleware.

Typical request flow:
1. Obtain a JWT from auth-service.
2. Send `Authorization: Bearer <token>` to protected endpoints.

## Secrets Management

- Do not use default secrets in production.
- Use Kubernetes Secrets or external secret managers (Vault, ExternalSecrets).
- Rotate `JWT_SECRET`, database credentials, and S3 credentials regularly.

## Network & Transport

- Use TLS at ingress and between services when possible.
- Restrict public exposure to only required endpoints.

# ADR 0005: Relicense from GPL-3.0 to Apache-2.0

## Status

Accepted

## Context

Eunomio ships as a public OSS repo plus a future private hosted repo under FSL-1.1-Apache-2.0. The project is sole-copyright; GPL-3.0 was a default choice without ideological attachment.

## Decision

Relicense the public `eunomio` repository to Apache License 2.0 with SPDX headers on code files.

## Consequences

- Apache-2.0 enables the hosted dual-licensing strategy described in `HOSTED_DEPLOYMENT.md`.
- Copyleft protections of GPL-3.0 are explicitly traded for permissive reuse and combination with the FSL hosted repo.
- No contributor CLA is required for this change.

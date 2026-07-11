# Design: MCP Streamable-HTTP Transport

## Context

The second of two MCP transports the original architecture always intended (PROPOSAL.md: "Transports: stdio ... and streamable HTTP on loopback with a bearer token"). Unlike every Phase 4/5 change so far, this one has no CDP protocol surface or real-OS side effects -- it's Rust HTTP-server plumbing wrapping the already-working `AibTools` tool surface, so the main design questions are about session/process lifecycle and the security posture of a locally-listening HTTP server, not protocol-behavior uncertainty.

## Goals / Non-Goals

**Goals:** an `aib mcp --http` mode serving the identical tool surface stdio does; loopback-only by construction; a bearer token gating every request; safe under multiple concurrent client sessions (each gets its own independent browser, no collision).

**Non-Goals:** TLS; cross-session browser sharing (a real daemon/multi-tenant model, not attempted here); rate limiting.

## Decisions

1. **`rmcp`'s `StreamableHttpService` is wrapped in a small `axum::Router`, not a hand-rolled HTTP server.** `rmcp`'s streamable-HTTP feature is `tower`-service-shaped, not a standalone listener; every `rmcp` example/test wires it into `axum::serve`. Writing a raw `hyper` server to avoid the `axum` dependency would just re-implement what `axum` already does correctly, for no benefit.
2. **A fresh `AibTools` per MCP session, via `StreamableHttpService::new`'s factory closure -- never a cloned/shared instance.** `AibTools` derives `Clone`, but that clone shares the *same* `Arc<Mutex<Option<Session>>>` (used elsewhere so multiple tool-handler methods on one session can run concurrently) -- cloning it into the factory would make every HTTP client share one browser, which is wrong. The factory calls `AibTools::with_browser_pref(..)` fresh each time, exactly mirroring how a new stdio process gets a fresh `AibTools` with an empty session.
3. **Each `AibTools` gets a unique profile-directory name, not the existing hard-coded `"aib-mcp"` literal.** Stdio's model is "one process, one profile directory," which was never a collision risk since only one process instance existed at a time for that literal name. HTTP mode breaks that assumption -- two concurrent sessions launching around the same time would both try to open the *same* Chrome user-data directory, which fails or silently corrupts state. `AibTools` gains a `profile_name` field (stdio's constructors keep passing the literal `"aib-mcp"`, unchanged behavior); the HTTP factory closure generates a random suffix per session (`format!("aib-mcp-http-{}", random_hex)`).
4. **Always bind `127.0.0.1`, no bind-address flag.** The stated threat model (PROPOSAL.md: "loopback with a bearer token") is a single trusted machine; there's no legitimate v1 use case for exposing this beyond loopback, and no flag exists to do it -- not "defaults to loopback," but "cannot be anything else" short of editing the source. A future genuine need for LAN/remote access should be its own explicitly-scoped change with its own threat-modeling, not a flag someone flips without thinking about it.
5. **Bearer token: explicit `--token` overrides; otherwise a random one is generated and printed once at startup.** Matches the common local-dev-tool pattern (Jupyter's token banner is the closest analog) -- no config file, no separate `aib token` command, just copy the value the process printed into the MCP client's config. Checked via a small `axum::middleware::from_fn` layer comparing the `Authorization: Bearer <token>` header with constant-ish string comparison (not cryptographically hardened against timing attacks -- loopback-only, low-value target, not worth the complexity for v1).
6. **Graceful shutdown on Ctrl+C via a `CancellationToken`**, matching `rmcp`'s own documented pattern (`StreamableHttpServerConfig::with_cancellation_token` + `axum::serve(..).with_graceful_shutdown(..)`), rather than letting in-flight requests die mid-response on SIGINT.

## Risks / Trade-offs

- [A locally-listening HTTP server is a bigger attack surface than a stdio pipe, even loopback-only -- any other process/user on the same machine can reach it] → mitigated by the bearer token (opt-in feature, off by default -- stdio remains the default transport) and loopback-only binding; accepted as the stated v1 threat model, same trade-off every local-dev-tool HTTP server makes.
- [Per-session browser launch means N concurrent HTTP clients means N Chrome processes] → intentional, matches the existing per-process cost model exactly (stdio already launches one browser per `aib mcp` process); not a new cost, just a new way to reach it.

## Migration Plan

Purely additive -- a new opt-in flag on an existing subcommand. Stdio behavior and its profile-directory naming are completely unchanged.

## Open Questions

None blocking.

# Project context

rs-proxy-facade is a local HTTP/HTTPS forward proxy configured through a Tauri desktop application with a Rust backend and Angular UI. Its goal is reliable operation in enterprise networks, including authenticated upstream proxies. This document describes target requirements; do not present them as implemented without verification.

## Engineering persona

Act as an experienced software architect and senior Rust engineer specializing in asynchronous networking, desktop applications, and enterprise proxy integration. Apply working knowledge of Tokio, Hyper, HTTP/HTTPS and CONNECT, PAC/system configuration, Windows authentication, Tauri, and Angular/TypeScript.

- Prioritize correctness, maintainability, and operational reliability. Choose the simplest architecture that meets verified requirements; avoid speculative abstractions and framework migrations driven only by feature counts.
- Explain architectural tradeoffs and challenge assumptions with concrete evidence. Distinguish documented capabilities, tested behavior, and hypotheses; do not claim expertise as a substitute for verification.
- Write idiomatic Rust with explicit ownership, meaningful types, and actionable errors. Review task lifetimes, cancellation, shared state, lock scope, blocking work, and bounded resource usage when designing asynchronous flows.
- Treat protocol behavior and authentication as architectural concerns. Prefer maintained libraries and native platform facilities over custom protocol or cryptographic implementations; validate their limitations against the project's enterprise scenarios.
- Keep Tauri commands thin and Rust/TypeScript contracts explicit. Separate presentation, configuration, route resolution, authentication, and transport without introducing unnecessary layers.
- Evaluate changes through realistic failure cases and focused tests. Communicate implementation status, compatibility limits, and remaining risks clearly, using the user's language.

## Configuration modes

- **DIRECT**: connect directly to the destination.
- **MANUAL**: use an explicitly configured upstream proxy, with authentication and bypass rules as applicable.
- **SYSTEM**: read the operating system's proxy settings and exclusions, resolving its PAC configuration when present. Treat environment variables and WPAD discovery as explicit capabilities, not automatic consequences of reading system settings.
- **PAC**: load a user-selected local PAC file or PAC URL and evaluate routing for each destination using a maintained evaluator and its PAC helper functions.

Keep listener settings separate from outbound routing settings. Preserve ordered PAC candidates and explicit DIRECT routes; never silently bypass a configured proxy after a resolution or authentication failure. Prevent routing back into the local listener.

## Authentication model

- Target flow: unauthenticated local client (such as Firefox) -> local proxy -> authenticated enterprise proxy -> destination. The local listener does not require a username/password. The Rust backend negotiates upstream authentication; browser credential/challenge relay is not the selected model.
- Keep authentication profiles separate from routing modes. Offer no authentication, explicit credentials, or Windows session identity where supported. Use native Windows SSPI for integrated authentication, subject to validation of the supported combinations and enterprise policies; do not implement NTLM or Kerberos cryptography manually.
- Knowing a MANUAL proxy address does not determine its authentication scheme. Inspect upstream 407 responses and Proxy-Authenticate challenges, then select a scheme allowed by the profile. Reading SYSTEM settings does not automatically supply credentials or integrated authentication.
- Consume upstream 407 challenges in the Rust backend; do not relay them to the browser or trigger browser proxy credential dialogs. If backend authentication cannot complete, return a sanitized gateway failure (for example, 502; 504 for an upstream timeout) and expose the actionable authentication state in the Tauri UI. Preserve origin 401 responses.
- Scope Windows session authentication to explicitly trusted upstreams, just like explicit credentials. Use the enterprise proxy's service identity/SPN for Negotiate, not the local listener or destination website. Validate DNS/SPN configuration, domain policies, and channel binding where required. Do not silently downgrade to a disallowed scheme; Basic credentials are not encrypted by Base64 and require an explicit transport-security policy.
- For MANUAL, associate a profile with the configured upstream. For SYSTEM/PAC, resolve the route first and look up the profile for the selected upstream. PAC routing does not supply credentials; DIRECT routes need no upstream authentication. Sharing a profile across multiple proxies requires an explicit association, never an automatic reuse of credentials for arbitrary PAC candidates.
- If authentication requires missing credentials, request them in the Tauri UI after identifying the upstream and challenge. Show the proxy host/port, request username/password and domain when applicable, and offer optional persistence through operating-system credential storage. Persist only secret references in configuration. Windows session authentication should not require password entry.
- Deduplicate credential prompts per upstream/profile across concurrent requests. Bound pending waits and negotiation attempts; cancellation or timeout must fail affected requests with actionable diagnostics, without silently switching to DIRECT or repeatedly prompting. New requests can use the supplied profile; do not promise automatic replay of already failed or partially transmitted requests.
- Treat browser Proxy-Authorization as unrelated to backend-managed upstream credentials: remove it and generate upstream authorization only for the intended proxy. Never send Proxy-Authorization to an origin server, including on DIRECT routes; preserve destination Authorization semantics. Do not log full requests containing authentication headers.
- Authentication state belongs to the upstream connection when the scheme is connection-bound, not to shared immutable configuration. Preserve affinity throughout negotiation and isolate reuse by upstream and identity.

## Transport and enterprise compatibility

- Support HTTP forwarding and HTTPS tunneling through CONNECT, both directly and through an upstream proxy. Report CONNECT success only after the upstream connection or tunnel is established. HTTPS tunneling does not require TLS interception; interception is outside the initial scope.
- Enterprise authentication is a core requirement. Establish and test a support matrix for Basic, Digest, NTLM, and Negotiate/Kerberos, including explicit credentials and operating-system identity where supported. Exact supported combinations remain to be validated.
- Separate upstream proxy authentication (407) from destination authentication (401). Scope proxy credentials to the intended upstream; never forward them to the destination or log secrets.
- Preserve connection affinity for connection-bound authentication and isolate authenticated connection reuse by identity. Do not assume generic connection pooling implements NTLM or Kerberos correctly.
- Use operating-system credential storage for persisted secrets. Respect configured certificate trust for PAC downloads and TLS connections; do not disable verification to accommodate enterprise certificates.
- Distinguish TLS trust from proxy authentication. For an opaque CONNECT tunnel, the client validates the destination certificate. When Rust downloads a PAC over HTTPS or establishes TLS to an upstream proxy, configure and test its own trust roots, including enterprise/system CAs where supported; raw TCP and Hyper alone do not load the operating-system trust store.
- Bound connection, resolution, and authentication attempts. Avoid replaying requests whose bodies may already have reached the destination. Stream traffic with bounded buffering and backpressure.
- Handle DNS failures, unavailable proxies, invalid PAC scripts, authentication failures, and network changes with useful errors rather than process panics.

## Application behavior

- Provide configuration validation, proxy start/stop/status, and actionable diagnostics in the UI.
- Distinguish configured mode, resolved candidate routes, and the route actually used for a connection. Different destinations can use different proxies concurrently.
- Define how configuration changes affect new and active connections. Stop and restart must manage listener and connection tasks deliberately.
- Bind to loopback by default. Broader network exposure must be an explicit configuration choice.

## Architecture and maintenance

The current implementation uses Hyper/Tokio. No migration to Pingora, Rama, or another networking framework has been decided. Keep UI/Tauri commands, configuration, system/PAC resolution, authentication, and transport separate so dependencies can be evaluated independently.

Validate the enterprise authentication approach early with a focused Windows prototype against the intended upstream: HTTP and CONNECT, SSPI session identity, connection affinity, and authentication failure behavior. Do not postpone this feasibility check until after PAC/SYSTEM and credential UI completion. Treat success with Basic or a controlled fixture as insufficient evidence of NTLM/Negotiate enterprise compatibility.

Evaluate libraries against desktop packaging, Windows compatibility, authenticated CONNECT, platform integration, dependency maintenance, and testability. Throughput alone is not the deciding criterion. Windows enterprise compatibility is a key evaluation case; define the supported operating-system matrix before claiming cross-platform completeness.

Use representative integration tests before claiming enterprise support: all four routing modes, PAC ordering and exclusions, authenticated HTTP and CONNECT, credential isolation, failed connections, and lifecycle changes. Use controlled fixtures for automated checks and validate integrated authentication in an appropriate enterprise test environment. Keep documentation clear about implemented behavior, planned capabilities, and platform limitations.

Include authentication challenge selection, multi-step connection affinity, concurrent prompt deduplication, cancellation/timeouts, and credential isolation across PAC-selected upstreams and DIRECT routes in those checks. Validate HTTP request-body handling on authentication failures without unsafe replay, and CONNECT success only after upstream authentication and tunnel establishment.

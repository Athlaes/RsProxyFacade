# rs-proxy-facade

A local HTTP/HTTPS forward proxy with a Tauri (Rust) desktop application and an Angular configuration UI.

The project aims to provide DIRECT, MANUAL, SYSTEM, and PAC routing for local clients without local proxy authentication. When an enterprise proxy is selected, the Rust backend will handle upstream authentication using configured credentials or the Windows session identity where supported.

Early development: these are product goals, not a statement of completed support. The current transport uses Hyper/Tokio; the networking architecture remains under evaluation.

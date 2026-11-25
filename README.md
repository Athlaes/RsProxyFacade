# rs-proxy-facade

A local HTTP/HTTPS forward proxy with a Tauri (Rust) desktop application and an Angular configuration UI.

The project aims to provide DIRECT, MANUAL, SYSTEM, and PAC routing for local clients without local proxy authentication. When an enterprise proxy is selected, the Rust backend will handle upstream authentication using configured credentials or the Windows session identity where supported.

Early development: these are product goals, not a statement of completed support. The current transport uses Hyper/Tokio; the networking architecture remains under evaluation.

System settings detection currently reads the Windows user's WinHTTP/Internet Options configuration, GNOME/Unity GSettings on Linux (`gsettings-desktop-schemas` required), or the global macOS SystemConfiguration proxy settings. It returns an explicit PAC URL or a single manual proxy (host, port) and exclusions. Windows and macOS WPAD are deliberately ignored; Linux automatic mode without an explicit PAC URL returns an error. Other Linux desktops and environment proxy variables are not implemented. No credentials are read or written.

The transport handles resolved DIRECT and HTTP proxy routes, including manual SYSTEM settings and bypass rules. PAC evaluation is not yet implemented. Advanced Windows proxy lists and protocol mappings are explicitly rejected. Linux supports one HTTP proxy shared by HTTP and HTTPS; a distinct HTTPS proxy or an active SOCKS proxy is rejected. FTP settings are outside this HTTP/HTTPS proxy's scope. Linux exclusions retain GNOME patterns/CIDRs, joined with semicolons in the current model.

HTTP forwarding uses origin-form for direct requests and absolute-form for upstream proxies. CONNECT through an upstream is acknowledged to the local client only after the upstream tunnel is established. Upstream authentication is not implemented: HTTP and CONNECT responses with status 407 become sanitized 502 responses, without forwarding challenges or replaying requests. Origin 401 responses are preserved. Connection establishment and waiting for upstream response headers have 30-second limits; tunnel and response bodies stream with backpressure. Non-CONNECT protocol upgrades are not supported.

On macOS, an enabled explicit PAC takes precedence. Manual configuration requires both HTTP and HTTPS enabled with the same host and port; protocol-specific configurations and SOCKS are rejected. Exceptions are joined with semicolons, with `<local>` representing the exclusion of simple hostnames. Interface-specific or supplemental VPN proxy rules are not resolved by this global settings reader. The macOS module and its tests have been cross-checked for Apple Silicon, but still require execution on macOS.

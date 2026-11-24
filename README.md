# rs-proxy-facade

A local HTTP/HTTPS forward proxy with a Tauri (Rust) desktop application and an Angular configuration UI.

The project aims to provide DIRECT, MANUAL, SYSTEM, and PAC routing for local clients without local proxy authentication. When an enterprise proxy is selected, the Rust backend will handle upstream authentication using configured credentials or the Windows session identity where supported.

Early development: these are product goals, not a statement of completed support. The current transport uses Hyper/Tokio; the networking architecture remains under evaluation.

System settings detection currently reads the Windows user's WinHTTP/Internet Options configuration, GNOME/Unity GSettings on Linux (`gsettings-desktop-schemas` required), or the global macOS SystemConfiguration proxy settings. It returns an explicit PAC URL or a single manual proxy (host, port) and exclusions. Windows and macOS WPAD are deliberately ignored; Linux automatic mode without an explicit PAC URL returns an error. Other Linux desktops and environment proxy variables are not implemented. No credentials are read or written.

This is configuration detection only: SYSTEM routing and PAC evaluation are not yet connected to the transport. Advanced Windows proxy lists and protocol mappings are explicitly rejected. Linux supports one HTTP proxy shared by HTTP and HTTPS; a distinct HTTPS proxy or an active SOCKS proxy is rejected. FTP settings are outside this HTTP/HTTPS proxy's scope. Linux exclusions retain GNOME patterns/CIDRs, joined with semicolons in the current model.

On macOS, an enabled explicit PAC takes precedence. Manual configuration requires both HTTP and HTTPS enabled with the same host and port; protocol-specific configurations and SOCKS are rejected. Exceptions are joined with semicolons, with `<local>` representing the exclusion of simple hostnames. Interface-specific or supplemental VPN proxy rules are not resolved by this global settings reader. The macOS module and its tests have been cross-checked for Apple Silicon, but still require execution on macOS.

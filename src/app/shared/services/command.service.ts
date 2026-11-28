import { Service } from "@angular/core";
import { ProxyPacServerConfig, ProxyServerConfig } from "../models/commands";
import { invoke } from "@tauri-apps/api/core";
import { BadgeStatus } from "../components/design-system/badge/badge.component";

@Service()
export class CommandService {
  startProxyServer(proxyConfig: ProxyServerConfig) {
    return invoke<string>("start_proxy_server", { config: proxyConfig });
  }

  stopProxyServer() {
    return invoke<string>("stop_proxy_server");
  }

  getProxyServerStatus(): Promise<BadgeStatus> {
    return invoke<BadgeStatus>("proxy_server_status");
  }

  startProxyPacServer(proxyConfig: ProxyPacServerConfig) {
    return invoke<string>("start_proxy_pac_server", { config: proxyConfig });
  }

  stopProxyPacServer() {
    return invoke<string>("stop_proxy_pac_server");
  }

  getPacServerStatus(): Promise<BadgeStatus> {
    return invoke<BadgeStatus>("proxy_pac_server_status");
  }
}

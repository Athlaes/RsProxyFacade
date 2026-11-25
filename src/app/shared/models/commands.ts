export interface ProxyServerConfig {
  host: string;
  port: number;
  proxyMode: ProxyMode;
}

export type ProxyMode =
  "direct" | "system" | { manual: { host: string; port: number; bypass: string } } | { pac: { url: string } };

export interface ProxyPacServerConfig {
  host: string;
  port: number;
  pacUrl: string;
}

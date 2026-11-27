import { inject, Service, signal } from "@angular/core";
import { CommandService } from "./command.service";
import { MatSnackBar } from "@angular/material/snack-bar";
import { ProxyPacServerConfig, ProxyServerConfig } from "../models/commands";

@Service()
export class ServerService {
  private readonly commandService = inject(CommandService);
  private readonly snackService = inject(MatSnackBar);

  isProxyPacServerStarted = signal(false);
  isProxyServerStarted = signal(false);

  startProxyPacServer(config: ProxyPacServerConfig) {
    return this.commandService
      .startProxyPacServer(config)
      .then((_) => {
        this.isProxyPacServerStarted.set(true);
        this.snackService.open("Proxy pac server started successfully", "Close", {
          panelClass: ["app-snackbar"],
          duration: 4000,
        });
      })
      .catch((err: unknown) =>
        this.snackService.open(this.parseError(err), "Close", {
          panelClass: ["app-snackbar", "fail"],
          duration: 4000,
        }),
      );
  }

  stopProxyPacServer() {
    this.commandService
      .stopProxyPacServer()
      .then((_) => {
        this.isProxyPacServerStarted.set(false);
        this.snackService.open("Proxy pac server stopped successfully", "Close", {
          panelClass: ["app-snackbar"],
          duration: 4000,
        });
      })
      .catch((err) => {
        this.snackService.open(this.parseError(err), "Close", {
          panelClass: ["app-snackbar", "fail"],
          duration: 4000,
        });
      });
  }

  startProxyServer(config: ProxyServerConfig) {
    return this.commandService
      .startProxyServer(config)
      .then((_) => {
        this.isProxyServerStarted.set(true);
        this.snackService.open("Proxy server started successfully", "Close", {
          panelClass: ["app-snackbar"],
          duration: 4000,
        });
      })
      .catch((err) =>
        this.snackService.open(this.parseError(err), "Close", {
          panelClass: ["app-snackbar", "fail"],
          duration: 4000,
        }),
      );
  }

  stopProxyServer() {
    this.commandService
      .stopProxyServer()
      .then((_) => {
        this.isProxyServerStarted.set(false);
        this.snackService.open("Proxy server stopped successfully", "Close", {
          panelClass: ["app-snackbar"],
          duration: 4000,
        });
      })
      .catch((err) => {
        this.snackService.open(this.parseError(err), "Close", {
          panelClass: ["app-snackbar", "fail"],
          duration: 4000,
        });
      });
  }

  parseError(err: unknown): string {
    if (typeof err === "string") return err;
    return "Unknown error occured";
  }
}

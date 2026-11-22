import { Component, effect, inject, signal } from "@angular/core";
import { toSignal } from "@angular/core/rxjs-interop";
import { FormBuilder, FormControl, FormGroup, FormsModule, ReactiveFormsModule, Validators } from "@angular/forms";
import { MatSnackBar } from "@angular/material/snack-bar";
import { debounceTime } from "rxjs";
import { CommandService } from "../../shared/services/command.service";
import { MatExpansionModule } from "@angular/material/expansion";
import { MatFormFieldModule } from "@angular/material/form-field";
import { ExpansionPanelComponent } from "../../shared/components/design-system/expansion-panel/expansion-panel.component";
import { MatTabChangeEvent, MatTabsModule } from "@angular/material/tabs";
import { ServerStatusComponent } from "../../shared/components/common/server-status/server-status.component";
import { MatIconModule } from "@angular/material/icon";
import { ProxyServerConfig, ProxyMode } from "../../shared/models/commands";
import { MatButtonModule } from "@angular/material/button";
import { MatInputModule } from "@angular/material/input";
import { PanelSectionComponent } from "../../shared/components/design-system/expansion-panel/panel-section/panel-section.component";
import { BadgeStatus } from "../../shared/components/design-system/badge/badge.component";
import { MatSlideToggleModule } from "@angular/material/slide-toggle";
import { ServerService } from "../../shared/services/server.service";
import { integerValidator } from "../../shared/config/validators";

type ProxyModeOptions = "Direct" | "System" | "Manual" | "Pac";

type CreateProxyForm = {
  host: FormControl<string>;
  port: FormControl<number>;
  proxyMode: FormControl<ProxyModeOptions>;
  proxyHost: FormControl<string>;
  proxyPort: FormControl<number>;
  proxyPacUrl: FormControl<string>;
  useExposedPacServer: FormControl<boolean>;
};

@Component({
  imports: [
    MatExpansionModule,
    MatFormFieldModule,
    MatInputModule,
    ReactiveFormsModule,
    FormsModule,
    ExpansionPanelComponent,
    MatTabsModule,
    ServerStatusComponent,
    PanelSectionComponent,
    MatIconModule,
    MatButtonModule,
    MatSlideToggleModule,
  ],
  selector: "app-proxy-server",
  styleUrl: "./proxy-server.component.scss",
  templateUrl: "./proxy-server.component.html",
})
export class ProxyServerComponent {
  private readonly snackService = inject(MatSnackBar);
  private readonly serverService = inject(ServerService);
  protected readonly commandService = inject(CommandService);
  proxyModeTabs: ProxyModeOptions[] = ["Direct", "System", "Manual", "Pac"];

  createProxyForm: FormGroup<CreateProxyForm> = new FormBuilder().group({
    host: new FormControl(localStorage.getItem("server_form_host") ?? "localhost", {
      nonNullable: true,
      validators: [Validators.required],
    }),
    port: new FormControl(Number(localStorage.getItem("server_form_port") ?? "3128"), {
      nonNullable: true,
      validators: [Validators.max(65535), Validators.min(1), Validators.required, integerValidator],
    }),
    proxyMode: new FormControl<ProxyModeOptions>(
      (localStorage.getItem("server_form_proxy_mode") as ProxyModeOptions) ?? "Direct",
      {
        nonNullable: true,
        validators: [Validators.required],
      },
    ),
    proxyHost: new FormControl(localStorage.getItem("server_form_proxy_host") ?? "", {
      nonNullable: true,
      validators: [],
    }),
    proxyPort: new FormControl(Number(localStorage.getItem("server_form_proxy_port") ?? "0"), {
      nonNullable: true,
      validators: [Validators.max(65535), Validators.min(1), integerValidator],
    }),
    proxyPacUrl: new FormControl(localStorage.getItem("server_form_proxy_pac_url") ?? "", {
      nonNullable: true,
      validators: [],
    }),
    useExposedPacServer: new FormControl(localStorage.getItem("server_form_proxy_use_exposed_pac_server") === "true", {
      nonNullable: true,
      validators: [],
    }),
  });
  private readonly createProxyFormChange = toSignal(this.createProxyForm.valueChanges.pipe(debounceTime(500)));
  private readonly exposedPacUrlChange = toSignal(this.createProxyForm.controls.useExposedPacServer.valueChanges);

  private readonly forcePacUrlWhenExposedIsTrue = effect(() => {
    let exposedPacUrl = this.exposedPacUrlChange();
    if (exposedPacUrl) {
      let storedPacHost = localStorage.getItem("server_pac_form_host");
      let storedPacPort = localStorage.getItem("server_pac_form_port");
      if (!storedPacHost || !storedPacPort) {
        this.snackService.open("Pac server not configured", "Close", {
          panelClass: ["app-snackbar", "error"],
          duration: 4000,
        });
        this.createProxyForm.controls.useExposedPacServer.setValue(false);
        return;
      }
      this.createProxyForm.controls.proxyPacUrl.disable();
      const pacUrl = `http://${storedPacHost}:${storedPacPort}/proxy.pac`;
      this.createProxyForm.controls.proxyPacUrl.setValue(pacUrl);
    } else {
      this.createProxyForm.controls.proxyPacUrl.enable();
    }
  });

  private disableFormWhenStarted = effect(() => {
    if (this.serverService.isProxyServerStarted() === true) {
      this.createProxyForm.disable();
    } else {
      this.createProxyForm.enable();
    }
  });

  private readonly syncWithLocalStorage = effect(() => {
    if (!this.createProxyFormChange()) return;
    const proxyForm = this.createProxyForm.getRawValue();
    localStorage.setItem("server_form_host", proxyForm?.host ?? "");
    localStorage.setItem("server_form_port", (proxyForm?.port ?? 3128).toString());
    localStorage.setItem("server_form_proxy_host", proxyForm?.proxyHost ?? "");
    localStorage.setItem("server_form_proxy_port", (proxyForm?.proxyPort ?? "1").toString());
    localStorage.setItem("server_form_proxy_pac_url", proxyForm?.proxyPacUrl ?? "");
    localStorage.setItem("server_form_proxy_use_exposed_pac_server", String(proxyForm?.useExposedPacServer ?? false));
  });

  updateStatus(status: BadgeStatus) {
    switch (status) {
      case "success":
        this.serverService.isProxyServerStarted.set(true);
        break;
      default:
        this.serverService.isProxyServerStarted.set(false);
    }
  }

  updateProxyModeControl(tabEvent: MatTabChangeEvent) {
    const controls = this.createProxyForm.controls;
    controls["proxyMode"].setValue(this.proxyModeTabs[tabEvent.index]);
    switch (this.proxyModeTabs[tabEvent.index]) {
      case "Direct":
      case "System":
        controls.proxyPacUrl.removeValidators([Validators.required]);
        controls.proxyHost.removeValidators([Validators.required]);
        controls.proxyPort.removeValidators([Validators.required]);
        break;
      case "Pac":
        controls.proxyPacUrl.addValidators([Validators.required]);
        controls.proxyHost.removeValidators([Validators.required]);
        controls.proxyPort.removeValidators([Validators.required]);
        break;
      case "Manual":
        controls.proxyPacUrl.removeValidators([Validators.required]);
        controls.proxyHost.addValidators([Validators.required]);
        controls.proxyPort.addValidators([Validators.required]);
    }
    for (const control of [controls.proxyHost, controls.proxyPort, controls.proxyPacUrl]) {
      control.updateValueAndValidity({ emitEvent: false });
    }
    localStorage.setItem("server_form_proxy_mode", this.proxyModeTabs[tabEvent.index]);
  }

  startProxyServer() {
    this.createProxyForm.markAllAsTouched();
    if (!this.createProxyForm.valid) {
      this.snackService.open("Invalid form, fix to continue", "Close", {
        panelClass: ["app-snackbar", "fail"],
        duration: 4000,
      });
      return;
    }
    if (
      this.createProxyForm.controls.proxyMode.getRawValue() === "Pac" &&
      this.createProxyForm.controls.useExposedPacServer.getRawValue() &&
      !this.serverService.isProxyPacServerStarted()
    ) {
      this.snackService.open("Server is configured to use exposed pac file but server is not started", "Close", {
        panelClass: ["app-snackbar", "fail"],
        duration: 4000,
      });
      return;
    }

    const proxyConfig = this.toProxyConfig(this.createProxyForm);
    this.serverService.startProxyServer(proxyConfig);
  }

  getInitialTabIndex() {
    const index = this.proxyModeTabs.indexOf(
      (localStorage.getItem("server_form_proxy_mode") as ProxyModeOptions) ?? "Direct",
    );
    this.updateProxyModeControl({ index } as MatTabChangeEvent);
    return index;
  }

  private toProxyConfig(createProxyForm: FormGroup<CreateProxyForm>): ProxyServerConfig {
    return {
      host: createProxyForm.controls.host.getRawValue(),
      port: createProxyForm.controls.port.getRawValue(),
      proxyMode: this.toProxyMode(createProxyForm),
    };
  }

  private toProxyMode(createProxyForm: FormGroup<CreateProxyForm>): ProxyMode {
    switch (createProxyForm.controls.proxyMode.getRawValue()) {
      case "Direct":
        return "direct";
      case "System":
        return "system";
      case "Manual":
        return {
          manual: {
            host: createProxyForm.controls.proxyHost.getRawValue(),
            port: createProxyForm.controls.proxyPort.getRawValue(),
          },
        };
      case "Pac":
        return {
          pac: { url: createProxyForm.controls.proxyPacUrl.getRawValue() },
        };
    }
  }
}

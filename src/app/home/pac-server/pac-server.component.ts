import { Component, computed, effect, signal } from "@angular/core";
import { PanelSectionComponent } from "../../shared/components/design-system/expansion-panel/panel-section/panel-section.component";
import { ExpansionPanelComponent } from "../../shared/components/design-system/expansion-panel/expansion-panel.component";
import { MatFormFieldModule } from "@angular/material/form-field";
import { FormBuilder, FormControl, FormGroup, ReactiveFormsModule, Validators } from "@angular/forms";
import { MatIconModule } from "@angular/material/icon";
import { MatInputModule } from "@angular/material/input";
import { MatButtonModule } from "@angular/material/button";
import { inject } from "@angular/core";
import { CommandService } from "../../shared/services/command.service";
import { MatSnackBar } from "@angular/material/snack-bar";
import { ServerStatusComponent } from "../../shared/components/common/server-status/server-status.component";
import { debounceTime } from "rxjs";
import { toSignal } from "@angular/core/rxjs-interop";
import { BadgeStatus } from "../../shared/components/design-system/badge/badge.component";
import { ServerService } from "../../shared/services/server.service";

type CreateProxyPacForm = {
  host: FormControl<string>;
  port: FormControl<number>;
  pacUrl: FormControl<string>;
};

@Component({
  imports: [
    PanelSectionComponent,
    ExpansionPanelComponent,
    ServerStatusComponent,
    MatFormFieldModule,
    ReactiveFormsModule,
    MatIconModule,
    MatInputModule,
    MatButtonModule,
  ],
  selector: "app-pac-server",
  styleUrl: "./pac-server.component.scss",
  templateUrl: "./pac-server.component.html",
})
export class PacServerComponent {
  protected readonly commandService = inject(CommandService);
  private readonly snackService = inject(MatSnackBar);
  private readonly serverService = inject(ServerService);

  createProxyPacForm = new FormBuilder().group<CreateProxyPacForm>({
    host: new FormControl(localStorage.getItem("server_pac_form_host") ?? "localhost", {
      nonNullable: true,
      validators: [Validators.required],
    }),
    port: new FormControl(Number.parseInt(localStorage.getItem("server_pac_form_port") ?? "3129"), {
      nonNullable: true,
      validators: [Validators.required],
    }),
    pacUrl: new FormControl(localStorage.getItem("server_pac_form_proxy_pac_url") ?? "", {
      nonNullable: true,
      validators: [Validators.required],
    }),
  });
  private readonly createProxyPacFormChange = toSignal(this.createProxyPacForm.valueChanges.pipe(debounceTime(500)));

  private readonly syncWithLocalSotrage = effect(() => {
    const form = this.createProxyPacFormChange();
    if (!form) return;
    localStorage.setItem("server_pac_form_host", form.host ?? "localhost");
    localStorage.setItem("server_pac_form_port", (form.port ?? 3129).toString());
    localStorage.setItem("server_pac_form_proxy_pac_url", form.pacUrl ?? "");
  });

  private disableFormWhenStarted = effect(() => {
    if (this.serverService.isProxyPacServerStarted() === true) {
      this.createProxyPacForm.disable();
    } else {
      this.createProxyPacForm.enable();
    }
  });

  updateStatus(status: BadgeStatus) {
    switch (status) {
      case "success":
        this.serverService.isProxyPacServerStarted.set(true);
        break;
      default:
        this.serverService.isProxyPacServerStarted.set(false);
    }
  }

  startProxyPacServer() {
    this.createProxyPacForm.markAllAsTouched();
    if (!this.createProxyPacForm.valid) {
      this.snackService.open("Invalid form, fix to continue", "Close", {
        panelClass: ["app-snackbar", "fail"],
        duration: 4000,
      });
      return;
    }

    this.serverService.startProxyPacServer(this.createProxyPacForm.getRawValue());
  }
}

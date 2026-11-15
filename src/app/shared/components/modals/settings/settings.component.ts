import { Component, inject, signal } from "@angular/core";
import { FormBuilder, FormControl, ReactiveFormsModule } from "@angular/forms";
import { MAT_DIALOG_DATA, MatDialogModule, MatDialogRef } from "@angular/material/dialog";
import { MatButtonModule } from "@angular/material/button";
import { MatFormFieldModule } from "@angular/material/form-field";
import { MatInputModule } from "@angular/material/input";
import { MatSlideToggleModule } from "@angular/material/slide-toggle";
import { settings } from "../../../config/settings";

@Component({
  imports: [
    MatDialogModule,
    MatButtonModule,
    ReactiveFormsModule,
    MatFormFieldModule,
    MatInputModule,
    MatSlideToggleModule,
  ],
  selector: "app-settings",
  styleUrl: "./settings.component.scss",
  templateUrl: "./settings.component.html",
})
export class SettingsComponent {
  private readonly dialogRef = inject(MatDialogRef);
  settings = settings;

  settingsForm = new FormBuilder().group({
    startProxyServerOnAppStart: new FormControl(false, { nonNullable: true }),
    startPacServerOnAppStart: new FormControl(false, { nonNullable: true }),
    startAppOnOsStart: new FormControl(false, { nonNullable: true }),
    minimizeToTaskbar: new FormControl(false, { nonNullable: true }),
    forceSystemToUseExposedProxyPac: new FormControl(false, { nonNullable: false }),
  });

  selectedCategorie = signal(settings.categories.find((c) => c.name === "general"));

  close() {
    this.dialogRef.close();
  }
}

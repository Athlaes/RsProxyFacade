import { Component, computed, effect, inject, signal } from "@angular/core";
import { RouterOutlet } from "@angular/router";
import { invoke } from "@tauri-apps/api/core";
import { MatToolbarModule } from "@angular/material/toolbar";
import { MatIconModule } from "@angular/material/icon";
import { MatButtonModule } from "@angular/material/button";
import { ThemeService } from "./shared/services/theme.service";
import { MatDialog } from "@angular/material/dialog";
import { SettingsComponent } from "./shared/modals/settings/settings.component";
import { openUrl } from "@tauri-apps/plugin-opener";

type LightModeIcon = "light_mode" | "light_mode_auto" | "light_mode_filled";

@Component({
  selector: "app-root",
  imports: [RouterOutlet, MatToolbarModule, MatIconModule, MatButtonModule],
  templateUrl: "./app.component.html",
  styleUrl: "./app.component.scss",
})
export class AppComponent {
  private readonly themeService = inject(ThemeService);
  private readonly matDialog = inject(MatDialog);

  lightModeIcon = computed<LightModeIcon>(() => {
    switch (this.themeService.currentTheme()) {
      case "auto":
        return "light_mode_auto";
      case "dark":
        return "light_mode_filled";
      case "light":
        return "light_mode";
    }
  });
  greetingMessage = signal("");

  nextTheme() {
    this.themeService.next_theme();
  }

  async openGithubUrl() {
    try {
      await openUrl("https://github.com/Athlaes");
    } catch (error) {
      console.error("Impossible d’ouvrir le navigateur", error);
    }
  }

  openSettings() {
    this.matDialog.open(SettingsComponent, {
      height: "80vh",
      width: "80vw",
    });
  }

  greet(event: SubmitEvent, name: string): void {
    event.preventDefault();

    // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
    invoke<string>("greet", { name }).then((text) => {
      this.greetingMessage.set(text);
    });
  }
}

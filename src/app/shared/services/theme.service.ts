import { effect, Service, signal } from "@angular/core";

type ThemeOptions = "light" | "dark" | "auto";
const themeOptions: ThemeOptions[] = ["light", "dark", "auto"];

@Service()
export class ThemeService {
  private _theme = signal<ThemeOptions>((localStorage.getItem("themeMode") as ThemeOptions) ?? "auto");
  currentTheme = this._theme.asReadonly();

  next_theme() {
    this._theme.set(themeOptions[(themeOptions.indexOf(this._theme()) + 1) % themeOptions.length]);
    localStorage.setItem("themeMode", this._theme());
  }

  private readonly currentThemeChange = effect(() => {
    switch (this.currentTheme()) {
      case "light":
        document.body.style.colorScheme = "light";
        break;
      case "auto":
        document.body.style.colorScheme = "light dark";
        break;
      case "dark":
        document.body.style.colorScheme = "dark";
    }
  });
}

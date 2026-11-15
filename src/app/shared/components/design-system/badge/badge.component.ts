import { Component, input } from "@angular/core";

export type BadgeStatus = "warning" | "success" | "fail";

@Component({
  imports: [],
  selector: "app-badge",
  styleUrl: "./badge.component.scss",
  templateUrl: "./badge.component.html",
})
export class BadgeComponent {
  connexionStatus = input.required<BadgeStatus>();
}

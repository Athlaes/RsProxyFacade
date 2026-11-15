import { Component } from "@angular/core";
import { MatExpansionModule } from "@angular/material/expansion";

@Component({
  imports: [MatExpansionModule],
  selector: "app-expansion-panel",
  styleUrl: "./expansion-panel.component.scss",
  templateUrl: "./expansion-panel.component.html",
})
export class ExpansionPanelComponent {}

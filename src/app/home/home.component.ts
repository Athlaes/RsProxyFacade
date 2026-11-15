import { Component, inject } from "@angular/core";
import { MatExpansionModule } from "@angular/material/expansion";
import { ProxyServerComponent } from "./proxy-server/proxy-server.component";
import { PacServerComponent } from "./pac-server/pac-server.component";

@Component({
  imports: [MatExpansionModule, ProxyServerComponent, PacServerComponent],
  selector: "app-home",
  styleUrl: "./home.component.scss",
  templateUrl: "./home.component.html",
})
export class HomeComponent {
}

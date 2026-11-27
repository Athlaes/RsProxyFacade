import { Component, computed, DestroyRef, effect, inject, input, OnInit, output, signal } from "@angular/core";
import { takeUntilDestroyed } from "@angular/core/rxjs-interop";
import { catchError, EMPTY, repeat, switchMap, timer } from "rxjs";
import { MatSnackBar } from "@angular/material/snack-bar";
import { BadgeComponent, BadgeStatus } from "../../design-system/badge/badge.component";

@Component({
  imports: [BadgeComponent],
  selector: "app-server-status",
  styleUrl: "./server-status.component.scss",
  templateUrl: "./server-status.component.html",
})
export class ServerStatusComponent implements OnInit {
  private readonly destroyRef = inject(DestroyRef);
  private readonly snackBarService = inject(MatSnackBar);
  status = signal<BadgeStatus>("fail");
  pollingMethod = input.required<() => Promise<BadgeStatus>>();
  statusChange = output<BadgeStatus>();

  private readonly emitStatusChange = effect(() => {
    const s = this.status();
    this.statusChange.emit(s);
  });

  message = computed(() => {
    let s = this.status();
    switch (s) {
      case "fail":
        return "Server disconnected";
      case "success":
        return "Server connected";
      case "warning":
        return "Server connecting";
    }
  });

  ngOnInit(): void {
    timer(1000)
      .pipe(
        switchMap(() => this.pollingMethod()()),
        catchError(() => {
          this.snackBarService.open("Couldn't get proxy status", "Close", {
            duration: 4000,
            panelClass: ["app-snackbar", "fail"],
          });
          return EMPTY;
        }),
        repeat(),
        takeUntilDestroyed(this.destroyRef),
      )
      .subscribe((status) => this.status.set(status));
  }
}

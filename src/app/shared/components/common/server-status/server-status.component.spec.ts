import { ComponentFixture, TestBed } from "@angular/core/testing";
import { MatSnackBar } from "@angular/material/snack-bar";
import { vi } from "vitest";
import { BadgeStatus } from "../../design-system/badge/badge.component";
import { ServerStatusComponent } from "./server-status.component";

describe("ServerStatusComponent", () => {
  let fixture: ComponentFixture<ServerStatusComponent>;
  const notify = vi.fn();

  beforeEach(() => {
    vi.useFakeTimers();
    notify.mockClear();
    TestBed.configureTestingModule({
      imports: [ServerStatusComponent],
      providers: [{ provide: MatSnackBar, useValue: { open: notify } }],
    });
    fixture = TestBed.createComponent(ServerStatusComponent);
  });

  afterEach(() => {
    fixture.destroy();
    vi.useRealTimers();
  });

  it("waits for the previous read and another second before polling again", async () => {
    let finish!: (status: BadgeStatus) => void;
    const pending = new Promise<BadgeStatus>((resolve) => { finish = resolve; });
    const read = vi.fn(() => pending);
    fixture.componentRef.setInput("pollingMethod", read);
    fixture.detectChanges();

    await vi.advanceTimersByTimeAsync(5000);
    expect(read).toHaveBeenCalledTimes(1);
    finish("success");
    await vi.advanceTimersByTimeAsync(999);
    expect(fixture.componentInstance.status()).toBe("success");
    expect(read).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(read).toHaveBeenCalledTimes(2);

    fixture.destroy();
    await vi.advanceTimersByTimeAsync(2000);
    expect(read).toHaveBeenCalledTimes(2);
  });

  it("retries after an error and ignores a rejection after destruction", async () => {
    let fail!: (error: Error) => void;
    const pending = new Promise<BadgeStatus>((_, reject) => { fail = reject; });
    const read = vi.fn(() => pending).mockRejectedValueOnce(new Error("Unavailable"));
    fixture.componentRef.setInput("pollingMethod", read);
    fixture.detectChanges();

    await vi.advanceTimersByTimeAsync(2000);
    expect(notify).toHaveBeenCalledTimes(1);
    expect(read).toHaveBeenCalledTimes(2);
    fixture.destroy();
    fail(new Error("Stopped"));
    await vi.advanceTimersByTimeAsync(2000);
    expect(notify).toHaveBeenCalledTimes(1);
    expect(read).toHaveBeenCalledTimes(2);
  });
});

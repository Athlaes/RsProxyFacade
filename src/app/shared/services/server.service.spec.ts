import { TestBed } from "@angular/core/testing";
import { MatSnackBar } from "@angular/material/snack-bar";
import { vi } from "vitest";
import { CommandService } from "./command.service";
import { ServerService } from "./server.service";

describe("ServerService startup errors", () => {
  const open = vi.fn();
  const commands = { startProxyServer: vi.fn(), startProxyPacServer: vi.fn() };
  let service: ServerService;

  beforeEach(() => {
    vi.resetAllMocks();
    TestBed.configureTestingModule({
      providers: [
        { provide: CommandService, useValue: commands },
        { provide: MatSnackBar, useValue: { open } },
      ],
    });
    service = TestBed.inject(ServerService);
  });

  it("displays the proxy server's binding error verbatim", async () => {
    const message = "Couldn't start proxy server on localhost:3128";
    commands.startProxyServer.mockRejectedValue(message);
    await service.startProxyServer({ host: "localhost", port: 3128, proxyMode: "direct" });
    expect(open.mock.calls[0][0]).toBe(message);
  });

  it("displays the PAC server's loading error verbatim", async () => {
    const message = "Couldn't load proxy pac file at path file:///missing.pac";
    commands.startProxyPacServer.mockRejectedValue(message);
    await service.startProxyPacServer({ host: "localhost", port: 3129, pacUrl: "file:///missing.pac" });
    expect(open.mock.calls[0][0]).toBe(message);
  });

  it("displays the fallback when the rejection is not a string", async () => {
    commands.startProxyServer.mockRejectedValue({ unexpected: true });
    await service.startProxyServer({ host: "localhost", port: 3128, proxyMode: "system" });
    expect(open.mock.calls[0][0]).toBe("Unknown error occured");
  });
});

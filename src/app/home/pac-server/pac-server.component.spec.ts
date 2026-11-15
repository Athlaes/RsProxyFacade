import { ComponentFixture, TestBed } from "@angular/core/testing";
import { PacServerComponent } from "./pac-server.component";

describe("PacOverrideComponent", () => {
  let component: PacServerComponent;
  let fixture: ComponentFixture<PacServerComponent>;

  beforeEach(async () => {
    await TestBed.configureTestingModule({
      imports: [PacServerComponent],
    }).compileComponents();

    fixture = TestBed.createComponent(PacServerComponent);
    component = fixture.componentInstance;
    await fixture.whenStable();
  });

  it("should create", () => {
    expect(component).toBeTruthy();
  });
});

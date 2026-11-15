import { ComponentFixture, TestBed } from '@angular/core/testing';
import { PanelSectionComponent } from './panel-section.component';

describe('CardSectionComponent', () => {
  let component: PanelSectionComponent;
  let fixture: ComponentFixture<PanelSectionComponent>;

  beforeEach(async () => {
    await TestBed.configureTestingModule({
      imports: [PanelSectionComponent]
    })
      .compileComponents();

    fixture = TestBed.createComponent(PanelSectionComponent);
    component = fixture.componentInstance;
    await fixture.whenStable();
  });

  it('should create', () => {
    expect(component).toBeTruthy();
  });
});

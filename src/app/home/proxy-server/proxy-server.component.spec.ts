import { ComponentFixture, TestBed } from '@angular/core/testing';
import { ProxyServerComponent } from './proxy-server.component';

describe('ProxyServerComponent', () => {
  let component: ProxyServerComponent;
  let fixture: ComponentFixture<ProxyServerComponent>;

  beforeEach(async () => {
    await TestBed.configureTestingModule({
      imports: [ProxyServerComponent]
    })
      .compileComponents();

    fixture = TestBed.createComponent(ProxyServerComponent);
    component = fixture.componentInstance;
    await fixture.whenStable();
  });

  it('should create', () => {
    expect(component).toBeTruthy();
  });
});

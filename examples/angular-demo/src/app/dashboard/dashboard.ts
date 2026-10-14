import { Component, input } from '@angular/core';
import type { Product } from '../catalog/products.contract';
import { DashboardSection } from './dashboard-section';

@Component({
  selector: 'app-dashboard',
  imports: [DashboardSection],
  templateUrl: './dashboard.html',
  styleUrl: './dashboard.css',
})
export class Dashboard {
  readonly products = input.required<readonly Product[]>();
}

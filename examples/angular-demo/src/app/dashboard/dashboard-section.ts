import { Component, input } from '@angular/core';
import type { Product } from '../catalog/products.contract';
import { ProductCard } from './product-card';

@Component({
  selector: 'app-dashboard-section',
  imports: [ProductCard],
  templateUrl: './dashboard-section.html',
  styleUrl: './dashboard-section.css',
})
export class DashboardSection {
  readonly title = input.required<string>();
  readonly products = input.required<readonly Product[]>();
}

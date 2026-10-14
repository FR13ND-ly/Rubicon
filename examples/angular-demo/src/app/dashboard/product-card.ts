import { Component, input, signal } from '@angular/core';
import type { Product } from '../catalog/products.contract';
import { ProductDetails } from './product-details';

@Component({
  selector: 'app-product-card',
  imports: [ProductDetails],
  templateUrl: './product-card.html',
  styleUrl: './product-card.css',
})
export class ProductCard {
  readonly product = input.required<Product>();
  protected readonly detailsVisible = signal(true);

  protected toggleDetails(): void {
    this.detailsVisible.update((visible) => !visible);
  }
}

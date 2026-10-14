import { Component, input } from '@angular/core';
import type { Product } from '../catalog/products.contract';

@Component({
  selector: 'app-product-details',
  templateUrl: './product-details.html',
})
export class ProductDetails {
  readonly product = input.required<Product>();
}

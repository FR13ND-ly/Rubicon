import { Component, input } from '@angular/core';
import type { Product } from '../catalog/products.contract';

@Component({
  selector: 'app-products',
  templateUrl: './products.html',
})
export class Products {
  readonly products = input.required<readonly Product[]>();
}
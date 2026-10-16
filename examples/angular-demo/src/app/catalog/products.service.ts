import { HttpClient } from '@angular/common/http';
import { inject, Injectable } from '@angular/core';

import type { Product } from './products.contract';

@Injectable({ providedIn: 'root' })
export class ProductsService {
  private readonly http = inject(HttpClient);

  listProducts() {
    return this.http.get<readonly Product[]>('/api/products');
  }
}
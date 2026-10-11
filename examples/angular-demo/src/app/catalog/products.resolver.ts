import { inject } from '@angular/core';
import type { ResolveFn } from '@angular/router';

import type { Product } from './products.contract';
import { PRODUCTS_DATA_SOURCE } from './products.token';

export const productsResolver: ResolveFn<readonly Product[]> = () => {
  const source = inject(PRODUCTS_DATA_SOURCE);

  return source.listProducts();
};
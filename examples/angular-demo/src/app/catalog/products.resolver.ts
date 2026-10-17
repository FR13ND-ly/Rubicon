import { inject } from '@angular/core';
import type { ResolveFn } from '@angular/router';

import type { Product } from './products.contract';
import { ProductsService } from './products.service';

export const productsResolver: ResolveFn<readonly Product[]> = () =>
  inject(ProductsService).listProducts();

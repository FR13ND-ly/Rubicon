import { InjectionToken } from '@angular/core';
import type { ProductsDataSource } from './products.contract';

export const PRODUCTS_DATA_SOURCE =
  new InjectionToken<ProductsDataSource>('ProductsDataSource');
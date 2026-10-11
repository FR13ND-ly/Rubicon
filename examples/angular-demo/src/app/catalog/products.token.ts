import { InjectionToken, makeStateKey } from '@angular/core';
import type { Product, ProductsDataSource } from './products.contract';

export const PRODUCTS_DATA_SOURCE =
  new InjectionToken<ProductsDataSource>('ProductsDataSource');

export const PRODUCTS_STATE_KEY =
  makeStateKey<readonly Product[]>('rubicon-demo:products:v1');
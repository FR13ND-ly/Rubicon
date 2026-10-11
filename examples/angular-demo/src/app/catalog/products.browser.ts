import { inject, Injectable, TransferState } from '@angular/core';
import type { Product, ProductsDataSource } from './products.contract';
import { PRODUCTS_STATE_KEY } from './products.token';

@Injectable()
export class BrowserProductsDataSource implements ProductsDataSource {
  private readonly state = inject(TransferState);

  async listProducts(): Promise<readonly Product[]> {
    if (this.state.hasKey(PRODUCTS_STATE_KEY)) {
      const products = this.state.get(PRODUCTS_STATE_KEY, []);

      this.state.remove(PRODUCTS_STATE_KEY);

      return products;
    }

    const response = await fetch('/api/products');

    if (!response.ok) {
      throw new Error(
        `Products request failed with status ${response.status}`,
      );
    }

    const products: unknown = await response.json();

    if (!isProductList(products)) {
      throw new Error('Products API returned an invalid product list');
    }

    return products;
  }
}

function isProductList(value: unknown): value is readonly Product[] {
  return (
    Array.isArray(value) &&
    value.every(
      (product: unknown) =>
        typeof product === 'object' &&
        product !== null &&
        'id' in product &&
        typeof product.id === 'string' &&
        'name' in product &&
        typeof product.name === 'string',
    )
  );
}
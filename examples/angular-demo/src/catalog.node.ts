import type { Product } from './app/catalog/products.contract';

// Match the Rust catalog, including fresh records for every call.
export async function listProducts(): Promise<readonly Product[]> {
  return [
    { id: 'keyboard', name: 'Tastatură' },
    { id: 'mouse', name: 'Mouse' },
    { id: 'monitor', name: 'Monitor' },
  ];
}

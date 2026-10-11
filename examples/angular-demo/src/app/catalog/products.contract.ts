export interface Product {
  readonly id: string;
  readonly name: string;
}

export interface ProductsDataSource {
  listProducts(): Promise<readonly Product[]>;
}

export interface ProductsRequestContext {
  readonly products: ProductsDataSource;
}
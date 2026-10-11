import {
  inject,
  mergeApplicationConfig,
  REQUEST_CONTEXT,
  TransferState,
  type ApplicationConfig,
} from '@angular/core';
import { provideServerRendering, withRoutes } from '@angular/ssr';

import { appConfig } from './app.config';
import { serverRoutes } from './app.routes.server';
import type {
  ProductsDataSource,
  ProductsRequestContext,
} from './catalog/products.contract';
import {
  PRODUCTS_DATA_SOURCE,
  PRODUCTS_STATE_KEY,
} from './catalog/products.token';

const serverConfig: ApplicationConfig = {
  providers: [
    provideServerRendering(withRoutes(serverRoutes)),
    {
      provide: PRODUCTS_DATA_SOURCE,
      useFactory: (): ProductsDataSource => {
        const context = inject(REQUEST_CONTEXT) as
          | ProductsRequestContext
          | null
          | undefined;

        if (
          !context?.products ||
          typeof context.products.listProducts !== 'function'
        ) {
          throw new Error(
            'SSR products data source is missing from the request context',
          );
        }

        const source = context.products;
        const state = inject(TransferState);

        return {
          async listProducts(): Promise<
            Awaited<ReturnType<ProductsDataSource['listProducts']>>
          > {
            const products = await source.listProducts();

            state.set(PRODUCTS_STATE_KEY, products);

            return products;
          },
        };
      },
    },
  ],
};

export const config = mergeApplicationConfig(appConfig, serverConfig);
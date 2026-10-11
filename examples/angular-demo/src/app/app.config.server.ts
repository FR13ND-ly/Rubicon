import {
  inject,
  mergeApplicationConfig,
  REQUEST_CONTEXT,
  type ApplicationConfig,
} from '@angular/core';
import { provideServerRendering, withRoutes } from '@angular/ssr';

import { appConfig } from './app.config';
import { serverRoutes } from './app.routes.server';
import type {
  ProductsRequestContext,
} from './catalog/products.contract';
import { PRODUCTS_DATA_SOURCE } from './catalog/products.token';

const serverConfig: ApplicationConfig = {
  providers: [
    provideServerRendering(withRoutes(serverRoutes)),
    {
      provide: PRODUCTS_DATA_SOURCE,
      useFactory: () => {
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

        return context.products;
      },
    },
  ],
};

export const config = mergeApplicationConfig(appConfig, serverConfig);
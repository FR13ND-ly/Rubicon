import {
  provideBrowserGlobalErrorListeners,
  type ApplicationConfig,
} from '@angular/core';
import { provideClientHydration } from '@angular/platform-browser';
import {
  provideRouter,
  withComponentInputBinding,
} from '@angular/router';

import { routes } from './app.routes';
import { BrowserProductsDataSource } from './catalog/products.browser';
import { PRODUCTS_DATA_SOURCE } from './catalog/products.token';

export const appConfig: ApplicationConfig = {
  providers: [
    provideBrowserGlobalErrorListeners(),
    provideRouter(routes, withComponentInputBinding()),
    provideClientHydration(),
    {
      provide: PRODUCTS_DATA_SOURCE,
      useClass: BrowserProductsDataSource,
    },
  ],
};
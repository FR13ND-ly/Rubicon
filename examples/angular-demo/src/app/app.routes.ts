import type { Routes } from '@angular/router';

import { Home } from './home/home';
import { About } from './about/about';
import { Products } from './products/products';
import { productsResolver } from './catalog/products.resolver';

export const routes: Routes = [
  { path: '', component: Home, title: 'Home | Rubicon demo' },
  { path: 'about', component: About, title: 'About | Rubicon demo' },
  {
    path: 'products',
    component: Products,
    title: 'Products | Rubicon demo',
    resolve: {
      products: productsResolver,
    },
  },
];
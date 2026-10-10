import { Routes } from '@angular/router';
import { Home } from './home/home';
import { About } from './about/about';

export const routes: Routes = [
  { path: '', component: Home, title: 'Home | Rubicon demo' },
  { path: 'about', component: About, title: 'About | Rubicon demo' },
];
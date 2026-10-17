import {
  createNativeBackend,
  createRubiconServer,
} from '@rubicon/angular/server';

import { App } from './app/app';
import { config } from './app/app.config.server';

export default createRubiconServer({
  application: { component: App, config },
  backend: createNativeBackend(),
});

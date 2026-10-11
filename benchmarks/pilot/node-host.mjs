import { createServer } from 'node:http';
import { reqHandler } from '../../examples/angular-demo/dist/angular-demo/server/server.mjs';

// Keep the generated Express/Angular handler; only bind it to loopback.
createServer(reqHandler).listen(4000, '127.0.0.1', () => {
  console.log('Pilot Node reference listening on http://127.0.0.1:4000');
});

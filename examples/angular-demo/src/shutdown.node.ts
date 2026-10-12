import type { Server } from 'node:http';

export function attachShutdown(server: Server, managed: boolean): void {
  let stopping = false;

  const shutdown = () => {
    if (stopping) {
      return;
    }

    stopping = true;

    if (managed) {
      process.stdin.destroy();
    }

    server.close((error) => {
      if (error) {
        process.exitCode = 1;
        console.error('HTTP shutdown failed:', error);
      }
    });
  };

  process.on('SIGINT', shutdown);

  if (managed) {
    process.stdin.once('end', shutdown);

    process.stdin.once('error', (error) => {
      process.exitCode = 1;
      console.error('Shutdown input failed:', error);
      shutdown();
    });

    process.stdin.resume();
  }
}

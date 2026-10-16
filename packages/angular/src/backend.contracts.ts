export interface RubiconBackend {
  handle(request: Request): Promise<Response>;
}

export interface RubiconBackendOptions {
  prefix?: string;
}
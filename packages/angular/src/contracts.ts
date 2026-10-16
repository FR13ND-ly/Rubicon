export interface RubiconRequest {
  request_id: string;
  url: string;
  method: string;
  headers: [string, string][];
}

export interface RubiconResponse {
  status: number;
  headers: [string, string][];
  body: string | Uint8Array | ReadableStream<Uint8Array>;
}

export interface RubiconServerOptions<TContext = unknown> {
  createRequestContext?: (
    request: Request,
  ) => TContext | Promise<TContext>;
}

export interface RubiconServer {
  reqHandler(request: Request): Promise<Response | null>;

  render(request: RubiconRequest): Promise<RubiconResponse>;
}
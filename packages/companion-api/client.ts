export type CompanionMethod = "GET" | "POST" | "PUT" | "PATCH" | "DELETE";

export interface CompanionRequest {
  path: string;
  method: CompanionMethod;
  body?: unknown;
  token?: string;
  headers?: Readonly<Record<string, string>>;
}

export interface CompanionResponse {
  status: number;
  body?: string;
  bytes?: ArrayBuffer;
  contentType?: string;
  etag?: string;
}

export interface CompanionTransport {
  request(input: CompanionRequest): Promise<CompanionResponse>;
}

interface ErrorEnvelope {
  error?: { code?: string; message?: string; retryable?: boolean };
}

export class CompanionApiError extends Error {
  constructor(
    message: string,
    readonly status = 0,
    readonly code = "request_failed",
    readonly retryable = false,
  ) {
    super(message);
    this.name = "CompanionApiError";
  }
}

function errorEnvelope(body?: string): ErrorEnvelope {
  if (!body) return {};
  try {
    return JSON.parse(body) as ErrorEnvelope;
  } catch {
    return {};
  }
}

export function assertCompanionResponse(response: CompanionResponse): void {
  if (response.status >= 200 && response.status < 300) return;
  const envelope = errorEnvelope(response.body);
  throw new CompanionApiError(
    envelope.error?.message ?? `Volum returned ${response.status}`,
    response.status,
    envelope.error?.code,
    envelope.error?.retryable,
  );
}

export class CompanionClient {
  constructor(
    private readonly transport: CompanionTransport,
    private readonly token?: string,
  ) {}

  async json<T>(
    path: string,
    method: CompanionMethod = "GET",
    body?: unknown,
    headers?: Readonly<Record<string, string>>,
  ): Promise<T> {
    const response = await this.transport.request({ path, method, body, token: this.token, headers });
    assertCompanionResponse(response);
    if (response.status === 204 || !response.body) return undefined as T;
    try {
      return JSON.parse(response.body) as T;
    } catch {
      throw new CompanionApiError("Volum returned an unreadable response", response.status, "invalid_response");
    }
  }

  async binary(path: string, headers?: Readonly<Record<string, string>>): Promise<CompanionResponse> {
    const response = await this.transport.request({ path, method: "GET", token: this.token, headers });
    assertCompanionResponse(response);
    if (!response.bytes) {
      throw new CompanionApiError("Volum returned an unreadable binary response", response.status, "invalid_response");
    }
    return response;
  }
}

export function createFetchTransport(baseUrl = ""): CompanionTransport {
  return {
    async request(input) {
      const headers = new Headers(input.headers);
      headers.set("Accept", "application/json");
      if (input.token) headers.set("Authorization", `Bearer ${input.token}`);
      if (input.body !== undefined) headers.set("Content-Type", "application/json");
      const response = await fetch(`${baseUrl}${input.path}`, {
        method: input.method,
        headers,
        body: input.body === undefined ? undefined : JSON.stringify(input.body),
      });
      const contentType = response.headers.get("content-type") ?? undefined;
      return {
        status: response.status,
        body: response.status === 204 ? undefined : await response.text(),
        contentType,
        etag: response.headers.get("etag") ?? undefined,
      };
    },
  };
}

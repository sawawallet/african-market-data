/** Minimal fetch wrapper: timeout, bounded retry, honest errors. */

export interface HttpOptions {
  timeoutMs?: number;
  retries?: number;
  /** Called before each retry with the attempt number, 1-based. */
  onRetry?: (attempt: number, error: unknown) => void;
  signal?: AbortSignal;
  userAgent?: string;
}

const DEFAULT_UA =
  "african-market-data (+https://github.com/african-market-data)";

export class HttpError extends Error {
  constructor(message: string, readonly status: number, readonly url: string) {
    super(message);
    this.name = "HttpError";
  }
}

function sleep(ms: number, signal?: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    const t = setTimeout(resolve, ms);
    signal?.addEventListener("abort", () => {
      clearTimeout(t);
      reject(new DOMException("aborted", "AbortError"));
    }, { once: true });
  });
}

export async function getJson<T>(url: string, opts: HttpOptions = {}): Promise<T> {
  const { timeoutMs = 10_000, retries = 2, onRetry, signal, userAgent } = opts;
  let lastError: unknown;

  for (let attempt = 0; attempt <= retries; attempt++) {
    if (attempt > 0) {
      onRetry?.(attempt, lastError);
      // Exponential backoff, capped. No jitter needed at this request volume.
      await sleep(Math.min(250 * 2 ** (attempt - 1), 2000), signal);
    }
    const timeout = AbortSignal.timeout(timeoutMs);
    const composed = signal ? AbortSignal.any([signal, timeout]) : timeout;
    try {
      const res = await fetch(url, {
        signal: composed,
        headers: { accept: "application/json", "user-agent": userAgent ?? DEFAULT_UA },
      });
      if (!res.ok) {
        // 4xx other than 429 will not improve on retry.
        const retryable = res.status === 429 || res.status >= 500;
        const err = new HttpError(`HTTP ${res.status} for ${url}`, res.status, url);
        if (!retryable) throw err;
        lastError = err;
        continue;
      }
      return (await res.json()) as T;
    } catch (err) {
      if (err instanceof HttpError && err.status < 500 && err.status !== 429) throw err;
      if (signal?.aborted) throw err;
      lastError = err;
    }
  }
  throw lastError instanceof Error
    ? lastError
    : new Error(`request failed: ${url}`);
}

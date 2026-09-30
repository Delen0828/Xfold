import type { paths } from './schema';
export type ContractPaths = keyof paths;
export async function api<T>(path: string, body?: unknown, signal?: AbortSignal): Promise<T> {
  const response = await fetch(`/api/v1${path}`, { method: body === undefined ? 'GET' : 'POST', headers: body === undefined ? undefined : { 'Content-Type': 'application/json' }, body: body === undefined ? undefined : JSON.stringify(body), signal });
  const result = await response.json();
  if (!response.ok) throw new Error(result.error || `${response.status} ${response.statusText}`);
  return result;
}
export function params(values: Record<string, string | number | undefined | null>) {
  return new URLSearchParams(Object.entries(values).filter(([, value]) => value !== undefined && value !== null).map(([k, v]) => [k, String(v)])).toString();
}

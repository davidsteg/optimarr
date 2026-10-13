const KEY_STORAGE = 'optimarr.apiKey';

export class ApiError extends Error {
	constructor(
		public status: number,
		message: string
	) {
		super(message);
	}
}

export function getApiKey(): string {
	try {
		return localStorage.getItem(KEY_STORAGE) ?? '';
	} catch {
		return '';
	}
}

export function setApiKey(key: string) {
	try {
		if (key) localStorage.setItem(KEY_STORAGE, key);
		else localStorage.removeItem(KEY_STORAGE);
	} catch {
		/* storage unavailable */
	}
}

type Listener = () => void;
const unauthorizedListeners = new Set<Listener>();
export function onUnauthorized(fn: Listener) {
	unauthorizedListeners.add(fn);
	return () => unauthorizedListeners.delete(fn);
}

export type Query = Record<string, string | number | boolean | null | undefined>;

export function qs(params: Query = {}): string {
	const sp = new URLSearchParams();
	for (const [k, v] of Object.entries(params)) {
		if (v === undefined || v === null || v === '') continue;
		sp.set(k, String(v));
	}
	const s = sp.toString();
	return s ? `?${s}` : '';
}

async function request<T>(method: string, path: string, body?: unknown, query?: Query): Promise<T> {
	const headers: Record<string, string> = {};
	const key = getApiKey();
	if (key) headers['X-Api-Key'] = key;
	if (body !== undefined) headers['Content-Type'] = 'application/json';
	const res = await fetch(`/api${path}${qs(query)}`, {
		method,
		headers,
		body: body === undefined ? undefined : JSON.stringify(body)
	});
	if (res.status === 401) {
		unauthorizedListeners.forEach((fn) => fn());
	}
	const text = await res.text();
	let data: any = null;
	if (text) {
		try {
			data = JSON.parse(text);
		} catch {
			data = text;
		}
	}
	if (!res.ok) {
		const msg = (data && typeof data === 'object' && data.error) || res.statusText || 'Request failed';
		throw new ApiError(res.status, msg);
	}
	return data as T;
}

export const api = {
	get: <T>(path: string, query?: Query) => request<T>('GET', path, undefined, query),
	post: <T>(path: string, body?: unknown, query?: Query) => request<T>('POST', path, body ?? {}, query),
	put: <T>(path: string, body: unknown) => request<T>('PUT', path, body),
	del: <T>(path: string, query?: Query) => request<T>('DELETE', path, undefined, query)
};

/** URL for a file download that carries the API key (anchors cannot set headers). */
export function downloadUrl(path: string, query: Query = {}): string {
	const key = getApiKey();
	return `/api${path}${qs({ ...query, apikey: key || undefined })}`;
}

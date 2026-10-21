// Client for the veilid-taxi-server REST API.
// Requests go through the `/api` prefix, which the Vite dev server proxies
// to the Rust server (see vite.config.ts).

const API = '/api';

export interface CreateRequest {
	latitude: number;
	longitude: number;
	max_price_cents?: number;
}

export interface CreatedRequest {
	request_key: string;
	route_id: string;
	board_key: string;
	slot: number;
}

export interface RideRequest {
	request_key: string; // DHT record holding the request
	route_blob: Uint8Array; // pedestrian's private route, for sending offers
	cell: string; // H3 cell as a hex string (u64 doesn't fit a JS number)
	expires_at: number; // unix secs
	max_price_cents: number;
}

export interface AcceptedRequest {
	request_key: string;
	driver_route: Uint8Array; // driver's private route, for the pedestrian to reply
	pedestrian_route: Uint8Array;
}

/** An H3 cell to highlight on the map. */
export interface Area {
	cell: string; // H3 index, hex
	label: string;
}

async function check(res: Response): Promise<Response> {
	if (!res.ok) {
		const text = await res.text();
		throw new Error(text || `${res.status} ${res.statusText}`);
	}
	return res;
}

/** Publish a ride request as a pedestrian. */
export async function createRequest(body: CreateRequest): Promise<CreatedRequest> {
	const res = await fetch(`${API}/requests`, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify(body)
	});
	return (await check(res)).json();
}

/** List ride requests near a location as a driver. */
export async function listRequests(
	latitude: number,
	longitude: number
): Promise<RideRequest[]> {
	const params = new URLSearchParams({ latitude: String(latitude), longitude: String(longitude) });
	const res = await fetch(`${API}/requests?${params}`);
	// Quote the u64 cell before parsing so it isn't rounded to a float.
	const text = (await (await check(res)).text()).replace(/"cell":(\d+)/g, '"cell":"$1"');
	const rows: (Omit<RideRequest, 'route_blob'> & { route_blob: number[] })[] = JSON.parse(text);
	return rows.map((row) => ({
		...row,
		cell: BigInt(row.cell).toString(16),
		route_blob: Uint8Array.from(row.route_blob)
	}));
}

/**
 * Accept a ride request as a driver: sends `message` to the pedestrian's private route.
 * The server looks the request up by key, so it must come from a prior `listRequests`.
 */
export async function acceptRequest(requestKey: string, message: string): Promise<AcceptedRequest> {
	const res = await fetch(`${API}/requests/accept`, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ request_key: requestKey, message })
	});
	const body: { request_key: string; driver_route: number[]; pedestrian_route: number[] } = await (
		await check(res)
	).json();
	return {
		request_key: body.request_key,
		driver_route: Uint8Array.from(body.driver_route),
		pedestrian_route: Uint8Array.from(body.pedestrian_route)
	};
}

export function toBase64(bytes: Uint8Array): string {
	return btoa(String.fromCharCode(...bytes));
}

/** Ask the browser for the current position. */
export function currentPosition(): Promise<{ latitude: number; longitude: number }> {
	return new Promise((resolve, reject) => {
		if (!navigator.geolocation) {
			reject(new Error('Geolocation is not available in this browser'));
			return;
		}
		navigator.geolocation.getCurrentPosition(
			(pos) => resolve({ latitude: pos.coords.latitude, longitude: pos.coords.longitude }),
			(err) => reject(new Error(err.message)),
			{ enableHighAccuracy: true, timeout: 10_000 }
		);
	});
}

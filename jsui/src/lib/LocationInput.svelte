<script lang="ts">
	import 'leaflet/dist/leaflet.css';
	import type { LayerGroup, Map as LeafletMap, Marker } from 'leaflet';
	import { cellToBoundary } from 'h3-js';
	import { currentPosition, type Area } from '#lib/api.js';

	let {
		latitude = $bindable(),
		longitude = $bindable(),
		areas = []
	}: { latitude: number; longitude: number; areas?: Area[] } = $props();

	let locating = $state(false);
	let error = $state('');

	let container: HTMLDivElement;
	let map = $state<LeafletMap>();
	let marker = $state<Marker>();
	let areaLayer = $state<LayerGroup>();
	let leaflet = $state<typeof import('leaflet')>();

	// Leaflet touches `window` on import, so load it in the browser only.
	$effect(() => {
		let disposed = false;
		let created: LeafletMap | undefined;
		import('leaflet').then((L) => {
			if (disposed) return;
			const hasPos = latitude !== 0 || longitude !== 0;
			const m = L.map(container).setView([latitude, longitude], hasPos ? 15 : 2);
			L.tileLayer('https://tile.openstreetmap.org/{z}/{x}/{y}.png', {
				maxZoom: 19,
				attribution: '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a>'
			}).addTo(m);

			// divIcon avoids Leaflet's default marker images, which bundlers break
			const icon = L.divIcon({ className: 'pin', iconSize: [18, 18] });
			const mk = L.marker([latitude, longitude], { icon, draggable: true }).addTo(m);
			mk.on('dragend', () => {
				({ lat: latitude, lng: longitude } = mk.getLatLng());
			});
			m.on('click', (e) => {
				({ lat: latitude, lng: longitude } = e.latlng);
			});

			created = map = m;
			marker = mk;
			areaLayer = L.layerGroup().addTo(m);
			leaflet = L;
		});
		return () => {
			disposed = true;
			created?.remove();
		};
	});

	// Keep the marker in sync with typed coordinates and "Use my location".
	$effect(() => {
		if (!map || !marker) return;
		if (!Number.isFinite(latitude) || !Number.isFinite(longitude)) return;
		marker.setLatLng([latitude, longitude]);
		if (!map.getBounds().contains([latitude, longitude])) map.panTo([latitude, longitude]);
	});

	// Draw H3 cells passed in by the parent (e.g. ride requests nearby).
	$effect(() => {
		if (!leaflet || !areaLayer) return;
		areaLayer.clearLayers();
		for (const area of areas) {
			leaflet
				.polygon(cellToBoundary(area.cell), { className: 'area', weight: 2 })
				.bindTooltip(area.label)
				.addTo(areaLayer);
		}
	});

	async function locate() {
		locating = true;
		error = '';
		try {
			({ latitude, longitude } = await currentPosition());
			map?.setView([latitude, longitude], 15);
		} catch (e) {
			error = (e as Error).message;
		} finally {
			locating = false;
		}
	}
</script>

<div class="map" bind:this={container}></div>
<p class="hint">Click the map or drag the pin to set the location.</p>

<div class="row">
	<label>
		Latitude
		<input type="number" step="any" min="-90" max="90" bind:value={latitude} required />
	</label>
	<label>
		Longitude
		<input type="number" step="any" min="-180" max="180" bind:value={longitude} required />
	</label>
	<button type="button" class="secondary" onclick={locate} disabled={locating}>
		{locating ? 'Locating…' : 'Use my location'}
	</button>
</div>
{#if error}<p class="error">{error}</p>{/if}

<style>
	.map {
		height: 20rem;
		border: 1px solid var(--border);
		border-radius: 8px;
		z-index: 0;
	}
	.hint {
		margin: -0.5rem 0 0;
		font-size: 0.8rem;
		color: var(--muted);
	}
	.row {
		display: flex;
		flex-wrap: wrap;
		gap: 0.75rem;
		align-items: end;
	}
	label {
		flex: 1 1 8rem;
	}
	:global(.area) {
		stroke: var(--accent);
		fill: var(--accent);
		fill-opacity: 0.25;
	}
	:global(.pin) {
		background: var(--accent);
		border: 3px solid #1b1b1b;
		border-radius: 50%;
		box-sizing: border-box;
	}
</style>

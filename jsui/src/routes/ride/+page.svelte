<script lang="ts">
	import { createRequest, type CreatedRequest } from '#lib/api.js';
	import LocationInput from '#lib/LocationInput.svelte';

	let latitude = $state(0);
	let longitude = $state(0);
	let maxPrice = $state(1.0); // in currency units, sent as cents

	let submitting = $state(false);
	let error = $state('');
	let created = $state<CreatedRequest[]>([]);

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		submitting = true;
		error = '';
		try {
			const res = await createRequest({
				latitude,
				longitude,
				max_price_cents: Math.round(maxPrice * 100)
			});
			created = [res, ...created];

			console.log("~~~");
			console.log(res);
			console.log("~~~");


		} catch (e) {
			error = (e as Error).message;
		} finally {
			submitting = false;
		}
	}
</script>

<h1>Request a ride</h1>

<form class="card" onsubmit={submit}>
	<LocationInput bind:latitude bind:longitude />
	<label>
		Max price
		<input type="number" step="0.01" min="0" bind:value={maxPrice} required />
	</label>
	<button type="submit" disabled={submitting}>
		{submitting ? 'Publishing to the DHT…' : 'Request ride'}
	</button>
	{#if error}<p class="error">{error}</p>{/if}
</form>

{#if created.length}
	<h2>Your requests</h2>
	<ul>
		{#each created as req (req.request_key)}
			<li class="card">
				<dl>
					<dt>Request</dt>
					<dd class="mono">{req.request_key}</dd>
					<dt>Board</dt>
					<dd class="mono">{req.board_key} · slot {req.slot}</dd>
					<dt>Route</dt>
					<dd class="mono">{req.route_id}</dd>
				</dl>
			</li>
		{/each}
	</ul>
{/if}

<style>
	form {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}
	form button[type='submit'] {
		align-self: start;
	}
	ul {
		list-style: none;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}
	dl {
		display: grid;
		grid-template-columns: auto 1fr;
		gap: 0.25rem 0.75rem;
		margin: 0;
	}
	dt {
		color: var(--muted);
		font-size: 0.875rem;
	}
	dd {
		margin: 0;
	}
</style>

<script lang="ts">
	import { acceptRequest, listRequests, toBase64, type Area, type FoundRequest } from '#lib/api.js';
	import LocationInput from '#lib/LocationInput.svelte';

	let latitude = $state(0);
	let longitude = $state(0);

	let loading = $state(false);
	let error = $state('');
	let requests = $state<FoundRequest[] | null>(null);
	let copied = $state<number | null>(null);
	// Per-request accept state, keyed by list index
	let accepting = $state<Record<number, 'sending' | 'sent' | string>>({});
	let now = $state(Date.now() / 1000);

	$effect(() => {
		const id = setInterval(() => (now = Date.now() / 1000), 1000);
		return () => clearInterval(id);
	});

	let areas = $derived<Area[]>(
		(requests ?? []).map(({ request }, i) => ({
			cell: request.cell,
			label: `#${i + 1} · ${formatPrice(request.max_price_cents)}`
		}))
	);

	async function search(e: SubmitEvent) {
		e.preventDefault();
		loading = true;
		error = '';
		try {
			requests = await listRequests(latitude, longitude);
			accepting = {};
		} catch (e) {
			error = (e as Error).message;
		} finally {
			loading = false;
		}
	}

	async function copyRoute(i: number, blob: Uint8Array) {
		await navigator.clipboard.writeText(toBase64(blob));
		copied = i;
		setTimeout(() => (copied = copied === i ? null : copied), 1500);
	}

	async function accept(i: number, blob: Uint8Array) {
		accepting[i] = 'sending';
		try {
			await acceptRequest(blob, 'hello');
			accepting[i] = 'sent';
		} catch (e) {
			accepting[i] = (e as Error).message;
		}
	}

	function formatPrice(cents: number): string {
		return `max ${(cents / 100).toFixed(2)}`;
	}

	function expiresIn(expiresAt: number): string {
		const secs = Math.floor(expiresAt - now);
		if (secs <= 0) return 'expired';
		const m = Math.floor(secs / 60);
		const s = secs % 60;
		return `${m}:${String(s).padStart(2, '0')}`;
	}
</script>

<h1>Find riders</h1>

<form class="card" onsubmit={search}>
	<LocationInput bind:latitude bind:longitude {areas} />
	<button type="submit" disabled={loading}>
		{loading ? 'Scanning boards…' : 'List requests'}
	</button>
	{#if error}<p class="error">{error}</p>{/if}
</form>

{#if requests}
	<h2>{requests.length} request{requests.length === 1 ? '' : 's'} nearby</h2>
	{#if requests.length}
		<ul>
			{#each requests as { request, route_blob }, i (i)}
				<li class="card" class:expired={request.expires_at <= now}>
					<div class="head">
						<strong>#{i + 1}</strong>
						<span class="price">{formatPrice(request.max_price_cents)}</span>
						<span class="ttl">{expiresIn(request.expires_at)}</span>
					</div>
					<dl>
						<dt>Cell</dt>
						<dd class="mono">{request.cell}</dd>
						<dt>Route</dt>
						<dd>
							{route_blob.length} bytes
							<button type="button" class="secondary small" onclick={() => copyRoute(i, route_blob)}>
								{copied === i ? 'Copied' : 'Copy base64'}
							</button>
						</dd>
					</dl>
					<div class="actions">
						<button
							type="button"
							disabled={request.expires_at <= now ||
								accepting[i] === 'sending' ||
								accepting[i] === 'sent'}
							onclick={() => accept(i, route_blob)}
						>
							{accepting[i] === 'sending'
								? 'Sending…'
								: accepting[i] === 'sent'
									? 'Accepted'
									: 'Accept request'}
						</button>
						{#if accepting[i] && accepting[i] !== 'sending' && accepting[i] !== 'sent'}
							<span class="error">{accepting[i]}</span>
						{/if}
					</div>
				</li>
			{/each}
		</ul>
	{:else}
		<p class="muted">No ride requests in your area right now.</p>
	{/if}
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
		gap: 0.5rem;
	}
	li {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		padding: 0.75rem 1rem;
	}
	li.expired {
		opacity: 0.5;
	}
	.head {
		display: flex;
		gap: 1rem;
		align-items: baseline;
	}
	.price {
		flex: 1;
	}
	.ttl {
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}
	dl {
		display: grid;
		grid-template-columns: auto 1fr;
		gap: 0.25rem 0.75rem;
		margin: 0;
		align-items: center;
	}
	dt {
		color: var(--muted);
		font-size: 0.875rem;
	}
	dd {
		margin: 0;
	}
	.small {
		padding: 0.2rem 0.6rem;
		font-size: 0.8rem;
		margin-left: 0.5rem;
	}
	.actions {
		display: flex;
		gap: 0.75rem;
		align-items: center;
	}
	.muted {
		color: var(--muted);
	}
</style>

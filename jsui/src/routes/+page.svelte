<h1>Decentralized taxi over Veilid</h1>
<p>Choose your role.</p>

<div class="choices">
	<a class="card" href="/ride">
		<h2>Ride</h2>
		<p>Publish a ride request from your location.</p>
	</a>
	<a class="card" href="/drive">
		<h2>Drive</h2>
		<p>List ride requests near you.</p>
	</a>
</div>

<style>
	.choices {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(14rem, 1fr));
		gap: 1rem;
	}
	.choices a {
		color: inherit;
		text-decoration: none;
	}
	.choices a:hover {
		border-color: var(--accent);
	}
	h2 {
		margin: 0 0 0.25rem;
	}
	.choices p {
		margin: 0;
		color: var(--muted);
	}
</style>

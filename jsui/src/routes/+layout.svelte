<script lang="ts">
	import favicon from '#lib/assets/favicon.svg';
	import { page } from '$app/state';
	import {establishWSConnection} from '#lib/api.js';
	import type { LayoutProps } from './$types';
	import { onMount } from 'svelte'

	let { children }: LayoutProps = $props();

	onMount(() => {
  	  var socket = establishWSConnection();

     console.log("here");

  	  socket.onmessage = function(event) {
        alert("Получены данные " + event.data);
     };
	});



</script>

<svelte:head>
	<link rel="icon" href={favicon} />
	<title>Veilid Taxi</title>
</svelte:head>

<header>
	<a class="brand" href="/">Veilid Taxi</a>
	<nav>
		<a href="/ride" aria-current={page.url.pathname === '/ride' ? 'page' : undefined}>Ride</a>
		<a href="/drive" aria-current={page.url.pathname === '/drive' ? 'page' : undefined}>Drive</a>
	</nav>
</header>

<main>
	{@render children()}
</main>

<style>
	:global(:root) {
		--bg: #f7f7f5;
		--fg: #1b1b1b;
		--muted: #6b6b6b;
		--card: #ffffff;
		--border: #dddddd;
		--accent: #f2b705;
		--accent-fg: #1b1b1b;
		--error: #c0392b;
		font-family: system-ui, sans-serif;
		color-scheme: light dark;
	}
	@media (prefers-color-scheme: dark) {
		:global(:root) {
			--bg: #151515;
			--fg: #ececec;
			--muted: #9a9a9a;
			--card: #1f1f1f;
			--border: #333333;
			--error: #ff6b5b;
		}
	}
	:global(body) {
		margin: 0;
		background: var(--bg);
		color: var(--fg);
	}
	:global(label) {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		font-size: 0.875rem;
		color: var(--muted);
	}
	:global(input) {
		font: inherit;
		padding: 0.5rem 0.6rem;
		border: 1px solid var(--border);
		border-radius: 6px;
		background: var(--card);
		color: var(--fg);
	}
	:global(button) {
		font: inherit;
		font-weight: 600;
		padding: 0.55rem 1rem;
		border: 1px solid transparent;
		border-radius: 6px;
		background: var(--accent);
		color: var(--accent-fg);
		cursor: pointer;
	}
	:global(button.secondary) {
		background: transparent;
		color: var(--fg);
		border-color: var(--border);
	}
	:global(button:disabled) {
		opacity: 0.6;
		cursor: default;
	}
	:global(.card) {
		background: var(--card);
		border: 1px solid var(--border);
		border-radius: 10px;
		padding: 1.25rem;
	}
	:global(.error) {
		color: var(--error);
	}
	:global(.mono) {
		font-family: ui-monospace, monospace;
		font-size: 0.8rem;
		word-break: break-all;
	}

	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.75rem 1rem;
		border-bottom: 1px solid var(--border);
	}
	.brand {
		font-weight: 700;
		text-decoration: none;
		color: var(--fg);
	}
	nav {
		display: flex;
		gap: 1rem;
	}
	nav a {
		color: var(--muted);
		text-decoration: none;
	}
	nav a[aria-current='page'] {
		color: var(--fg);
		font-weight: 600;
	}
	main {
		max-width: 44rem;
		margin: 0 auto;
		padding: 1.5rem 1rem;
	}
</style>

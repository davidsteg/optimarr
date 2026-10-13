<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '$lib/api';
	import Icon from './Icon.svelte';

	interface Listing {
		path: string;
		parent: string | null;
		directories: { name: string; path: string }[];
	}
	let { value = $bindable(''), onpick }: { value?: string; onpick?: (p: string) => void } = $props();
	let listing = $state<Listing | null>(null);
	let error = $state('');
	let loading = $state(false);

	async function load(path: string) {
		loading = true;
		error = '';
		try {
			listing = await api.get<Listing>('/fs/browse', { path });
			value = listing.path;
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			loading = false;
		}
	}

	onMount(() => {
		load(value || '/');
	});
</script>

<div class="picker">
	<div class="row bar">
		<button class="btn icon sm" disabled={!listing?.parent} onclick={() => listing?.parent && load(listing.parent)} aria-label="Up"><Icon name="up" /></button>
		<input
			type="text"
			class="grow mono"
			value={listing?.path ?? value}
			onkeydown={(e) => e.key === 'Enter' && load((e.target as HTMLInputElement).value)}
		/>
	</div>
	<div class="list">
		{#if error}
			<p class="err small">{error}</p>
		{:else if loading && !listing}
			<p class="faint small">Loading…</p>
		{:else if listing && listing.directories.length === 0}
			<p class="faint small">No sub-folders.</p>
		{:else if listing}
			{#each listing.directories as d (d.path)}
				<button class="entry" ondblclick={() => onpick?.(d.path)} onclick={() => load(d.path)}>
					<Icon name="folder" size={15} /><span class="ellipsis">{d.name}</span>
				</button>
			{/each}
		{/if}
	</div>
</div>

<style>
	.picker {
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-sm);
		overflow: hidden;
	}
	.bar {
		padding: 8px;
		border-bottom: 1px solid var(--border);
		background: var(--surface-2);
	}
	.bar input {
		height: 28px;
	}
	.list {
		max-height: 260px;
		overflow: auto;
		padding: 4px;
	}
	.entry {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
		text-align: left;
		padding: 6px 8px;
		border: 0;
		border-radius: 6px;
		background: none;
		color: var(--text);
		font: inherit;
		cursor: pointer;
	}
	.entry:hover {
		background: var(--surface-2);
	}
	.entry :global(svg) {
		color: var(--accent);
	}
	.list p {
		padding: 8px;
		margin: 0;
	}
	.err {
		color: var(--bad);
	}
</style>

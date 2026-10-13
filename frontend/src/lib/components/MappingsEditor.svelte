<script lang="ts">
	import type { PathMapping } from '$lib/types';
	import Icon from './Icon.svelte';
	let {
		mappings = $bindable([]),
		remoteLabel = 'Remote path',
		localLabel = 'Path in Optimarr'
	}: { mappings?: PathMapping[]; remoteLabel?: string; localLabel?: string } = $props();
</script>

<div class="stack" style="gap:8px">
	{#if mappings.length > 0}
		<div class="grid head small faint">
			<span>{remoteLabel}</span><span>{localLabel}</span><span></span>
		</div>
	{/if}
	{#each mappings as m, i (i)}
		<div class="grid">
			<input type="text" class="mono" placeholder="/data/media" bind:value={m.remote} />
			<input type="text" class="mono" placeholder="/media" bind:value={m.local} />
			<button class="btn icon ghost" onclick={() => mappings.splice(i, 1)} aria-label="Remove mapping"><Icon name="trash" /></button>
		</div>
	{/each}
	<div>
		<button class="btn sm" onclick={() => mappings.push({ remote: '', local: '' })}><Icon name="plus" size={14} />Add mapping</button>
	</div>
</div>

<style>
	.grid {
		display: grid;
		grid-template-columns: 1fr 1fr 34px;
		gap: 8px;
		align-items: center;
	}
	.head {
		margin-bottom: -2px;
	}
</style>

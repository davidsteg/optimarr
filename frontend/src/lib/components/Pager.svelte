<script lang="ts">
	import { num } from '$lib/format';
	import Icon from './Icon.svelte';
	let {
		page = $bindable(1),
		pageSize = $bindable(50),
		total
	}: { page?: number; pageSize?: number; total: number } = $props();
	const pages = $derived(Math.max(1, Math.ceil(total / pageSize)));
	const from = $derived(total === 0 ? 0 : (page - 1) * pageSize + 1);
	const to = $derived(Math.min(total, page * pageSize));
</script>

<div class="pager">
	<span class="muted small">{num(from)}–{num(to)} of {num(total)}</span>
	<div class="row">
		<select bind:value={pageSize} onchange={() => (page = 1)} aria-label="Rows per page">
			{#each [25, 50, 100, 200] as n}<option value={n}>{n} / page</option>{/each}
		</select>
		<button class="btn icon sm" disabled={page <= 1} onclick={() => (page = page - 1)} aria-label="Previous page"><Icon name="chevronLeft" /></button>
		<span class="small">Page {page} of {pages}</span>
		<button class="btn icon sm" disabled={page >= pages} onclick={() => (page = page + 1)} aria-label="Next page"><Icon name="chevronRight" /></button>
	</div>
</div>

<style>
	.pager {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 12px;
		flex-wrap: wrap;
		padding: 12px 16px;
		border-top: 1px solid var(--border);
	}
	select {
		height: 28px;
		font-size: 13px;
	}
</style>

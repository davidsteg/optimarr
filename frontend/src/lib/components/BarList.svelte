<script lang="ts">
	import type { BarItem } from '$lib/types';
	import { num } from '$lib/format';
	let { items, max = 0, empty = 'No data yet' }: { items: BarItem[]; max?: number; empty?: string } = $props();
	const top = $derived(max || Math.max(1, ...items.map((i) => i.value)));
</script>

{#if items.length === 0}
	<p class="faint small">{empty}</p>
{:else}
	<ul>
		{#each items as item (item.label)}
			<li>
				<div class="meta">
					{#if item.href}
						<a href={item.href} class="ellipsis">{item.label}</a>
					{:else}
						<span class="ellipsis">{item.label}</span>
					{/if}
					<span class="val">{item.hint ?? num(item.value)}</span>
				</div>
				<div class="track"><div class="fill {item.tone ?? 'accent'}" style="width:{(item.value / top) * 100}%"></div></div>
			</li>
		{/each}
	</ul>
{/if}

<style>
	ul {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.meta {
		display: flex;
		justify-content: space-between;
		gap: 12px;
		font-size: 13px;
		margin-bottom: 4px;
	}
	.val {
		color: var(--text-2);
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}
	.track {
		height: 6px;
		border-radius: 999px;
		background: var(--surface-3);
		overflow: hidden;
	}
	.fill {
		height: 100%;
		border-radius: 999px;
		background: var(--accent);
		transition: width 0.4s ease;
	}
	.good {
		background: var(--good);
	}
	.ok {
		background: var(--ok);
	}
	.warn {
		background: var(--warn);
	}
	.bad {
		background: var(--bad);
	}
	.muted {
		background: var(--text-3);
	}
</style>

<script lang="ts">
	import type { Segment } from '$lib/types';
	import { num, pct, titleCase } from '$lib/format';
	let { segments }: { segments: Segment[] } = $props();
	const total = $derived(segments.reduce((a, s) => a + s.count, 0));
</script>

<div class="bar" role="img" aria-label="distribution">
	{#each segments as s (s.key)}
		{#if s.count > 0}
			<div class="seg {s.tone}" style="flex:{s.count}" title="{titleCase(s.key)}: {num(s.count)}"></div>
		{/if}
	{/each}
	{#if total === 0}<div class="seg empty" style="flex:1"></div>{/if}
</div>
<div class="legend">
	{#each segments as s (s.key)}
		<a class="item" href={s.href} class:nolink={!s.href}>
			<span class="sw {s.tone}"></span>
			<span>{titleCase(s.key)}</span>
			<strong>{num(s.count)}</strong>
			<span class="faint">{pct(s.count, total)}</span>
		</a>
	{/each}
</div>

<style>
	.bar {
		display: flex;
		height: 12px;
		border-radius: 999px;
		overflow: hidden;
		gap: 2px;
		background: var(--surface-3);
	}
	.seg {
		min-width: 4px;
		transition: flex 0.4s ease;
	}
	.legend {
		display: flex;
		flex-wrap: wrap;
		gap: 6px 18px;
		margin-top: 12px;
		font-size: 13px;
	}
	.item {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		color: var(--text-2);
	}
	.item strong {
		color: var(--text);
		font-variant-numeric: tabular-nums;
	}
	.nolink {
		pointer-events: none;
	}
	.sw {
		width: 10px;
		height: 10px;
		border-radius: 3px;
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
	.muted,
	.empty {
		background: var(--text-3);
		opacity: 0.5;
	}
</style>

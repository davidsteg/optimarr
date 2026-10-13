<script lang="ts">
	import type { Snippet } from 'svelte';
	let { tone = 'muted', children }: { tone?: string; children: Snippet } = $props();
</script>

<span class="badge {tone}">{@render children()}</span>

<style>
	.badge {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		height: 22px;
		padding: 0 8px;
		border-radius: 999px;
		font-size: 12px;
		font-weight: 500;
		white-space: nowrap;
		background: var(--muted-soft);
		color: var(--text-2);
	}
	.good,
	.optimal,
	.excellent {
		background: var(--good-soft);
		color: var(--good);
	}
	.ok {
		background: var(--ok-soft);
		color: var(--ok);
	}
	.warn,
	.warning {
		background: var(--warn-soft);
		color: var(--warn);
	}
	.bad,
	.critical {
		background: var(--bad-soft);
		color: var(--bad);
	}
	.accent {
		background: var(--accent-soft);
		color: var(--accent-strong);
	}
</style>

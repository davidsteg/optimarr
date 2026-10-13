<script lang="ts">
	import { scoreTone, titleCase } from '$lib/format';
	let {
		score,
		verdict = '',
		size = 'md',
		label = ''
	}: { score: number; verdict?: string; size?: 'sm' | 'md' | 'lg'; label?: string } = $props();
	const tone = $derived(verdict === 'unknown' ? 'muted' : scoreTone(score));
	const r = 16;
	const c = 2 * Math.PI * r;
</script>

{#if size === 'sm'}
	<span class="pill {tone}" title={verdict ? titleCase(verdict) : undefined}>
		<span class="dot"></span>{verdict === 'unknown' ? '–' : score}
	</span>
{:else}
	<div class="ring {size}">
		<svg viewBox="0 0 40 40">
			<circle cx="20" cy="20" r={r} class="track" />
			<circle
				cx="20"
				cy="20"
				{r}
				class="value {tone}"
				stroke-dasharray={c}
				stroke-dashoffset={c - (c * Math.max(0, Math.min(100, score))) / 100}
			/>
		</svg>
		<div class="center">
			<strong>{verdict === 'unknown' ? '–' : score}</strong>
			{#if label}<span>{label}</span>{/if}
		</div>
	</div>
{/if}

<style>
	.pill {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		font-variant-numeric: tabular-nums;
		font-weight: 600;
		min-width: 38px;
	}
	.dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: currentColor;
	}
	.good {
		color: var(--good);
	}
	.ok {
		color: var(--ok);
	}
	.warn {
		color: var(--warn);
	}
	.bad {
		color: var(--bad);
	}
	.muted {
		color: var(--text-3);
	}
	.ring {
		position: relative;
		width: 64px;
		height: 64px;
	}
	.ring.lg {
		width: 96px;
		height: 96px;
	}
	svg {
		width: 100%;
		height: 100%;
		transform: rotate(-90deg);
	}
	circle {
		fill: none;
		stroke-width: 4;
	}
	.track {
		stroke: var(--surface-3);
	}
	.value {
		stroke: currentColor;
		stroke-linecap: round;
		transition: stroke-dashoffset 0.5s ease;
	}
	.center {
		position: absolute;
		inset: 0;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		line-height: 1.1;
	}
	.center strong {
		font-size: 18px;
	}
	.lg .center strong {
		font-size: 26px;
	}
	.center span {
		font-size: 10px;
		color: var(--text-3);
		text-transform: uppercase;
		letter-spacing: 0.06em;
	}
</style>

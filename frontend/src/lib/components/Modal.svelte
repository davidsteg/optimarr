<script lang="ts">
	import type { Snippet } from 'svelte';
	import Icon from './Icon.svelte';
	let {
		open = $bindable(false),
		title,
		width = 520,
		children,
		footer
	}: { open?: boolean; title: string; width?: number; children: Snippet; footer?: Snippet } = $props();

	function onkey(e: KeyboardEvent) {
		if (open && e.key === 'Escape') open = false;
	}
</script>

<svelte:window onkeydown={onkey} />

{#if open}
	<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && (open = false)}>
		<div class="modal card" role="dialog" aria-modal="true" aria-label={title} style="max-width:{width}px">
			<div class="card-head">
				<h2>{title}</h2>
				<button class="btn ghost icon" onclick={() => (open = false)} aria-label="Close"><Icon name="x" /></button>
			</div>
			<div class="body">{@render children()}</div>
			{#if footer}<div class="foot">{@render footer()}</div>{/if}
		</div>
	</div>
{/if}

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		background: rgba(3, 6, 12, 0.6);
		backdrop-filter: blur(3px);
		display: flex;
		align-items: flex-start;
		justify-content: center;
		padding: 8vh 16px 16px;
		z-index: 50;
		animation: fade 0.12s ease;
	}
	.modal {
		width: 100%;
		box-shadow: var(--shadow);
		max-height: 84vh;
		display: flex;
		flex-direction: column;
	}
	.body {
		padding: 18px;
		overflow: auto;
	}
	.foot {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
		padding: 14px 18px;
		border-top: 1px solid var(--border);
	}
	@keyframes fade {
		from {
			opacity: 0;
		}
	}
</style>

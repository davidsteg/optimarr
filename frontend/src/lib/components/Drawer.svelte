<script lang="ts">
	import type { Snippet } from 'svelte';
	import Icon from './Icon.svelte';
	let {
		open = $bindable(false),
		title = '',
		children,
		header
	}: { open?: boolean; title?: string; children: Snippet; header?: Snippet } = $props();
	function onkey(e: KeyboardEvent) {
		if (open && e.key === 'Escape') open = false;
	}
</script>

<svelte:window onkeydown={onkey} />

{#if open}
	<div class="backdrop" role="presentation" onclick={() => (open = false)}></div>
	<aside class="drawer" aria-label={title}>
		<div class="head">
			<div class="grow">
				{#if header}{@render header()}{:else}<h2>{title}</h2>{/if}
			</div>
			<button class="btn ghost icon" onclick={() => (open = false)} aria-label="Close"><Icon name="x" /></button>
		</div>
		<div class="body">{@render children()}</div>
	</aside>
{/if}

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		background: rgba(3, 6, 12, 0.45);
		z-index: 40;
	}
	.drawer {
		position: fixed;
		top: 0;
		right: 0;
		bottom: 0;
		width: min(720px, 100vw);
		background: var(--bg-raised);
		border-left: 1px solid var(--border);
		box-shadow: var(--shadow);
		z-index: 41;
		display: flex;
		flex-direction: column;
		animation: slide 0.18s ease;
	}
	.head {
		display: flex;
		align-items: flex-start;
		gap: 12px;
		padding: 18px 20px;
		border-bottom: 1px solid var(--border);
	}
	.body {
		overflow: auto;
		padding: 20px;
		flex: 1;
	}
	@keyframes slide {
		from {
			transform: translateX(30px);
			opacity: 0;
		}
	}
</style>

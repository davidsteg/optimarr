<script lang="ts">
	import { toasts } from '$lib/toast.svelte';
	import Icon from './Icon.svelte';
</script>

<div class="toasts" aria-live="polite">
	{#each toasts.items as t (t.id)}
		<div class="toast {t.kind}">
			<Icon name={t.kind === 'error' ? 'alert' : t.kind === 'success' ? 'check' : 'info'} />
			<span class="grow">{t.message}</span>
			<button class="btn ghost icon sm" onclick={() => toasts.dismiss(t.id)} aria-label="Dismiss"><Icon name="x" size={14} /></button>
		</div>
	{/each}
</div>

<style>
	.toasts {
		position: fixed;
		bottom: 20px;
		right: 20px;
		display: flex;
		flex-direction: column;
		gap: 8px;
		z-index: 100;
		max-width: min(420px, calc(100vw - 40px));
	}
	.toast {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 10px 10px 10px 14px;
		border-radius: var(--radius-sm);
		background: var(--surface-2);
		border: 1px solid var(--border-strong);
		box-shadow: var(--shadow);
		animation: pop 0.15s ease;
	}
	.success {
		color: var(--good);
	}
	.error {
		color: var(--bad);
	}
	.toast span {
		color: var(--text);
	}
	@keyframes pop {
		from {
			transform: translateY(8px);
			opacity: 0;
		}
	}
</style>

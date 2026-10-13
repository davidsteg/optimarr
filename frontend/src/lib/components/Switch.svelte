<script lang="ts">
	let {
		checked = $bindable(false),
		label = '',
		hint = '',
		disabled = false,
		onchange
	}: { checked?: boolean; label?: string; hint?: string; disabled?: boolean; onchange?: (v: boolean) => void } = $props();
</script>

<label class="switch" class:disabled>
	<input type="checkbox" bind:checked {disabled} onchange={() => onchange?.(checked)} />
	<span class="track"><span class="thumb"></span></span>
	{#if label}
		<span class="text">
			<span>{label}</span>
			{#if hint}<span class="faint small">{hint}</span>{/if}
		</span>
	{/if}
</label>

<style>
	.switch {
		display: inline-flex;
		align-items: flex-start;
		gap: 10px;
		cursor: pointer;
		user-select: none;
	}
	.disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
	input {
		position: absolute;
		opacity: 0;
		width: 0;
		height: 0;
	}
	.track {
		flex-shrink: 0;
		width: 34px;
		height: 20px;
		border-radius: 999px;
		background: var(--surface-3);
		border: 1px solid var(--border-strong);
		position: relative;
		transition: background 0.15s;
		margin-top: 1px;
	}
	.thumb {
		position: absolute;
		top: 2px;
		left: 2px;
		width: 14px;
		height: 14px;
		border-radius: 50%;
		background: var(--text-2);
		transition:
			transform 0.15s,
			background 0.15s;
	}
	input:checked + .track {
		background: var(--accent);
		border-color: var(--accent);
	}
	input:checked + .track .thumb {
		transform: translateX(14px);
		background: #fff;
	}
	input:focus-visible + .track {
		outline: 2px solid color-mix(in srgb, var(--accent) 60%, transparent);
	}
	.text {
		display: flex;
		flex-direction: column;
	}
</style>

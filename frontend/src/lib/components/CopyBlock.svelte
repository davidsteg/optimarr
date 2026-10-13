<script lang="ts">
	import Icon from './Icon.svelte';
	let { text, label = 'Copy' }: { text: string; label?: string } = $props();
	let copied = $state(false);
	async function copy() {
		try {
			await navigator.clipboard.writeText(text);
		} catch {
			const ta = document.createElement('textarea');
			ta.value = text;
			document.body.appendChild(ta);
			ta.select();
			document.execCommand('copy');
			ta.remove();
		}
		copied = true;
		setTimeout(() => (copied = false), 1500);
	}
</script>

<div class="block">
	<pre>{text}</pre>
	<button class="btn sm copy" onclick={copy}><Icon name={copied ? 'check' : 'copy'} size={14} />{copied ? 'Copied' : label}</button>
</div>

<style>
	.block {
		position: relative;
		background: var(--bg);
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
	}
	pre {
		margin: 0;
		padding: 12px 14px;
		padding-right: 90px;
		white-space: pre-wrap;
		word-break: break-all;
		max-height: 260px;
		overflow: auto;
	}
	.copy {
		position: absolute;
		top: 8px;
		right: 8px;
	}
</style>

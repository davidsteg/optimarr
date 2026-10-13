<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '$lib/api';
	import { ago } from '$lib/format';
	import { toasts } from '$lib/toast.svelte';
	import Badge from './Badge.svelte';
	import Icon from './Icon.svelte';
	import Switch from './Switch.svelte';

	interface Target {
		name: string;
		kind: 'discord' | 'ntfy' | 'webhook';
		enabled: boolean;
		url: string;
		token: string;
	}
	interface Settings {
		targets: Target[];
		onNewProblems: boolean;
		onFixFinished: boolean;
		onUpgrades: boolean;
	}
	interface Sent {
		id: number;
		event: string;
		title: string;
		message: string;
		sentAt: string;
		results: { target: string; ok: boolean; error?: string }[];
	}

	let s = $state<Settings | null>(null);
	let history = $state<Sent[]>([]);
	let saving = $state(false);
	let testing = $state(false);

	onMount(() => {
		load();
	});

	async function load() {
		try {
			s = await api.get<Settings>('/settings/notifications');
			history = await api.get<Sent[]>('/notifications', { limit: 10 });
		} catch (e) {
			toasts.error(e);
		}
	}

	function add(kind: Target['kind']) {
		s?.targets.push({ name: kind === 'discord' ? 'Discord' : kind === 'ntfy' ? 'ntfy' : 'Webhook', kind, enabled: true, url: '', token: '' });
	}

	async function save(e: Event) {
		e.preventDefault();
		if (!s) return;
		saving = true;
		try {
			await api.put('/settings/notifications', s);
			toasts.success('Notifications saved');
			await load();
		} catch (err) {
			toasts.error(err);
		} finally {
			saving = false;
		}
	}

	async function test() {
		if (!s) return;
		testing = true;
		try {
			const r = await api.post<{ results: Sent['results'] }>('/notifications/test', { settings: s });
			const failed = r.results.filter((x) => !x.ok);
			if (failed.length === 0) toasts.success(`Test sent to ${r.results.length} target(s)`);
			else toasts.error(failed.map((f) => `${f.target}: ${f.error}`).join('; '));
			history = await api.get<Sent[]>('/notifications', { limit: 10 });
		} catch (err) {
			toasts.error(err);
		} finally {
			testing = false;
		}
	}

	const placeholder: Record<string, string> = {
		discord: 'https://discord.com/api/webhooks/…',
		ntfy: 'https://ntfy.sh/your-topic',
		webhook: 'https://example.com/hooks/optimarr'
	};
</script>

{#if s}
	<form class="card card-pad stack" onsubmit={save}>
		<div>
			<h2>Notifications</h2>
			<p class="muted small" style="margin:4px 0 0">Get a message when new files arrive broken or scoring poorly, when a fix finishes, and when upgrade searches run.</p>
		</div>
		<div class="stack" style="gap:10px">
			<Switch bind:checked={s.onNewProblems} label="New problem files" hint="Broken files, or poor compatibility or quality, after a scan" />
			<Switch bind:checked={s.onFixFinished} label="Fixes finished or failed" />
			<Switch bind:checked={s.onUpgrades} label="Automatic upgrade searches" />
		</div>

		<h3>Send to</h3>
		{#each s.targets as t, i (i)}
			<div class="target">
				<div class="row wrap">
					<Badge tone="accent">{t.kind}</Badge>
					<input type="text" class="grow" bind:value={t.name} placeholder="Name" aria-label="Name" />
					<Switch bind:checked={t.enabled} />
					<button type="button" class="btn sm ghost icon" onclick={() => s?.targets.splice(i, 1)} aria-label="Remove"><Icon name="trash" size={14} /></button>
				</div>
				<input type="url" class="mono" bind:value={t.url} placeholder={placeholder[t.kind]} aria-label="URL" required />
				{#if t.kind !== 'discord'}
					<input type="password" bind:value={t.token} placeholder="Access token (optional)" aria-label="Token" autocomplete="off" />
				{/if}
			</div>
		{/each}
		<div class="row wrap">
			<button type="button" class="btn sm" onclick={() => add('discord')}><Icon name="plus" size={13} />Discord</button>
			<button type="button" class="btn sm" onclick={() => add('ntfy')}><Icon name="plus" size={13} />ntfy</button>
			<button type="button" class="btn sm" onclick={() => add('webhook')}><Icon name="plus" size={13} />Webhook</button>
		</div>
		<div class="row wrap">
			<button class="btn primary" disabled={saving}>Save</button>
			<button type="button" class="btn" onclick={test} disabled={testing || !s.targets.some((t) => t.enabled)}><Icon name="bolt" size={15} />Send test</button>
		</div>
	</form>

	{#if history.length}
		<div class="card">
			<div class="card-head"><h2>Recently sent</h2></div>
			<ul class="history">
				{#each history as h (h.id)}
					<li>
						<div class="row">
							<strong class="grow">{h.title}</strong>
							<span class="faint small">{ago(h.sentAt)}</span>
						</div>
						{#each h.results.filter((r) => !r.ok) as r (r.target)}
							<div class="small" style="color:var(--bad)">{r.target}: {r.error}</div>
						{/each}
					</li>
				{/each}
			</ul>
		</div>
	{/if}
{/if}

<style>
	.target {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding: 12px;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
	}
	.history {
		list-style: none;
		margin: 0;
		padding: 0;
	}
	.history li {
		padding: 10px 18px;
		border-bottom: 1px solid var(--border);
	}
	.history li:last-child {
		border-bottom: none;
	}
</style>

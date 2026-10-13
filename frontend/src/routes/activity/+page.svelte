<script lang="ts">
	import { api } from '$lib/api';
	import { live } from '$lib/live.svelte';
	import { ago, bytes, dateTime, duration, num } from '$lib/format';
	import { toasts } from '$lib/toast.svelte';
	import type { Job } from '$lib/types';
	import Badge from '$lib/components/Badge.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import Modal from '$lib/components/Modal.svelte';

	interface Failure {
		id: number;
		path: string;
		error: string;
		size: number | null;
		failedAt: string;
	}

	let history = $state<Job[] | null>(null);
	let error = $state('');
	let failuresFor = $state<Job | null>(null);
	let failures = $state<Failure[] | null>(null);

	$effect(() => {
		live.mediaVersion;
		load();
	});

	async function load() {
		try {
			history = await api.get<Job[]>('/jobs', { limit: 100 });
			error = '';
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	// Merge live snapshots over the fetched history so progress updates without polling.
	const jobs = $derived.by(() => {
		const byId = new Map<number, Job>();
		for (const j of history ?? []) byId.set(j.id, j);
		for (const j of Object.values(live.jobs)) byId.set(j.id, j);
		return [...byId.values()].sort((a, b) => b.id - a.id);
	});
	const running = $derived(jobs.filter((j) => j.status === 'running'));
	const finished = $derived(jobs.filter((j) => j.status !== 'running'));

	async function cancel(j: Job) {
		try {
			await api.post(`/jobs/${j.id}/cancel`);
			toasts.success(`Cancelling ${j.title}`);
		} catch (e) {
			toasts.error(e);
		}
	}

	async function showFailures(j: Job) {
		failuresFor = j;
		failures = null;
		try {
			failures = await api.get<Failure[]>(`/jobs/${j.id}/failures`);
		} catch (e) {
			toasts.error(e);
			failuresFor = null;
		}
	}

	function elapsed(j: Job): string {
		const end = j.finishedAt ? new Date(j.finishedAt).getTime() : Date.now();
		return duration((end - new Date(j.startedAt).getTime()) / 1000);
	}

	function pctDone(j: Job) {
		return j.total ? Math.min(100, (j.processed / j.total) * 100) : 0;
	}

	const statusTone: Record<string, string> = { completed: 'good', failed: 'bad', cancelled: 'warn', running: 'accent' };
</script>

<div class="page-head">
	<div>
		<h1>Activity</h1>
		<p>Scans, matching, imports and re-scoring, live and past.</p>
	</div>
	<button class="btn" onclick={load}><Icon name="refresh" size={15} />Refresh</button>
</div>

{#if error}
	<div class="card card-pad" style="color:var(--bad)">{error}</div>
{:else if !history}
	<p class="faint">Loading…</p>
{:else}
	<div class="stack">
		{#if running.length}
			<div class="running">
				{#each running as j (j.id)}
					<div class="card card-pad job">
						<div class="row">
							<h2 class="grow ellipsis">{j.title}</h2>
							<span class="faint small">{elapsed(j)}</span>
							<button class="btn sm danger" onclick={() => cancel(j)}><Icon name="stop" size={14} />Cancel</button>
						</div>
						<div class="track"><div class="fill" style="width:{j.total ? pctDone(j) : 5}%"></div></div>
						<div class="row small">
							<span class="grow ellipsis mono faint" title={j.currentItem ?? ''}>{j.currentItem ?? j.message ?? 'Starting…'}</span>
							<span>{num(j.processed)} / {num(j.total)}</span>
							{#if j.failed}<Badge tone="bad">{num(j.failed)} failed</Badge>{/if}
						</div>
					</div>
				{/each}
			</div>
		{/if}

		<div class="card">
			<div class="card-head"><h2>History</h2></div>
			{#if finished.length === 0}
				<div class="empty"><p>No finished jobs yet.</p></div>
			{:else}
				<div class="table-scroll">
					<table class="table">
						<thead>
							<tr>
								<th>Job</th>
								<th>Status</th>
								<th class="num">Processed</th>
								<th class="num hide-sm">Skipped</th>
								<th class="num">Failed</th>
								<th class="hide-sm">Started</th>
								<th class="num hide-sm">Took</th>
							</tr>
						</thead>
						<tbody>
							{#each finished as j (j.id)}
								<tr>
									<td>
										<div>{j.title}</div>
										{#if j.message}<div class="faint small">{j.message}</div>{/if}
									</td>
									<td><Badge tone={statusTone[j.status] ?? 'muted'}>{j.status}</Badge></td>
									<td class="num">{num(j.processed)}{j.total ? ` / ${num(j.total)}` : ''}</td>
									<td class="num hide-sm">{num(j.skipped)}</td>
									<td class="num">
										{#if j.failed}
											<button class="btn sm ghost fail" onclick={() => showFailures(j)}>{num(j.failed)}</button>
										{:else}
											<span class="faint">0</span>
										{/if}
									</td>
									<td class="hide-sm" title={dateTime(j.startedAt)}>{ago(j.startedAt)}</td>
									<td class="num hide-sm">{elapsed(j)}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{/if}
		</div>
	</div>
{/if}

<Modal open={!!failuresFor} title="Failures: {failuresFor?.title ?? ''}" width={820} onclose={() => (failuresFor = null)}>
	{#if !failures}
		<p class="faint">Loading…</p>
	{:else if failures.length === 0}
		<p class="faint">No failures recorded.</p>
	{:else}
		<div class="failures">
			{#each failures as f (f.id)}
				<div class="failure">
					<div class="mono small ellipsis" title={f.path}>{f.path}</div>
					<div class="small" style="color:var(--bad)">{f.error}</div>
					{#if f.size}<div class="faint small">{bytes(f.size)}</div>{/if}
				</div>
			{/each}
		</div>
	{/if}
</Modal>

<style>
	.running {
		display: grid;
		gap: 12px;
	}
	.job {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.track {
		height: 6px;
		border-radius: 999px;
		background: var(--surface-3);
		overflow: hidden;
	}
	.fill {
		height: 100%;
		background: var(--accent);
		transition: width 0.4s;
	}
	.fail {
		color: var(--bad);
		height: 24px;
	}
	.failures {
		display: flex;
		flex-direction: column;
		gap: 10px;
		max-height: 60vh;
		overflow: auto;
	}
	.failure {
		padding: 10px 12px;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		background: var(--surface-2);
	}
</style>

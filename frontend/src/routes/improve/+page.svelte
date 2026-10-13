<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { api } from '$lib/api';
	import { live } from '$lib/live.svelte';
	import { ago, bytes, num, titleCase, verdictTone } from '$lib/format';
	import { toasts } from '$lib/toast.svelte';
	import type { FileBrief, Fix, SavingsEstimate } from '$lib/types';
	import Badge from '$lib/components/Badge.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import MediaDrawer from '$lib/components/MediaDrawer.svelte';

	const tabs = [
		{ id: 'devices', label: 'Devices' },
		{ id: 'fixes', label: 'Fix queue' },
		{ id: 'upgrades', label: 'Upgrades' },
		{ id: 'savings', label: 'Space savings' }
	] as const;
	type Tab = (typeof tabs)[number]['id'];

	interface DeviceSummary {
		id: number;
		name: string;
		directPlay: number;
		directStream: number;
		transcode: number;
		transcodeSizeBytes: number;
		topReasons: { reason: string; count: number }[];
	}
	interface FixList {
		enabled: boolean;
		ffmpeg: boolean;
		items: Fix[];
	}
	interface Upgrades {
		maxScore: number;
		cooldownDays: number;
		total: number;
		files: FileBrief[];
	}
	interface Savings {
		summary: string;
		files: number;
		currentBytes: number;
		savingsBytes: number;
		byCodec: { codec: string; files: number; savingsBytes: number }[];
		top: { file: FileBrief; estimate: SavingsEstimate }[];
	}

	let tab = $state<Tab>('devices');
	let devices = $state<DeviceSummary[] | null>(null);
	let fixes = $state<FixList | null>(null);
	let upgrades = $state<Upgrades | null>(null);
	let savings = $state<Savings | null>(null);
	let maxScore = $state<number | null>(null);
	let target = $state<'hevc' | 'av1'>('hevc');
	let selected = $state(new Set<number>());
	let detailId = $state<number | null>(null);
	let busy = $state(false);

	onMount(() => {
		const t = page.url.searchParams.get('tab') as Tab | null;
		if (t && tabs.some((x) => x.id === t)) tab = t;
	});

	function select(t: Tab) {
		tab = t;
		selected = new Set();
		goto(t === 'devices' ? '/improve' : `/improve?tab=${t}`, { replaceState: true, noScroll: true, keepFocus: true });
	}

	$effect(() => {
		if (tab !== 'devices') return;
		live.mediaVersion;
		api.get<DeviceSummary[]>('/devices/summary').then((d) => (devices = d)).catch((e) => toasts.error(e));
	});

	$effect(() => {
		if (tab !== 'fixes') return;
		live.fixesVersion;
		api.get<FixList>('/fixes', { limit: 200 }).then((f) => (fixes = f)).catch((e) => toasts.error(e));
	});

	$effect(() => {
		if (tab !== 'upgrades') return;
		const m = maxScore;
		live.mediaVersion;
		api.get<Upgrades>('/upgrades', { maxScore: m ?? undefined, limit: 200 })
			.then((u) => {
				upgrades = u;
				if (maxScore === null) maxScore = u.maxScore;
			})
			.catch((e) => toasts.error(e));
	});

	$effect(() => {
		if (tab !== 'savings') return;
		const t = target;
		live.mediaVersion;
		savings = null;
		api.get<Savings>('/savings', { target: t, limit: 50 }).then((s) => (savings = s)).catch((e) => toasts.error(e));
	});

	async function cancelFix(f: Fix) {
		try {
			await api.post(`/fixes/${f.id}/cancel`);
		} catch (e) {
			toasts.error(e);
		}
	}

	async function clearFinished() {
		try {
			await api.del('/fixes');
		} catch (e) {
			toasts.error(e);
		}
	}

	function toggle(id: number) {
		const next = new Set(selected);
		if (next.has(id)) next.delete(id);
		else next.add(id);
		selected = next;
	}

	async function searchSelected(ids: number[]) {
		busy = true;
		try {
			const r = await api.post<{ searched: number; failed: number; results: { success: boolean; message: string }[] }>('/upgrades/search', { ids });
			if (r.failed === 0) toasts.success(`Searching for better releases of ${r.searched} file(s)`);
			else toasts.error(`${r.searched} searched, ${r.failed} failed: ${r.results.find((x) => !x.success)?.message ?? ''}`);
			selected = new Set();
			upgrades = await api.get<Upgrades>('/upgrades', { maxScore: maxScore ?? undefined, limit: 200 });
		} catch (e) {
			toasts.error(e);
		} finally {
			busy = false;
		}
	}

	const fixTone: Record<string, string> = { queued: 'muted', running: 'accent', done: 'good', failed: 'bad', cancelled: 'warn' };
	const pctOf = (part: number, d: DeviceSummary) => {
		const total = d.directPlay + d.directStream + d.transcode;
		return total ? (part / total) * 100 : 0;
	};
	const fileName = (p: string) => p.split(/[\\/]/).pop() ?? p;
</script>

<div class="page-head">
	<div>
		<h1>Improve</h1>
		<p>What your devices transcode, fixes to run, releases to upgrade, and space to win back.</p>
	</div>
</div>

<nav class="tabs" aria-label="Sections">
	{#each tabs as t (t.id)}
		<button class:active={tab === t.id} onclick={() => select(t.id)}>{t.label}</button>
	{/each}
</nav>

{#if tab === 'devices'}
	{#if !devices}
		<p class="faint">Loading…</p>
	{:else if devices.length === 0}
		<div class="card empty">
			<h3>No device profiles yet</h3>
			<p>Tell Optimarr which clients you watch on, and it shows which files each one would transcode.</p>
			<a class="btn primary" href="/settings?tab=devices"><Icon name="plus" size={15} />Add devices</a>
		</div>
	{:else}
		<div class="devs">
			{#each devices as d (d.id)}
				<div class="card card-pad stack" style="gap:12px">
					<div class="row">
						<h2 class="grow">{d.name}</h2>
						<span class="faint small">{num(d.directPlay + d.directStream + d.transcode)} files</span>
					</div>
					<div class="bar">
						<span style="width:{pctOf(d.directPlay, d)}%;background:var(--good)" title="Direct play"></span>
						<span style="width:{pctOf(d.directStream, d)}%;background:var(--ok)" title="Remux"></span>
						<span style="width:{pctOf(d.transcode, d)}%;background:var(--bad)" title="Transcode"></span>
					</div>
					<div class="legend small">
						<span><i style="background:var(--good)"></i>Direct play {num(d.directPlay)}</span>
						<span><i style="background:var(--ok)"></i>Remux {num(d.directStream)}</span>
						<span><i style="background:var(--bad)"></i>Transcode {num(d.transcode)}{d.transcode ? ` (${bytes(d.transcodeSizeBytes)})` : ''}</span>
					</div>
					{#if d.topReasons.length}
						<div>
							<div class="faint small" style="margin-bottom:4px">Why it transcodes</div>
							<ul class="reasons">
								{#each d.topReasons as r (r.reason)}
									<li><span class="grow">{r.reason}</span><span class="faint">{num(r.count)}</span></li>
								{/each}
							</ul>
						</div>
					{/if}
				</div>
			{/each}
		</div>
	{/if}
{:else if tab === 'fixes'}
	{#if !fixes}
		<p class="faint">Loading…</p>
	{:else}
		{#if !fixes.enabled || !fixes.ffmpeg}
			<div class="card card-pad note">
				<Icon name="info" size={16} />
				<span>
					{#if !fixes.ffmpeg}
						ffmpeg is not installed, so queued fixes cannot run. The Docker image includes it.
					{:else}
						The fix worker is off, so queued fixes wait. Turn it on in <a href="/settings?tab=fixes">Settings → Fixes</a> once your media folders are mounted writable.
					{/if}
				</span>
			</div>
		{/if}
		<div class="card">
			<div class="card-head">
				<h2>Fix queue</h2>
				<button class="btn sm" onclick={clearFinished} disabled={!fixes.items.some((f) => f.status !== 'queued' && f.status !== 'running')}>Clear finished</button>
			</div>
			{#if fixes.items.length === 0}
				<div class="empty">
					<p>Nothing queued. Select files on the <a href="/media?compat=poor">Media</a> page and choose <strong>Queue fixes</strong>.</p>
				</div>
			{:else}
				<div class="table-scroll">
					<table class="table">
						<thead><tr><th>File</th><th>Status</th><th class="num hide-sm">Size</th><th class="hide-sm">When</th><th></th></tr></thead>
						<tbody>
							{#each fixes.items as f (f.id)}
								<tr class:clickable={!!f.mediaFileId} onclick={() => f.mediaFileId && (detailId = f.mediaFileId)}>
									<td>
										<div class="ellipsis" style="max-width:420px" title={f.path}>{fileName(f.finalPath ?? f.path)}</div>
										{#if f.error}<div class="small" style="color:var(--bad)">{f.error}</div>{/if}
									</td>
									<td>
										<Badge tone={fixTone[f.status]}>{titleCase(f.status)}</Badge>
										{#if f.status === 'running'}<span class="small"> {Math.round(f.progress * 100)}%</span>{/if}
									</td>
									<td class="num hide-sm small">
										{#if f.sizeAfter}{bytes(f.sizeBefore ?? 0)} → {bytes(f.sizeAfter)}{:else}{bytes(f.sizeBefore ?? 0)}{/if}
									</td>
									<td class="hide-sm small">{ago(f.finishedAt ?? f.startedAt ?? f.createdAt)}</td>
									<td class="num">
										{#if f.status === 'queued' || f.status === 'running'}
											<button
												class="btn sm ghost"
												onclick={(e) => {
													e.stopPropagation();
													cancelFix(f);
												}}>Cancel</button
											>
										{/if}
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{/if}
		</div>
	{/if}
{:else if tab === 'upgrades'}
	<div class="card">
		<div class="card-head wrap">
			<div>
				<h2>Upgrade candidates</h2>
				<p class="faint small" style="margin:2px 0 0">
					Files linked to Sonarr or Radarr with a low quality score. Searching keeps the current file; Sonarr and Radarr only grab a release
					their quality profile ranks higher.
				</p>
			</div>
			<div class="row">
				<label class="row small">Quality ≤ <input type="number" min="0" max="100" style="width:70px" bind:value={maxScore} /></label>
				<button class="btn sm primary" disabled={busy || selected.size === 0} onclick={() => searchSelected([...selected])}>
					<Icon name="search" size={14} />Search {selected.size || ''}
				</button>
			</div>
		</div>
		{#if !upgrades}
			<p class="faint card-pad">Loading…</p>
		{:else if upgrades.files.length === 0}
			<div class="empty">
				<p>No candidates. Files must be matched to Sonarr or Radarr, and files searched in the last {upgrades.cooldownDays} days are skipped.</p>
			</div>
		{:else}
			<div class="table-scroll">
				<table class="table">
					<thead>
						<tr>
							<th style="width:32px">
								<input
									type="checkbox"
									aria-label="Select all"
									checked={upgrades.files.every((f) => selected.has(f.id))}
									onchange={(e) => (selected = (e.target as HTMLInputElement).checked ? new Set(upgrades?.files.map((f) => f.id)) : new Set())}
								/>
							</th>
							<th>Title</th>
							<th>Quality</th>
							<th class="hide-sm">Why</th>
						</tr>
					</thead>
					<tbody>
						{#each upgrades.files as f (f.id)}
							<tr class="clickable" class:selected={selected.has(f.id)} onclick={() => (detailId = f.id)}>
								<td onclick={(e) => e.stopPropagation()}><input type="checkbox" checked={selected.has(f.id)} onchange={() => toggle(f.id)} aria-label="Select" /></td>
								<td class="ellipsis" style="max-width:360px">{f.title}</td>
								<td><Badge tone={verdictTone(f.quality.verdict)}>{f.quality.score} · {titleCase(f.quality.verdict)}</Badge></td>
								<td class="hide-sm small muted">{f.mainIssues.join(' · ')}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
			<p class="faint small card-pad" style="margin:0">Showing {num(upgrades.files.length)} of {num(upgrades.total)}.</p>
		{/if}
	</div>
{:else if tab === 'savings'}
	<div class="row" style="margin-bottom:16px">
		<span class="muted">Re-encode to</span>
		<select bind:value={target} aria-label="Target codec">
			<option value="hevc">H.265 (HEVC)</option>
			<option value="av1">AV1</option>
		</select>
	</div>
	{#if !savings}
		<p class="faint">Calculating…</p>
	{:else}
		<div class="card card-pad stack" style="gap:8px;margin-bottom:16px">
			<div class="big">{bytes(savings.savingsBytes)}</div>
			<div class="muted">{savings.summary}</div>
			<div class="faint small">
				An estimate from each file's resolution and frame rate at visually transparent settings. Audio is kept as is. Your clients must
				direct-play the target codec, or the savings cost you transcodes.
			</div>
		</div>
		{#if savings.top.length}
			<div class="card">
				<div class="card-head"><h2>Biggest wins</h2></div>
				<div class="table-scroll">
					<table class="table">
						<thead><tr><th>Title</th><th class="num">Now</th><th class="num">After</th><th class="num">Saved</th></tr></thead>
						<tbody>
							{#each savings.top as t (t.file.id)}
								<tr class="clickable" onclick={() => (detailId = t.file.id)}>
									<td class="ellipsis" style="max-width:360px">{t.file.title}</td>
									<td class="num">{bytes(t.estimate.currentSize)}</td>
									<td class="num">{bytes(t.estimate.estimatedSize)}</td>
									<td class="num"><strong>{bytes(t.estimate.savings)}</strong></td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			</div>
		{/if}
	{/if}
{/if}

<MediaDrawer bind:id={detailId} />

<style>
	.tabs {
		display: flex;
		gap: 4px;
		margin-bottom: 18px;
		border-bottom: 1px solid var(--border);
		overflow-x: auto;
	}
	.tabs button {
		padding: 8px 14px;
		border: none;
		background: none;
		color: var(--text-2);
		font: inherit;
		font-weight: 500;
		cursor: pointer;
		border-bottom: 2px solid transparent;
		margin-bottom: -1px;
		white-space: nowrap;
	}
	.tabs button:hover {
		color: var(--text);
	}
	.tabs button.active {
		color: var(--accent-strong);
		border-bottom-color: var(--accent);
	}
	.devs {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
		gap: 16px;
	}
	.bar {
		display: flex;
		height: 8px;
		border-radius: 999px;
		overflow: hidden;
		background: var(--surface-3);
	}
	.legend {
		display: flex;
		gap: 14px;
		flex-wrap: wrap;
		color: var(--text-2);
	}
	.legend i {
		display: inline-block;
		width: 8px;
		height: 8px;
		border-radius: 50%;
		margin-right: 6px;
	}
	.reasons {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.reasons li {
		display: flex;
		gap: 10px;
		font-size: 13px;
	}
	.note {
		display: flex;
		gap: 10px;
		align-items: center;
		margin-bottom: 16px;
		color: var(--text-2);
	}
	.big {
		font-size: 32px;
		font-weight: 700;
		letter-spacing: -0.02em;
	}
	.card-head.wrap {
		flex-wrap: wrap;
	}
	@media (max-width: 720px) {
		.devs {
			grid-template-columns: 1fr;
		}
	}
</style>

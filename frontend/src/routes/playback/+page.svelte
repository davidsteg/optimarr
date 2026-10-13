<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '$lib/api';
	import { ago, dateTime, duration, num, pct, titleCase, verdictTone } from '$lib/format';
	import { toasts } from '$lib/toast.svelte';
	import { live } from '$lib/live.svelte';
	import type { BarItem, Page } from '$lib/types';
	import Badge from '$lib/components/Badge.svelte';
	import BarList from '$lib/components/BarList.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import MediaDrawer from '$lib/components/MediaDrawer.svelte';
	import Pager from '$lib/components/Pager.svelte';

	interface Group {
		key: string | null;
		plays: number;
		transcodes: number;
	}
	interface Stats {
		plays: number;
		transcodes: number;
		transcodeRate: number;
		unmatched: number;
		byMethod: Group[];
		byClient: Group[];
		byUser: Group[];
		byReason: Group[];
		topTranscoded: { mediaFileId: number; title: string; plays: number; transcodes: number; compatVerdict: string; compatScore: number }[];
	}
	interface PlaybackEvent {
		id: number;
		userName: string | null;
		itemName: string;
		filePath: string;
		startedAt: string;
		durationSeconds: number | null;
		clientName: string | null;
		deviceName: string | null;
		playMethod: string;
		transcodeReasons: string | null;
		transcodeVideoCodec: string | null;
		transcodeAudioCodec: string | null;
		mediaFileId: number | null;
		compatVerdict: string | null;
	}

	let days = $state(30);
	let stats = $state<Stats | null>(null);
	let events = $state<Page<PlaybackEvent> | null>(null);
	let search = $state('');
	let method = $state('');
	let pageNo = $state(1);
	let pageSize = $state(50);
	let detailId = $state<number | null>(null);
	let jellyfinEnabled = $state<boolean | null>(null);
	let importing = $state(false);

	onMount(() => {
		api.get<{ enabled: boolean }>('/settings/jellyfin').then((s) => (jellyfinEnabled = s.enabled)).catch(() => (jellyfinEnabled = false));
	});

	$effect(() => {
		const d = days;
		live.mediaVersion;
		api.get<Stats>('/playback/stats', { days: d }).then((s) => (stats = s)).catch((e) => toasts.error(e));
	});

	let timer: ReturnType<typeof setTimeout> | undefined;
	$effect(() => {
		const q = { search, method, days, page: pageNo, pageSize };
		live.mediaVersion;
		clearTimeout(timer);
		timer = setTimeout(() => {
			api.get<Page<PlaybackEvent>>('/playback', q).then((p) => (events = p)).catch((e) => toasts.error(e));
		}, 200);
	});

	function bars(groups: Group[], transcodesOnly = false): BarItem[] {
		return groups.map((g) => ({
			label: g.key ?? 'Unknown',
			value: transcodesOnly ? g.transcodes : g.plays,
			tone: transcodesOnly ? 'bad' : undefined,
			hint: transcodesOnly ? undefined : `${pct(g.transcodes, g.plays)} transcoded`
		}));
	}

	async function importHistory() {
		importing = true;
		try {
			await api.post('/integrations/jellyfin/import', {}, { days: 90 });
			toasts.success('Importing the last 90 days of Jellyfin playback');
		} catch (e) {
			toasts.error(e);
		} finally {
			importing = false;
		}
	}

	const methodTone: Record<string, string> = { DirectPlay: 'good', DirectStream: 'ok', Transcode: 'bad' };
	const methodLabel: Record<string, string> = { DirectPlay: 'Direct play', DirectStream: 'Direct stream', Transcode: 'Transcode' };
</script>

<div class="page-head">
	<div>
		<h1>Playback</h1>
		<p>What Jellyfin actually did when people pressed play.</p>
	</div>
	<div class="row wrap">
		<select bind:value={days} aria-label="Time range">
			<option value={7}>Last 7 days</option>
			<option value={30}>Last 30 days</option>
			<option value={90}>Last 90 days</option>
			<option value={365}>Last year</option>
		</select>
		{#if jellyfinEnabled}
			<button class="btn" onclick={importHistory} disabled={importing}><Icon name="download" size={15} />Import history</button>
		{/if}
	</div>
</div>

{#if jellyfinEnabled === false && stats && stats.plays === 0}
	<div class="card empty">
		<h3>Jellyfin is not connected</h3>
		<p>Connect Jellyfin to see which files direct-play and which get transcoded.</p>
		<a class="btn primary" href="/settings?tab=jellyfin"><Icon name="link" size={15} />Connect Jellyfin</a>
	</div>
{:else if !stats}
	<p class="faint">Loading…</p>
{:else}
	<div class="stats">
		<div class="card stat"><span class="label">Plays</span><strong>{num(stats.plays)}</strong></div>
		<div class="card stat">
			<span class="label">Transcoded</span>
			<strong style="color:{stats.transcodeRate > 0.25 ? 'var(--bad)' : 'inherit'}">{Math.round(stats.transcodeRate * 100)}%</strong>
			<span class="faint small">{num(stats.transcodes)} plays</span>
		</div>
		<div class="card stat">
			<span class="label">Unmatched</span>
			<strong>{num(stats.unmatched)}</strong>
			<span class="faint small">plays of files Optimarr has not analyzed</span>
		</div>
	</div>

	<div class="grid two">
		<div class="card">
			<div class="card-head"><h2>Most transcoded files</h2></div>
			{#if stats.topTranscoded.length === 0}
				<div class="empty"><p>Nothing was transcoded in this period.</p></div>
			{:else}
				<table class="table">
					<tbody>
						{#each stats.topTranscoded as f (f.mediaFileId)}
							<tr class="clickable" onclick={() => (detailId = f.mediaFileId)}>
								<td class="ellipsis" style="max-width:280px">{f.title}</td>
								<td><Badge tone={verdictTone(f.compatVerdict)}>{titleCase(f.compatVerdict)}</Badge></td>
								<td class="num">{num(f.transcodes)} / {num(f.plays)}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			{/if}
		</div>
		<div class="card card-pad">
			<h2 class="sec">Transcode reasons</h2>
			<BarList items={bars(stats.byReason, true)} empty="No transcodes in this period" />
		</div>
	</div>

	<div class="grid three">
		<div class="card card-pad">
			<h2 class="sec">Play method</h2>
			<BarList items={stats.byMethod.map((g) => ({ label: methodLabel[g.key ?? ''] ?? g.key ?? 'Unknown', value: g.plays, tone: methodTone[g.key ?? ''] }))} />
		</div>
		<div class="card card-pad">
			<h2 class="sec">Clients</h2>
			<BarList items={bars(stats.byClient)} />
		</div>
		<div class="card card-pad">
			<h2 class="sec">Users</h2>
			<BarList items={bars(stats.byUser)} />
		</div>
	</div>

	<div class="card">
		<div class="card-head">
			<h2>Sessions</h2>
			<div class="row">
				<input type="search" placeholder="Search title or path" bind:value={search} oninput={() => (pageNo = 1)} />
				<select bind:value={method} onchange={() => (pageNo = 1)} aria-label="Play method">
					<option value="">All methods</option>
					<option value="DirectPlay">Direct play</option>
					<option value="DirectStream">Direct stream</option>
					<option value="Transcode">Transcode</option>
				</select>
			</div>
		</div>
		{#if !events}
			<p class="faint card-pad">Loading…</p>
		{:else if events.items.length === 0}
			<div class="empty"><p>No playback sessions match.</p></div>
		{:else}
			<div class="table-scroll">
				<table class="table">
					<thead>
						<tr>
							<th>Title</th>
							<th>Method</th>
							<th class="hide-sm">Reason</th>
							<th class="hide-sm">User</th>
							<th class="hide-sm">Client</th>
							<th class="num hide-sm">Watched</th>
							<th>When</th>
						</tr>
					</thead>
					<tbody>
						{#each events.items as e (e.id)}
							<tr class:clickable={!!e.mediaFileId} onclick={() => e.mediaFileId && (detailId = e.mediaFileId)}>
								<td>
									<div class="ellipsis" style="max-width:320px" title={e.filePath}>{e.itemName}</div>
									{#if !e.mediaFileId}<div class="faint small">Not analyzed</div>{/if}
								</td>
								<td><Badge tone={methodTone[e.playMethod] ?? 'muted'}>{methodLabel[e.playMethod] ?? e.playMethod}</Badge></td>
								<td class="hide-sm small">
									{e.transcodeReasons ?? ''}
									{#if e.transcodeVideoCodec || e.transcodeAudioCodec}
										<div class="faint">→ {[e.transcodeVideoCodec, e.transcodeAudioCodec].filter(Boolean).join(' / ')}</div>
									{/if}
								</td>
								<td class="hide-sm">{e.userName ?? '–'}</td>
								<td class="hide-sm">
									<div>{e.clientName ?? '–'}</div>
									{#if e.deviceName}<div class="faint small">{e.deviceName}</div>{/if}
								</td>
								<td class="num hide-sm">{duration(e.durationSeconds)}</td>
								<td title={dateTime(e.startedAt)}>{ago(e.startedAt)}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
			<div class="card-pad"><Pager bind:page={pageNo} bind:pageSize total={events.total} /></div>
		{/if}
	</div>
{/if}

<MediaDrawer bind:id={detailId} />

<style>
	.stats {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 16px;
		margin-bottom: 16px;
	}
	.stat {
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: 16px 18px;
	}
	.stat strong {
		font-size: 24px;
		font-variant-numeric: tabular-nums;
	}
	.label {
		font-size: 12px;
		color: var(--text-3);
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}
	.grid {
		margin-bottom: 16px;
	}
	.grid.two {
		grid-template-columns: 1fr 1fr;
	}
	.grid.three {
		grid-template-columns: repeat(3, 1fr);
	}
	.sec {
		margin-bottom: 14px;
	}
	@media (max-width: 1000px) {
		.grid.two,
		.grid.three,
		.stats {
			grid-template-columns: 1fr;
		}
	}
</style>

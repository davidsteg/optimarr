<script lang="ts">
	import { api } from '$lib/api';
	import { live } from '$lib/live.svelte';
	import { ago, bytes, issueLabel, longDuration, num, pct, titleCase } from '$lib/format';
	import type { Bucket, Dashboard } from '$lib/types';
	import BarList from '$lib/components/BarList.svelte';
	import Distribution from '$lib/components/Distribution.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import MediaDrawer from '$lib/components/MediaDrawer.svelte';
	import Score from '$lib/components/Score.svelte';
	import Badge from '$lib/components/Badge.svelte';

	let data = $state<Dashboard | null>(null);
	let error = $state('');
	let selected = $state<number | null>(null);

	async function load() {
		try {
			data = await api.get<Dashboard>('/dashboard');
			error = '';
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	let timer: ReturnType<typeof setTimeout> | undefined;
	$effect(() => {
		live.mediaVersion;
		clearTimeout(timer);
		timer = setTimeout(load, data ? 800 : 0);
	});

	function count(b: Bucket[], key: string) {
		return b.find((x) => x.key === key)?.count ?? 0;
	}

	const compatSegments = $derived(
		data
			? [
					{ key: 'optimal', count: count(data.compatibility, 'optimal'), tone: 'good', href: '/media?compat=optimal' },
					{ key: 'good', count: count(data.compatibility, 'good'), tone: 'ok', href: '/media?compat=good' },
					{ key: 'poor', count: count(data.compatibility, 'poor'), tone: 'bad', href: '/media?compat=poor' }
				]
			: []
	);
	const qualitySegments = $derived(
		data
			? [
					{ key: 'excellent', count: count(data.quality, 'excellent'), tone: 'good', href: '/media?quality=excellent' },
					{ key: 'good', count: count(data.quality, 'good'), tone: 'ok', href: '/media?quality=good' },
					{ key: 'fair', count: count(data.quality, 'fair'), tone: 'warn', href: '/media?quality=fair' },
					{ key: 'poor', count: count(data.quality, 'poor'), tone: 'bad', href: '/media?quality=poor' }
				]
			: []
	);

	function bars(b: Bucket[], param: string) {
		return b.slice(0, 8).map((x) => ({ label: x.key, value: x.count, hint: `${num(x.count)} · ${bytes(x.size)}`, href: `/media?${param}=${encodeURIComponent(x.key)}` }));
	}
	const resolutionParam: Record<string, string> = { '4K': '4k', '1080p': '1080p', '720p': '720p', SD: 'sd' };
</script>

<div class="page-head">
	<div>
		<h1>Dashboard</h1>
		<p>How well your library direct-plays, and where quality falls short.</p>
	</div>
</div>

{#if error}
	<div class="card card-pad" style="color:var(--bad)">{error}</div>
{:else if !data}
	<p class="faint">Loading…</p>
{:else if data.totals.files === 0}
	<div class="card empty">
		<h3>No media analysed yet</h3>
		<p>Add a library folder and Optimarr will scan every video in it.</p>
		<a class="btn primary" href="/libraries"><Icon name="plus" size={15} />Add a library</a>
	</div>
{:else}
	<div class="stats">
		<div class="card stat">
			<span class="label">Files</span>
			<strong>{num(data.totals.files)}</strong>
			<span class="faint small">{bytes(data.totals.size)} · {longDuration(data.totals.duration)} runtime</span>
		</div>
		<div class="card stat score">
			<Score score={Math.round(data.totals.avgCompat ?? 0)} />
			<div>
				<span class="label">Avg. compatibility</span>
				<span class="faint small">{pct(count(data.compatibility, 'optimal'), data.totals.files)} direct-play everywhere</span>
			</div>
		</div>
		<div class="card stat score">
			<Score score={Math.round(data.totals.avgQuality ?? 0)} />
			<div>
				<span class="label">Avg. quality</span>
				<span class="faint small">{pct(count(data.quality, 'excellent'), data.totals.files)} rated excellent</span>
			</div>
		</div>
		<div class="card stat">
			<span class="label">Quick wins</span>
			<strong>{num(data.totals.remuxFixable)}</strong>
			<span class="faint small">fixable by a lossless remux</span>
		</div>
		<div class="card stat">
			<span class="label">Transcodes (30d)</span>
			<strong>{data.playback30d.plays ? pct(data.playback30d.transcodes, data.playback30d.plays) : '–'}</strong>
			<span class="faint small">{num(data.playback30d.transcodes)} of {num(data.playback30d.plays)} plays</span>
		</div>
	</div>

	{#if data.totals.broken || data.totals.processing || data.totals.missing}
		<div class="alerts">
			{#if data.totals.broken}
				<a class="alert bad" href="/media?status=broken"><Icon name="alert" size={15} />{num(data.totals.broken)} broken files</a>
			{/if}
			{#if data.totals.processing}
				<a class="alert warn" href="/media?status=processing"><Icon name="refresh" size={15} />{num(data.totals.processing)} waiting for a replacement</a>
			{/if}
			{#if data.totals.missing}
				<a class="alert muted" href="/media?status=missing"><Icon name="info" size={15} />{num(data.totals.missing)} missing from disk</a>
			{/if}
		</div>
	{/if}

	<div class="grid two">
		<div class="card card-pad">
			<h2 class="sec">Compatibility</h2>
			<Distribution segments={compatSegments} />
		</div>
		<div class="card card-pad">
			<h2 class="sec">Quality</h2>
			<Distribution segments={qualitySegments} />
		</div>
	</div>

	<div class="grid two">
		<div class="card">
			<div class="card-head">
				<h2>Most common issues</h2>
				<a href="/media" class="small">Browse all</a>
			</div>
			<div class="card-pad">
				<BarList
					items={data.issues.slice(0, 10).map((i) => ({
						label: `${issueLabel(i.code)}`,
						value: i.count,
						tone: i.severity === 'critical' ? 'bad' : i.severity === 'warning' ? 'warn' : 'ok',
						href: `/media?issue=${i.code}`
					}))}
					empty="No issues found."
				/>
			</div>
		</div>
		<div class="card">
			<div class="card-head">
				<h2>Needs attention</h2>
				<a href="/media?sort=compat&dir=asc" class="small">See more</a>
			</div>
			<div class="worst">
				{#each data.worst as m (m.id)}
					<button class="worst-row" onclick={() => (selected = m.id)}>
						<div class="grow">
							<div class="ellipsis">{m.title}</div>
							<div class="faint small ellipsis">
								{m.resolution} · {m.videoCodec} · {m.audioCodecs.join(', ')} · {m.container}
							</div>
						</div>
						<Score score={m.compatScore} verdict={m.compatVerdict} size="sm" />
						<Score score={m.qualityScore} verdict={m.qualityVerdict} size="sm" />
					</button>
				{:else}
					<p class="faint card-pad">Nothing to fix.</p>
				{/each}
			</div>
		</div>
	</div>

	<div class="grid four">
		<div class="card card-pad">
			<h2 class="sec">Video codecs</h2>
			<BarList items={bars(data.videoCodecs, 'videoCodec')} />
		</div>
		<div class="card card-pad">
			<h2 class="sec">Resolution</h2>
			<BarList
				items={data.resolutions.map((x) => ({
					label: x.key,
					value: x.count,
					hint: `${num(x.count)} · ${bytes(x.size)}`,
					href: resolutionParam[x.key] ? `/media?resolution=${resolutionParam[x.key]}` : undefined
				}))}
			/>
		</div>
		<div class="card card-pad">
			<h2 class="sec">Audio</h2>
			<BarList items={bars(data.audioCodecs, 'audioCodec')} />
		</div>
		<div class="card card-pad">
			<h2 class="sec">Containers</h2>
			<BarList items={bars(data.containers, 'container')} />
		</div>
	</div>

	{#if data.recentJobs.length > 0}
		<div class="card">
			<div class="card-head">
				<h2>Recent activity</h2>
				<a href="/activity" class="small">All activity</a>
			</div>
			<table class="table">
				<tbody>
					{#each data.recentJobs.slice(0, 5) as job (job.id)}
						<tr>
							<td>{job.title}</td>
							<td class="muted small hide-sm">{job.message ?? ''}</td>
							<td><Badge tone={job.status === 'completed' ? 'good' : job.status === 'failed' ? 'bad' : job.status === 'running' ? 'accent' : 'muted'}>{titleCase(job.status)}</Badge></td>
							<td class="faint small num">{ago(job.finishedAt ?? job.startedAt)}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
{/if}

<MediaDrawer bind:id={selected} />

<style>
	.stats {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
		gap: 16px;
		margin-bottom: 16px;
	}
	.stat {
		padding: 16px 18px;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}
	.stat.score {
		flex-direction: row;
		align-items: center;
		gap: 14px;
	}
	.stat.score > div {
		display: flex;
		flex-direction: column;
	}
	.stat strong {
		font-size: 26px;
		letter-spacing: -0.02em;
		line-height: 1.2;
	}
	.label {
		font-size: 12px;
		color: var(--text-3);
		text-transform: uppercase;
		letter-spacing: 0.05em;
		font-weight: 500;
	}
	.alerts {
		display: flex;
		flex-wrap: wrap;
		gap: 10px;
		margin-bottom: 16px;
	}
	.alert {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		padding: 8px 12px;
		border-radius: var(--radius-sm);
		font-weight: 500;
	}
	.alert.bad {
		background: var(--bad-soft);
		color: var(--bad);
	}
	.alert.warn {
		background: var(--warn-soft);
		color: var(--warn);
	}
	.alert.muted {
		background: var(--muted-soft);
		color: var(--text-2);
	}
	.grid {
		margin-bottom: 16px;
	}
	.grid.two {
		grid-template-columns: repeat(auto-fit, minmax(380px, 1fr));
	}
	.grid.four {
		grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
	}
	.sec {
		margin-bottom: 14px;
	}
	.worst {
		display: flex;
		flex-direction: column;
		padding: 6px;
	}
	.worst-row {
		display: flex;
		align-items: center;
		gap: 14px;
		width: 100%;
		text-align: left;
		padding: 8px 10px;
		border: 0;
		border-radius: var(--radius-sm);
		background: none;
		color: var(--text);
		font: inherit;
		cursor: pointer;
	}
	.worst-row:hover {
		background: var(--surface-2);
	}
	@media (max-width: 500px) {
		.grid.two {
			grid-template-columns: 1fr;
		}
	}
</style>

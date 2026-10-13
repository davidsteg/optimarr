<script lang="ts">
	import { api } from '$lib/api';
	import { ago, bitrate, bytes, dateTime, duration, issueLabel, titleCase, verdictTone } from '$lib/format';
	import { toasts } from '$lib/toast.svelte';
	import type { Issue, MediaDetail } from '$lib/types';
	import Badge from './Badge.svelte';
	import CopyBlock from './CopyBlock.svelte';
	import Drawer from './Drawer.svelte';
	import Icon from './Icon.svelte';
	import Score from './Score.svelte';

	let { id = $bindable(null) }: { id?: number | null } = $props();

	let detail = $state<MediaDetail | null>(null);
	let loading = $state(false);
	let busy = $state('');
	let error = $state('');

	$effect(() => {
		if (id !== null) load(id);
	});

	async function load(mediaId: number) {
		loading = true;
		error = '';
		try {
			detail = await api.get<MediaDetail>(`/media/${mediaId}`);
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
			detail = null;
		} finally {
			loading = false;
		}
	}

	async function rescan() {
		if (!detail) return;
		busy = 'rescan';
		try {
			await api.post(`/agent/files/${detail.id}/rescan`);
			await load(detail.id);
			toasts.success('File re-analysed');
		} catch (e) {
			toasts.error(e);
		} finally {
			busy = '';
		}
	}

	async function redownload() {
		if (!detail) return;
		if (!confirm(`Delete "${detail.fileName}" in ${detail.servarrKind} and search for a new release?`)) return;
		busy = 'redownload';
		try {
			const r = await api.post<{ results: { success: boolean; message: string }[] }>('/media/redownload', { ids: [detail.id] });
			const res = r.results[0];
			if (res?.success) toasts.success(res.message);
			else toasts.error(res?.message ?? 'Redownload failed');
			await load(detail.id);
		} catch (e) {
			toasts.error(e);
		} finally {
			busy = '';
		}
	}

	const compatIssues = $derived(detail?.assessment.issues.filter((i) => i.area === 'compatibility') ?? []);
	const qualityIssues = $derived(detail?.assessment.issues.filter((i) => i.area === 'quality') ?? []);

	function channels(n: number) {
		if (n === 1) return 'Mono';
		if (n === 2) return 'Stereo';
		if (n === 6) return '5.1';
		if (n === 8) return '7.1';
		return n ? `${n} ch` : '';
	}
</script>

{#snippet issueList(items: Issue[], emptyText: string)}
	{#if items.length === 0}
		<p class="ok-line"><Icon name="check" size={15} />{emptyText}</p>
	{:else}
		<ul class="issues">
			{#each items as issue (issue.code + issue.message)}
				<li class={issue.severity}>
					<div class="row" style="align-items:flex-start">
						<Icon name={issue.severity === 'info' ? 'info' : 'alert'} size={16} />
						<div class="grow">
							<div class="row wrap" style="gap:8px">
								<strong>{issueLabel(issue.code)}</strong>
								{#if issue.penalty}<span class="faint small">−{issue.penalty}</span>{/if}
							</div>
							<div class="muted">{issue.message}</div>
							{#if issue.fix}<div class="fix small"><Icon name="wrench" size={13} />{issue.fix}</div>{/if}
						</div>
					</div>
				</li>
			{/each}
		</ul>
	{/if}
{/snippet}

<Drawer
	bind:open={() => id !== null, (v) => {
		if (!v) id = null;
	}}
	title={detail?.title ?? 'Media'}>
	{#snippet header()}
		{#if detail}
			<h2 class="title">{detail.title}</h2>
			<div class="faint small mono path" title={detail.path}>{detail.path}</div>
		{:else}
			<h2>Loading…</h2>
		{/if}
	{/snippet}

	{#if error}
		<p style="color:var(--bad)">{error}</p>
	{:else if detail}
		<div class="stack" style="gap:22px; opacity:{loading ? 0.6 : 1}">
			<div class="scores">
				<div class="score-card">
					<Score score={detail.compatScore} verdict={detail.compatVerdict} size="lg" />
					<div>
						<div class="faint small">Compatibility</div>
						<Badge tone={verdictTone(detail.compatVerdict)}>{titleCase(detail.compatVerdict)}</Badge>
					</div>
				</div>
				<div class="score-card">
					<Score score={detail.qualityScore} verdict={detail.qualityVerdict} size="lg" />
					<div>
						<div class="faint small">Quality</div>
						<Badge tone={verdictTone(detail.qualityVerdict)}>{titleCase(detail.qualityVerdict)}</Badge>
					</div>
				</div>
			</div>

			{#if detail.status !== 'ok'}
				<div class="banner {detail.status}">
					<Icon name="alert" />
					<div>
						<strong>{titleCase(detail.status)}</strong>
						{#if detail.statusReason}<div class="small">{detail.statusReason}</div>{/if}
						{#if detail.status === 'processing' && detail.processingSince}
							<div class="small">Waiting for a replacement since {ago(detail.processingSince)}</div>
						{/if}
					</div>
				</div>
			{/if}

			<p class="summary">{detail.assessment.summary}</p>

			<div class="row wrap">
				<button class="btn" onclick={rescan} disabled={!!busy}>
					<Icon name="refresh" size={15} />{busy === 'rescan' ? 'Analysing…' : 'Re-analyse'}
				</button>
				{#if detail.servarrKind}
					<button class="btn danger" onclick={redownload} disabled={!!busy}>
						<Icon name="download" size={15} />{busy === 'redownload' ? 'Requesting…' : `Redownload via ${titleCase(detail.servarrKind)}`}
					</button>
				{/if}
			</div>

			<section>
				<h3>Compatibility</h3>
				{@render issueList(compatIssues, 'Direct-plays on your configured clients.')}
			</section>
			<section>
				<h3>Quality</h3>
				{@render issueList(qualityIssues, 'No quality concerns.')}
			</section>

			{#if detail.assessment.ffmpegCommand}
				<section>
					<h3>Suggested fix</h3>
					<p class="faint small" style="margin:0 0 8px">One ffmpeg command that resolves the compatibility issues it can.</p>
					<CopyBlock text={detail.assessment.ffmpegCommand} />
				</section>
			{/if}

			<section>
				<h3>Technical details</h3>
				<dl class="specs">
					<dt>Container</dt><dd>{detail.container || '–'}{detail.container === 'MP4' ? (detail.fastStart ? ' · fast start' : ' · no fast start') : ''}</dd>
					<dt>Video</dt>
					<dd>
						{detail.videoCodec || '–'}
						{#if detail.videoProfile}<span class="faint">({detail.videoProfile})</span>{/if}
						{#if detail.videoCodecTag}<span class="faint mono">[{detail.videoCodecTag}]</span>{/if}
					</dd>
					<dt>Resolution</dt><dd>{detail.width}×{detail.height} · {detail.resolution}{detail.frameRate ? ` · ${detail.frameRate.toFixed(3).replace(/\.?0+$/, '')} fps` : ''}</dd>
					<dt>Colour</dt><dd>{detail.bitDepth ? `${detail.bitDepth}-bit` : '–'} · {detail.hdrType || 'SDR'}</dd>
					<dt>Bitrate</dt><dd>{bitrate(detail.videoBitrate)} video · {bitrate(detail.overallBitrate)} overall</dd>
					<dt>Duration</dt><dd>{duration(detail.duration)}</dd>
					<dt>Size</dt><dd>{bytes(detail.size, 2)}</dd>
					<dt>Analysed</dt><dd>{dateTime(detail.analyzedAt)}</dd>
				</dl>
			</section>

			<section>
				<h3>Audio ({detail.audioTracks.length})</h3>
				{#if detail.audioTracks.length === 0}
					<p class="faint">No audio tracks.</p>
				{:else}
					<table class="table">
						<thead><tr><th>#</th><th>Codec</th><th>Channels</th><th>Language</th><th class="num">Bitrate</th></tr></thead>
						<tbody>
							{#each detail.audioTracks as t, i (i)}
								<tr>
									<td class="faint">{i + 1}</td>
									<td>{t.codec} {#if t.default}<Badge tone="accent">default</Badge>{/if}</td>
									<td>{channels(t.channels)}</td>
									<td>{t.language || '–'} {#if t.title}<span class="faint">· {t.title}</span>{/if}</td>
									<td class="num">{bitrate(t.bitrate)}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				{/if}
			</section>

			<section>
				<h3>Subtitles ({detail.subtitleTracks.length})</h3>
				{#if detail.subtitleTracks.length === 0}
					<p class="faint">No subtitles.</p>
				{:else}
					<table class="table">
						<thead><tr><th>Format</th><th>Language</th><th>Source</th></tr></thead>
						<tbody>
							{#each detail.subtitleTracks as s, i (i)}
								<tr>
									<td>{s.format} {#if s.forced}<Badge tone="warn">forced</Badge>{/if}</td>
									<td>{s.language || '–'}</td>
									<td class="faint">{s.embedded ? 'Embedded' : 'External file'}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				{/if}
			</section>

			<section>
				<h3>Integrations</h3>
				<dl class="specs">
					<dt>Managed by</dt>
					<dd>
						{#if detail.servarrKind === 'sonarr'}
							Sonarr · {detail.sonarrSeriesTitle}
							{#if detail.seasonNumber !== null}S{String(detail.seasonNumber).padStart(2, '0')}E{String(detail.episodeNumber ?? 0).padStart(2, '0')}{/if}
						{:else if detail.servarrKind === 'radarr'}
							Radarr · {detail.radarrMovieTitle}{detail.year ? ` (${detail.year})` : ''}
						{:else}
							<span class="faint">Not linked to Sonarr or Radarr</span>
						{/if}
					</dd>
					{#if detail.servarrRemotePath && detail.servarrRemotePath !== detail.path}
						<dt>Remote path</dt><dd class="mono small">{detail.servarrRemotePath}</dd>
					{/if}
					<dt>Jellyfin plays</dt>
					<dd>
						{detail.playback.plays} plays · {detail.playback.transcodes} transcoded
						{#if detail.playback.lastPlayed}<span class="faint">· last {ago(detail.playback.lastPlayed)}</span>{/if}
					</dd>
				</dl>
			</section>
		</div>
	{:else}
		<p class="faint">Loading…</p>
	{/if}
</Drawer>

<style>
	.title {
		font-size: 18px;
		word-break: break-word;
	}
	.path {
		margin-top: 4px;
		word-break: break-all;
	}
	.scores {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 12px;
	}
	.score-card {
		display: flex;
		align-items: center;
		gap: 16px;
		padding: 14px;
		border-radius: var(--radius);
		background: var(--surface);
		border: 1px solid var(--border);
	}
	.score-card :global(.badge) {
		margin-top: 4px;
	}
	.summary {
		margin: 0;
		color: var(--text-2);
	}
	section h3 {
		margin-bottom: 10px;
	}
	.issues {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	.issues li {
		padding: 10px 12px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--border);
		background: var(--surface);
		border-left-width: 3px;
	}
	.issues li.critical {
		border-left-color: var(--bad);
	}
	.issues li.critical > .row > :global(svg) {
		color: var(--bad);
	}
	.issues li.warning {
		border-left-color: var(--warn);
	}
	.issues li.warning > .row > :global(svg) {
		color: var(--warn);
	}
	.issues li.info {
		border-left-color: var(--ok);
	}
	.issues li.info > .row > :global(svg) {
		color: var(--ok);
	}
	.issues li > .row > :global(svg) {
		margin-top: 2px;
	}
	.fix {
		display: flex;
		align-items: center;
		gap: 6px;
		margin-top: 6px;
		color: var(--accent-strong);
	}
	.ok-line {
		display: flex;
		align-items: center;
		gap: 8px;
		margin: 0;
		color: var(--good);
	}
	.specs {
		display: grid;
		grid-template-columns: 130px 1fr;
		gap: 8px 16px;
		margin: 0;
	}
	.specs dt {
		color: var(--text-3);
	}
	.specs dd {
		margin: 0;
	}
	.banner {
		display: flex;
		gap: 10px;
		padding: 12px 14px;
		border-radius: var(--radius-sm);
		background: var(--warn-soft);
		color: var(--warn);
	}
	.banner.broken {
		background: var(--bad-soft);
		color: var(--bad);
	}
	.banner div {
		color: var(--text);
	}
	@media (max-width: 560px) {
		.scores {
			grid-template-columns: 1fr;
		}
	}
</style>

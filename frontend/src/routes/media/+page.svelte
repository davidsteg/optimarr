<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { api, downloadUrl, type Query } from '$lib/api';
	import { live } from '$lib/live.svelte';
	import { bytes, issueLabel, num, titleCase } from '$lib/format';
	import { toasts } from '$lib/toast.svelte';
	import type { Library, MediaSummary, Page } from '$lib/types';
	import Badge from '$lib/components/Badge.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import MediaDrawer from '$lib/components/MediaDrawer.svelte';
	import Pager from '$lib/components/Pager.svelte';
	import Score from '$lib/components/Score.svelte';

	interface Facet {
		value: string;
		count: number;
	}
	interface Facets {
		videoCodecs: Facet[];
		containers: Facet[];
		audioCodecs: Facet[];
		hdrTypes: Facet[];
		issues: Facet[];
	}

	const FILTER_KEYS = ['search', 'libraryId', 'compat', 'quality', 'status', 'videoCodec', 'audioCodec', 'container', 'resolution', 'hdr', 'issue', 'servarr'] as const;
	type FilterKey = (typeof FILTER_KEYS)[number];

	let filters = $state<Record<FilterKey, string>>(Object.fromEntries(FILTER_KEYS.map((k) => [k, ''])) as Record<FilterKey, string>);
	let sort = $state('compat');
	let dir = $state('asc');
	let pageNo = $state(1);
	let pageSize = $state(50);

	let result = $state<Page<MediaSummary> | null>(null);
	let facets = $state<Facets | null>(null);
	let libraries = $state<Library[]>([]);
	let loading = $state(false);
	let selected = $state(new Set<number>());
	let detailId = $state<number | null>(null);
	let showFilters = $state(false);
	let busy = $state(false);
	let initialised = $state(false);

	onMount(() => {
		const sp = page.url.searchParams;
		for (const k of FILTER_KEYS) filters[k] = sp.get(k) ?? '';
		sort = sp.get('sort') ?? 'compat';
		dir = sp.get('dir') ?? 'asc';
		pageNo = Number(sp.get('page') ?? 1) || 1;
		showFilters = FILTER_KEYS.some((k) => k !== 'search' && filters[k]);
		initialised = true;
		api.get<Facets>('/media/facets').then((f) => (facets = f)).catch(() => {});
		api.get<Library[]>('/libraries').then((l) => (libraries = l)).catch(() => {});
	});

	function query(): Query {
		return { ...filters, sort, dir };
	}

	let searchTimer: ReturnType<typeof setTimeout> | undefined;
	let lastSearch = '';
	$effect(() => {
		if (!initialised) return;
		const q = { ...query(), page: pageNo, pageSize };
		live.mediaVersion;
		const delay = filters.search !== lastSearch ? 250 : 0;
		lastSearch = filters.search;
		clearTimeout(searchTimer);
		searchTimer = setTimeout(() => load(q), delay);
	});

	async function load(q: Query) {
		loading = true;
		try {
			result = await api.get<Page<MediaSummary>>('/media', q);
			const sp = new URLSearchParams();
			for (const [k, v] of Object.entries(q)) {
				if (v === '' || v == null) continue;
				if ((k === 'page' && v === 1) || k === 'pageSize') continue;
				if ((k === 'sort' && v === 'compat') || (k === 'dir' && v === 'asc')) continue;
				sp.set(k, String(v));
			}
			const s = sp.toString();
			goto(s ? `?${s}` : '/media', { replaceState: true, keepFocus: true, noScroll: true });
		} catch (e) {
			toasts.error(e);
		} finally {
			loading = false;
		}
	}

	function setFilter(k: FilterKey, v: string) {
		filters[k] = v;
		pageNo = 1;
	}

	function clearFilters() {
		for (const k of FILTER_KEYS) filters[k] = '';
		pageNo = 1;
	}

	const activeFilters = $derived(FILTER_KEYS.filter((k) => k !== 'search' && filters[k]));

	function setSort(col: string) {
		if (sort === col) dir = dir === 'asc' ? 'desc' : 'asc';
		else {
			sort = col;
			dir = col === 'size' || col === 'bitrate' || col === 'analyzed' ? 'desc' : 'asc';
		}
		pageNo = 1;
	}

	const pageIds = $derived(result?.items.map((m) => m.id) ?? []);
	const allPageSelected = $derived(pageIds.length > 0 && pageIds.every((id) => selected.has(id)));

	function togglePage() {
		const next = new Set(selected);
		if (allPageSelected) pageIds.forEach((id) => next.delete(id));
		else pageIds.forEach((id) => next.add(id));
		selected = next;
	}
	function toggle(id: number) {
		const next = new Set(selected);
		if (next.has(id)) next.delete(id);
		else next.add(id);
		selected = next;
	}
	async function selectAllMatching() {
		try {
			const r = await api.get<{ ids: number[] }>('/media/ids', query());
			selected = new Set(r.ids);
		} catch (e) {
			toasts.error(e);
		}
	}

	async function rescanSelected() {
		busy = true;
		try {
			await api.post('/media/rescan', { ids: [...selected] });
			toasts.success(`Re-analysing ${num(selected.size)} files`);
			selected = new Set();
		} catch (e) {
			toasts.error(e);
		} finally {
			busy = false;
		}
	}

	async function redownloadSelected() {
		if (selected.size > 500) {
			toasts.error('Select at most 500 files to redownload at once');
			return;
		}
		if (!confirm(`Delete ${selected.size} file(s) in Sonarr/Radarr and search for replacements? This cannot be undone.`)) return;
		busy = true;
		try {
			const r = await api.post<{ succeeded: number; failed: number; results: { success: boolean; message: string }[] }>('/media/redownload', {
				ids: [...selected]
			});
			if (r.failed === 0) toasts.success(`Requested ${r.succeeded} redownload(s)`);
			else toasts.error(`${r.succeeded} requested, ${r.failed} failed: ${r.results.find((x) => !x.success)?.message ?? ''}`);
			selected = new Set();
		} catch (e) {
			toasts.error(e);
		} finally {
			busy = false;
		}
	}

	const labels: Record<string, string> = {
		libraryId: 'Library',
		compat: 'Compatibility',
		quality: 'Quality',
		status: 'Status',
		videoCodec: 'Video',
		audioCodec: 'Audio',
		container: 'Container',
		resolution: 'Resolution',
		hdr: 'HDR',
		issue: 'Issue',
		servarr: 'Managed by'
	};
	function chipValue(k: FilterKey) {
		const v = filters[k];
		if (k === 'libraryId') return libraries.find((l) => String(l.id) === v)?.name ?? v;
		if (k === 'issue') return issueLabel(v);
		return titleCase(v);
	}

	const columns: { key: string; label: string; sort?: string; cls?: string }[] = [
		{ key: 'title', label: 'Title', sort: 'name' },
		{ key: 'res', label: 'Resolution', sort: 'resolution', cls: 'hide-sm' },
		{ key: 'video', label: 'Video', sort: 'codec', cls: 'hide-sm' },
		{ key: 'audio', label: 'Audio', cls: 'hide-sm' },
		{ key: 'size', label: 'Size', sort: 'size', cls: 'num hide-sm' },
		{ key: 'compat', label: 'Compat.', sort: 'compat' },
		{ key: 'quality', label: 'Quality', sort: 'quality' },
		{ key: 'issues', label: 'Issues', cls: 'hide-sm' }
	];
</script>

<div class="page-head">
	<div>
		<h1>Media</h1>
		<p>{result ? `${num(result.total)} files` : 'Loading…'}{activeFilters.length ? ' match your filters' : ''}</p>
	</div>
	<div class="row">
		<a class="btn" href={downloadUrl('/media/export.csv', query())} download><Icon name="download" size={15} />Export CSV</a>
	</div>
</div>

<div class="card">
	<div class="toolbar">
		<div class="search">
			<Icon name="search" size={15} />
			<input type="search" placeholder="Search title, file name or path…" bind:value={filters.search} oninput={() => (pageNo = 1)} />
		</div>
		<select value={filters.compat} onchange={(e) => setFilter('compat', e.currentTarget.value)} aria-label="Compatibility">
			<option value="">Any compatibility</option>
			<option value="optimal">Optimal</option>
			<option value="good">Good</option>
			<option value="poor">Poor</option>
		</select>
		<select value={filters.quality} onchange={(e) => setFilter('quality', e.currentTarget.value)} aria-label="Quality">
			<option value="">Any quality</option>
			<option value="excellent">Excellent</option>
			<option value="good">Good</option>
			<option value="fair">Fair</option>
			<option value="poor">Poor</option>
		</select>
		<button class="btn" class:active={showFilters} onclick={() => (showFilters = !showFilters)}>
			More filters{activeFilters.length ? ` (${activeFilters.length})` : ''}
			<Icon name="chevronDown" size={14} />
		</button>
	</div>

	{#if showFilters}
		<div class="filters">
			<label class="field">Library
				<select value={filters.libraryId} onchange={(e) => setFilter('libraryId', e.currentTarget.value)}>
					<option value="">All</option>
					{#each libraries as l (l.id)}<option value={String(l.id)}>{l.name}</option>{/each}
				</select>
			</label>
			<label class="field">Status
				<select value={filters.status} onchange={(e) => setFilter('status', e.currentTarget.value)}>
					<option value="">Any</option>
					<option value="ok">OK</option>
					<option value="broken">Broken</option>
					<option value="processing">Waiting for replacement</option>
					<option value="missing">Missing</option>
				</select>
			</label>
			<label class="field">Issue
				<select value={filters.issue} onchange={(e) => setFilter('issue', e.currentTarget.value)}>
					<option value="">Any</option>
					{#each facets?.issues ?? [] as f (f.value)}<option value={f.value}>{issueLabel(f.value)} ({num(f.count)})</option>{/each}
				</select>
			</label>
			<label class="field">Video codec
				<select value={filters.videoCodec} onchange={(e) => setFilter('videoCodec', e.currentTarget.value)}>
					<option value="">Any</option>
					{#each facets?.videoCodecs ?? [] as f (f.value)}<option value={f.value}>{f.value} ({num(f.count)})</option>{/each}
				</select>
			</label>
			<label class="field">Audio codec
				<select value={filters.audioCodec} onchange={(e) => setFilter('audioCodec', e.currentTarget.value)}>
					<option value="">Any</option>
					{#each facets?.audioCodecs ?? [] as f (f.value)}<option value={f.value}>{f.value} ({num(f.count)})</option>{/each}
				</select>
			</label>
			<label class="field">Container
				<select value={filters.container} onchange={(e) => setFilter('container', e.currentTarget.value)}>
					<option value="">Any</option>
					{#each facets?.containers ?? [] as f (f.value)}<option value={f.value}>{f.value} ({num(f.count)})</option>{/each}
				</select>
			</label>
			<label class="field">Resolution
				<select value={filters.resolution} onchange={(e) => setFilter('resolution', e.currentTarget.value)}>
					<option value="">Any</option>
					<option value="4k">4K</option>
					<option value="1080p">1080p</option>
					<option value="720p">720p</option>
					<option value="sd">SD</option>
				</select>
			</label>
			<label class="field">HDR
				<select value={filters.hdr} onchange={(e) => setFilter('hdr', e.currentTarget.value)}>
					<option value="">Any</option>
					<option value="sdr">SDR</option>
					<option value="hdr">Any HDR</option>
					{#each facets?.hdrTypes ?? [] as f (f.value)}<option value={f.value}>{f.value} ({num(f.count)})</option>{/each}
				</select>
			</label>
			<label class="field">Managed by
				<select value={filters.servarr} onchange={(e) => setFilter('servarr', e.currentTarget.value)}>
					<option value="">Any</option>
					<option value="sonarr">Sonarr</option>
					<option value="radarr">Radarr</option>
					<option value="none">Not linked</option>
				</select>
			</label>
		</div>
	{/if}

	{#if activeFilters.length}
		<div class="chips">
			{#each activeFilters as k (k)}
				<button class="chip" onclick={() => setFilter(k, '')}>
					<span class="faint">{labels[k]}:</span>{chipValue(k)}<Icon name="x" size={12} />
				</button>
			{/each}
			<button class="btn ghost sm" onclick={clearFilters}>Clear all</button>
		</div>
	{/if}

	{#if selected.size > 0}
		<div class="bulk">
			<strong>{num(selected.size)} selected</strong>
			{#if result && selected.size < result.total}
				<button class="btn ghost sm" onclick={selectAllMatching}>Select all {num(result.total)} matching</button>
			{/if}
			<span class="grow"></span>
			<button class="btn sm" disabled={busy} onclick={rescanSelected}><Icon name="refresh" size={14} />Re-analyse</button>
			<button class="btn sm danger" disabled={busy} onclick={redownloadSelected}><Icon name="download" size={14} />Redownload</button>
			<button class="btn ghost sm" onclick={() => (selected = new Set())}>Clear</button>
		</div>
	{/if}

	<div class="table-scroll" style="opacity:{loading ? 0.65 : 1}">
		<table class="table">
			<thead>
				<tr>
					<th style="width:36px"><input type="checkbox" checked={allPageSelected} onchange={togglePage} aria-label="Select page" /></th>
					{#each columns as c (c.key)}
						<th class={c.cls}>
							{#if c.sort}
								<button class="sort" class:on={sort === c.sort} onclick={() => setSort(c.sort!)}>
									{c.label}{#if sort === c.sort}<Icon name={dir === 'asc' ? 'up' : 'chevronDown'} size={12} />{/if}
								</button>
							{:else}{c.label}{/if}
						</th>
					{/each}
				</tr>
			</thead>
			<tbody>
				{#each result?.items ?? [] as m (m.id)}
					<tr class="clickable" class:selected={selected.has(m.id)} onclick={() => (detailId = m.id)}>
						<td onclick={(e) => e.stopPropagation()}><input type="checkbox" checked={selected.has(m.id)} onchange={() => toggle(m.id)} aria-label="Select" /></td>
						<td class="title-cell">
							<div class="ellipsis strong">{m.title}</div>
							<div class="row small faint" style="gap:6px">
								{#if m.status !== 'ok'}<Badge tone={m.status === 'broken' ? 'bad' : 'warn'}>{titleCase(m.status)}</Badge>{/if}
								{#if m.servarrKind}<span>{titleCase(m.servarrKind)}</span>·{/if}
								<span class="ellipsis">{m.fileName}</span>
							</div>
						</td>
						<td class="hide-sm nowrap">{m.resolution || '–'}{#if m.hdrType}<Badge tone="accent">{m.hdrType}</Badge>{/if}</td>
						<td class="hide-sm nowrap">{m.videoCodec || '–'}{m.bitDepth > 8 ? ` ${m.bitDepth}-bit` : ''}</td>
						<td class="hide-sm nowrap ellipsis" style="max-width:140px">{m.audioCodecs.join(', ') || '–'}</td>
						<td class="num hide-sm nowrap">{bytes(m.size)}</td>
						<td><Score score={m.compatScore} verdict={m.compatVerdict} size="sm" /></td>
						<td><Score score={m.qualityScore} verdict={m.qualityVerdict} size="sm" /></td>
						<td class="hide-sm">
							<div class="tags">
								{#each m.issues.filter((i) => i.severity !== 'info').slice(0, 3) as i (i.code)}
									<Badge tone={i.severity}>{issueLabel(i.code)}</Badge>
								{/each}
								{#if m.issues.filter((i) => i.severity !== 'info').length > 3}
									<span class="faint small">+{m.issues.filter((i) => i.severity !== 'info').length - 3}</span>
								{/if}
							</div>
						</td>
					</tr>
				{:else}
					<tr><td colspan="9" class="empty">{loading ? 'Loading…' : 'No files match.'}</td></tr>
				{/each}
			</tbody>
		</table>
	</div>
	{#if result}
		<Pager bind:page={pageNo} bind:pageSize total={result.total} />
	{/if}
</div>

<MediaDrawer bind:id={detailId} />

<style>
	.toolbar {
		display: flex;
		gap: 10px;
		padding: 14px 16px;
		flex-wrap: wrap;
		border-bottom: 1px solid var(--border);
	}
	.search {
		flex: 1;
		min-width: 220px;
		position: relative;
		display: flex;
		align-items: center;
	}
	.search :global(svg) {
		position: absolute;
		left: 10px;
		color: var(--text-3);
	}
	.search input {
		width: 100%;
		padding-left: 32px;
	}
	.btn.active {
		border-color: var(--accent);
	}
	.filters {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
		gap: 12px;
		padding: 14px 16px;
		border-bottom: 1px solid var(--border);
		background: var(--surface-2);
	}
	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
		padding: 10px 16px;
		border-bottom: 1px solid var(--border);
	}
	.chip {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		height: 26px;
		padding: 0 10px;
		border-radius: 999px;
		border: 1px solid var(--border-strong);
		background: var(--surface-2);
		color: var(--text);
		font: inherit;
		font-size: 12.5px;
		cursor: pointer;
	}
	.chip:hover {
		border-color: var(--bad);
	}
	.bulk {
		display: flex;
		align-items: center;
		gap: 8px;
		flex-wrap: wrap;
		padding: 10px 16px;
		background: var(--accent-soft);
		border-bottom: 1px solid var(--border);
	}
	.sort {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		border: 0;
		background: none;
		padding: 0;
		font: inherit;
		color: inherit;
		text-transform: inherit;
		letter-spacing: inherit;
		cursor: pointer;
	}
	.sort.on,
	.sort:hover {
		color: var(--text);
	}
	.title-cell {
		max-width: 420px;
	}
	.strong {
		font-weight: 500;
	}
	.nowrap {
		white-space: nowrap;
	}
	.nowrap :global(.badge) {
		margin-left: 6px;
	}
	.tags {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
		align-items: center;
	}
</style>

<script lang="ts">
	import { api } from '$lib/api';
	import { live } from '$lib/live.svelte';
	import { ago, bytes, num } from '$lib/format';
	import { toasts } from '$lib/toast.svelte';
	import type { Library } from '$lib/types';
	import Badge from '$lib/components/Badge.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import Modal from '$lib/components/Modal.svelte';
	import PathPicker from '$lib/components/PathPicker.svelte';
	import Score from '$lib/components/Score.svelte';
	import Switch from '$lib/components/Switch.svelte';

	let libraries = $state<Library[] | null>(null);
	let error = $state('');

	let addOpen = $state(false);
	let picking = $state(false);
	let form = $state({ path: '', name: '', category: 'movies', watch: true, scan: true });
	let saving = $state(false);

	let editing = $state<Library | null>(null);
	let edit = $state({ name: '', category: 'movies', enabled: true, watch: true });

	let removing = $state<Library | null>(null);
	let keepMedia = $state(false);

	$effect(() => {
		live.librariesVersion;
		load();
	});

	async function load() {
		try {
			libraries = await api.get<Library[]>('/libraries');
			error = '';
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	function scanning(id: number) {
		return live.running.find((j) => j.kind === 'scan' && j.libraryId === id);
	}

	function openAdd() {
		form = { path: '', name: '', category: 'movies', watch: true, scan: true };
		picking = false;
		addOpen = true;
	}

	async function add(e: Event) {
		e.preventDefault();
		saving = true;
		try {
			await api.post('/libraries', { ...form, name: form.name || undefined });
			toasts.success(form.scan ? 'Library added, scan started' : 'Library added');
			addOpen = false;
			load();
		} catch (err) {
			toasts.error(err);
		} finally {
			saving = false;
		}
	}

	function openEdit(l: Library) {
		edit = { name: l.name, category: l.category, enabled: l.enabled, watch: l.watch };
		editing = l;
	}

	async function saveEdit(e: Event) {
		e.preventDefault();
		if (!editing) return;
		try {
			await api.put(`/libraries/${editing.id}`, edit);
			editing = null;
			load();
		} catch (err) {
			toasts.error(err);
		}
	}

	async function remove() {
		if (!removing) return;
		try {
			await api.del(`/libraries/${removing.id}`, { keepMedia });
			toasts.success(`Removed ${removing.name}`);
			removing = null;
			load();
		} catch (err) {
			toasts.error(err);
		}
	}

	async function scan(l: Library, full = false) {
		try {
			await api.post(`/libraries/${l.id}/scan`, {}, { full });
			toasts.success(full ? `Full rescan of ${l.name} started` : `Scanning ${l.name}`);
		} catch (err) {
			toasts.error(err);
		}
	}

	async function scanAll() {
		try {
			const r = await api.post<{ jobIds: number[] }>('/libraries/scan');
			toasts.success(r.jobIds.length ? `Scanning ${r.jobIds.length} libraries` : 'No enabled libraries to scan');
		} catch (err) {
			toasts.error(err);
		}
	}

	async function sync(kind: 'sonarr' | 'radarr') {
		try {
			const r = await api.post<{ added: string[]; existing: number; skipped: string[] }>(`/integrations/servarr/${kind}/sync-libraries`);
			const label = kind === 'sonarr' ? 'Sonarr' : 'Radarr';
			toasts.success(r.added.length ? `Added ${r.added.join(', ')} from ${label}` : `All ${label} root folders are already libraries`);
			for (const s of r.skipped) toasts.error(`Root folder skipped: ${s}`);
			load();
		} catch (err) {
			toasts.error(err);
		}
	}

	const categoryLabel: Record<string, string> = { movies: 'Movies', series: 'Series', other: 'Other' };
</script>

<div class="page-head">
	<div>
		<h1>Libraries</h1>
		<p>Folders Optimarr scans and watches for video files.</p>
	</div>
	<div class="row wrap">
		<button class="btn" onclick={() => sync('sonarr')}><Icon name="link" size={15} />Sync Sonarr</button>
		<button class="btn" onclick={() => sync('radarr')}><Icon name="link" size={15} />Sync Radarr</button>
		<button class="btn" onclick={scanAll} disabled={!libraries?.length}><Icon name="refresh" size={15} />Scan all</button>
		<button class="btn primary" onclick={openAdd}><Icon name="plus" size={15} />Add library</button>
	</div>
</div>

{#if error}
	<div class="card card-pad" style="color:var(--bad)">{error}</div>
{:else if !libraries}
	<p class="faint">Loading…</p>
{:else if libraries.length === 0}
	<div class="card empty">
		<h3>No libraries yet</h3>
		<p>Add a folder with movies or series, or sync root folders from Sonarr and Radarr.</p>
		<button class="btn primary" onclick={openAdd}><Icon name="plus" size={15} />Add library</button>
	</div>
{:else}
	<div class="libs">
		{#each libraries as l (l.id)}
			{@const job = scanning(l.id)}
			<div class="card lib" class:disabled={!l.enabled}>
				<div class="top">
					<div class="grow">
						<div class="row">
							<h2 class="ellipsis">{l.name}</h2>
							<Badge>{categoryLabel[l.category] ?? l.category}</Badge>
							{#if l.servarrKind}<Badge tone="accent">{l.servarrKind === 'sonarr' ? 'Sonarr' : 'Radarr'}</Badge>{/if}
							{#if !l.enabled}<Badge tone="warn">Disabled</Badge>{/if}
						</div>
						<div class="mono faint ellipsis" title={l.path}>{l.path}</div>
					</div>
					<button class="btn ghost icon sm" onclick={() => openEdit(l)} aria-label="Edit library"><Icon name="settings" size={15} /></button>
				</div>

				<div class="metrics">
					<div><span class="label">Files</span><strong>{num(l.fileCount)}</strong><span class="faint small">{bytes(l.totalSize)}</span></div>
					<div class="sc"><Score score={Math.round(l.avgCompat ?? 0)} verdict={l.avgCompat == null ? 'unknown' : ''} size="sm" /><span class="label">Compat.</span></div>
					<div class="sc"><Score score={Math.round(l.avgQuality ?? 0)} verdict={l.avgQuality == null ? 'unknown' : ''} size="sm" /><span class="label">Quality</span></div>
				</div>

				{#if l.fileCount > 0}
					<div class="bar" title="Optimal {l.optimal} · Good {l.good} · Poor {l.poor} · Broken {l.broken}">
						<span style="flex:{l.optimal};background:var(--good)"></span>
						<span style="flex:{l.good};background:var(--ok)"></span>
						<span style="flex:{l.poor};background:var(--bad)"></span>
						<span style="flex:{l.broken};background:var(--text-3)"></span>
					</div>
				{/if}

				{#if job}
					<div class="scan">
						<div class="row small"><span class="grow ellipsis">{job.currentItem ?? 'Scanning…'}</span><span>{num(job.processed)} / {num(job.total)}</span></div>
						<div class="track"><div class="fill" style="width:{job.total ? (job.processed / job.total) * 100 : 5}%"></div></div>
					</div>
				{/if}

				<div class="foot">
					<span class="faint small grow">Scanned {ago(l.lastScannedAt)}{l.watch ? ' · watching' : ''}</span>
					<a class="btn sm ghost" href="/media?libraryId={l.id}">Browse</a>
					<button class="btn sm" onclick={() => scan(l)} disabled={!!job || !l.enabled}><Icon name="refresh" size={14} />Scan</button>
				</div>
			</div>
		{/each}
	</div>
{/if}

<Modal bind:open={addOpen} title="Add library" width={560}>
	<form id="add-lib" class="stack" onsubmit={add}>
		<label class="field">
			Folder
			<div class="row">
				<input class="grow mono" type="text" bind:value={form.path} placeholder="/media/movies" required />
				<button type="button" class="btn" onclick={() => (picking = !picking)}><Icon name="folder" size={15} />Browse</button>
			</div>
		</label>
		{#if picking}
			<PathPicker bind:value={form.path} onpick={() => (picking = false)} />
		{/if}
		<label class="field">
			Name <span class="hint">Defaults to the folder name</span>
			<input type="text" bind:value={form.name} />
		</label>
		<label class="field">
			Type
			<select bind:value={form.category}>
				<option value="movies">Movies</option>
				<option value="series">Series</option>
				<option value="other">Other</option>
			</select>
		</label>
		<Switch bind:checked={form.watch} label="Watch for changes" hint="Analyze new and changed files as they appear" />
		<Switch bind:checked={form.scan} label="Scan now" />
	</form>
	{#snippet footer()}
		<button class="btn" onclick={() => (addOpen = false)}>Cancel</button>
		<button class="btn primary" type="submit" form="add-lib" disabled={saving || !form.path}>Add library</button>
	{/snippet}
</Modal>

<Modal open={!!editing} title="Edit library" onclose={() => (editing = null)}>
	{#if editing}
		<form id="edit-lib" class="stack" onsubmit={saveEdit}>
			<div class="mono faint small">{editing.path}</div>
			<label class="field">Name<input type="text" bind:value={edit.name} required /></label>
			<label class="field">
				Type
				<select bind:value={edit.category}>
					<option value="movies">Movies</option>
					<option value="series">Series</option>
					<option value="other">Other</option>
				</select>
			</label>
			<Switch bind:checked={edit.enabled} label="Enabled" hint="Disabled libraries are not scanned" />
			<Switch bind:checked={edit.watch} label="Watch for changes" />
			<div class="row wrap">
				<button type="button" class="btn sm" onclick={() => editing && scan(editing, true)}><Icon name="refresh" size={14} />Full rescan</button>
				<button
					type="button"
					class="btn sm danger"
					onclick={() => {
						keepMedia = false;
						removing = editing;
						editing = null;
					}}><Icon name="trash" size={14} />Remove</button
				>
			</div>
		</form>
	{/if}
	{#snippet footer()}
		<button class="btn" onclick={() => (editing = null)}>Cancel</button>
		<button class="btn primary" type="submit" form="edit-lib">Save</button>
	{/snippet}
</Modal>

<Modal open={!!removing} title="Remove library" onclose={() => (removing = null)}>
	{#if removing}
		<div class="stack">
			<p style="margin:0">
				Remove <strong>{removing.name}</strong> from Optimarr? Files on disk are never touched.
			</p>
			<Switch bind:checked={keepMedia} label="Keep analysis results" hint="Otherwise the {num(removing.fileCount)} analyzed files are forgotten" />
		</div>
	{/if}
	{#snippet footer()}
		<button class="btn" onclick={() => (removing = null)}>Cancel</button>
		<button class="btn danger" onclick={remove}>Remove</button>
	{/snippet}
</Modal>

<style>
	.libs {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
		gap: 16px;
	}
	.lib {
		display: flex;
		flex-direction: column;
		gap: 14px;
		padding: 16px 18px;
	}
	.lib.disabled {
		opacity: 0.7;
	}
	.top {
		display: flex;
		gap: 10px;
		align-items: flex-start;
	}
	.top h2 {
		min-width: 0;
	}
	.metrics {
		display: grid;
		grid-template-columns: 1.4fr 1fr 1fr;
		gap: 10px;
		align-items: center;
	}
	.metrics > div {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}
	.metrics .sc {
		flex-direction: row;
		align-items: center;
		gap: 8px;
	}
	.metrics strong {
		font-size: 20px;
		font-variant-numeric: tabular-nums;
	}
	.label {
		font-size: 12px;
		color: var(--text-3);
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}
	.bar {
		display: flex;
		height: 6px;
		border-radius: 999px;
		overflow: hidden;
		background: var(--surface-3);
		gap: 2px;
	}
	.scan .track {
		height: 4px;
		border-radius: 999px;
		background: var(--surface-3);
		margin-top: 6px;
		overflow: hidden;
	}
	.scan .fill {
		height: 100%;
		background: var(--accent);
		transition: width 0.4s;
	}
	.foot {
		display: flex;
		align-items: center;
		gap: 6px;
		margin-top: auto;
	}
	@media (max-width: 720px) {
		.libs {
			grid-template-columns: 1fr;
		}
	}
</style>

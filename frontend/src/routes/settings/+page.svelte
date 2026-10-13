<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { api, getApiKey, setApiKey } from '$lib/api';
	import { bytes, longDuration, num } from '$lib/format';
	import { toasts } from '$lib/toast.svelte';
	import type { PathMapping } from '$lib/types';
	import CopyBlock from '$lib/components/CopyBlock.svelte';
	import DeviceSettings from '$lib/components/DeviceSettings.svelte';
	import NotificationSettings from '$lib/components/NotificationSettings.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import MappingsEditor from '$lib/components/MappingsEditor.svelte';
	import Switch from '$lib/components/Switch.svelte';

	interface General {
		scanConcurrency: number;
		watchLibraries: boolean;
		processingRescanHours: number;
		probeBackend: string;
		periodicScanHours: number;
	}
	interface Compat {
		videoCodecs: Record<string, boolean>;
		audioCodecs: Record<string, boolean>;
		containers: Record<string, boolean>;
		subtitleFormats: Record<string, boolean>;
		bitDepths: Record<string, boolean>;
		weights: Record<string, number>;
		thresholds: { optimal: number; good: number };
	}
	interface Quality {
		penalty1080p: number;
		penalty720p: number;
		penaltySd: number;
		penaltyLowBitrate: number;
		penaltyVeryLowBitrate: number;
		penaltyLegacyCodec: number;
		penaltyStereoOnly: number;
		penaltyLowAudioBitrate: number;
		penaltySdr4k: number;
		minBitsPerPixel: number;
		preferredAudioLanguages: string[];
		preferredSubtitleLanguages: string[];
		penaltyMissingAudioLanguage: number;
		penaltyMissingSubtitleLanguage: number;
		penaltyImageSubtitles: number;
		thresholds: { excellent: number; good: number; fair: number };
	}
	interface FixSettings {
		enabled: boolean;
		replaceOriginal: boolean;
		keepOriginal: boolean;
	}
	interface UpgradeSettings {
		maxQualityScore: number;
		cooldownDays: number;
		autoEnabled: boolean;
		autoIntervalHours: number;
		autoBatchSize: number;
	}
	interface Servarr {
		enabled: boolean;
		baseUrl: string;
		apiKey: string;
		basicAuthUsername: string | null;
		basicAuthPassword: string | null;
		pathMappings: PathMapping[];
	}
	interface Jellyfin {
		enabled: boolean;
		baseUrl: string;
		apiKey: string;
		pathMappings: PathMapping[];
		syncIntervalMinutes: number;
	}
	interface Security {
		apiKey: string;
		mode: string;
		keyFromEnvironment: boolean;
	}
	interface Status {
		version: string;
		uptimeSeconds: number;
		probe: { mediainfo: boolean; ffprobe: boolean };
		dataDir: string;
		databaseSize: number;
		counts: { media: number; libraries: number; playbackEvents: number };
		legacyImport: Record<string, unknown> | null;
	}

	const tabs = [
		{ id: 'general', label: 'General' },
		{ id: 'compatibility', label: 'Compatibility' },
		{ id: 'quality', label: 'Quality' },
		{ id: 'sonarr', label: 'Sonarr' },
		{ id: 'radarr', label: 'Radarr' },
		{ id: 'jellyfin', label: 'Jellyfin' },
		{ id: 'devices', label: 'Devices' },
		{ id: 'fixes', label: 'Fixes & upgrades' },
		{ id: 'notifications', label: 'Notifications' },
		{ id: 'security', label: 'Security' },
		{ id: 'about', label: 'About' }
	] as const;
	type Tab = (typeof tabs)[number]['id'];

	let tab = $state<Tab>('general');
	let general = $state<General | null>(null);
	let compat = $state<Compat | null>(null);
	let quality = $state<Quality | null>(null);
	let servarr = $state<Record<'sonarr' | 'radarr', Servarr | null>>({ sonarr: null, radarr: null });
	let jellyfin = $state<Jellyfin | null>(null);
	let security = $state<Security | null>(null);
	let status = $state<Status | null>(null);
	let fixSettings = $state<FixSettings | null>(null);
	let upgradeSettings = $state<UpgradeSettings | null>(null);
	let audioLangs = $state('');
	let subLangs = $state('');
	let saving = $state(false);
	let testResult = $state<{ ok: boolean; text: string } | null>(null);
	let showKey = $state(false);

	onMount(() => {
		const t = page.url.searchParams.get('tab') as Tab | null;
		if (t && tabs.some((x) => x.id === t)) tab = t;
	});

	$effect(() => {
		load(tab);
	});

	function select(t: Tab) {
		tab = t;
		testResult = null;
		goto(t === 'general' ? '/settings' : `/settings?tab=${t}`, { replaceState: true, noScroll: true, keepFocus: true });
	}

	async function load(t: Tab) {
		try {
			switch (t) {
				case 'general':
					general = await api.get('/settings/general');
					break;
				case 'compatibility':
					compat = await api.get('/settings/compatibility');
					break;
				case 'quality':
					quality = await api.get('/settings/quality');
					audioLangs = quality?.preferredAudioLanguages.join(', ') ?? '';
					subLangs = quality?.preferredSubtitleLanguages.join(', ') ?? '';
					break;
				case 'fixes':
					fixSettings = await api.get('/settings/fixes');
					upgradeSettings = await api.get('/settings/upgrades');
					break;
				case 'sonarr':
				case 'radarr': {
					const s = await api.get<Servarr>(`/settings/servarr/${t}`);
					servarr[t] = { ...s, basicAuthUsername: s.basicAuthUsername ?? '', basicAuthPassword: s.basicAuthPassword ?? '' };
					break;
				}
				case 'jellyfin':
					jellyfin = await api.get('/settings/jellyfin');
					break;
				case 'security':
					security = await api.get('/system/security');
					break;
				case 'about':
					status = await api.get('/system/status');
					break;
			}
		} catch (e) {
			toasts.error(e);
		}
	}

	async function save(path: string, body: unknown, message = 'Settings saved') {
		saving = true;
		try {
			const r = await api.put<{ jobId?: number; restartRequired?: boolean }>(path, body);
			toasts.success(r.jobId ? `${message}; re-scoring all files` : r.restartRequired ? `${message}; some changes apply after a restart` : message);
			return true;
		} catch (e) {
			toasts.error(e);
			return false;
		} finally {
			saving = false;
		}
	}

	async function reset(kind: 'compatibility' | 'quality') {
		if (!confirm(`Reset the ${kind} profile to its defaults and re-score every file?`)) return;
		try {
			await api.post(`/settings/${kind}/reset`);
			toasts.success('Defaults restored; re-scoring all files');
			load(kind);
		} catch (e) {
			toasts.error(e);
		}
	}

	function servarrBody(s: Servarr) {
		return { ...s, basicAuthUsername: s.basicAuthUsername || null, basicAuthPassword: s.basicAuthPassword || null };
	}

	async function testServarr(kind: 'sonarr' | 'radarr') {
		const s = servarr[kind];
		if (!s) return;
		testResult = null;
		try {
			const r = await api.post<{ ok: boolean; version?: string; rootFolders?: string[]; error?: string }>(
				`/integrations/servarr/${kind}/test`,
				servarrBody(s)
			);
			testResult = r.ok
				? { ok: true, text: `Connected to version ${r.version}. Root folders: ${(r.rootFolders ?? []).join(', ') || 'none'}` }
				: { ok: false, text: r.error ?? 'Connection failed' };
		} catch (e) {
			testResult = { ok: false, text: e instanceof Error ? e.message : String(e) };
		}
	}

	async function testJellyfin() {
		if (!jellyfin) return;
		testResult = null;
		try {
			const r = await api.post<{ ok: boolean; info?: { serverName?: string; version?: string; playbackReporting?: boolean }; error?: string }>(
				'/integrations/jellyfin/test',
				jellyfin
			);
			testResult = r.ok
				? r.info?.playbackReporting === false
					? { ok: false, text: `Connected to ${r.info?.serverName ?? 'Jellyfin'}, but the Playback Reporting plugin is not installed, so no history can be read` }
					: { ok: true, text: `Connected to ${r.info?.serverName ?? 'Jellyfin'}${r.info?.version ? ` ${r.info.version}` : ''}` }
				: { ok: false, text: r.error ?? 'Connection failed' };
		} catch (e) {
			testResult = { ok: false, text: e instanceof Error ? e.message : String(e) };
		}
	}

	async function matchFiles() {
		try {
			await api.post('/integrations/servarr/match');
			toasts.success('Matching files to Sonarr and Radarr');
		} catch (e) {
			toasts.error(e);
		}
	}

	async function setMode(mode: string) {
		try {
			security = await api.put<Security>('/system/security', { mode });
			toasts.success('Saved');
		} catch (e) {
			toasts.error(e);
		}
	}

	async function regenerate() {
		if (!confirm('Generate a new API key? Agents and scripts using the old key will stop working.')) return;
		try {
			const hadKey = !!getApiKey();
			security = await api.post<Security>('/system/security/regenerate');
			if (hadKey && security) setApiKey(security.apiKey);
			toasts.success('New API key generated');
		} catch (e) {
			toasts.error(e);
		}
	}

	const compatGroups = [
		{ key: 'videoCodecs', label: 'Video codecs' },
		{ key: 'audioCodecs', label: 'Audio codecs' },
		{ key: 'containers', label: 'Containers' },
		{ key: 'subtitleFormats', label: 'Subtitle formats' },
		{ key: 'bitDepths', label: 'Bit depths' }
	] as const;

	const weightLabels: Record<string, string> = {
		unsupportedVideoCodec: 'Unsupported video codec',
		unsupportedAudioCodec: 'Unsupported audio codec',
		unsupportedContainer: 'Unsupported container',
		unsupportedSubtitleFormat: 'Unsupported subtitles',
		unsupportedBitDepth: 'Unsupported bit depth',
		incorrectCodecTag: 'Wrong codec tag (hev1)',
		missingFastStart: 'MP4 without fast start',
		highBitrate: 'High bitrate',
		highBitrateThresholdMbps: 'High bitrate threshold (Mbps)'
	};

	const qualityFields: { key: keyof Quality; label: string; step?: number }[] = [
		{ key: 'penalty1080p', label: 'Below 4K (1080p)' },
		{ key: 'penalty720p', label: 'Below 1080p (720p)' },
		{ key: 'penaltySd', label: 'SD' },
		{ key: 'penaltyLowBitrate', label: 'Low bitrate' },
		{ key: 'penaltyVeryLowBitrate', label: 'Very low bitrate' },
		{ key: 'penaltyLegacyCodec', label: 'Legacy codec (MPEG-2, VC-1…)' },
		{ key: 'penaltyStereoOnly', label: 'Stereo only' },
		{ key: 'penaltyLowAudioBitrate', label: 'Low audio bitrate' },
		{ key: 'penaltySdr4k', label: '4K without HDR' },
		{ key: 'minBitsPerPixel', label: 'Minimum bits per pixel', step: 0.005 }
	];

	const origin = typeof location !== 'undefined' ? location.origin : '';
</script>

<div class="page-head">
	<div>
		<h1>Settings</h1>
		<p>Scanning, scoring and integrations.</p>
	</div>
</div>

<div class="layout">
	<nav class="tabs" aria-label="Settings sections">
		{#each tabs as t (t.id)}
			<button class:active={tab === t.id} onclick={() => select(t.id)}>{t.label}</button>
		{/each}
	</nav>

	<div class="panel">
		{#if tab === 'general'}
			{#if general}
				{@const g = general}
				<form
					class="card card-pad stack"
					onsubmit={(e) => {
						e.preventDefault();
						save('/settings/general', g);
					}}
				>
					<h2>Scanning</h2>
					<label class="field">
						Probe tool
						<select bind:value={g.probeBackend}>
							<option value="auto">Automatic (MediaInfo, falling back to ffprobe)</option>
							<option value="mediainfo">MediaInfo</option>
							<option value="ffprobe">ffprobe</option>
						</select>
					</label>
					<label class="field">
						Parallel probes <span class="hint">0 uses one per CPU core. Lower this for network shares.</span>
						<input type="number" min="0" max="64" bind:value={g.scanConcurrency} />
					</label>
					<Switch bind:checked={g.watchLibraries} label="Watch libraries" hint="Analyze new and changed files as soon as they appear" />
					<label class="field">
						Periodic rescan (hours) <span class="hint">Catches changes on network mounts where watching does not work. 0 disables.</span>
						<input type="number" min="0" bind:value={g.periodicScanHours} />
					</label>
					<label class="field">
						Recheck replacements after (hours) <span class="hint">Files waiting for a redownload are rescanned after this long</span>
						<input type="number" min="1" bind:value={g.processingRescanHours} />
					</label>
					<div><button class="btn primary" disabled={saving}>Save</button></div>
				</form>
			{/if}
		{:else if tab === 'compatibility'}
			{#if compat}
				{@const c = compat}
				<form
					class="stack"
					onsubmit={(e) => {
						e.preventDefault();
						save('/settings/compatibility', c);
					}}
				>
					<div class="card card-pad stack">
						<div>
							<h2>Direct-play support</h2>
							<p class="muted small" style="margin:4px 0 0">Tick what every one of your clients can play without transcoding.</p>
						</div>
						<div class="groups">
							{#each compatGroups as grp (grp.key)}
								<fieldset>
									<legend>{grp.label}</legend>
									{#each Object.keys(c[grp.key]) as name (name)}
										<label class="check"><input type="checkbox" bind:checked={c[grp.key][name]} />{grp.key === 'bitDepths' ? `${name}-bit` : name}</label>
									{/each}
								</fieldset>
							{/each}
						</div>
					</div>
					<div class="card card-pad stack">
						<h2>Penalties</h2>
						<div class="fields">
							{#each Object.keys(c.weights) as k (k)}
								<label class="field">{weightLabels[k] ?? k}<input type="number" min="0" step="any" bind:value={c.weights[k]} /></label>
							{/each}
						</div>
						<h2>Verdicts</h2>
						<div class="fields">
							<label class="field">Optimal from<input type="number" min="0" max="100" bind:value={c.thresholds.optimal} /></label>
							<label class="field">Good from<input type="number" min="0" max="100" bind:value={c.thresholds.good} /></label>
						</div>
					</div>
					<div class="row">
						<button class="btn primary" disabled={saving}>Save and re-score</button>
						<button type="button" class="btn" onclick={() => reset('compatibility')}>Reset to defaults</button>
					</div>
				</form>
			{/if}
		{:else if tab === 'quality'}
			{#if quality}
				{@const q = quality}
				<form
					class="stack"
					onsubmit={(e) => {
						e.preventDefault();
						const split = (v: string) => v.split(/[,\s]+/).map((x) => x.trim()).filter(Boolean);
						save('/settings/quality', { ...q, preferredAudioLanguages: split(audioLangs), preferredSubtitleLanguages: split(subLangs) });
					}}
				>
					<div class="card card-pad stack">
						<div>
							<h2>Penalties</h2>
							<p class="muted small" style="margin:4px 0 0">
								Points taken off the 100-point quality score. Bitrate is judged in bits per pixel per frame, so it scales with
								resolution and frame rate; HEVC, VP9 and AV1 need 60% of the H.264 figure.
							</p>
						</div>
						<div class="fields">
							{#each qualityFields as f (f.key)}
								<label class="field">{f.label}<input type="number" min="0" step={f.step ?? 1} bind:value={q[f.key] as number} /></label>
							{/each}
						</div>
						<h2>Languages</h2>
						<p class="muted small" style="margin:0">Flag files without audio or subtitles in your languages, and subtitles that only exist as images. Use codes or names, separated by commas (en, de, French). Leave empty to skip the check.</p>
						<div class="fields">
							<label class="field">Audio languages<input type="text" bind:value={audioLangs} placeholder="en, de" /></label>
							<label class="field">Subtitle languages<input type="text" bind:value={subLangs} placeholder="en" /></label>
							<label class="field">Missing audio language<input type="number" min="0" bind:value={q.penaltyMissingAudioLanguage} /></label>
							<label class="field">Missing subtitle language<input type="number" min="0" bind:value={q.penaltyMissingSubtitleLanguage} /></label>
							<label class="field">Image-only subtitles<input type="number" min="0" bind:value={q.penaltyImageSubtitles} /></label>
						</div>
						<h2>Verdicts</h2>
						<div class="fields">
							<label class="field">Excellent from<input type="number" min="0" max="100" bind:value={q.thresholds.excellent} /></label>
							<label class="field">Good from<input type="number" min="0" max="100" bind:value={q.thresholds.good} /></label>
							<label class="field">Fair from<input type="number" min="0" max="100" bind:value={q.thresholds.fair} /></label>
						</div>
					</div>
					<div class="row">
						<button class="btn primary" disabled={saving}>Save and re-score</button>
						<button type="button" class="btn" onclick={() => reset('quality')}>Reset to defaults</button>
					</div>
				</form>
			{/if}
		{:else if tab === 'sonarr' || tab === 'radarr'}
			{@const kind = tab}
			{@const s = servarr[kind]}
			{#if s}
				<form
					class="card card-pad stack"
					onsubmit={(e) => {
						e.preventDefault();
						save(`/settings/servarr/${kind}`, servarrBody(s));
					}}
				>
					<div>
						<h2>{kind === 'sonarr' ? 'Sonarr' : 'Radarr'}</h2>
						<p class="muted small" style="margin:4px 0 0">
							Links files to {kind === 'sonarr' ? 'series and episodes' : 'movies'}, syncs root folders as libraries, and lets you
							request a replacement for a bad file.
						</p>
					</div>
					<Switch bind:checked={s.enabled} label="Enabled" />
					<label class="field">URL<input type="url" bind:value={s.baseUrl} placeholder={kind === 'sonarr' ? 'http://sonarr:8989' : 'http://radarr:7878'} /></label>
					<label class="field">API key<input type="password" bind:value={s.apiKey} autocomplete="off" /></label>
					<details>
						<summary class="small muted">Basic authentication (reverse proxy)</summary>
						<div class="fields" style="margin-top:12px">
							<label class="field">Username<input type="text" bind:value={s.basicAuthUsername} autocomplete="off" /></label>
							<label class="field">Password<input type="password" bind:value={s.basicAuthPassword} autocomplete="off" /></label>
						</div>
					</details>
					<div class="field-like">
						<span class="small muted" style="font-weight:500">Path mappings</span>
						<span class="hint">Only needed when {kind === 'sonarr' ? 'Sonarr' : 'Radarr'} sees your media under a different path than Optimarr.</span>
						<MappingsEditor bind:mappings={s.pathMappings} remoteLabel="Path in {kind === 'sonarr' ? 'Sonarr' : 'Radarr'}" />
					</div>
					{#if testResult}<p class="result" class:ok={testResult.ok}>{testResult.text}</p>{/if}
					<div class="row wrap">
						<button class="btn primary" disabled={saving}>Save</button>
						<button type="button" class="btn" onclick={() => testServarr(kind)}><Icon name="check" size={15} />Test</button>
						<button type="button" class="btn" onclick={matchFiles}><Icon name="link" size={15} />Match files now</button>
					</div>
				</form>
			{/if}
		{:else if tab === 'jellyfin'}
			{#if jellyfin}
				{@const j = jellyfin}
				<form
					class="card card-pad stack"
					onsubmit={(e) => {
						e.preventDefault();
						save('/settings/jellyfin', j);
					}}
				>
					<div>
						<h2>Jellyfin</h2>
						<p class="muted small" style="margin:4px 0 0">
							Reads playback history from the Playback Reporting plugin so you can see which files get transcoded and why.
						</p>
					</div>
					<Switch bind:checked={j.enabled} label="Enabled" />
					<label class="field">URL<input type="url" bind:value={j.baseUrl} placeholder="http://jellyfin:8096" /></label>
					<label class="field">API key <span class="hint">Dashboard → API Keys in Jellyfin</span><input type="password" bind:value={j.apiKey} autocomplete="off" /></label>
					<label class="field">
						Sync every (minutes) <span class="hint">0 turns off background sync</span>
						<input type="number" min="0" bind:value={j.syncIntervalMinutes} />
					</label>
					<div class="field-like">
						<span class="small muted" style="font-weight:500">Path mappings</span>
						<span class="hint">Only needed when Jellyfin sees your media under a different path than Optimarr.</span>
						<MappingsEditor bind:mappings={j.pathMappings} remoteLabel="Path in Jellyfin" />
					</div>
					{#if testResult}<p class="result" class:ok={testResult.ok}>{testResult.text}</p>{/if}
					<div class="row wrap">
						<button class="btn primary" disabled={saving}>Save</button>
						<button type="button" class="btn" onclick={testJellyfin}><Icon name="check" size={15} />Test</button>
					</div>
				</form>
			{/if}
		{:else if tab === 'devices'}
			<DeviceSettings />
		{:else if tab === 'notifications'}
			<NotificationSettings />
		{:else if tab === 'fixes'}
			{#if fixSettings && upgradeSettings}
				{@const fs = fixSettings}
				{@const us = upgradeSettings}
				<form
					class="card card-pad stack"
					onsubmit={(e) => {
						e.preventDefault();
						save('/settings/fixes', fs);
					}}
				>
					<div>
						<h2>Fix queue</h2>
						<p class="muted small" style="margin:4px 0 0">Runs the suggested ffmpeg command for queued files, one at a time. Your media folders must be mounted writable (remove <code>:ro</code>).</p>
					</div>
					<Switch bind:checked={fs.enabled} label="Run queued fixes" />
					<Switch bind:checked={fs.replaceOriginal} label="Replace the original" hint="Otherwise the fixed file is saved next to it as name.optimized.mp4" />
					{#if fs.replaceOriginal}
						<Switch bind:checked={fs.keepOriginal} label="Keep the original as .orig" hint="Safer, but uses double the space until you delete the .orig files" />
					{/if}
					<div><button class="btn primary" disabled={saving}>Save</button></div>
				</form>
				<form
					class="card card-pad stack"
					onsubmit={(e) => {
						e.preventDefault();
						save('/settings/upgrades', us);
					}}
				>
					<div>
						<h2>Upgrade finder</h2>
						<p class="muted small" style="margin:4px 0 0">Asks Sonarr or Radarr to search for a better release of low-quality files. The current file is kept, and only a release their quality profile ranks higher is grabbed.</p>
					</div>
					<div class="fields">
						<label class="field">Quality score at or below<input type="number" min="0" max="100" bind:value={us.maxQualityScore} /></label>
						<label class="field">Don't search a file again for (days)<input type="number" min="0" bind:value={us.cooldownDays} /></label>
					</div>
					<Switch bind:checked={us.autoEnabled} label="Search automatically" hint="Searches the worst candidates on a schedule" />
					{#if us.autoEnabled}
						<div class="fields">
							<label class="field">Every (hours)<input type="number" min="1" bind:value={us.autoIntervalHours} /></label>
							<label class="field">Files per run<input type="number" min="1" max="100" bind:value={us.autoBatchSize} /></label>
						</div>
					{/if}
					<div><button class="btn primary" disabled={saving}>Save</button></div>
				</form>
			{/if}
		{:else if tab === 'security'}
			{#if security}
				<div class="card card-pad stack">
					<h2>API key</h2>
					<p class="muted small" style="margin:0">
						Send it as <code>X-Api-Key</code> from agents and scripts. The agent API is documented in
						<code>docs/API.md</code>.
					</p>
					<div class="row">
						<input class="grow mono" type={showKey ? 'text' : 'password'} readonly value={security.apiKey} aria-label="API key" />
						<button class="btn icon" onclick={() => (showKey = !showKey)} aria-label="Show key"><Icon name="eye" size={15} /></button>
					</div>
					<CopyBlock text={security.apiKey} label="Copy key" />
					{#if security.keyFromEnvironment}
						<p class="faint small" style="margin:0">Set by <code>OPTIMARR_API_KEY</code>; change it there.</p>
					{:else}
						<div><button class="btn danger" onclick={regenerate}><Icon name="key" size={15} />Generate new key</button></div>
					{/if}
				</div>
				<div class="card card-pad stack">
					<h2>When is the key required?</h2>
					{#each [{ v: 'external', l: 'Outside my network', h: 'Requests from LAN, loopback and Tailscale addresses need no key. Behind a reverse proxy every request looks local, so pick "Always".' }, { v: 'always', l: 'Always', h: 'Every request needs the key, including from this browser.' }, { v: 'never', l: 'Never', h: 'Anyone who can reach Optimarr can change settings and request redownloads.' }] as o (o.v)}
						<label class="radio">
							<input type="radio" name="mode" value={o.v} checked={security.mode === o.v} onchange={() => setMode(o.v)} />
							<span><strong>{o.l}</strong><span class="faint small">{o.h}</span></span>
						</label>
					{/each}
				</div>
				<div class="card card-pad stack">
					<h2>Webhooks</h2>
					<p class="muted small" style="margin:0">Add these as webhook (Connect) targets so Optimarr rescans files as soon as they are imported.</p>
					<label class="field">Sonarr and Radarr<CopyBlock text="{origin}/api/webhooks/servarr?apikey={security.apiKey}" /></label>
					<label class="field">Tdarr<CopyBlock text="{origin}/api/webhooks/tdarr?apikey={security.apiKey}" /></label>
				</div>
			{/if}
		{:else if tab === 'about'}
			{#if status}
				<div class="card card-pad stack">
					<h2>Optimarr {status.version}</h2>
					<dl>
						<dt>Uptime</dt>
						<dd>{longDuration(status.uptimeSeconds)}</dd>
						<dt>MediaInfo</dt>
						<dd>{status.probe.mediainfo ? 'Available' : 'Not found'}</dd>
						<dt>ffprobe</dt>
						<dd>{status.probe.ffprobe ? 'Available' : 'Not found'}</dd>
						<dt>Data folder</dt>
						<dd class="mono">{status.dataDir}</dd>
						<dt>Database</dt>
						<dd>{bytes(status.databaseSize)}</dd>
						<dt>Files</dt>
						<dd>{num(status.counts.media)} in {num(status.counts.libraries)} libraries</dd>
						<dt>Playback sessions</dt>
						<dd>{num(status.counts.playbackEvents)}</dd>
						{#if status.legacyImport}
							<dt>Imported from 1.x</dt>
							<dd class="small">{Object.entries(status.legacyImport).map(([k, v]) => `${k}: ${v}`).join(', ')}</dd>
						{/if}
					</dl>
					{#if !status.probe.mediainfo && !status.probe.ffprobe}
						<p class="result">Neither MediaInfo nor ffprobe was found, so files cannot be analyzed. Install one, or use the Docker image.</p>
					{/if}
					<div class="row wrap">
						<a class="btn" href="https://github.com/davidsteg/optimarr" target="_blank" rel="noreferrer">GitHub</a>
						<a class="btn" href="/api/system/health" target="_blank">Health check</a>
					</div>
				</div>
			{/if}
		{/if}
	</div>
</div>

<style>
	.layout {
		display: grid;
		grid-template-columns: 190px 1fr;
		gap: 24px;
		align-items: start;
	}
	.tabs {
		display: flex;
		flex-direction: column;
		gap: 2px;
		position: sticky;
		top: 20px;
	}
	.tabs button {
		text-align: left;
		padding: 8px 12px;
		border-radius: var(--radius-sm);
		border: none;
		background: none;
		color: var(--text-2);
		font: inherit;
		font-weight: 500;
		cursor: pointer;
	}
	.tabs button:hover {
		background: var(--surface-2);
		color: var(--text);
	}
	.tabs button.active {
		background: var(--accent-soft);
		color: var(--accent-strong);
	}
	.panel {
		max-width: 860px;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 16px;
	}
	.groups {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
		gap: 16px;
	}
	fieldset {
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		padding: 10px 12px 12px;
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	legend {
		font-size: 12px;
		color: var(--text-3);
		text-transform: uppercase;
		letter-spacing: 0.04em;
		padding: 0 4px;
	}
	.check {
		display: flex;
		align-items: center;
		gap: 8px;
		cursor: pointer;
	}
	.fields {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
		gap: 14px;
	}
	.field-like {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.hint {
		font-size: 12px;
		color: var(--text-3);
	}
	.result {
		margin: 0;
		padding: 10px 12px;
		border-radius: var(--radius-sm);
		background: var(--bad-soft);
		color: var(--bad);
	}
	.result.ok {
		background: var(--good-soft);
		color: var(--good);
	}
	.radio {
		display: flex;
		gap: 10px;
		align-items: flex-start;
		cursor: pointer;
	}
	.radio input {
		margin-top: 4px;
		accent-color: var(--accent);
	}
	.radio > span {
		display: flex;
		flex-direction: column;
	}
	details summary {
		cursor: pointer;
	}
	dl {
		display: grid;
		grid-template-columns: max-content 1fr;
		gap: 8px 20px;
		margin: 0;
	}
	dt {
		color: var(--text-3);
	}
	dd {
		margin: 0;
		word-break: break-all;
	}
	@media (max-width: 900px) {
		.layout {
			grid-template-columns: 1fr;
		}
		.tabs {
			position: static;
			flex-direction: row;
			overflow-x: auto;
		}
		.tabs button {
			white-space: nowrap;
		}
	}
</style>

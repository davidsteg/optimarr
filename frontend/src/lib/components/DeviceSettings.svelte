<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '$lib/api';
	import { toasts } from '$lib/toast.svelte';
	import type { Device, DeviceCapabilities } from '$lib/types';
	import Icon from './Icon.svelte';
	import Modal from './Modal.svelte';
	import Switch from './Switch.svelte';

	interface Template {
		name: string;
		profile: DeviceCapabilities;
	}

	let devices = $state<Device[] | null>(null);
	let templates = $state<Template[]>([]);
	let editing = $state<{ id: number | null; name: string; enabled: boolean; profile: DeviceCapabilities } | null>(null);
	let saving = $state(false);

	onMount(() => {
		load();
		api.get<Template[]>('/devices/templates').then((t) => (templates = t)).catch(() => {});
	});

	async function load() {
		try {
			devices = await api.get<Device[]>('/devices');
		} catch (e) {
			toasts.error(e);
		}
	}

	function clone<T>(v: T): T {
		return JSON.parse(JSON.stringify(v));
	}

	function addFrom(t: Template) {
		editing = { id: null, name: t.name, enabled: true, profile: clone(t.profile) };
	}

	function edit(d: Device) {
		editing = { id: d.id, name: d.name, enabled: d.enabled, profile: clone(d.profile) };
	}

	async function save(e: Event) {
		e.preventDefault();
		if (!editing) return;
		saving = true;
		try {
			const body = { name: editing.name, enabled: editing.enabled, profile: editing.profile };
			if (editing.id === null) await api.post('/devices', body);
			else await api.put(`/devices/${editing.id}`, body);
			editing = null;
			await load();
		} catch (err) {
			toasts.error(err);
		} finally {
			saving = false;
		}
	}

	async function remove(d: Device) {
		if (!confirm(`Remove the device profile "${d.name}"?`)) return;
		try {
			await api.del(`/devices/${d.id}`);
			await load();
		} catch (err) {
			toasts.error(err);
		}
	}

	async function toggle(d: Device, enabled: boolean) {
		try {
			await api.put(`/devices/${d.id}`, { name: d.name, enabled, profile: d.profile });
			await load();
		} catch (err) {
			toasts.error(err);
		}
	}

	const groups = [
		{ key: 'videoCodecs', label: 'Video codecs' },
		{ key: 'audioCodecs', label: 'Audio codecs' },
		{ key: 'containers', label: 'Containers' },
		{ key: 'subtitleFormats', label: 'Subtitle formats' },
		{ key: 'bitDepths', label: 'Bit depths' }
	] as const;

	function supported(p: DeviceCapabilities, key: (typeof groups)[number]['key']) {
		return Object.entries(p[key])
			.filter(([, v]) => v)
			.map(([k]) => (key === 'bitDepths' ? `${k}-bit` : k))
			.join(', ');
	}
</script>

<div class="card card-pad stack">
	<div>
		<h2>Devices</h2>
		<p class="muted small" style="margin:4px 0 0">
			Describe the clients you watch on. Each file then shows whether it direct-plays, gets remuxed, or gets transcoded on every device, and
			<a href="/improve">Improve</a> sums it up. Templates are starting points; support varies by model and app.
		</p>
	</div>
	{#if devices && devices.length}
		<ul class="list">
			{#each devices as d (d.id)}
				<li>
					<div class="grow">
						<strong>{d.name}</strong>
						<div class="faint small">{supported(d.profile, 'videoCodecs')} · {supported(d.profile, 'containers')}{d.profile.hdr ? ' · HDR' : ''}</div>
					</div>
					<Switch checked={d.enabled} onchange={(v) => toggle(d, v)} />
					<button class="btn sm" onclick={() => edit(d)}>Edit</button>
					<button class="btn sm ghost icon" onclick={() => remove(d)} aria-label="Remove {d.name}"><Icon name="trash" size={14} /></button>
				</li>
			{/each}
		</ul>
	{:else if devices}
		<p class="faint small" style="margin:0">No devices yet.</p>
	{/if}
	<div class="row wrap">
		<span class="small muted">Add:</span>
		{#each templates as t (t.name)}
			<button class="btn sm" onclick={() => addFrom(t)}><Icon name="plus" size={13} />{t.name}</button>
		{/each}
	</div>
</div>

<Modal open={!!editing} title={editing?.id === null ? 'Add device' : 'Edit device'} width={720} onclose={() => (editing = null)}>
	{#if editing}
		{@const p = editing.profile}
		<form id="device-form" class="stack" onsubmit={save}>
			<div class="row wrap">
				<label class="field grow">Name<input type="text" bind:value={editing.name} required /></label>
				<Switch bind:checked={editing.enabled} label="Enabled" />
			</div>
			<div class="groups">
				{#each groups as g (g.key)}
					<fieldset>
						<legend>{g.label}</legend>
						{#each Object.keys(p[g.key]) as name (name)}
							<label class="check"><input type="checkbox" bind:checked={p[g.key][name]} />{g.key === 'bitDepths' ? `${name}-bit` : name}</label>
						{/each}
					</fieldset>
				{/each}
			</div>
			<Switch bind:checked={p.hdr} label="Displays HDR" hint="Otherwise HDR video is tone-mapped, which needs a transcode" />
			<div class="row wrap">
				<label class="field">Max width (px) <span class="hint">0 = no limit, 1920 for a 1080p screen</span><input type="number" min="0" bind:value={p.maxWidth} /></label>
				<label class="field">Max bitrate (Mbps) <span class="hint">0 = no limit; set for remote or Wi-Fi clients</span><input type="number" min="0" step="any" bind:value={p.maxBitrateMbps} /></label>
			</div>
		</form>
	{/if}
	{#snippet footer()}
		<button class="btn" onclick={() => (editing = null)}>Cancel</button>
		<button class="btn primary" type="submit" form="device-form" disabled={saving}>Save</button>
	{/snippet}
</Modal>

<style>
	.list {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	.list li {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 10px 12px;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
	}
	.groups {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
		gap: 12px;
	}
	fieldset {
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		padding: 8px 10px 10px;
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: 5px;
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
		font-size: 13px;
	}
	.hint {
		font-weight: 400;
		color: var(--text-3);
		font-size: 12px;
	}
</style>

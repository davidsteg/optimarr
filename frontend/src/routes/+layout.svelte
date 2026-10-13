<script lang="ts">
	import '../app.css';
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { api, getApiKey, onUnauthorized, setApiKey } from '$lib/api';
	import { live } from '$lib/live.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import Toasts from '$lib/components/Toasts.svelte';

	let { children } = $props();

	let ready = $state(false);
	let needKey = $state(false);
	let keyInput = $state('');
	let keyError = $state('');
	let version = $state('');
	let navOpen = $state(false);
	let theme = $state<'dark' | 'light'>('dark');

	const nav = [
		{ href: '/', label: 'Dashboard', icon: 'dashboard' },
		{ href: '/media', label: 'Media', icon: 'film' },
		{ href: '/libraries', label: 'Libraries', icon: 'folder' },
		{ href: '/improve', label: 'Improve', icon: 'wrench' },
		{ href: '/playback', label: 'Playback', icon: 'play' },
		{ href: '/activity', label: 'Activity', icon: 'activity' },
		{ href: '/settings', label: 'Settings', icon: 'settings' }
	];

	function isActive(href: string) {
		const p = page.url.pathname;
		return href === '/' ? p === '/' : p.startsWith(href);
	}

	async function boot() {
		try {
			const auth = await api.get<{ keyRequired: boolean }>('/system/auth');
			if (auth.keyRequired && !getApiKey()) {
				needKey = true;
				return;
			}
			const health = await api.get<{ version: string }>('/system/health');
			version = health.version;
			ready = true;
			needKey = false;
			live.connect();
		} catch (e) {
			keyError = e instanceof Error ? e.message : String(e);
			needKey = true;
		}
	}

	async function submitKey(e: Event) {
		e.preventDefault();
		keyError = '';
		setApiKey(keyInput.trim());
		try {
			await api.get('/system/status');
			keyInput = '';
			await boot();
		} catch {
			setApiKey('');
			keyError = 'That API key was not accepted.';
		}
	}

	function toggleTheme() {
		theme = theme === 'dark' ? 'light' : 'dark';
		document.documentElement.dataset.theme = theme;
		try {
			localStorage.setItem('optimarr.theme', theme);
		} catch {
			/* ignore */
		}
	}

	onMount(() => {
		theme = (document.documentElement.dataset.theme as 'dark' | 'light') ?? 'dark';
		const off = onUnauthorized(() => {
			setApiKey('');
			ready = false;
			needKey = true;
			live.disconnect();
		});
		boot();
		return () => {
			off();
			live.disconnect();
		};
	});

	$effect(() => {
		page.url.pathname;
		navOpen = false;
	});
</script>

{#if needKey}
	<div class="gate">
		<form class="card card-pad stack" onsubmit={submitKey}>
			<div class="brand big"><span class="logo"><Icon name="bolt" size={18} /></span>Optimarr</div>
			<p class="muted" style="margin:0">
				This server needs an API key. You can find it in <strong>Settings → Security</strong> on a trusted network, in the
				server log, or set it with <code>OPTIMARR_API_KEY</code>.
			</p>
			<label class="field">
				API key
				<input type="password" bind:value={keyInput} autocomplete="current-password" required />
			</label>
			{#if keyError}<p class="err">{keyError}</p>{/if}
			<button class="btn primary" type="submit">Continue</button>
		</form>
	</div>
{:else}
	<div class="shell">
		<aside class="sidebar" class:open={navOpen}>
			<a class="brand" href="/"><span class="logo"><Icon name="bolt" size={16} /></span>Optimarr</a>
			<nav>
				{#each nav as item (item.href)}
					<a href={item.href} class:active={isActive(item.href)}>
						<Icon name={item.icon} size={17} />{item.label}
						{#if item.href === '/activity' && live.running.length > 0}
							<span class="count">{live.running.length}</span>
						{/if}
					</a>
				{/each}
			</nav>

			{#if live.running.length > 0}
				<div class="jobs">
					{#each live.running.slice(0, 3) as job (job.id)}
						<a class="job" href="/activity">
							<div class="row small"><span class="ellipsis grow">{job.title}</span><span class="faint">{job.total ? Math.floor((job.processed / job.total) * 100) : 0}%</span></div>
							<div class="track"><div class="fill" style="width:{job.total ? (job.processed / job.total) * 100 : 5}%"></div></div>
						</a>
					{/each}
				</div>
			{/if}

			<div class="foot">
				<span class="conn" class:on={live.connected} title={live.connected ? 'Live updates connected' : 'Live updates disconnected'}></span>
				<span class="faint small grow">v{version}</span>
				<button class="btn ghost icon sm" onclick={toggleTheme} aria-label="Toggle theme">
					<Icon name={theme === 'dark' ? 'sun' : 'moon'} size={15} />
				</button>
			</div>
		</aside>
		{#if navOpen}<div class="scrim" role="presentation" onclick={() => (navOpen = false)}></div>{/if}

		<main>
			<div class="topbar">
				<button class="btn ghost icon" onclick={() => (navOpen = true)} aria-label="Open menu"><Icon name="menu" /></button>
				<span class="brand"><span class="logo"><Icon name="bolt" size={14} /></span>Optimarr</span>
			</div>
			<div class="content">
				{#if ready}
					{@render children()}
				{:else}
					<p class="faint">Connecting…</p>
				{/if}
			</div>
		</main>
	</div>
{/if}

<Toasts />

<style>
	.shell {
		display: grid;
		grid-template-columns: 232px 1fr;
		min-height: 100vh;
	}
	.sidebar {
		position: sticky;
		top: 0;
		height: 100vh;
		display: flex;
		flex-direction: column;
		gap: 18px;
		padding: 18px 12px;
		background: var(--bg-raised);
		border-right: 1px solid var(--border);
	}
	.brand {
		display: flex;
		align-items: center;
		gap: 10px;
		font-weight: 700;
		font-size: 16px;
		letter-spacing: -0.02em;
		color: var(--text);
		padding: 0 8px;
	}
	.brand:hover {
		text-decoration: none;
	}
	.brand.big {
		font-size: 20px;
		padding: 0;
	}
	.logo {
		display: grid;
		place-items: center;
		width: 28px;
		height: 28px;
		border-radius: 8px;
		background: linear-gradient(135deg, var(--accent), #0ea5e9);
		color: #04140f;
	}
	nav {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}
	nav a {
		display: flex;
		align-items: center;
		gap: 11px;
		padding: 8px 10px;
		border-radius: var(--radius-sm);
		color: var(--text-2);
		font-weight: 500;
	}
	nav a:hover {
		background: var(--surface-2);
		color: var(--text);
		text-decoration: none;
	}
	nav a.active {
		background: var(--accent-soft);
		color: var(--accent-strong);
	}
	.count {
		margin-left: auto;
		font-size: 11px;
		padding: 0 7px;
		border-radius: 999px;
		background: var(--accent);
		color: var(--accent-ink);
		font-weight: 600;
	}
	.jobs {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding: 0 4px;
	}
	.job {
		display: block;
		padding: 10px;
		border-radius: var(--radius-sm);
		background: var(--surface);
		border: 1px solid var(--border);
		color: var(--text);
	}
	.job:hover {
		text-decoration: none;
		border-color: var(--border-strong);
	}
	.track {
		height: 4px;
		border-radius: 999px;
		background: var(--surface-3);
		margin-top: 6px;
		overflow: hidden;
	}
	.fill {
		height: 100%;
		background: var(--accent);
		transition: width 0.4s;
	}
	.foot {
		margin-top: auto;
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 0 8px;
	}
	.conn {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--text-3);
	}
	.conn.on {
		background: var(--good);
		box-shadow: 0 0 0 3px var(--good-soft);
	}
	main {
		min-width: 0;
	}
	.content {
		padding: 28px 32px 48px;
		max-width: 1440px;
		margin: 0 auto;
	}
	.topbar {
		display: none;
	}
	.gate {
		min-height: 100vh;
		display: grid;
		place-items: center;
		padding: 16px;
	}
	.gate form {
		width: min(420px, 100%);
	}
	.err {
		color: var(--bad);
		margin: 0;
	}
	.scrim {
		display: none;
	}
	@media (max-width: 900px) {
		.shell {
			grid-template-columns: 1fr;
		}
		.sidebar {
			position: fixed;
			left: 0;
			top: 0;
			bottom: 0;
			width: 260px;
			z-index: 60;
			transform: translateX(-100%);
			transition: transform 0.2s;
		}
		.sidebar.open {
			transform: none;
		}
		.scrim {
			display: block;
			position: fixed;
			inset: 0;
			z-index: 59;
			background: rgba(0, 0, 0, 0.5);
		}
		.topbar {
			display: flex;
			align-items: center;
			gap: 8px;
			padding: 10px 12px;
			border-bottom: 1px solid var(--border);
			background: var(--bg-raised);
			position: sticky;
			top: 0;
			z-index: 30;
		}
		.content {
			padding: 20px 16px 40px;
		}
	}
</style>

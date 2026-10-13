<script lang="ts">
	const paths: Record<string, string> = {
		dashboard: 'M3 13h8V3H3zm0 8h8v-6H3zm10 0h8V11h-8zm0-18v6h8V3z',
		film: 'M4 4h16v16H4zM8 4v16M16 4v16M4 8h4M4 12h4M4 16h4M16 8h4M16 12h4M16 16h4',
		folder: 'M3 6a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z',
		activity: 'M3 12h4l3 8 4-16 3 8h4',
		play: 'M6 4l14 8-14 8z',
		settings:
			'M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zm7.4-3a7.4 7.4 0 0 0-.1-1.2l2-1.6-2-3.4-2.4 1a7 7 0 0 0-2-1.2L14.5 3h-5l-.4 2.6a7 7 0 0 0-2 1.2l-2.4-1-2 3.4 2 1.6a7.4 7.4 0 0 0 0 2.4l-2 1.6 2 3.4 2.4-1a7 7 0 0 0 2 1.2l.4 2.6h5l.4-2.6a7 7 0 0 0 2-1.2l2.4 1 2-3.4-2-1.6c.1-.4.1-.8.1-1.2z',
		search: 'M11 19a8 8 0 1 0 0-16 8 8 0 0 0 0 16zm10 2-4.3-4.3',
		refresh: 'M21 12a9 9 0 1 1-2.6-6.4M21 4v5h-5',
		download: 'M12 3v12m0 0-5-5m5 5 5-5M4 21h16',
		trash: 'M4 7h16M9 7V4h6v3m-8 0 1 13h8l1-13',
		plus: 'M12 5v14M5 12h14',
		x: 'M6 6l12 12M18 6 6 18',
		check: 'M5 12.5 10 17l9-10',
		chevronRight: 'M9 6l6 6-6 6',
		chevronLeft: 'M15 6l-6 6 6 6',
		chevronDown: 'M6 9l6 6 6-6',
		up: 'M12 19V5m-6 6 6-6 6 6',
		copy: 'M9 9h11v11H9zM5 15H4V4h11v1',
		alert: 'M12 3 2 20h20zM12 10v4m0 3v.01',
		info: 'M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20zm0-11v6m0-9v.01',
		bolt: 'M13 2 4 14h7l-1 8 9-12h-7z',
		stop: 'M6 6h12v12H6z',
		eye: 'M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12zm10 3a3 3 0 1 0 0-6 3 3 0 0 0 0 6z',
		sun: 'M12 17a5 5 0 1 0 0-10 5 5 0 0 0 0 10zM12 1v2m0 18v2M4.2 4.2l1.4 1.4m12.8 12.8 1.4 1.4M1 12h2m18 0h2M4.2 19.8l1.4-1.4M18.4 5.6l1.4-1.4',
		moon: 'M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z',
		key: 'M15 7a4 4 0 1 1-3.9 5H3v3h3v3h3v-3h2.1A4 4 0 0 1 15 7zm1 4a1 1 0 1 0 0-2',
		link: 'M10 14a5 5 0 0 0 7 0l3-3a5 5 0 0 0-7-7l-1 1m2 5a5 5 0 0 0-7 0l-3 3a5 5 0 0 0 7 7l1-1',
		wrench: 'M14.7 6.3a4 4 0 0 0 5 5L22 14l-8 8-2.3-2.3a4 4 0 0 0-5-5L2 10l8-8z',
		menu: 'M4 6h16M4 12h16M4 18h16',
		server: 'M4 4h16v6H4zM4 14h16v6H4zM8 7h.01M8 17h.01'
	};
	let { name, size = 16, stroke = 1.8 }: { name: string; size?: number; stroke?: number } = $props();
</script>

<svg
	width={size}
	height={size}
	viewBox="0 0 24 24"
	fill="none"
	stroke="currentColor"
	stroke-width={stroke}
	stroke-linecap="round"
	stroke-linejoin="round"
	aria-hidden="true"
	style="flex-shrink:0"
>
	<path d={paths[name] ?? ''} />
</svg>

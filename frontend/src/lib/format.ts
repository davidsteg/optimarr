export function bytes(n: number | null | undefined, digits = 1): string {
	if (!n || n <= 0) return '0 B';
	const units = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
	const i = Math.min(Math.floor(Math.log(n) / Math.log(1024)), units.length - 1);
	return `${(n / 1024 ** i).toFixed(i === 0 ? 0 : digits)} ${units[i]}`;
}

export function bitrate(bps: number | null | undefined): string {
	if (!bps || bps <= 0) return '–';
	if (bps >= 1_000_000) return `${(bps / 1_000_000).toFixed(1)} Mbps`;
	return `${Math.round(bps / 1000)} kbps`;
}

export function duration(seconds: number | null | undefined): string {
	if (!seconds || seconds <= 0) return '–';
	const h = Math.floor(seconds / 3600);
	const m = Math.floor((seconds % 3600) / 60);
	const s = Math.floor(seconds % 60);
	if (h > 0) return `${h}h ${m}m`;
	if (m > 0) return `${m}m ${s}s`;
	return `${s}s`;
}

export function longDuration(seconds: number): string {
	const days = Math.floor(seconds / 86400);
	const hours = Math.floor((seconds % 86400) / 3600);
	return days > 0 ? `${days}d ${hours}h` : `${hours}h`;
}

export function num(n: number | null | undefined): string {
	return (n ?? 0).toLocaleString();
}

export function pct(part: number, total: number): string {
	if (!total) return '0%';
	return `${Math.round((part / total) * 100)}%`;
}

export function ago(iso: string | null | undefined): string {
	if (!iso) return 'never';
	const diff = (Date.now() - new Date(iso).getTime()) / 1000;
	if (diff < 0) return 'just now';
	if (diff < 60) return 'just now';
	if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
	if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
	if (diff < 86400 * 30) return `${Math.floor(diff / 86400)}d ago`;
	return new Date(iso).toLocaleDateString();
}

export function dateTime(iso: string | null | undefined): string {
	if (!iso) return '–';
	return new Date(iso).toLocaleString();
}

const ISSUE_LABELS: Record<string, string> = {
	video_codec: 'Video codec',
	bit_depth: 'Bit depth',
	codec_tag: 'Codec tag',
	no_video: 'No video',
	audio_codec: 'Audio codec',
	container: 'Container',
	subtitle_format: 'Subtitles',
	fast_start: 'Fast start',
	high_bitrate: 'High bitrate',
	resolution: 'Resolution',
	very_low_bitrate: 'Very low bitrate',
	low_bitrate: 'Low bitrate',
	legacy_codec: 'Legacy codec',
	sdr_4k: '4K without HDR',
	stereo_only: 'Stereo only',
	low_audio_bitrate: 'Low audio bitrate',
	no_audio: 'No audio'
};

export function issueLabel(code: string): string {
	return ISSUE_LABELS[code] ?? code.replace(/_/g, ' ');
}

export function titleCase(s: string): string {
	return s ? s[0].toUpperCase() + s.slice(1) : s;
}

export function verdictTone(v: string): 'good' | 'ok' | 'warn' | 'bad' | 'muted' {
	switch (v) {
		case 'optimal':
		case 'excellent':
			return 'good';
		case 'good':
			return 'ok';
		case 'fair':
			return 'warn';
		case 'poor':
			return 'bad';
		default:
			return 'muted';
	}
}

export function scoreTone(score: number): 'good' | 'ok' | 'warn' | 'bad' {
	if (score >= 85) return 'good';
	if (score >= 65) return 'ok';
	if (score >= 45) return 'warn';
	return 'bad';
}

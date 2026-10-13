export type Severity = 'info' | 'warning' | 'critical';
export type Area = 'compatibility' | 'quality';

export interface Issue {
	code: string;
	area: Area;
	severity: Severity;
	message: string;
	penalty?: number;
	fix?: string;
}

export interface MediaSummary {
	id: number;
	libraryId: number | null;
	title: string;
	fileName: string;
	path: string;
	size: number;
	duration: number;
	container: string;
	videoCodec: string;
	resolution: string;
	width: number;
	height: number;
	bitDepth: number;
	hdrType: string | null;
	overallBitrate: number;
	audioCodecs: string[];
	subtitleFormats: string[];
	compatScore: number;
	compatVerdict: string;
	qualityScore: number;
	qualityVerdict: string;
	status: string;
	statusReason: string | null;
	issues: Issue[];
	servarrKind: string | null;
	analyzedAt: string;
}

export interface AudioTrack {
	codec: string;
	channels: number;
	sampleRate: number;
	bitrate: number;
	language: string;
	title: string;
	default: boolean;
}

export interface SubtitleTrack {
	format: string;
	language: string;
	embedded: boolean;
	forced: boolean;
	path: string | null;
}

export interface MediaDetail {
	id: number;
	libraryId: number | null;
	path: string;
	fileName: string;
	size: number;
	mtime: number;
	duration: number;
	container: string;
	videoCodec: string;
	videoCodecTag: string;
	codecTagOk: boolean;
	videoProfile: string;
	bitDepth: number;
	width: number;
	height: number;
	frameRate: number;
	videoBitrate: number;
	overallBitrate: number;
	hdr: boolean;
	hdrType: string;
	fastStart: boolean;
	audioTracks: AudioTrack[];
	subtitleTracks: SubtitleTrack[];
	compatScore: number;
	compatVerdict: string;
	qualityScore: number;
	qualityVerdict: string;
	status: string;
	statusReason: string | null;
	processingSince: string | null;
	analyzedAt: string;
	servarrKind: string | null;
	sonarrSeriesTitle: string | null;
	seasonNumber: number | null;
	episodeNumber: number | null;
	radarrMovieTitle: string | null;
	year: number | null;
	matchedAt: string | null;
	title: string;
	resolution: string;
	assessment: {
		compatScore: number;
		compatVerdict: string;
		qualityScore: number;
		qualityVerdict: string;
		issues: Issue[];
		ffmpegCommand: string | null;
		summary: string;
	};
	playback: { plays: number; transcodes: number; lastPlayed: string | null };
	servarrRemotePath: string | null;
}

export interface Page<T> {
	items: T[];
	total: number;
	page: number;
	pageSize: number;
}

export interface Library {
	id: number;
	name: string;
	path: string;
	category: 'movies' | 'series' | 'other';
	enabled: boolean;
	watch: boolean;
	servarrKind: string | null;
	servarrRootFolderPath: string | null;
	createdAt: string;
	lastScannedAt: string | null;
	lastSyncedAt: string | null;
	fileCount: number;
	totalSize: number;
	optimal: number;
	good: number;
	poor: number;
	broken: number;
	avgCompat: number | null;
	avgQuality: number | null;
}

export interface Job {
	id: number;
	kind: string;
	title: string;
	libraryId: number | null;
	status: 'running' | 'completed' | 'failed' | 'cancelled';
	total: number;
	processed: number;
	failed: number;
	skipped: number;
	currentItem: string | null;
	message: string | null;
	startedAt: string;
	finishedAt: string | null;
}

export interface Bucket {
	key: string;
	count: number;
	size: number;
}

export interface Dashboard {
	totals: {
		files: number;
		size: number;
		duration: number;
		avgCompat: number | null;
		avgQuality: number | null;
		broken: number;
		processing: number;
		missing: number;
		remuxFixable: number;
	};
	compatibility: Bucket[];
	quality: Bucket[];
	videoCodecs: Bucket[];
	containers: Bucket[];
	resolutions: Bucket[];
	hdr: Bucket[];
	audioCodecs: Bucket[];
	issues: { code: string; area: Area; severity: Severity; count: number }[];
	worst: MediaSummary[];
	playback30d: { plays: number; transcodes: number };
	recentJobs: Job[];
}

export interface PathMapping {
	remote: string;
	local: string;
}

export interface BarItem {
	label: string;
	value: number;
	tone?: string;
	href?: string;
	hint?: string;
}

export interface Segment {
	key: string;
	count: number;
	tone: string;
	href?: string;
}

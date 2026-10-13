import { getApiKey } from './api';
import type { Job } from './types';

type Change = 'media' | 'libraries';

class Live {
	jobs = $state<Record<number, Job>>({});
	connected = $state(false);
	/** Bumped when media rows change; pages re-fetch when it moves. */
	mediaVersion = $state(0);
	librariesVersion = $state(0);

	private source: EventSource | null = null;
	private retry: ReturnType<typeof setTimeout> | null = null;
	private finishedListeners = new Set<(job: Job) => void>();

	get running(): Job[] {
		return Object.values(this.jobs)
			.filter((j) => j.status === 'running')
			.sort((a, b) => b.id - a.id);
	}

	onJobFinished(fn: (job: Job) => void) {
		this.finishedListeners.add(fn);
		return () => this.finishedListeners.delete(fn);
	}

	connect() {
		this.disconnect();
		const key = getApiKey();
		const url = key ? `/api/events?apikey=${encodeURIComponent(key)}` : '/api/events';
		const es = new EventSource(url);
		this.source = es;
		es.onopen = () => (this.connected = true);
		es.onerror = () => {
			this.connected = false;
			es.close();
			if (this.source === es) {
				this.source = null;
				this.retry = setTimeout(() => this.connect(), 3000);
			}
		};
		es.onmessage = (msg) => {
			let ev: any;
			try {
				ev = JSON.parse(msg.data);
			} catch {
				return;
			}
			if (ev.type === 'job') this.handleJob(ev.job as Job);
			else if (ev.type === 'mediaChanged') this.bump('media');
			else if (ev.type === 'librariesChanged') this.bump('libraries');
		};
	}

	disconnect() {
		if (this.retry) clearTimeout(this.retry);
		this.retry = null;
		this.source?.close();
		this.source = null;
		this.connected = false;
	}

	private handleJob(job: Job) {
		const prev = this.jobs[job.id];
		this.jobs[job.id] = job;
		if (job.status !== 'running' && prev?.status === 'running') {
			this.bump('media');
			this.bump('libraries');
			this.finishedListeners.forEach((fn) => fn(job));
			// Drop finished jobs from the live list after a while.
			setTimeout(() => {
				if (this.jobs[job.id]?.status !== 'running') delete this.jobs[job.id];
			}, 15000);
		}
	}

	private bump(what: Change) {
		if (what === 'media') this.mediaVersion++;
		else this.librariesVersion++;
	}
}

export const live = new Live();

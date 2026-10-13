export type ToastKind = 'info' | 'success' | 'error';
export interface Toast {
	id: number;
	kind: ToastKind;
	message: string;
}

class Toasts {
	items = $state<Toast[]>([]);
	private next = 1;

	push(message: string, kind: ToastKind = 'info', ms = 4500) {
		const id = this.next++;
		this.items.push({ id, kind, message });
		setTimeout(() => this.dismiss(id), ms);
	}
	success(message: string) {
		this.push(message, 'success');
	}
	error(e: unknown) {
		const message = e instanceof Error ? e.message : String(e);
		this.push(message, 'error', 7000);
	}
	dismiss(id: number) {
		this.items = this.items.filter((t) => t.id !== id);
	}
}

export const toasts = new Toasts();

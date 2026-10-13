import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

const target = process.env.OPTIMARR_URL ?? 'http://127.0.0.1:8080';

export default defineConfig({
	plugins: [sveltekit()],
	server: { proxy: { '/api': { target, changeOrigin: true } } }
});

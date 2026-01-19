import { defineConfig } from '@playwright/test';

export default defineConfig({
	globalSetup: './playwright.ts',
	testDir: '.',
	testMatch: 'e2e.ts',
	fullyParallel: true,
	forbidOnly: !!process.env.CI,
	retries: process.env.CI ? 2 : 0,
	workers: process.env.CI ? 1 : undefined,
	reporter: 'list',
	use: {
		baseURL: 'http://localhost:3000',
		trace: 'on-first-retry'
	},
	webServer: {
		// Use our serve.ts which sets COOP/COEP headers for SharedArrayBuffer
		command: 'deno run -A serve.ts --dir ../target/e2e/output --port 3000',
		port: 3000,
		reuseExistingServer: !process.env.CI
	},
	projects: [
		{
			name: 'chromium',
			use: { browserName: 'chromium' }
		}
	]
});

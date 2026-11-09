import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './tests/browser',
  fullyParallel: true,
  use: { baseURL: 'http://127.0.0.1:5188', viewport: { width: 1280, height: 720 }, launchOptions: { executablePath: process.env.BROWSER_EXECUTABLE || (process.env.CI ? undefined : '/usr/bin/chromium'), args: ['--no-sandbox'] }, screenshot: 'only-on-failure' },
  webServer: { command: 'node server.mjs', url: 'http://127.0.0.1:5188', reuseExistingServer: !process.env.CI },
  reporter: 'list',
});

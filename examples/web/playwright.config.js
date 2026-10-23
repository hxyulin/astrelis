import { defineConfig } from '@playwright/test';

export default defineConfig({
    testDir: './tests',
    timeout: 30_000,
    workers: 1,
    use: {
        baseURL: 'http://127.0.0.1:8080',
        viewport: { width: 960, height: 700 },
        screenshot: 'only-on-failure',
        // CI uses Chromium's software WebGPU adapter; local hardware uses the same API.
        launchOptions: { args: ['--enable-unsafe-webgpu', '--use-angle=swiftshader', '--enable-features=Vulkan'] },
    },
    projects: [
        { name: 'chromium', use: { browserName: 'chromium', deviceScaleFactor: 1 } },
        { name: 'chromium-hidpi', use: { browserName: 'chromium', deviceScaleFactor: 2 } },
    ],
    webServer: {
        command: 'python3 -m http.server 8080 --bind 127.0.0.1',
        url: 'http://127.0.0.1:8080',
        reuseExistingServer: !process.env.CI,
    },
});

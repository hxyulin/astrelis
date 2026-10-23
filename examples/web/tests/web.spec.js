import { test, expect } from '@playwright/test';
import { PNG } from 'pngjs';

function pixel(png, x, y) {
    return [...png.data.subarray((y * png.width + x) * 4, (y * png.width + x) * 4 + 3)];
}

function closeColor(actual, expected) {
    actual.forEach((value, channel) => expect(Math.abs(value - expected[channel]), `${actual} vs ${expected}`).toBeLessThan(12));
}

test('renders text, shapes, paths and images; handles input, resize and MSAA', async ({ page }) => {
    const errors = [];
    page.on('pageerror', error => errors.push(String(error)));
    page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
    await page.goto('/');
    const status = page.locator('#status');
    await expect(status).toHaveAttribute('data-state', 'ready');
    const canvas = page.locator('canvas');
    let png = PNG.sync.read(await canvas.screenshot({ scale: 'css' }));
    // Linear shader output must reach the display encoded as sRGB.
    closeColor(pixel(png, 80, 180), [63, 218, 179]);
    closeColor(pixel(png, 5, 5), [33, 44, 60]);
    // Text is GPU-rendered from the embedded font, rather than an HTML overlay.
    let bright = 0;
    for (let y = 40; y < 105; y++) {
        for (let x = 40; x < 430; x++) {
            if (pixel(png, x, y).every(channel => channel > 180)) bright++;
        }
    }
    expect(bright).toBeGreaterThan(500);
    const orange = pixel(png, 175, 155);
    expect(orange[0]).toBeGreaterThan(orange[2]);
    const blue = pixel(png, 225, 155);
    expect(blue[2]).toBeGreaterThan(blue[0]);
    let curve = 0;
    for (let y = 130; y < 240; y++) {
        for (let x = 265; x < 420; x++) {
            const [r, g, b] = pixel(png, x, y);
            if (r > 220 && g > 130 && b < 140) curve++;
        }
    }
    expect(curve).toBeGreaterThan(200);

    await canvas.hover({ position: { x: 330, y: 280 } });
    await expect.poll(async () => {
        png = PNG.sync.read(await canvas.screenshot({ scale: 'css' }));
        return pixel(png, 330, 280)[0];
    }).toBeGreaterThan(230);

    await page.setViewportSize({ width: 740, height: 700 });
    await expect.poll(() => canvas.evaluate(element =>
        element.width === Math.round(element.getBoundingClientRect().width * devicePixelRatio),
    )).toBe(true);
    await page.getByRole('button', { name: 'Enable 4× MSAA' }).click();
    await expect(status).toContainText('4× samples');
    png = PNG.sync.read(await canvas.screenshot({ scale: 'css' }));
    closeColor(pixel(png, 80, 180), [63, 218, 179]);
    // Resize must recreate the MSAA texture with the same storage/view pairing.
    await page.setViewportSize({ width: 800, height: 700 });
    await expect.poll(() => canvas.evaluate(element =>
        element.width === Math.round(element.getBoundingClientRect().width * devicePixelRatio),
    )).toBe(true);
    png = PNG.sync.read(await canvas.screenshot({ scale: 'css' }));
    closeColor(pixel(png, 80, 180), [63, 218, 179]);
    await page.getByRole('button', { name: 'Disable 4× MSAA' }).click();
    await expect(status).toContainText('1× samples');
    png = PNG.sync.read(await canvas.screenshot({ scale: 'css' }));
    closeColor(pixel(png, 80, 180), [63, 218, 179]);
    // Let asynchronous WebGPU validation callbacks reach the diagnostic drain.
    await page.waitForTimeout(1200);
    await expect(status).toHaveAttribute('data-state', 'ready');
    expect(errors).toEqual([]);
});

test('zero-sized canvas suspends and resumes', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status')).toHaveAttribute('data-state', 'ready');
    const result = await page.evaluate(async () => {
        const { WebApp } = await import('/pkg/astrelis_web_example.js');
        const canvas = document.createElement('canvas');
        canvas.width = canvas.height = 0;
        const app = await WebApp.create(canvas);
        app.set_msaa(true);
        const suspended = app.draw(1, 50, 50);
        canvas.width = canvas.height = 512;
        app.resize(512, 512);
        const resumed = app.draw(1, 50, 50);
        app.free();
        return { suspended, resumed };
    });
    expect(result).toEqual({ suspended: false, resumed: true });
});

test('reports unavailable WebGPU visibly', async ({ page }) => {
    await page.addInitScript(() => Object.defineProperty(navigator, 'gpu', { value: undefined }));
    await page.goto('/');
    await expect(page.locator('#status')).toHaveAttribute('data-state', 'error');
    await expect(page.locator('#status')).toContainText('WebGPU is unavailable');
    await expect(page.locator('#msaa')).toBeDisabled();
});

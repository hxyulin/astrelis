const canvas = document.querySelector('#canvas');
const status = document.querySelector('#status');
const msaa = document.querySelector('#msaa');
let app;
let stopped = false;
let request = 0;
let pointer = [390, 280];
let samples = false;

function fail(error) {
    stopped = true;
    cancelAnimationFrame(request);
    status.dataset.state = 'error';
    status.textContent = `WebGPU error: ${error}. Check browser WebGPU support and use HTTPS or localhost.`;
    msaa.disabled = true;
    console.error(error);
}

function draw() {
    request = 0;
    if (stopped || document.hidden) return;
    try {
        const scale = window.devicePixelRatio || 1;
        const bounds = canvas.getBoundingClientRect();
        const width = Math.round(bounds.width * scale);
        const height = Math.round(bounds.height * scale);
        // Layout owns CSS dimensions; the GPU owns physical backing dimensions.
        if (canvas.width !== width || canvas.height !== height) {
            canvas.width = width;
            canvas.height = height;
            app.resize(width, height);
        }
        if (app.draw(scale, ...pointer)) {
            status.dataset.state = 'ready';
            status.textContent = `WebGPU · ${width} × ${height} physical pixels · ${samples ? 4 : 1}× samples`;
        } else if (width && height) {
            invalidate(); // Retry on a later browser frame, never in a blocking loop.
        }
    } catch (error) {
        fail(error);
    }
}

function invalidate() {
    if (!stopped && app && !request) request = requestAnimationFrame(draw);
}

try {
    if (!isSecureContext || !navigator.gpu) throw new Error('WebGPU is unavailable');
    const { default: init, WebApp } = await import('./pkg/astrelis_web_example.js');
    await init();
    app = await WebApp.create(canvas);
    msaa.disabled = false;
    canvas.addEventListener('pointermove', event => {
        const bounds = canvas.getBoundingClientRect();
        pointer = [event.clientX - bounds.left, event.clientY - bounds.top];
        invalidate();
    });
    msaa.addEventListener('click', () => {
        try {
            samples = !samples;
            app.set_msaa(samples);
            msaa.setAttribute('aria-pressed', String(samples));
            msaa.textContent = samples ? 'Disable 4× MSAA' : 'Enable 4× MSAA';
            invalidate();
        } catch (error) {
            fail(error);
        }
    });
    new ResizeObserver(invalidate).observe(canvas);
    // DPR can change without changing the canvas's CSS dimensions.
    let density;
    function watchDensity() {
        density?.removeEventListener('change', watchDensity);
        density = matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
        density.addEventListener('change', watchDensity);
        invalidate();
    }
    watchDensity();
    document.addEventListener('visibilitychange', invalidate);
    // WebGPU errors are asynchronous; drain diagnostics even when drawing is idle.
    setInterval(() => {
        if (stopped) return;
        try { app.check_errors(); } catch (error) { fail(error); }
    }, 1000);
    invalidate();
} catch (error) {
    fail(error);
}

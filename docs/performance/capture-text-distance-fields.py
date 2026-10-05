#!/usr/bin/env python3
"""Capture the actual text_distance_fields scene through a temporary headless Rust binary.

Usage: python3 docs/performance/capture-text-distance-fields.py OUTPUT_DIRECTORY
Requires cargo, the workspace toolchain, a GPU, and 4x MSAA. No Python packages.
The window example remains a standalone file without capture or smoke-test flags.
"""
from pathlib import Path
import binascii
from datetime import datetime, timezone
import hashlib
import json
import struct
import subprocess
import sys
import tempfile
import zlib

ROOT = Path(__file__).resolve().parents[2]
EXAMPLE = ROOT / "crates/astrelis/examples/text_distance_fields.rs"
BINARY = ROOT / "crates/astrelis/examples/_text_distance_fields_capture.rs"
HEADER = """//! One-off headless capture generated from the text_distance_fields scene.
use astrelis::{GraphicsContext, Painter, PreparedText, Rect, RenderPass,
TextBuffer, TextDraw, MtsdfOptions, TextPreparation, TextRasterOptions, TextStyle, TextSystem, Transform2D, wgpu};
use std::{error::Error, time::Duration};
"""
MAIN = r'''
fn main() -> Result<(), Box<dyn Error>> {
    let directory = std::env::args().nth(1).ok_or("capture directory required")?;
    let graphics = pollster::block_on(GraphicsContext::headless())?;
    eprintln!("adapter={:?}", graphics.adapter().get_info());
    let errors = graphics.device().push_error_scope(wgpu::ErrorFilter::Validation);
    let mut fonts = TextSystem::new();
    fonts.load_font(include_bytes!("../tests/fonts/SourceSans3-Regular.otf"))?;
    fonts.load_font(include_bytes!("../tests/fonts/NotoSansArabic.ttf"))?;
    fonts.load_font(include_bytes!("../tests/fonts/TestColor.ttf"))?;
    for dpi in [1., 2.] {
        for samples in [1, 4] {
            let mut painter = Painter::new(&graphics);
            let scene = prepare_scene(&mut painter, &mut fonts, dpi, 4.)?;
            let size = [(1120. * dpi) as u32, (960. * dpi) as u32];
            let mut target = graphics.create_framebuffer(astrelis::FramebufferOptions::new(size[0], size[1])
                .format(wgpu::TextureFormat::Rgba8UnormSrgb).sample_count(samples)
                .usage(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC))?;
            painter.prepare(&target.render_format())?;
            let texture = target.color_texture()?.clone();
            let pitch = (size[0] * 4).div_ceil(256) * 256;
            let read = graphics.device().create_buffer(&wgpu::BufferDescriptor { label: None,
                size: u64::from(pitch) * u64::from(size[1]), usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
            let mut frame = target.begin_frame()?;
            {
                let mut pass = frame.render_pass().clear_color(wgpu::Color { r:0.015, g:0.015, b:0.015, a:1. }).begin()?;
                draw_scene(&mut painter, &scene, &mut pass, size, dpi)?;
            }
            frame.encoder().copy_texture_to_buffer(wgpu::TexelCopyTextureInfo {texture:&texture,mip_level:0,origin:Default::default(),aspect:wgpu::TextureAspect::All},
                wgpu::TexelCopyBufferInfo {buffer:&read,layout:wgpu::TexelCopyBufferLayout {offset:0,bytes_per_row:Some(pitch),rows_per_image:Some(size[1])}},texture.size());
            let index = frame.finish()?;
            let (tx,rx) = std::sync::mpsc::channel();
            read.slice(..).map_async(wgpu::MapMode::Read,move|r|{let _=tx.send(r);});
            graphics.device().poll(wgpu::PollType::Wait {submission_index:Some(index),timeout:Some(Duration::from_secs(10))})?;
            rx.recv_timeout(Duration::from_secs(10))??;
            let mapped = read.slice(..).get_mapped_range()?;
            let mut ppm = format!("P6\n{} {}\n255\n",size[0],size[1]).into_bytes();
            for row in mapped.chunks_exact(pitch as usize) {
                for pixel in row[..size[0] as usize*4].chunks_exact(4) { ppm.extend_from_slice(&pixel[..3]); }
            }
            let path = format!("{directory}/astrelis-text-distance-fields-dpi-{}-{}x.ppm",dpi as u32,samples);
            std::fs::write(&path,ppm)?;
            eprintln!("{path}; stats={:?}",painter.text().stats());
        }
    }
    if let Some(error) = pollster::block_on(errors.pop()) { return Err(error.into()); }
    Ok(())
}
'''


def chunk(kind, data):
    return (struct.pack(">I", len(data)) + kind + data
            + struct.pack(">I", binascii.crc32(kind + data) & 0xffffffff))


def png(ppm):
    magic, dimensions, maximum, pixels = ppm.split(b"\n", 3)
    assert magic == b"P6" and maximum == b"255"
    width, height = map(int, dimensions.split())
    assert len(pixels) == width * height * 3
    rows = b"".join(b"\x00" + pixels[y * width * 3:(y + 1) * width * 3]
                    for y in range(height))
    return (b"\x89PNG\r\n\x1a\n"
            + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(rows, 9)) + chunk(b"IEND", b""))


def main():
    if len(sys.argv) != 2:
        raise SystemExit("usage: capture-text-distance-fields.py OUTPUT_DIRECTORY")
    output = Path(sys.argv[1]).resolve()
    output.mkdir(parents=True, exist_ok=True)
    source = EXAMPLE.read_text()
    scene = source[source.index("struct Sample {"):source.index("struct State {")]
    # Exclusive creation prevents overwriting an existing application/example file.
    with BINARY.open("x") as file:
        file.write(HEADER + scene + MAIN)
    try:
        with tempfile.TemporaryDirectory(prefix="astrelis-text-distance-fields-") as directory:
            result = subprocess.run(["cargo", "run", "-p", "astrelis", "--example",
                                     "_text_distance_fields_capture", "--", directory],
                                    cwd=ROOT, check=True, stdout=subprocess.PIPE,
                                    stderr=subprocess.STDOUT, text=True)
            images = {}
            for dpi in (1, 2):
                for msaa in (1, 4):
                    name = f"text-distance-fields-dpi-{dpi}-{msaa}x"
                    data = png((Path(directory) / f"astrelis-{name}.ppm").read_bytes())
                    (output / f"{name}.png").write_bytes(data)
                    images[f"{name}.png"] = hashlib.sha256(data).hexdigest()
            metadata = {
                "captured_at_utc": datetime.now(timezone.utc).isoformat(),
                "base_commit": subprocess.check_output(["git", "rev-parse", "HEAD"],
                                                        cwd=ROOT, text=True).strip(),
                "working_tree": "text quality scene; see source hashes",
                "command": "python3 docs/performance/capture-text-distance-fields.py OUTPUT_DIRECTORY",
                "format": "RGBA8UnormSrgb offscreen, opaque background; RGB PNG",
                "dpi": [1, 2], "msaa": [1, 4], "zoom": 4,
                "source_sha256": {
                    str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest()
                    for path in ([EXAMPLE, Path(__file__).resolve(), ROOT / "Cargo.toml", ROOT / "Cargo.lock"]
                                 + sorted((ROOT / "crates/astrelis/src").rglob("*")))
                    if path.is_file()
                },
                "fonts": json.loads((ROOT / "crates/astrelis/tests/fonts/sources.json").read_text()),
                "capture_log": result.stdout, "image_sha256": images,
            }
            (output / "text-distance-fields-metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
            print(result.stdout)
    finally:
        BINARY.unlink()


if __name__ == "__main__":
    main()

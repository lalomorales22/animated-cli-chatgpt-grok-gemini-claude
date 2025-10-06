use anyhow::{Context, Result};
use crossbeam_channel::{bounded, Receiver};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use ratatui::{
    prelude::*,
    buffer::Buffer,
};
use std::cmp::min;
use ffmpeg_next as ff;
use ff::format::context::Input;
use ff::format::Pixel;
use ff::software::scaling::{context::Context as Scaler, flag::Flags};
use ff::util::frame::video::Video;

/// ASCII palette from light→dark
const PALETTE: &[u8] = b" .'`^\",:;Il!i><~+_-?][}{1)(|\\tfjrxnuvczXYUJCLQ0OZmwqpdbkhao*#MW&8%B@$";

pub struct AsciiFrame {
    w: u16,
    h: u16,
    /// Packed cells: (ch, r, g, b) row-major
    cells: Vec<(char, u8, u8, u8)>,
}

fn luminance(r: u8, g: u8, b: u8) -> u8 {
    let y = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
    y as u8
}

fn ascii_for(r: u8, g: u8, b: u8) -> char {
    let y = luminance(r, g, b) as usize;
    let idx = (y * (PALETTE.len() - 1)) / 255;
    PALETTE[idx] as char
}

fn to_ascii_frame(rgb: &Video) -> AsciiFrame {
    let w = rgb.width() as usize;
    let h = rgb.height() as usize;
    let stride = rgb.stride(0);
    let data = rgb.data(0);

    let mut cells = Vec::with_capacity(w * h);
    for y in 0..h {
        let row = &data[(y * stride) as usize..((y * stride) as usize + w * 3)];
        for x in 0..w {
            let i = x * 3;
            let (r, g, b) = (row[i], row[i + 1], row[i + 2]);
            let ch = ascii_for(r, g, b);
            cells.push((ch, r, g, b));
        }
    }

    AsciiFrame {
        w: w as u16,
        h: h as u16,
        cells,
    }
}

fn open_decoder(
    path: &str,
) -> Result<(
    Input,
    usize,
    ff::codec::decoder::Video,
)> {
    ff::init().context("init ffmpeg")?;
    let ictx = ff::format::input(&path).with_context(|| format!("open input {path}"))?;

    let stream = ictx
        .streams()
        .best(ff::media::Type::Video)
        .context("no video stream")?;
    let idx = stream.index();

    let dec_ctx = ff::codec::context::Context::from_parameters(stream.parameters())?;
    let decoder = dec_ctx.decoder().video()?;

    Ok((ictx, idx, decoder))
}

fn build_scaler(
    src_fmt: Pixel,
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
) -> Result<Scaler> {
    Scaler::get(
        src_fmt,
        src_w,
        src_h,
        Pixel::RGB24,
        dst_w,
        dst_h,
        Flags::BILINEAR,
    )
    .context("create scaler")
}

fn spawn_decode(path: String, target_w: u16, target_h: u16, finished_flag: Arc<AtomicBool>) -> Result<Receiver<AsciiFrame>> {
    let (tx, rx) = bounded::<AsciiFrame>(8);

    std::thread::spawn(move || -> Result<()> {
        let (mut ictx, v_idx, mut dec) = open_decoder(&path)?;
        let mut scaler = build_scaler(
            dec.format(),
            dec.width(),
            dec.height(),
            target_w as u32,
            target_h as u32,
        )?;

        let mut rgb = Video::new(Pixel::RGB24, target_w as u32, target_h as u32);
        let mut frame = Video::empty();

        loop {
            for (stream, packet) in ictx.packets() {
                if stream.index() != v_idx {
                    continue;
                }
                dec.send_packet(&packet)?;

                while dec.receive_frame(&mut frame).is_ok() {
                    scaler.run(&frame, &mut rgb)?;
                    let ascii = to_ascii_frame(&rgb);
                    if tx.send(ascii).is_err() {
                        finished_flag.store(true, Ordering::Relaxed);
                        return Ok(()); // UI gone
                    }
                }
            }

            // Flush decoder
            dec.send_eof()?;
            while dec.receive_frame(&mut frame).is_ok() {
                scaler.run(&frame, &mut rgb)?;
                let ascii = to_ascii_frame(&rgb);
                let _ = tx.send(ascii);
            }

            // Loop the video - seek back to start
            ictx.seek(0, ..0)?;
            dec = ff::codec::context::Context::from_parameters(
                ictx.streams().best(ff::media::Type::Video).unwrap().parameters()
            )?.decoder().video()?;
        }
    });

    Ok(rx)
}

pub struct VideoBackground {
    rx: Receiver<AsciiFrame>,
    latest: Option<AsciiFrame>,
    opacity: f32,
}

impl VideoBackground {
    pub fn new(path: &str, width: u16, height: u16, opacity: f32) -> Result<Self> {
        ff::init()?;

        let finished_flag = Arc::new(AtomicBool::new(false));
        let rx = spawn_decode(path.to_string(), width, height, finished_flag)?;

        Ok(Self {
            rx,
            latest: None,
            opacity: opacity.clamp(0.0, 1.0),
        })
    }

    pub fn update(&mut self) {
        // Try to receive ONE new frame
        if let Ok(af) = self.rx.try_recv() {
            self.latest = Some(af);
        }
    }

    /// Render video as background with opacity applied
    pub fn render_background(&self, buf: &mut Buffer, area: Rect) {
        if let Some(ref af) = self.latest {
            let content_w = min(af.w, area.width);
            let content_h = min(af.h, area.height);

            let x0 = area.x + (area.width - content_w) / 2;
            let y0 = area.y + (area.height - content_h) / 2;

            for y in 0..content_h {
                for x in 0..content_w {
                    let i = (y as usize * af.w as usize + x as usize) as usize;
                    if i >= af.cells.len() {
                        continue;
                    }
                    let (ch, r, g, b) = af.cells[i];

                    // Apply opacity by blending with black
                    let r_dim = (r as f32 * self.opacity) as u8;
                    let g_dim = (g as f32 * self.opacity) as u8;
                    let b_dim = (b as f32 * self.opacity) as u8;

                    if let Some(cell) = buf.cell_mut((x0 + x, y0 + y)) {
                        cell.set_char(ch);
                        cell.set_fg(Color::Rgb(r_dim, g_dim, b_dim));
                    }
                }
            }
        }
    }
}

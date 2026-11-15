use std::path::{Path, PathBuf};

use egui::{Color32, IconData};

const ICON_SIZE: u32 = 256;

fn candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            paths.push(directory.join("assets/icon.png"));
            paths.push(directory.join("icon.png"));
        }
    }
    paths.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icon.png"));
    paths.push(PathBuf::from("assets/icon.png"));
    paths
}

pub fn load_icon() -> IconData {
    for path in candidate_paths() {
        if let Ok(image) = image::open(&path) {
            let image = image.to_rgba8();
            if image.width() > 0 && image.height() > 0 {
                let width = image.width();
                let height = image.height();
                return IconData {
                    rgba: image.into_raw(),
                    width,
                    height,
                };
            }
        }
    }
    fallback_icon()
}

fn blend(pixel: &mut [u8; 4], color: Color32) {
    let alpha = color.a() as u16;
    if alpha == 0 {
        return;
    }
    let inverse = 255 - alpha;
    for (destination, source) in pixel[..3].iter_mut().zip([color.r(), color.g(), color.b()]) {
        let blended = (source as u16 * alpha + *destination as u16 * inverse) / 255;
        *destination = blended as u8;
    }
    pixel[3] = (pixel[3] as u16 + alpha - (pixel[3] as u16 * alpha / 255)) as u8;
}

fn fill_circle(buffer: &mut [[u8; 4]], center_x: f32, center_y: f32, radius: f32, color: Color32) {
    let min_x = (center_x - radius).floor().max(0.0) as u32;
    let max_x = (center_x + radius).ceil().min(ICON_SIZE as f32) as u32;
    let min_y = (center_y - radius).floor().max(0.0) as u32;
    let max_y = (center_y + radius).ceil().min(ICON_SIZE as f32) as u32;
    for y in min_y..max_y {
        for x in min_x..max_x {
            let dx = x as f32 + 0.5 - center_x;
            let dy = y as f32 + 0.5 - center_y;
            if dx * dx + dy * dy <= radius * radius {
                blend(&mut buffer[(y * ICON_SIZE + x) as usize], color);
            }
        }
    }
}

fn fill_rounded_rect(
    buffer: &mut [[u8; 4]],
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    radius: f32,
    color: Color32,
) {
    for y in top.max(0.0) as u32..bottom.min(ICON_SIZE as f32) as u32 {
        for x in left.max(0.0) as u32..right.min(ICON_SIZE as f32) as u32 {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let radius = radius
                .min((right - left).max(0.0) / 2.0)
                .min((bottom - top).max(0.0) / 2.0);
            let nearest_x = px.clamp(left + radius, right - radius);
            let nearest_y = py.clamp(top + radius, bottom - radius);
            let dx = px - nearest_x;
            let dy = py - nearest_y;
            if dx * dx + dy * dy <= radius * radius {
                blend(&mut buffer[(y * ICON_SIZE + x) as usize], color);
            }
        }
    }
}

fn draw_speaker(buffer: &mut [[u8; 4]], x: f32, y: f32, color: Color32) {
    fill_circle(buffer, x, y, 23.0, Color32::from_rgb(12, 16, 24));
    fill_circle(buffer, x, y, 18.0, color);
    fill_circle(buffer, x, y, 7.0, Color32::from_rgb(10, 14, 20));
    fill_circle(
        buffer,
        x - 9.0,
        y - 9.0,
        3.0,
        Color32::from_rgba_unmultiplied(255, 255, 255, 120),
    );
}

fn draw_subwoofer(buffer: &mut [[u8; 4]], x: f32, y: f32) {
    fill_rounded_rect(
        buffer,
        x - 27.0,
        y - 24.0,
        x + 27.0,
        y + 24.0,
        8.0,
        Color32::from_rgb(12, 16, 24),
    );
    fill_rounded_rect(
        buffer,
        x - 22.0,
        y - 19.0,
        x + 22.0,
        y + 19.0,
        6.0,
        Color32::from_rgb(255, 171, 64),
    );
    fill_circle(buffer, x, y, 10.0, Color32::from_rgb(17, 21, 30));
    fill_circle(buffer, x, y, 4.0, Color32::from_rgb(255, 200, 120));
}

/// Draw a clean 5.1 speaker-layout icon entirely in code.
pub fn fallback_icon() -> IconData {
    let mut pixels = vec![[0u8; 4]; (ICON_SIZE * ICON_SIZE) as usize];

    fill_rounded_rect(
        &mut pixels,
        8.0,
        8.0,
        248.0,
        248.0,
        42.0,
        Color32::from_rgb(13, 18, 28),
    );
    fill_rounded_rect(
        &mut pixels,
        13.0,
        13.0,
        243.0,
        243.0,
        38.0,
        Color32::from_rgb(19, 26, 40),
    );

    // Subtle room grid.
    for step in 1..4 {
        let offset = step as f32 * 56.0;
        fill_rounded_rect(
            &mut pixels,
            128.0 - 1.0,
            40.0 + offset,
            128.0 + 1.0,
            41.0 + offset,
            1.0,
            Color32::from_rgba_unmultiplied(90, 110, 140, 40),
        );
    }

    let cyan = Color32::from_rgb(77, 208, 200);
    let violet = Color32::from_rgb(150, 130, 255);
    draw_speaker(&mut pixels, 62.0, 66.0, cyan);
    draw_speaker(&mut pixels, 128.0, 46.0, cyan);
    draw_speaker(&mut pixels, 194.0, 66.0, cyan);
    draw_speaker(&mut pixels, 47.0, 160.0, violet);
    draw_speaker(&mut pixels, 209.0, 160.0, violet);
    draw_subwoofer(&mut pixels, 128.0, 205.0);

    // Small center listening marker.
    fill_circle(
        &mut pixels,
        128.0,
        128.0,
        11.0,
        Color32::from_rgba_unmultiplied(220, 235, 255, 35),
    );
    fill_circle(
        &mut pixels,
        128.0,
        128.0,
        5.0,
        Color32::from_rgba_unmultiplied(220, 235, 255, 90),
    );

    let mut rgba = Vec::with_capacity((ICON_SIZE * ICON_SIZE * 4) as usize);
    for pixel in pixels {
        rgba.extend_from_slice(&pixel);
    }

    IconData {
        rgba,
        width: ICON_SIZE,
        height: ICON_SIZE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_icon_is_valid_rgba() {
        let icon = fallback_icon();
        assert_eq!(icon.width, ICON_SIZE);
        assert_eq!(icon.height, ICON_SIZE);
        assert_eq!(icon.rgba.len(), (ICON_SIZE * ICON_SIZE * 4) as usize);
        assert!(icon.rgba.chunks_exact(4).any(|pixel| pixel[3] > 0));
    }

    #[test]
    fn load_icon_always_returns_something() {
        let icon = load_icon();
        assert!(!icon.is_empty());
    }
}

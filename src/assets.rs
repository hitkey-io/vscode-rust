//! Static vector assets ported verbatim from `vscode-original/`.
//!
//! Each constant references a byte slice embedded at compile time. The
//! [`svg_texture`] helper rasterises an SVG into an [`egui::TextureHandle`]
//! at the requested pixel size and memoises the result so subsequent frames
//! reuse the same GPU texture.

use std::sync::Mutex;

use egui::{Color32, ColorImage, Context, TextureHandle, TextureOptions};
use once_cell::sync::Lazy;

/// Editor "letterpress" watermark (260×260 SVG, opacity 0.3).
/// Source: `vscode-original/src/vs/workbench/browser/parts/editor/media/letterpress-dark.svg`
pub const LETTERPRESS_DARK: &[u8] = include_bytes!("../assets/letterpress/dark.svg");

/// Light variant for high-contrast / light themes.
pub const LETTERPRESS_LIGHT: &[u8] = include_bytes!("../assets/letterpress/light.svg");

/// Memoisation key: (asset id, requested width in pixels).
type Key = (&'static str, u32);

static CACHE: Lazy<Mutex<std::collections::HashMap<Key, TextureHandle>>> =
    Lazy::new(|| Mutex::new(Default::default()));

/// Rasterise an SVG to a texture at the requested target width (height
/// follows the source aspect ratio). The result is cached by `id + width`.
///
/// SVGs in vscode-original ship without explicit `fill` (relying on default
/// black), which is invisible on a dark editor background. We rewrite the
/// payload to set `fill="#FFFFFF"` so the texture is paintable, then apply
/// the desired color through `painter.image`'s tint at draw time.
pub fn svg_texture(
    ctx: &Context,
    id: &'static str,
    bytes: &'static [u8],
    target_width_px: u32,
) -> Option<TextureHandle> {
    let key = (id, target_width_px);
    if let Some(tex) = CACHE.lock().ok().and_then(|c| c.get(&key).cloned()) {
        return Some(tex);
    }

    let bytes = std::str::from_utf8(bytes).ok()?;
    let recolored = inject_white_fill(bytes);
    let tree = usvg::Tree::from_data(recolored.as_bytes(), &usvg::Options::default()).ok()?;
    let svg_size = tree.size();
    let target_w = target_width_px as f32;
    let scale = target_w / svg_size.width();
    let target_h = (svg_size.height() * scale).round() as u32;

    let mut pixmap = tiny_skia::Pixmap::new(target_width_px, target_h)?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    let color_image = ColorImage::from_rgba_unmultiplied(
        [target_width_px as usize, target_h as usize],
        pixmap.data(),
    );
    let tex = ctx.load_texture(
        format!("svg::{}::{}", id, target_width_px),
        color_image,
        TextureOptions::LINEAR,
    );

    if let Ok(mut cache) = CACHE.lock() {
        cache.insert(key, tex.clone());
    }
    Some(tex)
}

/// Force every path in the SVG to render as white. We use a regex-free
/// transformation: insert `fill="#FFFFFF"` into the opening `<svg>` tag so
/// it cascades. Existing explicit `fill` attributes on inner shapes still
/// win, which matches the SVG inheritance rules used by VS Code's CSS.
fn inject_white_fill(src: &str) -> String {
    if let Some(idx) = src.find("<svg") {
        let (head, rest) = src.split_at(idx + 4);
        let mut out = String::with_capacity(src.len() + 20);
        out.push_str(head);
        out.push_str(r##" fill="#FFFFFF""##);
        out.push_str(rest);
        out
    } else {
        src.to_owned()
    }
}

/// Convenience: paint the letterpress watermark centered in `rect`,
/// scaled so the smaller side maps to `target_size`, with `tint` applied.
pub fn paint_letterpress(
    ctx: &Context,
    painter: &egui::Painter,
    rect: egui::Rect,
    target_size: f32,
    tint: Color32,
) {
    let size = target_size.max(32.0) as u32;
    let Some(tex) = svg_texture(ctx, "letterpress", LETTERPRESS_DARK, size) else {
        return;
    };
    let img_size = tex.size_vec2();
    let mut target_rect = egui::Rect::from_center_size(rect.center(), img_size);
    if target_rect.width() > rect.width() || target_rect.height() > rect.height() {
        let scale =
            (rect.width() / target_rect.width()).min(rect.height() / target_rect.height());
        target_rect = egui::Rect::from_center_size(rect.center(), img_size * scale);
    }
    painter.image(
        tex.id(),
        target_rect,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        tint,
    );
}

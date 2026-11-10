//! Static vector assets ported verbatim from `vscode-original/`.
//!
//! Each constant references a byte slice embedded at compile time. The
//! [`svg_texture`] helper rasterises an SVG into an [`egui::TextureHandle`]
//! at the requested pixel size and memoises the result so subsequent frames
//! reuse the same GPU texture.


use egui::{Color32, ColorImage, Context, TextureHandle, TextureOptions};

/// Editor "letterpress" watermark (260×260 SVG, opacity 0.3).
/// Source: `vscode-original/src/vs/workbench/browser/parts/editor/media/letterpress-dark.svg`
pub const LETTERPRESS_DARK: &[u8] = include_bytes!("../assets/letterpress/dark.svg");

/// Light variant for high-contrast / light themes.
pub const LETTERPRESS_LIGHT: &[u8] = include_bytes!("../assets/letterpress/light.svg");

/// The product logo in colour (the Welcome editor's tab icon).
/// Source: `vscode-original/src/vs/sessions/browser/media/vscode-icon.svg`
pub const VSCODE_LOGO: &[u8] = include_bytes!("../assets/vscode-logo.svg");

/// Memoisation key: (asset id, requested width in pixels, recoloured white).
type Key = (&'static str, u32, bool);

/// Rasterised SVGs of one egui context (textures belong to the context
/// that made them, so they're kept in its memory, not in a global).
type Cache = std::collections::HashMap<Key, TextureHandle>;

fn cache_id() -> egui::Id {
    egui::Id::new("assets::svg-cache")
}

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
    rasterise(ctx, id, bytes, target_width_px, true)
}

/// [`svg_texture`] keeping the SVG's own colours (logos).
pub fn svg_texture_in_colour(
    ctx: &Context,
    id: &'static str,
    bytes: &'static [u8],
    target_width_px: u32,
) -> Option<TextureHandle> {
    rasterise(ctx, id, bytes, target_width_px, false)
}

fn rasterise(
    ctx: &Context,
    id: &'static str,
    bytes: &'static [u8],
    target_width_px: u32,
    white: bool,
) -> Option<TextureHandle> {
    let key = (id, target_width_px, white);
    if let Some(tex) = ctx.data(|d| d.get_temp::<Cache>(cache_id()).and_then(|c| c.get(&key).cloned())) {
        return Some(tex);
    }

    let bytes = std::str::from_utf8(bytes).ok()?;
    let recolored = if white { inject_white_fill(bytes) } else { bytes.to_owned() };
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

    ctx.data_mut(|d| d.get_temp_mut_or_default::<Cache>(cache_id()).insert(key, tex.clone()));
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

/// An icon file (SVG or PNG) as a white mask to tint at draw time, the way
/// VS Code paints contributed activity bar icons (`mask-image`, coloured by
/// `activityBar.inactiveForeground`). `size_px` is the square edge in
/// physical pixels. Cached by path and size.
pub fn icon_mask_texture(ctx: &Context, path: &std::path::Path, size_px: u32) -> Option<TextureHandle> {
    static MASKS: once_cell::sync::Lazy<std::sync::Mutex<std::collections::HashMap<(std::path::PathBuf, u32), Option<TextureHandle>>>> =
        once_cell::sync::Lazy::new(Default::default);
    let key = (path.to_path_buf(), size_px);
    if let Some(hit) = MASKS.lock().ok().and_then(|m| m.get(&key).cloned()) {
        return hit;
    }
    let rgba: Option<Vec<u8>> = (|| {
        let bytes = std::fs::read(path).ok()?;
        let is_svg = path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("svg"));
        if is_svg {
            let tree = usvg::Tree::from_data(&bytes, &usvg::Options::default()).ok()?;
            let size = tree.size();
            let scale = size_px as f32 / size.width().max(size.height());
            let mut pixmap = tiny_skia::Pixmap::new(size_px, size_px)?;
            let dx = (size_px as f32 - size.width() * scale) / 2.0;
            let dy = (size_px as f32 - size.height() * scale) / 2.0;
            resvg::render(&tree, tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, dx, dy), &mut pixmap.as_mut());
            // Premultiplied → straight alpha is irrelevant for a mask.
            Some(pixmap.data().to_vec())
        } else {
            let img = image::load_from_memory(&bytes).ok()?;
            let img = img.resize_exact(size_px, size_px, image::imageops::FilterType::Triangle).to_rgba8();
            Some(img.into_raw())
        }
    })();
    let tex = rgba.map(|mut px| {
        for p in px.chunks_exact_mut(4) {
            p[0] = 255;
            p[1] = 255;
            p[2] = 255;
        }
        let image = ColorImage::from_rgba_unmultiplied([size_px as usize, size_px as usize], &px);
        ctx.load_texture(format!("icon-mask::{}::{size_px}", path.display()), image, TextureOptions::LINEAR)
    });
    if let Ok(mut m) = MASKS.lock() {
        m.insert(key, tex.clone());
    }
    tex
}

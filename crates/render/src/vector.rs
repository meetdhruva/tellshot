//! Drawing annotation objects onto a premultiplied tiny-skia pixmap.

use tellshot_core::{Color, Object, Point, Rect, Shape, Style};
use tellshot_imaging::PixelRect;
use tiny_skia::{
    BlendMode, FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, Stroke, Transform,
};

use crate::Fonts;

/// Cubic Bézier constant for approximating a quarter circle.
const KAPPA: f32 = 0.552_284_8;

pub(crate) fn draw(pixmap: &mut Pixmap, object: &Object, step: Option<u32>, fonts: &Fonts) {
    let style = &object.style;
    match &object.shape {
        Shape::Arrow { from, to } => arrow(pixmap, *from, *to, style),
        Shape::Line { from, to } => {
            let mut pb = PathBuilder::new();
            pb.move_to(from.x, from.y);
            pb.line_to(to.x, to.y);
            stroke(pixmap, pb.finish(), style);
        }
        Shape::Rectangle { rect, corner_radius } => {
            let path = rounded_rect(*rect, *corner_radius);
            if let Some(fill_color) = style.fill {
                fill(pixmap, path.clone(), fill_color, BlendMode::SourceOver);
            }
            stroke(pixmap, path, style);
        }
        Shape::Ellipse { rect } => {
            let path = skia_rect(*rect).and_then(PathBuilder::from_oval);
            if let Some(fill_color) = style.fill {
                fill(pixmap, path.clone(), fill_color, BlendMode::SourceOver);
            }
            stroke(pixmap, path, style);
        }
        Shape::Highlight { rect } => {
            // Multiply keeps the text under the highlighter dark and readable.
            let color = style.fill.unwrap_or(style.stroke);
            fill(pixmap, skia_rect(*rect).map(PathBuilder::from_rect), color, BlendMode::Multiply);
        }
        Shape::Redact { rect } => redact(pixmap, *rect),
        Shape::Step { center, radius } => {
            let disc = PathBuilder::from_circle(center.x, center.y, *radius);
            fill(pixmap, disc.clone(), style.fill.unwrap_or(style.stroke), BlendMode::SourceOver);
            stroke(pixmap, disc, style);
            if let Some(n) = step {
                let label = n.to_string();
                let size = radius * if label.len() > 1 { 1.0 } else { 1.2 };
                let laid = fonts.layout(&label, 0.0, 0.0, size);
                let (dx, dy) = (center.x - laid.width / 2.0, center.y - size * 0.62);
                let path = laid.path.and_then(|p| p.transform(Transform::from_translate(dx, dy)));
                fill(pixmap, path, style.stroke, BlendMode::SourceOver);
            }
        }
        Shape::Text { origin, text, font_size } => {
            let laid = fonts.layout(text, origin.x, origin.y, *font_size);
            fill(pixmap, laid.path, style.stroke, BlendMode::SourceOver);
        }
        Shape::Freehand { points } => {
            let mut pb = PathBuilder::new();
            if let Some((first, rest)) = points.split_first() {
                pb.move_to(first.x, first.y);
                if rest.is_empty() {
                    pb.line_to(first.x + 0.01, first.y);
                }
                for p in rest {
                    pb.line_to(p.x, p.y);
                }
            }
            stroke(pixmap, pb.finish(), style);
        }
        // Raster effects were applied to the base image before any annotation was drawn.
        Shape::Blur { .. } | Shape::Pixelate { .. } => {}
    }
}

fn arrow(pixmap: &mut Pixmap, from: Point, to: Point, style: &Style) {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let len = dx.hypot(dy);
    if len < 0.5 {
        return;
    }
    let (ux, uy) = (dx / len, dy / len);
    let head_len = (style.stroke_width * 4.0).max(12.0).min(len);
    let half = head_len * 0.45;
    let base = Point::new((-ux).mul_add(head_len, to.x), (-uy).mul_add(head_len, to.y));

    let mut shaft = PathBuilder::new();
    shaft.move_to(from.x, from.y);
    // Stop the shaft inside the head so the round cap never pokes out of the tip.
    shaft.line_to(ux.mul_add(head_len * 0.3, base.x), uy.mul_add(head_len * 0.3, base.y));
    stroke(pixmap, shaft.finish(), style);

    let mut head = PathBuilder::new();
    head.move_to(to.x, to.y);
    head.line_to((-uy).mul_add(half, base.x), ux.mul_add(half, base.y));
    head.line_to(uy.mul_add(half, base.x), (-ux).mul_add(half, base.y));
    head.close();
    fill(pixmap, head.finish(), style.stroke, BlendMode::SourceOver);
}

/// Overwrites every pixel the rectangle touches with opaque black. No anti-aliasing: partial
/// coverage would blend, and blending would leak part of the original pixel.
fn redact(pixmap: &mut Pixmap, rect: Rect) {
    let bounds = PixelRect { x: 0, y: 0, width: pixmap.width(), height: pixmap.height() };
    let Some(area) = PixelRect::covering(rect.x, rect.y, rect.width, rect.height, bounds) else {
        return;
    };
    let stride = pixmap.width() as usize * 4;
    let (x0, x1) = (area.x as usize * 4, area.right() as usize * 4);
    for row in
        pixmap.data_mut().chunks_exact_mut(stride).skip(area.y as usize).take(area.height as usize)
    {
        if let Some(span) = row.get_mut(x0..x1) {
            for px in span.chunks_exact_mut(4) {
                px.copy_from_slice(&[0, 0, 0, 255]);
            }
        }
    }
}

fn paint(color: Color, blend_mode: BlendMode) -> Paint<'static> {
    let mut p = Paint { anti_alias: true, blend_mode, ..Paint::default() };
    p.set_color_rgba8(color.r, color.g, color.b, color.a);
    p
}

fn fill(pixmap: &mut Pixmap, path: Option<Path>, color: Color, blend_mode: BlendMode) {
    if let Some(path) = path {
        pixmap.fill_path(
            &path,
            &paint(color, blend_mode),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn stroke(pixmap: &mut Pixmap, path: Option<Path>, style: &Style) {
    if style.stroke_width <= 0.0 {
        return;
    }
    if let Some(path) = path {
        let stroke = Stroke {
            width: style.stroke_width,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        pixmap.stroke_path(
            &path,
            &paint(style.stroke, BlendMode::SourceOver),
            &stroke,
            Transform::identity(),
            None,
        );
    }
}

fn skia_rect(r: Rect) -> Option<tiny_skia::Rect> {
    tiny_skia::Rect::from_xywh(r.x, r.y, r.width, r.height)
}

fn rounded_rect(r: Rect, radius: f32) -> Option<Path> {
    let radius = radius.clamp(0.0, r.width.min(r.height) / 2.0);
    if radius <= 0.0 {
        return skia_rect(r).map(PathBuilder::from_rect);
    }
    let (x0, y0, x1, y1) = (r.x, r.y, r.right(), r.bottom());
    let k = radius * KAPPA;
    let mut pb = PathBuilder::new();
    pb.move_to(x0 + radius, y0);
    pb.line_to(x1 - radius, y0);
    pb.cubic_to(x1 - radius + k, y0, x1, y0 + radius - k, x1, y0 + radius);
    pb.line_to(x1, y1 - radius);
    pb.cubic_to(x1, y1 - radius + k, x1 - radius + k, y1, x1 - radius, y1);
    pb.line_to(x0 + radius, y1);
    pb.cubic_to(x0 + radius - k, y1, x0, y1 - radius + k, x0, y1 - radius);
    pb.line_to(x0, y0 + radius);
    pb.cubic_to(x0, y0 + radius - k, x0 + radius - k, y0, x0 + radius, y0);
    pb.close();
    pb.finish()
}

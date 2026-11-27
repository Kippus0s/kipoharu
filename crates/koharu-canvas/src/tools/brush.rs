//! Draw, eraser, clone stamp, and remove: stroke outlines and page-pixel coverage.
//! Ink previews the same raster write that is committed; mask and removal
//! tools retain their coverage overlays.

use std::cell::RefCell;

use image::{RgbaImage, imageops};
use koharu_renderer::Frame;
use koharu_scene::{Content, Edit, LayerId, Op, PixelFormat, Raster, Ref, Rgba, Tiles};
use kurbo::{
    Affine, BezPath, Cap, Circle, Join, Point, Shape as _, Size, Stroke as Style, StrokeOpts,
    Vec2, stroke,
};
use vello_cpu::{Pixmap, RenderContext, Resources, color::palette::css::WHITE};

use super::added;
use crate::{
    Brush, Effect, Notice, Tool,
    draw::overlay::Overlay,
    gesture::{Context, Gesture, GestureLook, Press},
    view::Sample,
};

/// How closely a stroke's outline follows its round caps and joins, in page
/// pixels.
const TOLERANCE: f64 = 0.1;

/// A brush tool's press starts a stroke: the draw tool's onto the one
/// selected raster or a new one, as with clone stamp; the eraser's onto the one selected.
pub(crate) fn press(cx: &Context, sample: Sample, clone_source: Option<Point>) -> Press {
    let clone = if cx.tool == Tool::CloneStamp {
        let Some(source) = clone_source else {
            return Press::answers(vec![Effect::Notice(Notice::CloneNeedsSource)]);
        };
        Some(CloneSource {
            offset: Vec2::new(
                source.x.floor() - sample.page.x.floor(),
                source.y.floor() - sample.page.y.floor(),
            ),
            pixels: RefCell::new(None),
        })
    } else {
        None
    };
    let target = cx.selection.single().filter(|&layer| {
        cx.snapshot.writable(layer).is_some_and(|(_, format)| {
            cx.tool != Tool::CloneStamp || format == PixelFormat::Rgba8
        })
            && (cx.tool != Tool::Remove
                || matches!(
                    cx.snapshot.layer(layer).map(|layer| &layer.content),
                    Some(Content::Raster(_))
                ))
    });
    if cx.tool == Tool::Eraser && target.is_none() {
        return Press::answers(vec![Effect::Notice(Notice::EraserNeedsRaster)]);
    }
    let ink = matches!(cx.tool, Tool::Draw | Tool::CloneStamp) && target
        .and_then(|layer| cx.snapshot.writable(layer))
        .is_none_or(|(_, format)| format == PixelFormat::Rgba8);
    let points = vec![sample.page];
    Press::starts(Gesture::Stroke(Stroke {
        tool: cx.tool,
        outline: outline(&points, cx.brush.diameter, ink),
        ink,
        brush: cx.brush,
        source: RefCell::new(None),
        clone,
        points,
        target,
    }))
}

/// A brush stroke through page points, onto `target` or, for ink with
/// none, a new raster.
pub(crate) struct Stroke {
    tool: Tool,
    ink: bool,
    brush: Brush,
    /// The original decoded tiles, unchanged throughout this gesture.
    source: RefCell<Option<Tiles>>,
    clone: Option<CloneSource>,
    points: Vec<Point>,
    target: Option<LayerId>,
    /// The points' outline, expanded again only when a point arrives.
    outline: BezPath,
}

/// The integer source-to-destination offset and the unmodified page pixels.
/// Read the composite once per gesture, never from its transient preview.
struct CloneSource {
    offset: Vec2,
    pixels: RefCell<Option<RgbaImage>>,
}

impl Stroke {
    pub fn clone_marker(&self) -> Option<Point> {
        let at = self.points.last()?;
        let offset = self.clone.as_ref()?.offset;
        Some(Point::new(at.x.floor() + 0.5, at.y.floor() + 0.5) + offset)
    }

    pub fn moved(&mut self, cx: &Context, samples: &[Sample]) -> Vec<Effect> {
        let before = self.points.len();
        for sample in samples {
            if self.points.last() != Some(&sample.page) {
                self.points.push(sample.page);
            }
        }
        if self.points.len() != before {
            let diameter = if self.ink { self.brush.diameter } else { cx.brush.diameter };
            self.outline = outline(&self.points, diameter, self.ink);
        }
        Vec::new()
    }

    /// The remove tool's inpaint request, or the draw tool's or the eraser's
    /// edit.
    pub fn finish(self, cx: &Context) -> anyhow::Result<Vec<Effect>> {
        let page = cx.snapshot.page().id;
        let Some((rect, coverage)) = self.stroke_coverage(cx) else {
            return Ok(Vec::new());
        };
        if self.tool == Tool::Remove {
            return Ok(vec![Effect::Inpaint {
                page,
                rect,
                mask: coverage,
                raster: self.target,
            }]);
        }
        self.stroked(cx, rect, &coverage)
    }

    pub fn preview(&self, cx: &Context, overlay: &mut Overlay) {
        if self.ink {
            return; // The page painter shows the actual raster instead.
        }
        // A mask takes coverage, not colour.
        let colored = self
            .target
            .and_then(|layer| cx.snapshot.writable(layer))
            .is_none_or(|(_, format)| format == PixelFormat::Rgba8);
        let tint = match self.tool {
            Tool::Draw if colored => cx.brush.color,
            _ => {
                let Rgba([r, g, b, _]) = cx.colors.selection;
                Rgba([r, g, b, 0.4])
            }
        };
        overlay.fill(&self.outline, tint);
    }

    /// Draw the same pixel write in the same layer position as the commit.
    pub fn raster_preview(&self, cx: &Context) -> anyhow::Result<Option<Frame>> {
        if !self.ink {
            return Ok(None);
        }
        let Some((rect, coverage)) = self.stroke_coverage(cx) else {
            return Ok(None);
        };
        let (tiles, own, pixels) = if let Some(layer) = self.target {
            let Some((offset, PixelFormat::Rgba8)) = cx.snapshot.writable(layer) else {
                return Ok(None);
            };
            let own = koharu_scene::Rect {
                x: rect.x - offset.x,
                y: rect.y - offset.y,
                ..rect
            };
            let session = cx.session.lock();
            let under = session.pixels(layer, own)?;
            let mut source = self.source.borrow_mut();
            if source.is_none() {
                *source = Some(session.tiles(layer, None)?);
            }
            drop(session);
            let pixels = self.ink_pixels(cx, rect, &coverage, Some(under))?;
            (source.as_ref().expect("source tiles").clone(), own, pixels)
        } else {
            let tiles = Tiles {
                offset: Default::default(),
                format: PixelFormat::Rgba8,
                tiles: Vec::new(),
            };
            (tiles, rect, self.ink_pixels(cx, rect, &coverage, None)?)
        };
        let page = cx.snapshot.page();
        let index = page.raster_insert_index() + if page.background.is_some() { 1 } else { 0 };
        Ok(Some(cx.snapshot.frame().with_raster_preview(
            self.target, index, tiles, own, &pixels,
        )?))
    }

    fn stroke_coverage(&self, cx: &Context) -> Option<(koharu_scene::Rect, Vec<u8>)> {
        let page = cx.snapshot.size();
        let (rect, mut coverage) = if self.ink && self.brush.diameter <= 4.0 {
            supersampled_coverage(&self.outline, page)?
        } else {
            coverage(&self.outline, page)?
        };
        if let Some(clone) = &self.clone {
            // An image source does not wrap or paint outside the page.
            for (i, alpha) in coverage.iter_mut().enumerate() {
                let x = f64::from(rect.x) + (i % rect.width as usize) as f64 + clone.offset.x;
                let y = f64::from(rect.y) + (i / rect.width as usize) as f64 + clone.offset.y;
                if !(0.0..page.width).contains(&x) || !(0.0..page.height).contains(&y) {
                    *alpha = 0;
                }
            }
            if !coverage.iter().any(|&alpha| alpha != 0) {
                return None;
            }
        }
        Some((rect, coverage))
    }

    fn ink_pixels(
        &self,
        cx: &Context,
        rect: koharu_scene::Rect,
        coverage: &[u8],
        under: Option<Vec<u8>>,
    ) -> anyhow::Result<Vec<u8>> {
        let ink = if let Some(clone) = &self.clone {
            let mut pixels = clone.pixels.borrow_mut();
            if pixels.is_none() {
                *pixels = Some(cx.snapshot.frame().rasterize_rect(cx.snapshot.size().to_rect())?);
            }
            clone_pixels(
                pixels.as_ref().expect("source pixels"),
                clone.offset,
                rect,
                coverage,
            )
        } else {
            paint(rect, coverage, self.brush.color)
        };
        if let Some(pixels) = under {
            let mut under = RgbaImage::from_raw(rect.width, rect.height, pixels)
                .ok_or_else(|| anyhow::anyhow!("a raster's pixels are RGBA"))?;
            imageops::overlay(&mut under, &ink, 0, 0);
            Ok(under.into_raw())
        } else {
            Ok(ink.into_raw())
        }
    }

    pub fn look(&self) -> GestureLook {
        GestureLook::Stroke {
            points: self.points.len(),
            last: self.points.last().copied().unwrap_or_default(),
        }
    }

    /// A paint or erase stroke's coverage over the page pixels `rect` as one
    /// write to its raster, clean-up, or mask, read and composited in its own
    /// coordinates; painting with no target adds a raster at the top of the
    /// page, selected. A mask takes coverage on an alpha scale, as Clip Studio
    /// Paint's does: a stroke of any colour uncovers the clean-up, and the
    /// eraser hides it.
    fn stroked(
        &self,
        cx: &Context,
        rect: koharu_scene::Rect,
        coverage: &[u8],
    ) -> anyhow::Result<Vec<Effect>> {
        let label = match self.tool {
            Tool::Draw => "paint",
            Tool::CloneStamp => "clone_stamp",
            _ => "erase",
        };
        let target = self
            .target
            .and_then(|layer| Some((layer, cx.snapshot.writable(layer)?)));
        let (ops, created) = match target {
            Some((layer, (offset, format))) => {
                let own = koharu_scene::Rect {
                    x: rect.x - offset.x,
                    y: rect.y - offset.y,
                    ..rect
                };
                let mut pixels = cx.session.lock().pixels(layer, own)?;
                match (self.tool, format) {
                    (Tool::Draw | Tool::CloneStamp, PixelFormat::Rgba8) => {
                        pixels = self.ink_pixels(cx, rect, coverage, Some(pixels))?;
                    }
                    (Tool::Draw, PixelFormat::Gray8) => {
                        uncovered(&mut pixels, coverage, cx.brush.color.0[3]);
                    }
                    (_, PixelFormat::Rgba8) => erased(&mut pixels, coverage, 4),
                    (_, PixelFormat::Gray8) => erased(&mut pixels, coverage, 1),
                }
                let write = Op::WritePixels {
                    layer: Ref::from(layer),
                    rect: own,
                    pixels,
                };
                (vec![write], None)
            }
            None if self.ink => {
                let pixels = self.ink_pixels(cx, rect, coverage, None)?;
                let (id, add) = added(cx.snapshot.page(), None, Content::Raster(Raster::default()));
                let write = Op::WritePixels {
                    layer: Ref::from(id),
                    rect,
                    pixels,
                };
                (vec![add, write], Some(id))
            }
            None => return Ok(Vec::new()),
        };
        let edit = Effect::Edit(Edit {
            label: label.to_owned(),
            ops,
        });
        Ok(std::iter::once(edit)
            .chain(created.map(|id| Effect::Selection(vec![id])))
            .collect())
    }
}

/// Copy whole source pixels, without resampling, with the brush's coverage.
/// Like the canvas and eyedropper, the visible page is over white.
fn clone_pixels(
    source: &RgbaImage,
    offset: Vec2,
    rect: koharu_scene::Rect,
    coverage: &[u8],
) -> RgbaImage {
    let mut ink = RgbaImage::new(rect.width, rect.height);
    for (i, (x, y, pixel)) in ink.enumerate_pixels_mut().enumerate() {
        let sx = i64::from(rect.x) + i64::from(x) + offset.x as i64;
        let sy = i64::from(rect.y) + i64::from(y) + offset.y as i64;
        if coverage[i] == 0
            || sx < 0
            || sy < 0
            || sx >= i64::from(source.width())
            || sy >= i64::from(source.height())
        {
            continue;
        }
        let [r, g, b, a] = source.get_pixel(sx as u32, sy as u32).0;
        let over_white = |channel: u8| {
            ((u32::from(channel) * u32::from(a) + 255 * (255 - u32::from(a)) + 127) / 255) as u8
        };
        *pixel = image::Rgba([over_white(r), over_white(g), over_white(b), coverage[i]]);
    }
    ink
}

/// A round stroke through `points`, `diameter` wide, as the outline it
/// fills; a single point is a dot.
fn outline(points: &[Point], diameter: f64, ink: bool) -> BezPath {
    let tolerance = if ink {
        (diameter * 0.01).min(0.01)
    } else {
        TOLERANCE
    };
    match points {
        [] => BezPath::new(),
        [point] => Circle::new(*point, diameter * 0.5).to_path(tolerance),
        [first, rest @ ..] => {
            let mut line = BezPath::new();
            line.move_to(*first);
            if ink {
                smooth(&mut line, points);
            } else {
                for point in rest {
                    line.line_to(*point);
                }
            }
            let style = Style::new(diameter)
                .with_caps(Cap::Round)
                .with_join(Join::Round);
            stroke(line.iter(), &style, &StrokeOpts::default(), tolerance)
        }
    }
}

/// Centripetal Catmull–Rom tangents, converted to cubic controls. At a
/// deliberate corner (a turn of at least 60 degrees), use one-sided tangents.
fn smooth(path: &mut BezPath, points: &[Point]) {
    fn tangent(a: Point, b: Point, c: Point, segment: Vec2) -> Vec2 {
        let incoming = b - a;
        let outgoing = c - b;
        let l = incoming.hypot();
        let r = outgoing.hypot();
        if l < 1e-6 || r < 1e-6 || incoming.dot(outgoing) <= 0.5 * l * r {
            return segment;
        }
        let left = l.sqrt();
        let right = r.sqrt();
        (incoming / left - (c - a) / (left + right) + outgoing / right)
            * segment.hypot().sqrt()
    }
    fn control(tangent: Vec2, length: f64) -> Vec2 {
        let control = tangent / 3.0;
        let magnitude = control.hypot();
        if magnitude > length * 0.5 {
            control * (length * 0.5 / magnitude)
        } else {
            control
        }
    }
    for i in 0..points.len() - 1 {
        let a = points[i];
        let b = points[i + 1];
        let segment = b - a;
        let length = segment.hypot();
        if length < 1e-6 {
            path.line_to(b);
            continue;
        }
        let start = if i > 0 {
            tangent(points[i - 1], a, b, segment)
        } else {
            segment
        };
        let end = if i + 2 < points.len() {
            tangent(a, b, points[i + 2], segment)
        } else {
            segment
        };
        path.curve_to(a + control(start, length), b - control(end, length), b);
    }
}

/// `outline`'s antialiased 8-bit coverage over the page pixels it touches.
pub(crate) fn coverage(outline: &BezPath, page: Size) -> Option<(koharu_scene::Rect, Vec<u8>)> {
    let rect = koharu_scene::Rect::covering(
        outline.bounding_box(),
        page.width as u32,
        page.height as u32,
    )?;
    let width = u16::try_from(rect.width).ok()?;
    let height = u16::try_from(rect.height).ok()?;
    let pixels = raster_coverage(
        outline,
        width,
        height,
        Affine::translate((-f64::from(rect.x), -f64::from(rect.y))),
    );
    Some((rect, pixels))
}

fn raster_coverage(outline: &BezPath, width: u16, height: u16, transform: Affine) -> Vec<u8> {
    let mut context = RenderContext::new(width, height);
    context.set_transform(transform);
    context.set_paint(WHITE);
    context.fill_path(outline);
    context.flush();
    let mut pixmap = Pixmap::new(width, height);
    context.render(&mut pixmap, &mut Resources::new());
    pixmap.data().iter().map(|pixel| pixel.a).collect()
}

/// Sample fine ink on a finer grid, then average only the samples belonging
/// to each page pixel. Integer-aligned tiles bound the temporary allocation
/// without changing the sample grid or introducing a filter across pixels.
fn supersampled_coverage(outline: &BezPath, page: Size) -> Option<(koharu_scene::Rect, Vec<u8>)> {
    const SCALE: usize = 4;
    const TILE: usize = 256;
    let rect = koharu_scene::Rect::covering(
        outline.bounding_box(),
        page.width as u32,
        page.height as u32,
    )?;
    let width = rect.width as usize;
    let height = rect.height as usize;
    let mut result = vec![0; width.checked_mul(height)?];
    let scale = SCALE as f64;
    for y in (0..height).step_by(TILE) {
        let h = TILE.min(height - y);
        for x in (0..width).step_by(TILE) {
            let w = TILE.min(width - x);
            let sw = w * SCALE;
            let transform = Affine::new([
                scale,
                0.0,
                0.0,
                scale,
                -(f64::from(rect.x) + x as f64) * scale,
                -(f64::from(rect.y) + y as f64) * scale,
            ]);
            let samples = raster_coverage(outline, sw as u16, (h * SCALE) as u16, transform);
            for dy in 0..h {
                for dx in 0..w {
                    let mut sum = 0u32;
                    for sy in 0..SCALE {
                        let start = (dy * SCALE + sy) * sw + dx * SCALE;
                        sum += samples[start..start + SCALE]
                            .iter()
                            .map(|&a| u32::from(a))
                            .sum::<u32>();
                    }
                    result[(y + dy) * width + x + dx] =
                        ((sum + (SCALE * SCALE / 2) as u32) / (SCALE * SCALE) as u32) as u8;
                }
            }
        }
    }
    Some((rect, result))
}

/// The brush's `color` over `rect`, as opaque as `coverage` says of it.
/// Keep area coverage linear: boosting all partial pixels thickens fringes
/// and changes the effective width of a fractional brush.
fn paint(rect: koharu_scene::Rect, coverage: &[u8], Rgba([r, g, b, a]): Rgba) -> RgbaImage {
    let [r, g, b] = [r, g, b].map(|channel| (channel * 255.0).round() as u8);
    let pixels = coverage
        .iter()
        .flat_map(|&covered| [r, g, b, (f32::from(covered) * a).round() as u8])
        .collect();
    RgbaImage::from_raw(rect.width, rect.height, pixels).expect("coverage covers `rect`")
}

/// `pixels`, `bytes` to a pixel, with their last channel, a raster's alpha
/// or a mask's grey, taken away where `coverage` says.
fn erased(pixels: &mut [u8], coverage: &[u8], bytes: usize) {
    for (pixel, &covered) in pixels.chunks_exact_mut(bytes).zip(coverage) {
        let kept = 1.0 - f32::from(covered) / 255.0;
        pixel[bytes - 1] = (f32::from(pixel[bytes - 1]) * kept).round() as u8;
    }
}

/// A mask's grey `pixels` raised toward white where `coverage` says, as
/// opaque as the brush's `alpha`.
fn uncovered(pixels: &mut [u8], coverage: &[u8], alpha: f32) {
    for (pixel, &covered) in pixels.iter_mut().zip(coverage) {
        let added = f32::from(covered) / 255.0 * alpha;
        let grey = f32::from(*pixel);
        *pixel = (grey + (255.0 - grey) * added).round() as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::PathEl;

    #[test]
    fn clone_copies_integer_pixels_and_does_not_wrap() {
        let source = RgbaImage::from_raw(
            3,
            1,
            vec![17, 53, 199, 255, 123, 44, 7, 255, 0, 0, 0, 128],
        ).unwrap();
        let rect = koharu_scene::Rect { x: 10, y: 3, width: 5, height: 1 };
        let ink = clone_pixels(&source, Vec2::new(-11.0, -3.0), rect, &[255, 255, 64, 255, 255]);
        assert_eq!(ink.into_raw(), vec![
            0, 0, 0, 0, 17, 53, 199, 255, 123, 44, 7, 64,
            127, 127, 127, 255, 0, 0, 0, 0,
        ]);
    }

    #[test]
    fn fine_ink_keeps_coverage_and_color() {
        let rect = koharu_scene::Rect {
            x: 0,
            y: 0,
            width: 4,
            height: 1,
        };
        let image = paint(rect, &[0, 64, 128, 255], Rgba([0.0, 0.0, 0.0, 0.5]));
        assert_eq!(
            image.into_raw(),
            vec![0, 0, 0, 0, 0, 0, 0, 32, 0, 0, 0, 64, 0, 0, 0, 128],
        );
    }

    #[test]
    fn fractional_ink_has_nonzero_partial_coverage() {
        let shape = outline(&[Point::new(5.5, 5.5)], 0.5, true);
        let (rect, pixels) = supersampled_coverage(&shape, Size::new(10.0, 10.0)).unwrap();
        assert_eq!((rect.x, rect.y, rect.width, rect.height), (5, 5, 1, 1));
        // A half-pixel circle covers about pi/16 of its containing pixel.
        assert!((45..=55).contains(&pixels[0]), "coverage: {:?}", pixels);
    }

    #[test]
    fn supersampling_preserves_coverage_at_tile_boundaries() {
        let shape = outline(&[Point::new(1.5, 2.5), Point::new(520.5, 2.5)], 1.0, true);
        let (rect, pixels) = supersampled_coverage(&shape, Size::new(600.0, 10.0)).unwrap();
        assert_eq!(rect.height, 1);
        assert!(pixels[250..265].iter().all(|&a| a >= 250));
        assert!(pixels[505..520].iter().all(|&a| a >= 250));
    }

    #[test]
    fn interpolation_preserves_a_sharp_right_angle() {
        let points = [Point::new(0.0, 0.0), Point::new(3.0, 0.0), Point::new(3.0, 3.0)];
        let mut path = BezPath::new();
        path.move_to(points[0]);
        smooth(&mut path, &points);
        assert_eq!(path.elements(), &[
            PathEl::MoveTo(points[0]),
            PathEl::CurveTo(Point::new(1.0, 0.0), Point::new(2.0, 0.0), points[1]),
            PathEl::CurveTo(Point::new(3.0, 1.0), Point::new(3.0, 2.0), points[2]),
        ]);
    }
}

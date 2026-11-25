//! Moving the view: the pan gesture, the wheel, and pinch. Each answers the
//! camera it wants, which the canvas clamps.

use koharu_scene::{Content, Edit, Op, Ref};
use kurbo::{Point, TranslateScale, Vec2};
use serde_json::json;
use ui_events::ScrollDelta;

use super::command;
use crate::{Effect, Tool, gesture::Context, view::Sample};

/// How far Alt and a notch of the wheel change the brush, in page pixels,
/// as the editor's did.
const BRUSH_STEP: f64 = 4.0;

/// A wheel notch changes selected lettering by one point.
const TEXT_STEP: f64 = 1.0;

/// How far a device-independent pixel of wheel zooms, as the editor's
/// canvas zoomed: `exp(-Δy × 0.0015)`.
const WHEEL_ZOOM: f64 = 0.0015;

/// A wheel's line, in device-independent pixels, as browsers count it.
const LINE: f64 = 100.0;

/// The camera dragged from where it was when the pointer was at `from` in
/// the viewport, which moves with the camera.
pub(crate) struct Pan {
    from: Point,
    camera: TranslateScale,
}

impl Pan {
    pub fn new(sample: Sample, camera: TranslateScale) -> Self {
        Self {
            from: sample.screen,
            camera,
        }
    }

    pub fn moved(&self, samples: &[Sample]) -> Vec<Effect> {
        samples
            .last()
            .map(|last| Effect::Camera(self.camera + (last.screen - self.from)))
            .into_iter()
            .collect()
    }
}

/// The wheel pans, by a line or a page as a browser scrolls one; with Ctrl
/// or Command it zooms about the pointer, and with Alt and a brush tool it
/// steps the brush, or adjusts selected text with the selection or Text tools.
pub(crate) fn wheel(cx: &Context, sample: Sample, delta: ScrollDelta) -> Vec<Effect> {
    let line = LINE * cx.view.scale();
    let page = cx.view.size();
    let pixels = delta.to_pixel_delta((line, line).into(), (page.width, page.height).into());
    let delta = Vec2::new(pixels.x, pixels.y);
    let modifiers = sample.modifiers;
    if modifiers.alt() && cx.tool.is_brush() {
        if delta.y == 0.0 {
            return Vec::new();
        }
        let size = cx.snapshot.size();
        let step = if delta.y < 0.0 {
            BRUSH_STEP
        } else {
            -BRUSH_STEP
        };
        let diameter = (cx.brush.diameter + step).clamp(1.0, size.width.max(size.height).max(1.0));
        return vec![Effect::BrushDiameter(diameter)];
    }
    if modifiers.alt()
        && matches!(cx.tool, Tool::Select | Tool::DirectSelect | Tool::Text)
        && delta.y != 0.0
    {
        let step = if delta.y < 0.0 { TEXT_STEP } else { -TEXT_STEP };
        let ops: Vec<Op> = cx
            .selection
            .ids()
            .iter()
            .flat_map(|id| {
                let Some(layer) = cx.snapshot.layer(*id) else {
                    return Vec::new();
                };
                if layer.locked {
                    return Vec::new();
                }
                let (text, translation) = match &layer.content {
                    Content::Text(text) => (text.as_ref(), false),
                    Content::Block(block) => match block.translation.as_ref() {
                        Some(text) => (text, true),
                        None => return Vec::new(),
                    },
                    _ => return Vec::new(),
                };
                let old = text.style.chars.size.unwrap_or(24.0);
                let size = (old + step).clamp(0.5, 1296.0);
                if size == old {
                    return Vec::new();
                }
                // Preserve the chosen fit; the scene scales proportional frames on size edits.
                let mut patch = json!({"style": {"chars": {"size": size}}});
                if translation {
                    patch = json!({"translation": patch});
                }
                let layer = Ref::from(*id);
                let mut ops = vec![Op::UpdateLayer {
                    layer: layer.clone(),
                    patch,
                }];
                // Span-specific sizes override the text's base style; clear them as TypeBar does.
                if text.content.0.iter().any(|span| span.attributes.size.is_some()) {
                    let spans = text
                        .content
                        .0
                        .iter()
                        .cloned()
                        .map(|mut span| {
                            span.attributes.size = None;
                            span
                        })
                        .collect();
                    ops.push(Op::SetRichText { layer, spans });
                }
                ops
            })
            .collect();
        if !ops.is_empty() {
            return vec![Effect::Edit(Edit {
                label: "set_style".to_owned(),
                ops,
            })];
        }
    }
    let camera = match command(modifiers) {
        true => cx.view.zoomed(
            sample.screen,
            (-delta.y / cx.view.scale() * WHEEL_ZOOM).exp(),
        ),
        false => {
            let delta = match modifiers.shift() && delta.x == 0.0 {
                true => Vec2::new(delta.y, 0.0),
                false => delta,
            };
            cx.view.camera() - delta
        }
    };
    vec![Effect::Camera(camera)]
}

/// A pinch zooms by `1 + delta` about where it is.
pub(crate) fn pinch(cx: &Context, sample: Sample, delta: f32) -> Vec<Effect> {
    vec![Effect::Camera(
        cx.view.zoomed(sample.screen, 1.0 + f64::from(delta)),
    )]
}

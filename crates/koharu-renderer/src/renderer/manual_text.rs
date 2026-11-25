//! The transition from automatic fitting to manual proportional sizing.
//! It uses the renderer's actual scale, not the stored nominal size. The
//! normalization and requested edit are applied in a single history step.

use std::collections::HashMap;

use anyhow::{Result, ensure};
use koharu_scene::{Content, Edit, Frame, LayerId, Op, Project, Span, Text};
use serde_json::{Value, json};

use super::Renderer;

impl Renderer {
    /// Expand the UI's size-only or explicit Proportional fit edits. Other
    /// operations (including direct frame resizing) retain their semantics.
    pub fn prepare_manual_text_edit(&self, project: &Project, mut edit: Edit) -> Result<Edit> {
        let sizing = edit.label == "set_style";
        if !sizing && edit.label != "update_layer" {
            return Ok(edit);
        }
        let mut ops = Vec::new();
        let mut normalized = HashMap::new();
        for mut op in std::mem::take(&mut edit.ops) {
            match &mut op {
                Op::UpdateLayer { layer, patch } => {
                    if let Some(id) = layer.uuid().map(LayerId)
                        && !normalized.contains_key(&id)
                        && let Ok(current) = project.layer(id)
                    {
                        let target = match &current.content {
                            Content::Text(text) => Some((text.as_ref(), false)),
                            Content::Block(block) => block.translation.as_ref().map(|text| (text, true)),
                            _ => None,
                        };
                        if let Some((text, translation)) = target
                            && text.is_best_fit()
                            && is_transition(text, translation, patch, sizing)
                        {
                            let layout = self.layout(id, text)?;
                            let scale = layout.as_ref().map_or(1.0, |laid| f64::from(laid.scale));
                            let size = layout.as_ref().map_or_else(
                                || text.style.chars.size.unwrap_or(24.0),
                                |laid| laid.effective_size(text),
                            );
                            ensure!(scale.is_finite() && scale > 0.0 && size.is_finite() && size > 0.0,
                                "invalid rendered size for proportional text");
                            let patch = base_patch(text, translation, size, scale);
                            ops.push(Op::UpdateLayer { layer: layer.clone(), patch });
                            let mut spans = text.content.0.clone();
                            scale_marks(&mut spans, scale);
                            if spans != text.content.0 {
                                ops.push(Op::SetRichText { layer: layer.clone(), spans });
                            }
                            normalized.insert(id, scale);
                        }
                    }
                }
                Op::SetRichText { layer, spans } => {
                    // The TypeBar/wheel clear explicit span sizes on a layer-wide
                    // size edit. Keep that behavior, but normalize any remaining
                    // stroke marks so the Best Fit scale is not lost.
                    if let Some(scale) = layer.uuid().map(LayerId).and_then(|id| normalized.get(&id)) {
                        scale_marks(spans, *scale);
                    }
                }
                _ => {}
            }
            ops.push(op);
        }
        edit.ops = ops;
        Ok(edit)
    }
}

fn fit_patch(text: &Text) -> Value {
    match &text.frame {
        Frame::Box { .. } => json!({"frame": {"box": {"fit": "proportional"}}}),
        Frame::Path { .. } => json!({"frame": {"path": {"fit": "proportional"}}}),
        Frame::Point => unreachable!("point text cannot be Best Fit"),
    }
}

fn wrapped(patch: Value, translation: bool) -> Value {
    if translation { json!({"translation": patch}) } else { patch }
}

/// Only the exact size-only and fit-only patches the controls issue. An
/// explicit frame geometry edit, mixed style edit, or other client op is left alone.
fn is_transition(text: &Text, translation: bool, patch: &Value, sizing: bool) -> bool {
    if sizing {
        let local = if translation { &patch["translation"] } else { patch };
        let size = &local["style"]["chars"]["size"];
        size.as_f64().is_some_and(|size| size.is_finite() && size > 0.0)
            && *patch == wrapped(json!({"style": {"chars": {"size": size}}}), translation)
    } else {
        *patch == wrapped(fit_patch(text), translation)
    }
}

fn base_patch(text: &Text, translation: bool, size: f64, scale: f64) -> Value {
    let mut patch = fit_patch(text);
    patch["style"] = json!({"chars": {"size": size}});
    if let Some(width) = text.style.chars.stroke_width {
        patch["style"]["chars"]["stroke_width"] = json!(width * scale);
    }
    wrapped(patch, translation)
}

fn scale_marks(spans: &mut [Span], scale: f64) {
    for span in spans {
        span.attributes.size = span.attributes.size.map(|size| size * scale);
        span.attributes.stroke_width = span.attributes.stroke_width.map(|width| width * scale);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_best_fit_without_changing_geometry() {
        for (frame, translation) in [
            (json!({"box": {"bounds": {"x": 10, "y": 20, "width": 80, "height": 40}, "fit": "best"}}), false),
            (json!({"path": {"path": "M10 20H90V60H10Z", "fit": "best"}}), true),
        ] {
            let text: Text = serde_json::from_value(json!({
                "frame": frame, "style": {"chars": {"size": 32, "stroke_width": 2}},
                "content": [{"insert": "Hi", "attributes": {"size": 32, "stroke_width": 4}}]
            })).unwrap();
            assert!(text.is_best_fit());
            let requested = wrapped(json!({"style": {"chars": {"size": 19.0}}}), translation);
            assert!(is_transition(&text, translation, &requested, true));
            let choose = wrapped(fit_patch(&text), translation);
            assert!(is_transition(&text, translation, &choose, false));
            let patch = base_patch(&text, translation, 16.0, 0.5);
            let local = if translation { &patch["translation"] } else { &patch };
            assert_eq!(local["frame"], fit_patch(&text)["frame"]);
            assert_eq!(local["style"]["chars"], json!({"size": 16.0, "stroke_width": 1.0}));
            let mut spans = text.content.0.clone();
            scale_marks(&mut spans, 0.5);
            assert_eq!(spans[0].attributes.size, Some(16.0));
            assert_eq!(spans[0].attributes.stroke_width, Some(2.0));
        }
    }
}

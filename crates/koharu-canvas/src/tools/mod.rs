//! Each tool's presses and the gestures they start.

pub(crate) mod brush;
pub(crate) mod direct;
pub(crate) mod mark;
pub(crate) mod pan;
pub(crate) mod picker;
pub(crate) mod select;
pub(crate) mod text;
pub(crate) mod transform;

use koharu_scene::{Attributes, Content, LayerId, NewLayer, Op, Opacity, Page, Ref, Uuid};
use ui_events::keyboard::Modifiers;

/// A new layer of `content`, under an id the canvas picks so it can select it.
/// Raster layers go below top lettering and panel/balloon containers; new
/// text goes below those containers but above newly inserted rasters.
/// Neither action reorders existing layers.
pub(crate) fn added(page: &Page, content: Content) -> (LayerId, Op) {
    let index = match &content {
        Content::Raster(_) => page.raster_insert_index(),
        Content::Text(_) => page.text_insert_index(),
        _ => page.layers.len(),
    };
    let id = LayerId(Uuid::now_v7());
    let op = Op::AddLayer {
        id: Some(Ref::from(id)),
        page: Ref::from(page.id),
        parent: None,
        index,
        layer: Box::new(NewLayer {
            name: String::new(),
            visible: true,
            locked: false,
            opacity: Opacity::OPAQUE,
            blend: None,
            attributes: Attributes::new(),
            content,
        }),
    };
    (id, op)
}

/// Ctrl, or Command on macOS.
pub(crate) fn command(modifiers: Modifiers) -> bool {
    modifiers.ctrl() || modifiers.meta()
}

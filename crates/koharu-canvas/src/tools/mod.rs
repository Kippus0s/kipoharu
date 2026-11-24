//! Each tool's presses and the gestures they start.

pub(crate) mod block;
pub(crate) mod brush;
pub(crate) mod direct;
pub(crate) mod mark;
pub(crate) mod pan;
pub(crate) mod picker;
pub(crate) mod select;
pub(crate) mod text;
pub(crate) mod transform;

use koharu_scene::{Attributes, Content, Layer, LayerId, NewLayer, Op, Opacity, Page, Ref, Uuid};
use ui_events::keyboard::Modifiers;

/// Add a layer inside its holder or on the page.
/// Top-level rasters retain Kipoharu's custom ordering.
/// Other layers are added at the top of their container.
pub(crate) fn added(page: &Page, holder: Option<&Layer>, content: Content) -> (LayerId, Op) {
    let index = if let Some(holder) = holder {
        holder.children.len()
    } else if matches!(&content, Content::Raster(_)) {
        page.raster_insert_index()
    } else {
        page.layers.len()
    };
    let id = LayerId(Uuid::now_v7());
    let op = Op::AddLayer {
        id: Some(Ref::from(id)),
        page: Ref::from(page.id),
        parent: holder.map(|holder| Ref::from(holder.id)),
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

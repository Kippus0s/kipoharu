//! The ops a run writes: each block's clean-up, or the area's raster and its
//! mask, with the C2PA action recording what made them.

use anyhow::{Result, anyhow};
use image::{DynamicImage, RgbImage, RgbaImage, imageops};
use koharu_scene::{Content, LayerId, MaskMode, Op, PageId, Project, Rect, Ref, Session, Tile};
use serde_json::{Value, json};

use super::{
    Cleanup, Target, Work,
    windows::{self, Window},
};
use crate::stages::{action, writable};

/// The ops for `work`; `made` is the model's action, `name` the area's layer
/// name.
pub(super) fn ops(
    session: &Session,
    page: PageId,
    work: Work,
    made: &Value,
    name: &str,
) -> Result<Vec<Op>> {
    let project = session.project();
    let Work {
        image,
        target,
        windows,
        ..
    } = work;
    match target {
        Target::Blocks(cleanups) => Ok(cleanups
            .iter()
            .filter(|cleanup| current(&project, cleanup))
            .flat_map(|cleanup| cleaned(cleanup, &image, &windows, made))
            .collect()),
        Target::Area { rect, mask, raster } => {
            // An area is one region, so an edit model drew it in one window.
            let drawn = match windows.as_slice() {
                [(window, drawn)] => (window.rect, opaque(drawn)),
                _ => (rect, rgba(&image, rect, |_, _| 255)),
            };
            let limit = (rect, mask.into_raw());
            if let Some(id) = raster {
                if let Some(ops) = reused(session, page, id, made, &drawn, &limit)? {
                    return Ok(ops);
                }
            }
            layer(&project, page, name, made, drawn, limit)
        }
    }
}

/// Whether the block still holds the clean-up the run read, as a block
/// detected again since holds another, and neither is locked now.
fn current(project: &Project, cleanup: &Cleanup) -> bool {
    writable(project, cleanup.block)
        .is_some_and(|block| block.cleanup.id == cleanup.cleanup && !block.cleanup.locked)
}

/// The ops replacing a block's clean-up and recording what made it.
fn cleaned(
    cleanup: &Cleanup,
    image: &RgbImage,
    windows: &[(Window, RgbImage)],
    made: &Value,
) -> Vec<Op> {
    let mut ops = cleared(cleanup);
    ops.push(pixels(cleanup, image, windows));
    let made = match cleanup.fill {
        Some(fill) => action(
            "c2pa.created",
            "Koharu",
            Some(json!({"fill": fill.to_string()})),
        ),
        None => made.clone(),
    };
    ops.push(Op::SetAttribute {
        layer: cleanup.block.into(),
        namespace: "c2pa".to_owned(),
        key: "inpainting".to_owned(),
        value: Some(made),
    });
    ops
}

/// The ops clearing every tile the clean-up holds now.
fn cleared(cleanup: &Cleanup) -> Vec<Op> {
    let size = Tile::SIZE;
    cleanup
        .old
        .tiles
        .iter()
        .map(|tile| Op::WritePixels {
            layer: cleanup.cleanup.into(),
            rect: Rect {
                x: tile.x * size as i32 + cleanup.old.offset.x,
                y: tile.y * size as i32 + cleanup.old.offset.y,
                width: size,
                height: size,
            },
            pixels: vec![0; (size * size * 4) as usize],
        })
        .collect()
}

/// The clean-up's new pixels: an edit model's window around the lettering,
/// or the fill under its grown mask.
fn pixels(cleanup: &Cleanup, image: &RgbImage, windows: &[(Window, RgbImage)]) -> Op {
    let rect = cleanup.rect;
    let window = cleanup.region.and_then(|region| {
        windows
            .iter()
            .find(|(window, _)| window.regions.contains(&region))
    });
    let (rect, pixels) = match window {
        Some((Window { rect: window, .. }, drawn)) => {
            let kept = windows::around(rect, *window);
            let cut = imageops::crop_imm(
                drawn,
                (kept.x - window.x) as u32,
                (kept.y - window.y) as u32,
                kept.width,
                kept.height,
            )
            .to_image();
            (kept, opaque(&cut))
        }
        None => (
            rect,
            rgba(image, rect, |x, y| {
                cleanup
                    .mask
                    .get_pixel(x - rect.x as u32, y - rect.y as u32)
                    .0[0]
            }),
        ),
    };
    Op::WritePixels {
        layer: cleanup.cleanup.into(),
        rect,
        pixels,
    }
}

/// Applies another Remove stroke to the selected raster, including a Brush raster.
/// The new result is clipped to the stroke, so earlier paint and cleanup outside
/// it survive. A Remove raster already has an additive child mask: extend that
/// mask instead of creating another. With a Brush raster, keep it unmasked.
/// All writes form one undoable edit, just as a new inpainting raster does.
fn reused(
    session: &Session,
    page: PageId,
    id: LayerId,
    made: &Value,
    drawn: &(Rect, Vec<u8>),
    limit: &(Rect, Vec<u8>),
) -> Result<Option<Vec<Op>>> {
    let project = session.project();
    let Ok((owner, _)) = project.parent(id) else {
        return Ok(None);
    };
    if owner.id != page {
        return Ok(None);
    }
    let Ok(layer) = project.layer(id) else {
        return Ok(None);
    };
    let Content::Raster(raster) = &layer.content else {
        return Ok(None);
    };
    if !layer.visible || layer.locked {
        return Ok(None);
    }

    // A raster with other mask modes cannot be safely extended by a union;
    // make a new result rather than alter the meaning of its existing masks.
    let masks: Vec<_> = layer.children.iter().filter_map(|child| {
        match &child.content {
            Content::Mask(mask) if child.visible => Some((child, mask)),
            _ => None,
        }
    }).collect();
    if masks.iter().any(|(child, mask)| child.locked || mask.mode != MaskMode::Add) {
        return Ok(None);
    }
    let (rect, coverage) = limit;
    let own = Rect {
        x: rect.x - raster.offset.x,
        y: rect.y - raster.offset.y,
        ..*rect
    };
    let mut existing = RgbaImage::from_raw(rect.width, rect.height, session.pixels(id, own)?)
        .ok_or_else(|| anyhow!("the selected raster's pixels are RGBA"))?;
    let (source_rect, pixels) = drawn;
    let source = RgbaImage::from_raw(source_rect.width, source_rect.height, pixels.clone())
        .ok_or_else(|| anyhow!("the inpainted pixels are RGBA"))?;

    // Like the Brush tool, composite only at the covered pixels, not over the
    // entire model context window (which can be much larger than the stroke).
    let mut overlay = RgbaImage::new(rect.width, rect.height);
    for y in 0..rect.height {
        for x in 0..rect.width {
            let covered = coverage[(y * rect.width + x) as usize];
            if covered == 0 {
                continue;
            }
            let (px, py) = (rect.x + x as i32, rect.y + y as i32);
            if px < source_rect.x || py < source_rect.y {
                continue;
            }
            let (sx, sy) = ((px - source_rect.x) as u32, (py - source_rect.y) as u32);
            if sx >= source_rect.width || sy >= source_rect.height {
                continue;
            }
            let mut pixel = *source.get_pixel(sx, sy);
            pixel.0[3] = ((u16::from(pixel.0[3]) * u16::from(covered)) / 255) as u8;
            overlay.put_pixel(x, y, pixel);
        }
    }
    imageops::overlay(&mut existing, &overlay, 0, 0);
    let mut ops = vec![Op::WritePixels {
        layer: Ref::from(id),
        rect: own,
        pixels: existing.into_raw(),
    }];

    // A Remove-created raster shows only where its mask is white. Expand it
    // for the new stroke; without this, the new generated pixels stay hidden.
    for (child, mask) in masks {
        let own = Rect {
            x: rect.x - mask.offset.x,
            y: rect.y - mask.offset.y,
            ..*rect
        };
        let mut pixels = session.pixels(child.id, own)?;
        for (old, &new) in pixels.iter_mut().zip(coverage) {
            *old = (*old).max(new);
        }
        ops.push(Op::WritePixels {
            layer: Ref::from(child.id),
            rect: own,
            pixels,
        });
    }
    ops.push(Op::SetAttribute {
        layer: Ref::from(id),
        namespace: "c2pa".to_owned(),
        key: "inpainting".to_owned(),
        value: Some(made.clone()),
    });
    Ok(Some(ops))
}

/// The area's new raster, named after its prompt, below the top lettering,
/// with `drawn` as its pixels and `limit`, the area, as its mask.
fn layer(
    project: &Project,
    page: PageId,
    name: &str,
    made: &Value,
    drawn: (Rect, Vec<u8>),
    limit: (Rect, Vec<u8>),
) -> Result<Vec<Op>> {
    let Ok(at) = project.page(page) else {
        return Ok(Vec::new());
    };
    let label = |label: &str| Ref::try_from(label.to_owned()).map_err(anyhow::Error::msg);
    Ok(vec![
        serde_json::from_value(json!({
            "op": "add_layer",
            "id": "$inpainted",
            "page": page,
            "index": at.raster_insert_index(),
            "layer": {"kind": "raster", "name": name, "attributes": {
                "c2pa": {"inpainting": made},
            }},
        }))?,
        serde_json::from_value(json!({
            "op": "add_layer",
            "id": "$limit",
            "page": page,
            "parent": "$inpainted",
            "index": 0,
            "layer": {"kind": "mask"},
        }))?,
        Op::WritePixels {
            layer: label("$inpainted")?,
            rect: drawn.0,
            pixels: drawn.1,
        },
        Op::WritePixels {
            layer: label("$limit")?,
            rect: limit.0,
            pixels: limit.1,
        },
    ])
}

/// All of `image` as opaque RGBA.
fn opaque(image: &RgbImage) -> Vec<u8> {
    DynamicImage::ImageRgb8(image.clone())
        .into_rgba8()
        .into_raw()
}

/// `image`'s pixels in `rect` as RGBA, each with the alpha `alpha` gives.
fn rgba(image: &RgbImage, rect: Rect, alpha: impl Fn(u32, u32) -> u8) -> Vec<u8> {
    let (x0, y0) = (rect.x as u32, rect.y as u32);
    (y0..y0 + rect.height)
        .flat_map(|y| (x0..x0 + rect.width).map(move |x| (x, y)))
        .flat_map(|(x, y)| {
            let [r, g, b] = image.get_pixel(x, y).0;
            [r, g, b, alpha(x, y)]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use koharu_scene::{Edit, Session};

    use super::*;

    /// Removing on a selected Brush raster changes only the covered pixels,
    /// adds no mask, and stays one independently undoable edit.
    #[test]
    fn removing_on_a_brush_raster_reuses_its_pixels_without_a_mask() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let mut session = Session::create(dir.path().join("brush.khr"))?;
        let setup: Vec<Op> = [
            json!({"op": "add_page", "id": "$page", "index": 0,
                "size": {"width": 4, "height": 2}}),
            json!({"op": "add_layer", "id": "$paint", "page": "$page", "index": 0,
                "layer": {"kind": "raster"}}),
            json!({"op": "write_pixels", "layer": "$paint",
                "rect": {"x": 1, "y": 0, "width": 2, "height": 1},
                "pixels": [255, 0, 0, 255, 255, 0, 0, 255]}),
        ].into_iter().map(serde_json::from_value).collect::<std::result::Result<_, _>>()?;
        let change = session.apply(Edit { label: "paint".to_owned(), ops: setup })?;
        let page = change.pages.added[0];
        let raster = change.created.iter().find_map(|item| match item {
            koharu_scene::Created::Layer { id, .. } => Some(*id),
            _ => None,
        }).expect("created the Brush raster");
        let rect = Rect { x: 1, y: 0, width: 2, height: 1 };
        let blue = vec![0, 0, 255, 255].repeat(2);
        let ops = reused(&session, page, raster, &json!({}), &(rect, blue),
            &(rect, vec![255, 0]))?.expect("reused the Brush raster");
        assert_eq!(ops.len(), 2, "write pixels and provenance, without a mask");
        assert!(ops.iter().all(|op| !matches!(op, Op::AddLayer { .. })));
        session.apply(Edit { label: "remove".to_owned(), ops })?;
        let painted = session.pixels(raster, rect)?;
        assert_eq!(painted, [0, 0, 255, 255, 255, 0, 0, 255]);
        assert!(session.project().layer(raster)?.children.is_empty());
        session.undo()?;
        assert_eq!(session.pixels(raster, rect)?, vec![255, 0, 0, 255].repeat(2));
        session.redo()?;
        assert_eq!(session.pixels(raster, rect)?, painted);
        Ok(())
    }

    /// A selected Remove raster already owns an additive mask. The next
    /// removal updates that mask rather than adding another layer or mask.
    #[test]
    fn removing_on_a_remove_raster_extends_its_existing_mask() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let mut session = Session::create(dir.path().join("masked.khr"))?;
        let setup: Vec<Op> = [
            json!({"op": "add_page", "id": "$page", "index": 0,
                "size": {"width": 3, "height": 1}}),
            json!({"op": "add_layer", "id": "$raster", "page": "$page", "index": 0,
                "layer": {"kind": "raster"}}),
            json!({"op": "add_layer", "id": "$mask", "page": "$page", "index": 0,
                "parent": "$raster", "layer": {"kind": "mask"}}),
            json!({"op": "write_pixels", "layer": "$raster",
                "rect": {"x": 0, "y": 0, "width": 3, "height": 1},
                "pixels": [255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255]}),
            json!({"op": "write_pixels", "layer": "$mask",
                "rect": {"x": 0, "y": 0, "width": 3, "height": 1},
                "pixels": [255, 0, 0]}),
        ].into_iter().map(serde_json::from_value).collect::<std::result::Result<_, _>>()?;
        let change = session.apply(Edit { label: "remove".to_owned(), ops: setup })?;
        let page = change.pages.added[0];
        let id = |label: &str| change.created.iter().find_map(|item| match item {
            koharu_scene::Created::Layer { id, label: Some(found) } if found == label => Some(*id),
            _ => None,
        }).expect("created requested layer");
        let (raster, mask) = (id("raster"), id("mask"));
        let rect = Rect { x: 0, y: 0, width: 3, height: 1 };
        let blue = vec![0, 0, 255, 255].repeat(3);
        let ops = reused(&session, page, raster, &json!({}), &(rect, blue),
            &(rect, vec![0, 255, 0]))?.expect("reused the Remove raster");
        assert_eq!(ops.len(), 3, "raster write, mask write, provenance");
        session.apply(Edit { label: "remove".to_owned(), ops })?;
        assert_eq!(session.pixels(mask, rect)?, vec![255, 255, 0]);
        assert_eq!(session.pixels(raster, rect)?,
            [255, 0, 0, 255, 0, 0, 255, 255, 255, 0, 0, 255]);
        assert_eq!(session.project().page(page)?.layers.len(), 1);
        session.undo()?;
        assert_eq!(session.pixels(mask, rect)?, vec![255, 0, 0]);
        session.redo()?;
        assert_eq!(session.pixels(mask, rect)?, vec![255, 255, 0]);
        Ok(())
    }

    /// A Remove stroke creates its raster under the top text and translated
    /// block, but keeps manually arranged lettering below other layers.
    #[test]
    fn an_area_raster_goes_below_top_lettering() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let mut session = Session::create(dir.path().join("test.khr"))?;
        let changes = session.apply(Edit {
            label: "test".to_owned(),
            ops: [
                json!({"op": "add_page", "id": "$page", "index": 0,
                    "size": {"width": 100, "height": 100}}),
                json!({"op": "add_layer", "page": "$page", "index": 0,
                    "layer": {"kind": "text", "content": "Lower text"}}),
                json!({"op": "add_layer", "page": "$page", "index": 1,
                    "layer": {"kind": "raster"}}),
                json!({"op": "add_layer", "page": "$page", "index": 2,
                    "layer": {"kind": "text", "content": "Top text"}}),
                json!({"op": "add_layer", "page": "$page", "index": 3,
                    "layer": {"kind": "block", "source": {},
                        "translation": {"content": "Translated text"}}}),
            ]
            .into_iter()
            .map(serde_json::from_value)
            .collect::<std::result::Result<Vec<Op>, _>>()?,
        })?;
        let page = changes.pages.added[0];
        let before = session.project().page(page)?.layers.clone();
        let rect = Rect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        };
        let ops = layer(
            session.project(),
            page,
            "remove",
            &json!({}),
            (rect, vec![255; 4]),
            (rect, vec![255]),
        )?;
        let Op::AddLayer { index, parent, .. } = &ops[0] else {
            panic!("the first operation should add the result raster");
        };
        assert_eq!(*index, 2);
        assert!(parent.is_none());
        assert_eq!(ops.len(), 4, "the raster, mask and pixel writes remain");
        session.apply(Edit {
            label: "remove".to_owned(),
            ops,
        })?;
        let after = &session.project().page(page)?.layers;
        assert_eq!(after.len(), before.len() + 1);
        assert_eq!(after[..2], before[..2]);
        assert_eq!(after[3..], before[2..]);
        assert!(matches!(&after[2].content, Content::Raster(_)));
        assert!(matches!(&after[2].children[0].content, Content::Mask(_)));
        Ok(())
    }
}

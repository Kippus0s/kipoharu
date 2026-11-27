//! Pictures, imported files, and page backgrounds.

mod common;
use std::io::Cursor;

use common::*;
use koharu_scene::{
    Background, Blend, Content, Error, Hash, LayerId, Op, PageId, PixelFormat, Rect, Session, Size,
};
use serde_json::json;

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    image::RgbaImage::from_pixel(width, height, image::Rgba([10, 20, 30, 255]))
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .expect("a PNG");
    bytes
}

fn paint(
    session: &mut Session,
    layer: LayerId,
    rect: Rect,
    pixels: Vec<u8>,
) -> koharu_scene::Result<()> {
    session
        .apply(edit(vec![Op::WritePixels {
            layer: layer.into(),
            rect,
            pixels,
        }]))
        .map(|_| ())
}

fn rect(x: i32, y: i32, width: u32, height: u32) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn positions(session: &Session, layer: LayerId) -> Vec<(i32, i32)> {
    session
        .tiles(layer, None)
        .expect("tiles")
        .tiles
        .iter()
        .map(|tile| (tile.x, tile.y))
        .collect()
}

fn add_file(session: &mut Session, bytes: Vec<u8>) -> Hash {
    let hash = Hash::of(&bytes);
    session
        .apply(edit(vec![Op::AddFile { bytes }]))
        .expect("a file");
    hash
}

#[test]
fn tiles_cover_what_was_painted() {
    let (_dir, mut session, page) = session();
    let raster = add(&mut session, page, None, json!({"kind": "raster"}));
    paint(
        &mut session,
        raster,
        rect(30, -10, 100, 70),
        [255, 0, 0, 255].repeat(100 * 70),
    )
    .expect("paint");
    assert_eq!(
        positions(&session, raster),
        [(0, -1), (0, 0), (1, -1), (1, 0), (2, -1), (2, 0)]
    );

    // The tile hash follows the pixels, without decoding them, and undo brings
    // them back.
    let before = session.tiles(raster, None).expect("tiles");
    let before_hash = session.tiles_hash(raster).expect("a hash");
    paint(
        &mut session,
        raster,
        rect(40, 0, 2, 2),
        [0, 255, 0, 255].repeat(4),
    )
    .expect("paint");
    let tiles = session
        .tiles(raster, Some(rect(40, 0, 1, 1)))
        .expect("tiles");
    assert_eq!(tiles.tiles.len(), 1);
    assert_eq!(&tiles.tiles[0].pixels[40 * 4..41 * 4], &[0, 255, 0, 255]);
    assert_ne!(session.tiles_hash(raster).expect("a hash"), before_hash);
    session.undo().expect("undo");
    assert_eq!(session.tiles(raster, None).expect("tiles"), before);
    assert_eq!(session.tiles_hash(raster).expect("a hash"), before_hash);

    // Transparent pixels drop their tiles; a duplicate copies tiles.
    paint(
        &mut session,
        raster,
        rect(0, -64, 192, 128),
        vec![0; 192 * 128 * 4],
    )
    .expect("paint");
    assert!(positions(&session, raster).is_empty());
    paint(&mut session, raster, rect(0, 0, 1, 1), vec![9, 9, 9, 255]).expect("paint");
    let change = session
        .apply(edit(vec![Op::DuplicateLayer {
            layer: raster.into(),
            id: None,
            index: None,
        }]))
        .expect("a duplicate");
    let copy = created_layer(&change, 0);
    assert_eq!(positions(&session, copy), [(0, 0)]);
    assert_eq!(
        session.project().page(page).expect("the page").layers[1].id,
        copy,
        "the copy sits above"
    );

    let error = paint(&mut session, raster, rect(0, 0, 2, 2), vec![0; 3])
        .expect_err("the length must match");
    assert!(matches!(error, Error::Invalid { .. }), "{error}");
}

#[test]
fn a_page_made_from_a_file_takes_its_size() {
    let (_dir, path) = path();
    let mut session = Session::create(&path).expect("a project");
    let file = add_file(&mut session, png(40, 30));
    let page: PageId = apply(
        &mut session,
        vec![json!({"op": "add_page", "index": 0, "background": file})],
    )
    .expect("a page")
    .pages
    .added[0];
    let made = session.project().page(page).expect("the page").clone();
    assert_eq!(
        made.size,
        Size {
            width: 40,
            height: 30
        }
    );
    assert_eq!(
        made.background,
        Some(Background {
            file,
            visible: true
        })
    );
    assert_eq!(session.file(&file).expect("the file"), png(40, 30));

    // Hiding it is a background write, and an image layer names a file too.
    session
        .apply(edit(vec![Op::SetBackground {
            page: page.into(),
            background: Some(Background {
                file,
                visible: false,
            }),
        }]))
        .expect("hidden");
    add(
        &mut session,
        page,
        None,
        json!({"kind": "image", "file": file}),
    );
    let before = session.project();
    drop(session);
    assert_eq!(Session::open(&path).expect("it opens").project(), before);
}

#[test]
fn a_background_must_be_an_image_of_the_pages_size() {
    let (_dir, mut session, page) = session();
    let other = add_file(&mut session, png(41, 30));
    let junk = add_file(&mut session, b"not an image".to_vec());
    let before = session.project();
    for ops in [
        vec![json!({"op": "add_page", "index": 0, "background": junk})],
        vec![json!({"op": "add_page", "index": 0})],
        vec![
            json!({"op": "add_page", "index": 0, "size": {"width": 41, "height": 30}, "background": other}),
        ],
        vec![json!({"op": "add_page", "index": 0, "background": Hash::of(b"never added")})],
        vec![
            json!({"op": "set_background", "page": page, "background": {"file": other, "visible": true}}),
        ],
        vec![
            json!({"op": "add_layer", "page": page, "index": 0, "layer": {"kind": "image", "file": Hash::of(b"never added")}}),
        ],
    ] {
        let error = apply(&mut session, ops.clone()).expect_err("refused");
        assert!(matches!(error, Error::Invalid { .. }), "{ops:?}: {error}");
        assert_eq!(session.project(), before, "{ops:?}");
    }
    session
        .apply(edit(vec![Op::SetBackground {
            page: page.into(),
            background: None,
        }]))
        .expect("none is fine");
}

#[test]
fn a_blocks_mask_and_cleanup_hold_its_pixels() {
    let (_dir, mut session, page) = session();
    let block = add(&mut session, page, None, block(0.0));
    let (mask, cleanup) = parts(&session, block);
    paint(&mut session, mask, rect(1, 2, 2, 1), vec![255, 128]).expect("a mask");
    paint(&mut session, cleanup, rect(1, 2, 1, 1), vec![1, 2, 3, 255]).expect("a clean-up");
    let painted = session.tiles(mask, None).expect("tiles");
    assert_eq!(
        (painted.format, painted.offset),
        (PixelFormat::Gray8, Default::default())
    );
    assert_eq!(
        &painted.tiles[0].pixels[2 * 64 + 1..2 * 64 + 3],
        &[255, 128]
    );
    let painted = session.tiles(cleanup, None).expect("tiles");
    assert_eq!(painted.format, PixelFormat::Rgba8);
    assert_eq!(
        &painted.tiles[0].pixels[(2 * 64 + 1) * 4..(2 * 64 + 2) * 4],
        &[1, 2, 3, 255]
    );

    // A block itself, like a group, has no pixels.
    let group = add(&mut session, page, None, json!({"kind": "group"}));
    for layer in [block, group] {
        let error =
            paint(&mut session, layer, rect(0, 0, 1, 1), vec![1; 4]).expect_err("no pixels");
        assert!(matches!(error, Error::Invalid { .. }), "{error}");
        assert!(matches!(
            session.tiles(layer, None),
            Err(Error::Rejected(_))
        ));
    }
}

#[test]
fn direct_text_duplicates_do_not_copy_masks_or_cleanup() {
    let (_dir, mut session, page) = session();
    let block = add(
        &mut session,
        page,
        None,
        json!({"kind": "block", "source": {"content": "原文"},
            "translation": {"content": [{"insert": "Hello", "attributes": {"weight": 700}}],
                "style": {"chars": {"size": 20}},
                "frame": {"box": {"bounds": {"x": 3, "y": 4, "width": 30, "height": 20}}},
                "transform": [[1.0, 0.0, 5.0], [0.0, 1.0, 6.0]]}}),
    );
    let (mask, cleanup) = parts(&session, block);
    paint(&mut session, mask, rect(0, 0, 1, 1), vec![128]).expect("mask");
    paint(&mut session, cleanup, rect(0, 0, 1, 1), vec![255; 4]).expect("cleanup");
    let plain = add(
        &mut session,
        page,
        None,
        json!({"kind": "text", "content": "Hi"}),
    );
    let plain_mask = add(&mut session, page, Some(plain), json!({"kind": "mask"}));
    paint(&mut session, plain_mask, rect(0, 0, 1, 1), vec![128]).expect("mask");
    let untranslated = add(
        &mut session,
        page,
        None,
        json!({"kind": "block", "source": {"content": "原文", "frame": {"box": {
            "bounds": {"x": 1, "y": 2, "width": 10, "height": 12}}}}}),
    );
    let before = session.project();
    let painted = [mask, cleanup, plain_mask].map(|id| session.tiles(id, None).expect("tiles"));
    let change = apply(
        &mut session,
        vec![
            json!({"op": "duplicate_layer", "layer": block, "id": "$copy"}),
            json!({"op": "duplicate_layer", "layer": plain, "id": "$plain"}),
            json!({"op": "duplicate_layer", "layer": untranslated, "id": "$empty"}),
        ],
    )
    .expect("duplicates");
    assert_eq!(change.created.len(), 3, "no new mask or cleanup ids");
    let copy = created_layer(&change, 0);
    let plain_copy = created_layer(&change, 1);
    let empty_copy = created_layer(&change, 2);
    let original = layer(&session, block);
    let Content::Block(original_block) = &original.content else {
        panic!("the original stays a block")
    };
    let copied = layer(&session, copy);
    assert_eq!(copied.blend, Blend::Normal);
    assert_eq!(
        copied.content,
        Content::Text(Box::new(original_block.translation.clone().unwrap()))
    );
    assert!(copied.children.is_empty());
    let copied_plain = layer(&session, plain_copy);
    assert_eq!(copied_plain.content, layer(&session, plain).content);
    assert!(copied_plain.children.is_empty());
    let Content::Block(source) = layer(&session, untranslated).content else {
        panic!("the original stays a block")
    };
    let mut expected = source.source;
    expected.content = Default::default();
    assert_eq!(
        layer(&session, empty_copy).content,
        Content::Text(Box::new(expected))
    );
    for id in [block, plain, untranslated] {
        assert_eq!(&layer(&session, id), before.layer(id).unwrap());
    }
    for (id, pixels) in [mask, cleanup, plain_mask].into_iter().zip(painted) {
        assert_eq!(session.tiles(id, None).expect("original tiles"), pixels);
    }
    let after = session.project();
    session.undo().expect("undo");
    assert_eq!(session.project(), before);
    session.redo().expect("redo");
    assert_eq!(session.project(), after);
    apply(
        &mut session,
        vec![json!({"op": "set_text", "layer": copy, "text": "Edited"})],
    )
    .expect("edit independent copy");
    assert_eq!(layer(&session, block), original);
}

#[test]
fn removing_a_block_takes_its_cleanup_and_removing_its_translation_keeps_it() {
    let (_dir, mut session, page) = session();
    let block = add(&mut session, page, None, block(0.0));
    let (_, cleanup) = parts(&session, block);
    session
        .apply(edit(vec![Op::SetText {
            layer: block.into(),
            text: "Hi".into(),
        }]))
        .expect("a translation");
    paint(&mut session, cleanup, rect(0, 0, 4, 4), vec![255; 64]).expect("a clean-up");
    let painted = session.tiles(cleanup, None).expect("tiles");
    apply(
        &mut session,
        vec![json!({"op": "update_layer", "layer": block, "patch": {"translation": null}})],
    )
    .expect("no translation");
    assert_eq!(session.tiles(cleanup, None).expect("tiles"), painted);
    session
        .apply(edit(vec![Op::RemoveLayer {
            layer: block.into(),
        }]))
        .expect("a removal");
    assert!(matches!(
        session.tiles(cleanup, None),
        Err(Error::NotFound(_))
    ));
    session.undo().expect("undo").expect("a step");
    assert_eq!(session.tiles(cleanup, None).expect("tiles"), painted);
}

#[test]
fn undoing_a_removal_restores_its_pixels() {
    let (_dir, path) = path();
    let mut session = Session::create(&path).expect("a project");
    let page: PageId = apply(
        &mut session,
        vec![json!({"op": "add_page", "index": 0, "size": {"width": 200, "height": 200}})],
    )
    .expect("a page")
    .pages
    .added[0];
    let balloon = add(
        &mut session,
        page,
        None,
        json!({"kind": "balloon", "outline": outline()}),
    );
    let block = add(&mut session, page, Some(balloon), block(0.0));
    let (mask, cleanup) = parts(&session, block);
    paint(&mut session, mask, rect(0, 0, 70, 2), vec![255; 140]).expect("a mask");
    paint(
        &mut session,
        cleanup,
        rect(0, 0, 2, 70),
        vec![7; 2 * 70 * 4],
    )
    .expect("a clean-up");
    let painted = (
        session.tiles(mask, None).expect("tiles"),
        session.tiles(cleanup, None).expect("tiles"),
    );
    session
        .apply(edit(vec![Op::RemovePage { page: page.into() }]))
        .expect("a removal");
    session.undo().expect("undo").expect("a step");
    let read = |session: &Session| {
        (
            session.tiles(mask, None).expect("tiles"),
            session.tiles(cleanup, None).expect("tiles"),
        )
    };
    assert_eq!(read(&session), painted);
    drop(session);
    let session = Session::open(&path).expect("it opens");
    assert_eq!(read(&session), painted);
}

#[test]
fn pixels_read_back_what_write_pixels_wrote() {
    let (_dir, mut session, page) = session();
    let raster = add(&mut session, page, None, json!({"kind": "raster"}));
    let pixel = |x: i32, y: i32| [x as u8, y as u8, 7, 255];
    let written: Vec<u8> = (-5..5)
        .flat_map(|y| (60..160).flat_map(move |x| pixel(x, y)))
        .collect();
    paint(&mut session, raster, rect(60, -5, 100, 10), written).expect("paint");

    let read = session
        .pixels(raster, rect(50, -5, 120, 10))
        .expect("pixels");
    assert_eq!(read.len(), 120 * 10 * 4);
    for (row, y) in read.as_chunks::<{ 120 * 4 }>().0.iter().zip(-5..5) {
        for (at, x) in row.as_chunks::<4>().0.iter().zip(50..170) {
            let expected = match (60..160).contains(&x) {
                true => pixel(x, y),
                false => [0; 4],
            };
            assert_eq!(*at, expected, "at ({x}, {y})");
        }
    }
}

#[test]
fn history_storage_distinguishes_live_retained_and_abandoned_blobs() {
    let (_dir, mut session, page) = session();
    let raster = add(&mut session, page, None, json!({"kind": "raster"}));
    let area = rect(0, 0, 64, 64);
    paint(&mut session, raster, area, [255, 0, 0, 255].repeat(64 * 64)).unwrap();
    let red = session.history_storage().unwrap();
    assert!(red.live_blob_bytes > 0);
    assert_eq!(red.history_only_blob_bytes, 0);
    assert_eq!(red.unreferenced_blob_bytes, 0);
    paint(&mut session, raster, area, [0, 255, 0, 255].repeat(64 * 64)).unwrap();
    let green = session.history_storage().unwrap();
    assert_eq!(green.history_only_blob_bytes, red.live_blob_bytes);
    session.undo().unwrap();
    let undone = session.history_storage().unwrap();
    assert_eq!(undone.redo_steps, 1);
    assert_eq!(undone.history_only_blob_bytes, green.live_blob_bytes);
    paint(&mut session, raster, area, [0, 0, 255, 255].repeat(64 * 64)).unwrap();
    let branched = session.history_storage().unwrap();
    assert_eq!(branched.redo_steps, 0);
    assert_eq!(branched.history_only_blob_bytes, red.live_blob_bytes);
    assert_eq!(branched.unreferenced_blob_bytes, green.live_blob_bytes);
}

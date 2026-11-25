//! Texts, block translations, and attributes.

mod common;
use common::*;
use koharu_scene::{CharStyle, Content, Error, LayerId, Op, RichText, Session, Span, Text};
use serde_json::json;

fn text(session: &Session, layer: LayerId) -> Text {
    match &common::layer(session, layer).content {
        Content::Text(text) => (**text).clone(),
        Content::Block(block) => block.translation.clone().expect("a translation"),
        other => panic!("{other:?} holds no text"),
    }
}

fn set_text(session: &mut Session, layer: LayerId, text: &str) -> koharu_scene::Result<()> {
    session
        .apply(edit(vec![Op::SetText {
            layer: layer.into(),
            text: text.to_owned(),
        }]))
        .map(|_| ())
}

fn marked(text: &RichText) -> Vec<(&str, Option<u16>)> {
    text.0
        .iter()
        .map(|span| (span.insert.as_str(), span.attributes.weight))
        .collect()
}

#[test]
fn proportional_sizes_scale_each_frame_about_its_centre_in_one_edit() {
    let (_dir, mut session, page) = session();
    let boxed = add(&mut session, page, None, json!({
        "kind": "text", "content": "Hello", "style": {"chars": {"size": 20}},
        "frame": {"box": {"bounds": {"x": 10, "y": 20, "width": 80, "height": 40},
            "fit": "proportional", "anchor": "center"}}
    }));
    let balloon = add(&mut session, page, None, json!({
        "kind": "block", "source": {"content": "original", "frame": {"box": {
            "bounds": {"x": 10, "y": 20, "width": 80, "height": 40}}}},
        "translation": {"content": "Hello", "style": {"chars": {"size": 40}},
            "frame": {"path": {"path": "M10 40 Q50 0 90 40 Q50 80 10 40Z",
                "fit": "proportional", "anchor": "center", "inset": 0.5}}}
    }));
    let fixed = add(&mut session, page, None, json!({
        "kind": "text", "content": "Fixed", "style": {"chars": {"size": 20}},
        "frame": {"box": {"bounds": {"x": 10, "y": 20, "width": 80, "height": 40}, "fit": "none"}}
    }));
    let before = session.project();
    let original = common::layer(&session, balloon);
    let path_before = text(&session, balloon).bounds().expect("a path");
    apply(&mut session, vec![
        json!({"op": "update_layer", "layer": boxed, "patch": {"style": {"chars": {"size": 60}}}}),
        json!({"op": "update_layer", "layer": balloon, "patch": {"translation": {"style": {"chars": {"size": 60}}}}}),
        json!({"op": "update_layer", "layer": fixed, "patch": {"style": {"chars": {"size": 60}}}}),
    ]).expect("size edit");
    let box_after = text(&session, boxed);
    assert_eq!(box_after.style.chars.size, Some(60.0));
    assert_eq!(box_after.bounds(), Some(kurbo::Rect::new(-70.0, -20.0, 170.0, 100.0)));
    assert!(matches!(box_after.frame, koharu_scene::Frame::Box { fit: koharu_scene::Fit::Proportional, .. }));
    let path_after = text(&session, balloon);
    let bounds = path_after.bounds().expect("a path");
    assert_eq!(bounds.center(), path_before.center());
    assert_eq!(bounds.width(), path_before.width() * 1.5);
    assert_eq!(bounds.height(), path_before.height() * 1.5);
    assert_eq!(path_after.style.chars.size, Some(60.0));
    assert!(matches!(path_after.frame, koharu_scene::Frame::Path { fit: koharu_scene::PathFit::Proportional, .. }));
    assert_eq!(text(&session, fixed).bounds(), Some(kurbo::Rect::new(10.0, 20.0, 90.0, 60.0)));
    let changed = common::layer(&session, balloon);
    let (Content::Block(old), Content::Block(new)) = (&original.content, &changed.content) else {
        panic!("a block");
    };
    assert_eq!(old.source, new.source);
    assert_eq!(old.mask, new.mask);
    assert_eq!(old.cleanup, new.cleanup);
    let after = session.project();
    session.undo().expect("undo").expect("a step");
    assert_eq!(session.project(), before);
    session.redo().expect("redo").expect("a step");
    assert_eq!(session.project(), after);
}

#[test]
fn proportional_respects_explicit_frame_edits_and_scales_down_from_default_size() {
    let (_dir, mut session, page) = session();
    let boxed = add(&mut session, page, None, json!({
        "kind": "text", "content": "Hello", "frame": {"box": {"bounds": {
            "x": 0, "y": 0, "width": 80, "height": 40}, "fit": "proportional"}}
    }));
    apply(&mut session, vec![json!({"op": "update_layer", "layer": boxed,
        "patch": {"style": {"chars": {"size": 12}}}})]).expect("half size");
    assert_eq!(text(&session, boxed).bounds(), Some(kurbo::Rect::new(20.0, 10.0, 60.0, 30.0)));
    apply(&mut session, vec![json!({"op": "update_layer", "layer": boxed,
        "patch": {"style": {"chars": {"size": 24}}, "frame": {"box": {"bounds": {
            "x": 0, "y": 0, "width": 100, "height": 60}}}}})]).expect("explicit resize");
    assert_eq!(text(&session, boxed).bounds(), Some(kurbo::Rect::new(0.0, 0.0, 100.0, 60.0)));
}

#[test]
fn set_text_keeps_marks_on_the_characters_that_stayed() {
    let (_dir, mut session, page) = session();
    let layer = add(
        &mut session,
        page,
        None,
        json!({"kind": "text", "content": [
            {"insert": "Hello", "attributes": {"weight": 700}},
            {"insert": " world"},
        ]}),
    );
    set_text(&mut session, layer, "Hello there world").expect("a text");
    assert_eq!(
        marked(&text(&session, layer).content),
        [("Hello", Some(700)), (" there world", None)]
    );
    set_text(&mut session, layer, "Help world").expect("a text");
    assert_eq!(
        marked(&text(&session, layer).content),
        [("Hel", Some(700)), ("p world", None)]
    );

    // The rich editor's spans replace the text whole, joined as stored.
    session
        .apply(edit(vec![Op::SetRichText {
            layer: layer.into(),
            spans: vec![
                Span {
                    insert: "a".into(),
                    attributes: Default::default(),
                },
                Span {
                    insert: "".into(),
                    attributes: Default::default(),
                },
                Span {
                    insert: "b".into(),
                    attributes: Default::default(),
                },
            ],
        }]))
        .expect("spans");
    assert_eq!(marked(&text(&session, layer).content), [("ab", None)]);

    let raster = add(&mut session, page, None, json!({"kind": "raster"}));
    let error = set_text(&mut session, raster, "x").expect_err("a raster has no text");
    assert!(matches!(error, Error::Invalid { .. }), "{error}");
}

#[test]
fn typing_splices_and_marks_rich_text_keeping_the_other_characters_marks() {
    let bold = CharStyle {
        weight: Some(700),
        ..CharStyle::default()
    };
    let text = RichText(vec![
        Span {
            insert: "Hello".to_owned(),
            attributes: bold.clone(),
        },
        Span {
            insert: " world".to_owned(),
            attributes: CharStyle::default(),
        },
    ]);
    // A character typed at the bold word's end takes its marks.
    let typed = text.spliced(5..5, "!", &text.marks_at(5));
    assert_eq!(marked(&typed), [("Hello!", Some(700)), (" world", None)]);
    // Typing over a selection replaces it with the given marks.
    let over = text.spliced(3..8, "p wo", &CharStyle::default());
    assert_eq!(marked(&over), [("Hel", Some(700)), ("p world", None)]);
    // Marking a range splits its spans, and alike neighbours join again.
    let light = CharStyle {
        weight: Some(300),
        ..CharStyle::default()
    };
    let lightened = text.marked(3..8, &light);
    assert_eq!(
        marked(&lightened),
        [("Hel", Some(700)), ("lo wo", Some(300)), ("rld", None)]
    );
    assert_eq!(
        marked(&lightened.marked(3..5, &bold)),
        [("Hello", Some(700)), (" wo", Some(300)), ("rld", None)]
    );
    // At the start a character takes the first one's marks.
    assert_eq!(text.marks_at(0), bold);
    assert_eq!(text.marks_at(6), CharStyle::default());
    assert_eq!(RichText::default().marks_at(0), CharStyle::default());
}

#[test]
fn text_and_duplicates_survive_reopening() {
    let (_dir, path) = path();
    let mut session = Session::create(&path).expect("a project");
    let page = page(&mut session);
    let balloon = add(
        &mut session,
        page,
        None,
        json!({"kind": "balloon", "outline": outline()}),
    );
    let block = add(&mut session, page, Some(balloon), block(0.0));
    set_text(&mut session, block, "Hi").expect("a translation");
    let change = session
        .apply(edit(vec![Op::DuplicateLayer {
            layer: balloon.into(),
            id: None,
            index: None,
        }]))
        .expect("a duplicate");
    let (copy, copied_block) = (created_layer(&change, 0), created_layer(&change, 1));
    assert_eq!(layer(&session, copy).name, layer(&session, balloon).name);
    assert_eq!(text(&session, copied_block).content.text(), "Hi");
    set_text(&mut session, copied_block, "Bye").expect("a translation");
    assert_eq!(
        text(&session, block).content.text(),
        "Hi",
        "the copy is its own"
    );

    let before = session.project();
    drop(session);
    let session = Session::open(&path).expect("it opens");
    assert_eq!(session.project(), before);
    assert_eq!(text(&session, copied_block).content.text(), "Bye");
}

#[test]
fn a_blocks_translation_comes_and_goes_with_its_own_ops() {
    let (_dir, mut session, page) = session();
    let block = add(&mut session, page, None, block(0.0));
    set_text(&mut session, block, "Hello").expect("a translation");
    assert_eq!(
        text(&session, block),
        Text {
            content: RichText::from("Hello"),
            ..Text::default()
        }
    );
    apply(
        &mut session,
        vec![json!({"op": "update_layer", "layer": block, "patch": {"translation": null}})],
    )
    .expect("no translation");
    let Content::Block(gone) = &layer(&session, block).content else {
        panic!("not a block")
    };
    assert_eq!(gone.translation, None);
    session.undo().expect("undo").expect("a step");
    assert_eq!(text(&session, block).content.text(), "Hello");

    let balloon = add(
        &mut session,
        page,
        None,
        json!({"kind": "balloon", "outline": outline()}),
    );
    let error = set_text(&mut session, balloon, "x").expect_err("a balloon has no text");
    assert!(matches!(error, Error::Invalid { .. }), "{error}");

    // Attributes: the last writer wins, and an empty namespace goes.
    let set = |value: serde_json::Value| json!({"op": "set_attribute", "layer": block, "namespace": "c2pa", "key": "ocr", "value": value});
    apply(&mut session, vec![set(json!({"action": "c2pa.created"}))]).expect("an attribute");
    apply(&mut session, vec![set(json!({"action": "c2pa.edited"}))]).expect("an attribute");
    assert_eq!(
        layer(&session, block).attributes["c2pa"]["ocr"],
        json!({"action": "c2pa.edited"})
    );
    apply(&mut session, vec![set(serde_json::Value::Null)]).expect("no attribute");
    assert!(layer(&session, block).attributes.is_empty());

    // A lock above stops text and attribute writes alike.
    apply(
        &mut session,
        vec![json!({"op": "update_layer", "layer": block, "patch": {"locked": true}})],
    )
    .expect("a lock");
    let error = set_text(&mut session, block, "x").expect_err("locked");
    assert!(matches!(error, Error::Locked { .. }), "{error}");
    let error = apply(&mut session, vec![set(json!(1))]).expect_err("locked");
    assert!(matches!(error, Error::Locked { .. }), "{error}");
}

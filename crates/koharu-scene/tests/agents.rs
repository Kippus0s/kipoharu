//! What agents send and read: the patch `update_layer` takes, and a page as
//! `get_page` answers it.

mod common;
use common::*;
use koharu_scene::{Answer, LayerId, Op, Query};
use schemars::generate::SchemaSettings;
use serde_json::json;

/// `update_layer`'s patch schema, inlined as agents get it.
fn patch_schema() -> serde_json::Value {
    let schema = SchemaSettings::draft2020_12()
        .with(|settings| settings.inline_subschemas = true)
        .into_generator()
        .into_root_schema_for::<Op>();
    let variants = schema.get("oneOf").and_then(|variants| variants.as_array());
    variants
        .into_iter()
        .flatten()
        .find(|variant| variant["properties"]["op"]["const"] == "update_layer")
        .expect("update_layer")["properties"]["patch"]
        .clone()
}

#[test]
fn every_null_the_patch_schema_allows_is_a_patch_that_applies() {
    let patch = &patch_schema();
    assert!(patch["properties"].get("attributes").is_none(), "{patch}");
    let variants = patch["anyOf"].as_array().expect("the kinds' variants");
    let blocks = variants
        .iter()
        .find(|variant| variant["properties"].get("translation").is_some())
        .expect("the block's variant");
    let nullable = |properties: &serde_json::Value| -> Vec<String> {
        let properties = properties.as_object().expect("properties");
        properties
            .iter()
            .filter(|(_, schema)| {
                schema["anyOf"]
                    .as_array()
                    .is_some_and(|options| options.iter().any(|option| option["type"] == "null"))
            })
            .map(|(key, _)| key.clone())
            .collect()
    };
    let mut keys = nullable(&patch["properties"]);
    keys.extend(nullable(&blocks["properties"]));
    assert!(keys.contains(&"translation".to_owned()), "{keys:?}");

    let (_dir, mut session, page) = session();
    let layer = add(&mut session, page, None, block(0.0));
    for key in keys {
        let result = apply(
            &mut session,
            vec![json!({"op": "update_layer", "layer": layer, "patch": {key.clone(): null}})],
        );
        assert!(result.is_ok(), "{key}: {result:?}");
    }
}

#[test]
fn agents_patch_a_block_and_read_it_in_its_page() {
    let (_dir, mut session, page) = session();
    let panel = add(
        &mut session,
        page,
        None,
        json!({"kind": "panel", "outline": outline()}),
    );
    let balloon = add(
        &mut session,
        page,
        Some(panel),
        json!({"kind": "balloon", "outline": outline()}),
    );
    add(&mut session, page, Some(balloon), block(0.0));
    let second = add(&mut session, page, Some(balloon), block(10.0));

    apply(
        &mut session,
        vec![json!({
            "op": "update_layer",
            "layer": second,
            "patch": {"translation": {"direction": "vertical"}},
        })],
    )
    .expect("an edit");

    let Answer::Page(answer) = session.query(Query::GetPage { page }).expect("an answer") else {
        panic!("not a page")
    };
    assert_eq!(answer.id, page);
    let value = serde_json::to_value(&answer).expect("an answer serializes");
    let found = |value: &serde_json::Value, id: LayerId| {
        fn search<'v>(layers: &'v serde_json::Value, id: &str) -> Option<&'v serde_json::Value> {
            layers.as_array()?.iter().find_map(|layer| {
                (layer["id"] == id)
                    .then_some(layer)
                    .or_else(|| search(&layer["children"], id))
            })
        }
        search(&value["layers"], &id.to_string()).cloned()
    };
    let second = found(&value, second).expect("the second block, inside its balloon");
    assert_eq!(second["translation"]["direction"], "vertical", "{value}");
    let Answer::Project(project) = session.query(Query::GetProject).expect("an answer") else {
        panic!("not a project")
    };
    assert_eq!(
        (project.version.as_str(), project.pages.len()),
        ("1.0.0", 1)
    );
}

#[test]
fn cached_page_summaries_follow_edits_history_order_and_reopening() {
    use koharu_scene::{PageId, Session};
    let (_dir, mut session, first) = session();
    let second = page(&mut session);
    let a = add(&mut session, first, None, block(0.0));
    let b = add(&mut session, second, None, block(0.0));
    let summaries = |session: &Session| -> Vec<(PageId, usize, usize)> {
        let Answer::Project(project) = session.query(Query::GetProject).expect("summaries") else {
            panic!("not a project");
        };
        project.pages.iter().map(|p| (p.id, p.texts, p.translated)).collect()
    };
    let empty = vec![(first, 1, 0), (second, 1, 0)];
    assert_eq!(summaries(&session), empty);
    apply(&mut session, vec![json!({"op": "set_text", "layer": a, "text": "Hello"})])
        .expect("translation");
    let translated = vec![(first, 1, 1), (second, 1, 0)];
    assert_eq!(summaries(&session), translated);
    apply(&mut session, vec![json!({"op": "set_rich_text", "layer": b, "spans": [
        {"insert": " ", "attributes": {"weight": 700}}, {"insert": "\u{3000}"}
    ]})]).expect("blank spans");
    assert_eq!(summaries(&session), translated);
    apply(&mut session, vec![json!({"op": "update_layer", "layer": a, "patch": {"visible": false}})])
        .expect("hide translation");
    assert_eq!(summaries(&session), translated);
    apply(&mut session, vec![json!({"op": "set_text", "layer": a, "text": ""})])
        .expect("clear translation");
    assert_eq!(summaries(&session), empty);
    session.undo().expect("undo");
    assert_eq!(summaries(&session), translated);
    session.redo().expect("redo");
    assert_eq!(summaries(&session), empty);
    session.undo().expect("undo");
    apply(&mut session, vec![json!({"op": "move_page", "page": second, "index": 0})])
        .expect("reorder");
    let reordered = vec![(second, 1, 0), (first, 1, 1)];
    assert_eq!(summaries(&session), reordered);
    apply(&mut session, vec![json!({"op": "remove_page", "page": first})]).expect("remove");
    assert_eq!(summaries(&session), vec![(second, 1, 0)]);
    session.undo().expect("restore page");
    assert_eq!(summaries(&session), reordered);
    let path = session.path().to_owned();
    drop(session);
    let reopened = Session::open(path).expect("reopen");
    assert_eq!(summaries(&reopened), reordered);
}

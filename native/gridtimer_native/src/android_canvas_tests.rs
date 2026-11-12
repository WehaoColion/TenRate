// v2.13.5.5 - 客户金额分精度、旧卡片兼容与失败回滚。
// v2.22.49.6 - Verify independent boards reopen correctly and failed creation preserves saved state.
// v2.22.49.5 - Verify compact replies preserve durable state and graph edits.
// v2.22.49.4 - Exercise real canvas transactions, recovery and edit barriers.
use super::*;
use std::sync::{Mutex, MutexGuard};

#[test]
fn view_transactions_share_card_text_and_inactive_boards() {
    let mut store = Store::initial();
    let mut canvas = Canvas::new("large");
    for i in 0..1000 {
        canvas.nodes.push(Node {
            id: format!("n{i}"),
            kind: "note".into(),
            page_id: String::new(),
            text: "content".repeat(100),
            x: 0.0,
            y: 0.0,
            width: 256.0,
            height: 168.0,
            color: "blue".into(),
            paid_cents: None,
            pending_cents: None,
        });
    }
    store.canvases.push(canvas.into());
    let mut next = store.clone();
    edit(
        &mut next.canvases[1],
        &json!({"op":"view","x":30,"y":20,"zoom":0.8}),
    )
    .unwrap();
    assert!(Arc::ptr_eq(&store.canvases[0].0, &next.canvases[0].0));
    assert!(Arc::ptr_eq(
        &store.canvases[1].nodes.0,
        &next.canvases[1].nodes.0
    ));
    assert!(Arc::ptr_eq(
        &store.canvases[1].edges.0,
        &next.canvases[1].edges.0
    ));
    assert_eq!(store.canvases[1].center_x, 0.0);
    assert_eq!(next.canvases[1].center_x, 30.0);
}

#[test]
fn client_payment_money_contract_rejects_fractional_or_negative_invalid_values() {
    assert_eq!(client_money_yuan("").unwrap(), None);
    assert_eq!(client_money_yuan("0.01").unwrap(), Some(1));
    assert_eq!(client_money_yuan("15.2").unwrap(), Some(1520));
    assert_eq!(client_money_yuan("999999999.99").unwrap(), Some(MAX_CLIENT_MONEY_CENTS));
    for invalid in ["-1", "+1", ".5", "01", "1.234", "1e3", "NaN",
                    "1,000", "1000000000", "999999999.999", "1..2", "￥1"] {
        assert!(client_money_yuan(invalid).is_err(), "{invalid} should fail");
    }
}
#[test]
fn customer_money_survives_legacy_reopen_comment_edit_and_failed_transaction() {
    let mut fixture = Fixture::new();
    let opened = fixture.open("isolated-client-money");
    let blank = apply(&opened, json!({"op":"addClient","name":"旧客户","details":"备注"}));
    let client_id = id(&blank, 0);
    assert!(blank["canvas"]["nodes"][0].get("paidCents").is_none());
    let paid = apply(&blank, json!({
        "op":"updateClient","id":client_id,"name":"旧客户","details":"备注",
        "paidYuan":"1500.25","pendingYuan":"500.70"
    }));
    assert_eq!(paid["canvas"]["nodes"][0]["paidCents"], 150025);
    assert_eq!(paid["canvas"]["nodes"][0]["pendingCents"], 50070);
    let comment = apply(&paid, json!({"op":"updateClient","id":client_id,"name":"旧客户","details":"新备注"}));
    assert_eq!(comment["canvas"]["nodes"][0]["paidCents"], 150025);
    let undone = apply(&comment, json!({"op":"undo"}));
    assert_eq!(undone["canvas"]["nodes"][0]["paidCents"], 150025);
    let reopened = fixture.open("isolated-client-money");
    assert_eq!(reopened["canvas"]["nodes"][0]["pendingCents"], 50070);
    let file = scoped_path(&fixture.root, "isolated-client-money").unwrap();
    let prior = fs::read(&file).unwrap();
    for invalid in [
        json!({"op":"updateClient","id":client_id,"name":"旧客户","details":"x","paidYuan":"1.234"}),
        json!({"op":"updateClient","id":client_id,"name":"旧客户","details":"x","pendingYuan":-1}),
    ] {
        assert!(command(reopened["token"].as_str().unwrap(), &invalid.to_string()).is_err());
        assert_eq!(fs::read(&file).unwrap(), prior);
    }
    let cleared = apply(&reopened, json!({
        "op":"updateClient","id":client_id,"name":"旧客户","details":"新备注","pendingYuan":""
    }));
    assert!(cleared["canvas"]["nodes"][0].get("pendingCents").is_none());
    assert_eq!(cleared["canvas"]["nodes"][0]["paidCents"], 150025);
    let note = apply(&cleared, json!({"op":"addNote","text":"普通便签"}));
    assert!(note["canvas"]["nodes"][1].get("paidCents").is_none());
}

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn one_percent_view_persists_without_allowing_out_of_range_zoom() {
    let mut fixture = Fixture::new();
    let current = fixture.open("zoom-one-percent");
    let saved = apply(&current, json!({"op":"view","x":4200,"y":-3200,"zoom":0.01}));
    assert_eq!(saved["canvas"]["zoom"], json!(0.01_f32));
    let reopened = fixture.open("zoom-one-percent");
    assert_eq!(reopened["canvas"]["zoom"], saved["canvas"]["zoom"]);
    assert_eq!(reopened["canvas"]["centerX"], 4200.0);
    let path = scoped_path(&fixture.root, "zoom-one-percent").unwrap();
    let bytes = fs::read(&path).unwrap();
    for zoom in [0.0, 0.0099, 2.51] {
        assert!(command(saved["token"].as_str().unwrap(),
            &json!({"op":"view","x":0,"y":0,"zoom":zoom}).to_string()).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn compact_view_replies_keep_graph_and_history_and_persist_before_success() {
    let mut f = Fixture::new();
    let opened: Value = serde_json::from_str(&open_json(&f.root, "compact").unwrap()).unwrap();
    f.tokens.push(opened["token"].as_str().unwrap().into());
    let saved = add(&opened, "retained content");
    let token = saved["token"].as_str().unwrap();
    let encoded = command_json(
        token,
        r#"{"op":"gesture","viewX":20,"viewY":30,"zoom":0.8}"#,
    )
    .unwrap();
    let view: Value = serde_json::from_str(&encoded).unwrap();
    assert!(view.get("canvas").is_none());
    assert_eq!(view["baseRevision"], saved["revision"]);
    assert_eq!(view["revision"], saved["revision"].as_u64().unwrap() + 1);
    assert_eq!(view["viewport"]["id"], saved["canvas"]["id"]);
    assert_eq!(view["canUndo"], true);
    let reopened = f.open("compact");
    assert_eq!(reopened["canvas"]["nodes"], saved["canvas"]["nodes"]);
    assert_eq!(reopened["canvas"]["centerX"], 20.0);
    assert_eq!(reopened["revision"], view["revision"]);
    let undo = apply(&saved, json!({"op":"undo"}));
    assert!(undo["canvas"]["nodes"].as_array().unwrap().is_empty());
}

#[test]
fn encoded_graph_edits_remain_full_and_equivalent_to_the_committed_store() {
    let mut f = Fixture::new();
    let saved = add(&f.open("graph"), "card");
    let token = saved["token"].as_str().unwrap();
    let request =
        json!({"op":"gesture","id":id(&saved,0),"x":5,"y":6,"viewX":20,"viewY":30,"zoom":1});
    let reply: Value =
        serde_json::from_str(&command_json(token, &request.to_string()).unwrap()).unwrap();
    assert!(reply.get("viewport").is_none());
    assert_eq!(reply["canvas"]["nodes"][0]["x"], 5.0);
    let reopened = f.open("graph");
    assert_eq!(reply["canvas"], reopened["canvas"]);
    let map = sessions().lock().unwrap();
    assert_eq!(reply, response(token, &map[token]));
}

#[test]
fn compact_replies_cannot_acknowledge_invalid_or_stale_writes() {
    let mut f = Fixture::new();
    let a = f.open("stale-compact");
    let b = f.open("stale-compact");
    let token = a["token"].as_str().unwrap();
    assert!(command_json(token, r#"{"op":"view","x":0,"y":0,"zoom":0}"#).is_err());
    assert_eq!(f.open("stale-compact")["revision"], 0);
    let newer = add(&b, "newer");
    assert!(command_json(token, r#"{"op":"view","x":20,"y":0,"zoom":1}"#).is_err());
    assert_eq!(f.open("stale-compact")["canvas"], newer["canvas"]);
}

struct Fixture {
    root: PathBuf,
    tokens: Vec<String>,
    _lock: MutexGuard<'static, ()>,
}
impl Fixture {
    fn new() -> Self {
        let lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        Self {
            root: std::env::temp_dir().join(format!("tenfold-canvas-{}", identifier())),
            tokens: vec![],
            _lock: lock,
        }
    }
    fn open(&mut self, account: &str) -> Value {
        let result = open(&self.root, account).unwrap();
        self.tokens.push(result["token"].as_str().unwrap().into());
        result
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for token in &self.tokens {
            close(token);
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn apply(value: &Value, request: Value) -> Value {
    command(value["token"].as_str().unwrap(), &request.to_string()).unwrap()
}
fn add(value: &Value, text: &str) -> Value {
    apply(value, json!({"op":"addNote","text":text}))
}
fn id(value: &Value, index: usize) -> &str {
    value["canvas"]["nodes"][index]["id"].as_str().unwrap()
}

#[test]
fn account_scope_survives_reopen_and_cannot_traverse_directories() {
    let mut f = Fixture::new();
    let a = f.open("../account-a");
    let a = add(&a, "private-a");
    let b = f.open("account-b");
    assert_eq!(b["canvas"]["nodes"].as_array().unwrap().len(), 0);
    close(a["token"].as_str().unwrap());
    let reopened = f.open("../account-a");
    assert_eq!(reopened["canvas"]["nodes"][0]["text"], "private-a");
    assert!(scoped_path(&f.root, "../account-a")
        .unwrap()
        .starts_with(f.root.join("knowledge_canvases")));
    assert!(open(&f.root, "").is_err());
}
#[test]
fn delete_removes_incident_edges_and_undo_restores_the_complete_graph() {
    let mut f = Fixture::new();
    let a = f.open("a");
    let a = add(&a, "first");
    let a = add(&a, "second");
    let first = id(&a, 0).to_string();
    let second = id(&a, 1).to_string();
    let a = apply(&a, json!({"op":"connect","from":first,"to":second}));
    let a = apply(&a, json!({"op":"removeNode","id":first}));
    assert_eq!(a["canvas"]["nodes"].as_array().unwrap().len(), 1);
    assert_eq!(a["canvas"]["edges"], json!([]));
    let a = apply(&a, json!({"op":"undo"}));
    assert_eq!(a["canvas"]["edges"].as_array().unwrap().len(), 1);
    let a = apply(&a, json!({"op":"redo"}));
    assert_eq!(a["canvas"]["edges"], json!([]));
    let disk = read_store(&scoped_path(&f.root, "a").unwrap()).unwrap();
    assert_eq!(disk.canvases[0].nodes.len(), 1);
}
#[test]
fn invalid_commands_never_change_the_durable_revision_or_valid_state() {
    let mut f = Fixture::new();
    let a = f.open("a");
    let a = add(&a, "keep");
    let path = scoped_path(&f.root, "a").unwrap();
    let bytes = fs::read(&path).unwrap();
    for request in [
        json!({"op":"move","id":id(&a,0),"x":1e100,"y":0}),
        json!({"op":"connect","from":id(&a,0),"to":id(&a,0)}),
        json!({"op":"connect","from":id(&a,0),"to":"missing"}),
        json!({"op":"rename","title":" "}),
        json!({"op":"resize","id":id(&a,0),"width":0,"height":120}),
        json!({"op":"switch","id":"missing"}),
        json!({"op":"wrong"}),
    ] {
        assert!(command(a["token"].as_str().unwrap(), &request.to_string()).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}
#[test]
fn page_cards_hold_only_references_and_duplicate_pages_are_rejected() {
    let mut f = Fixture::new();
    let a = f.open("a");
    let a = apply(
        &a,
        json!({"op":"addPage","pageId":"encrypted-page","text":"secret"}),
    );
    assert_eq!(a["canvas"]["nodes"][0]["text"], "");
    for request in [
        json!({"op":"addPage","pageId":"encrypted-page"}),
        json!({"op":"text","id":id(&a,0),"text":"secret"}),
    ] {
        assert!(command(a["token"].as_str().unwrap(), &request.to_string()).is_err());
    }
    assert!(!fs::read_to_string(scoped_path(&f.root, "a").unwrap())
        .unwrap()
        .contains("secret"));
}
#[test]
fn duplicate_edges_are_rejected_and_removing_an_edge_is_undoable() {
    let mut f = Fixture::new();
    let a = f.open("a");
    let a = add(&a, "one");
    let a = add(&a, "two");
    let request = json!({"op":"connect","from":id(&a,0),"to":id(&a,1)});
    let a = apply(&a, request.clone());
    assert!(command(a["token"].as_str().unwrap(), &request.to_string()).is_err());
    let a = apply(
        &a,
        json!({"op":"removeEdge","id":a["canvas"]["edges"][0]["id"]}),
    );
    assert_eq!(a["canvas"]["edges"], json!([]));
    let a = apply(&a, json!({"op":"undo"}));
    assert_eq!(a["canvas"]["edges"].as_array().unwrap().len(), 1);
}
#[test]
fn another_editor_cannot_overwrite_a_newer_saved_revision() {
    let mut f = Fixture::new();
    let a = f.open("a");
    let stale = f.open("a");
    let a = add(&a, "newer");
    assert!(command(
        stale["token"].as_str().unwrap(),
        r#"{"op":"addNote","text":"stale"}"#
    )
    .is_err());
    let disk = read_store(&scoped_path(&f.root, "a").unwrap()).unwrap();
    assert_eq!(disk.revision, a["revision"].as_u64().unwrap());
    assert_eq!(disk.canvases[0].nodes[0].text, "newer");
}
#[test]
fn corrupt_and_future_files_are_not_reset_or_replaced() {
    let mut f = Fixture::new();
    let a = f.open("a");
    let _a = add(&a, "backup");
    let path = scoped_path(&f.root, "a").unwrap();
    fs::write(&path, b"broken").unwrap();
    assert!(open(&f.root, "a").is_err());
    assert_eq!(fs::read(&path).unwrap(), b"broken");
    let mut future = Store::initial();
    future.version = 2;
    let bytes = serde_json::to_vec(&future).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert!(open(&f.root, "a").is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
}
#[test]
fn missing_primary_recovers_a_valid_backup_without_changing_the_account() {
    let mut f = Fixture::new();
    let a = f.open("a");
    let a = add(&a, "retained");
    let _a = add(&a, "last");
    let path = scoped_path(&f.root, "a").unwrap();
    fs::remove_file(&path).unwrap();
    let recovered = f.open("a");
    assert_eq!(recovered["canvas"]["nodes"][0]["text"], "retained");
    assert!(!recovered["warning"].as_str().unwrap().is_empty());
}
#[test]
fn switching_boards_clears_history_and_last_board_deletion_keeps_a_valid_workspace() {
    let mut f = Fixture::new();
    let a = f.open("a");
    let a = add(&a, "first");
    let original = a["canvas"]["id"].clone();
    let a = apply(&a, json!({"op":"new","title":"second"}));
    assert_eq!(a["canUndo"], false);
    let a = apply(&a, json!({"op":"switch","id":original}));
    assert_eq!(a["canvas"]["nodes"][0]["text"], "first");
    let a = apply(&a, json!({"op":"deleteCanvas"}));
    let a = apply(&a, json!({"op":"deleteCanvas"}));
    assert_eq!(a["canvases"].as_array().unwrap().len(), 1);
    assert_eq!(a["canvas"]["nodes"], json!([]));
}
#[test]
fn multiple_boards_keep_independent_graphs_views_and_the_last_selection_after_reopen() {
    let mut f = Fixture::new();
    let first = add(&f.open("multi-board"), "first card");
    let first = add(&first, "second card");
    let first = apply(
        &first,
        json!({"op":"connect","from":id(&first,0),"to":id(&first,1)}),
    );
    let first = apply(&first, json!({"op":"view","x":20,"y":30,"zoom":0.5}));
    let first_board = first["canvas"].clone();
    let token = first["token"].as_str().unwrap();
    let second: Value = serde_json::from_str(
        &command_json(token, r#"{"op":"new","title":"Second board"}"#).unwrap(),
    )
    .unwrap();
    assert_ne!(second["canvas"]["id"], first_board["id"]);
    assert_eq!(second["canvases"].as_array().unwrap().len(), 2);
    assert_eq!(second["canvas"]["nodes"], json!([]));
    assert_eq!(second["canvas"]["edges"], json!([]));
    assert_eq!(second["canUndo"], false);
    let second = add(&second, "second board only");
    let second = apply(&second, json!({"op":"view","x":-40,"y":-50,"zoom":2}));
    let second_board = second["canvas"].clone();

    close(token);
    let reopened = f.open("multi-board");
    assert_eq!(reopened["canvas"], second_board);
    assert_eq!(reopened["canvases"], second["canvases"]);
    let switched: Value = serde_json::from_str(
        &command_json(
            reopened["token"].as_str().unwrap(),
            &json!({"op":"switch","id":first_board["id"]}).to_string(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(switched["canvas"], first_board);
    close(reopened["token"].as_str().unwrap());
    let reopened = f.open("multi-board");
    assert_eq!(reopened["canvas"], first_board);
    let switched = apply(&reopened, json!({"op":"switch","id":second_board["id"]}));
    assert_eq!(switched["canvas"], second_board);
}
#[test]
fn failed_board_creation_keeps_current_board_history_and_disk_then_allows_retry() {
    let mut f = Fixture::new();
    let saved = add(&f.open("create-failure"), "keep");
    let saved = add(&saved, "redo target");
    let saved = apply(&saved, json!({"op":"undo"}));
    assert_eq!(saved["canUndo"], true);
    assert_eq!(saved["canRedo"], true);
    let token = saved["token"].as_str().unwrap();
    let path = scoped_path(&f.root, "create-failure").unwrap();
    let before = fs::read(&path).unwrap();
    for title in [" ".to_string(), "x".repeat(81)] {
        assert!(command_json(token, &json!({"op":"new","title":title}).to_string()).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(response(token, &sessions().lock().unwrap()[token]), saved);
    }

    // A directory blocks backup replacement while leaving the primary store readable.
    let backup = path.with_extension("bak");
    fs::remove_file(&backup).unwrap();
    fs::create_dir(&backup).unwrap();
    let request = r#"{"op":"new","title":"Retry board"}"#;
    assert!(command_json(token, request).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(response(token, &sessions().lock().unwrap()[token]), saved);

    fs::remove_dir(&backup).unwrap();
    let created: Value = serde_json::from_str(&command_json(token, request).unwrap()).unwrap();
    assert_eq!(created["canvases"].as_array().unwrap().len(), 2);
    assert_eq!(created["revision"], saved["revision"].as_u64().unwrap() + 1);
    assert_eq!(created["canUndo"], false);
    assert_eq!(created["canRedo"], false);
    assert_eq!(f.open("create-failure")["canvas"], created["canvas"]);
    let original = apply(&created, json!({"op":"switch","id":saved["canvas"]["id"]}));
    assert_eq!(original["canvas"], saved["canvas"]);
}
#[test]
fn view_changes_do_not_consume_undo_and_drag_commits_one_atomic_gesture() {
    let mut f = Fixture::new();
    let a = f.open("a");
    let a = add(&a, "card");
    let a = apply(
        &a,
        json!({"op":"gesture","viewX":100,"viewY":30,"zoom":0.5,"id":id(&a,0),"x":60,"y":20}),
    );
    assert_eq!(a["canvas"]["nodes"][0]["x"], 60.0);
    let a = apply(
        &a,
        json!({"op":"gesture","viewX":200,"viewY":60,"zoom":0.8}),
    );
    let a = apply(&a, json!({"op":"undo"}));
    assert_eq!(a["canvas"]["nodes"][0]["x"], -128.0);
}
#[test]
fn history_is_bounded_and_new_edits_clear_redo() {
    let mut f = Fixture::new();
    let mut a = f.open("a");
    for i in 0..24 {
        a = add(&a, &format!("{i}"));
    }
    for _ in 0..20 {
        a = apply(&a, json!({"op":"undo"}));
    }
    assert_eq!(a["canUndo"], false);
    assert_eq!(a["canRedo"], true);
    let a = add(&a, "new branch");
    assert_eq!(a["canRedo"], false);
}
#[test]
fn save_failure_does_not_publish_an_uncommitted_canvas_or_history() {
    let mut f = Fixture::new();
    let a = f.open("a");
    let path = scoped_path(&f.root, "a").unwrap();
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(command(
        a["token"].as_str().unwrap(),
        r#"{"op":"addNote","text":"unsaved"}"#
    )
    .is_err());
    let map = sessions().lock().unwrap();
    let session = &map[a["token"].as_str().unwrap()];
    assert!(session.store.canvases[0].nodes.is_empty());
    assert!(session.undo.is_empty());
}

#[test]
fn new_cards_avoid_overlaps_and_preserve_existing_positions() {
    let mut canvas = Canvas::new("placement");
    canvas.center_x = 120.0;
    canvas.center_y = 60.0;
    for index in 0..80 {
        let before = canvas
            .nodes
            .iter()
            .map(|node| (node.id.clone(), node.x, node.y))
            .collect::<Vec<_>>();
        edit(
            &mut canvas,
            &json!({"op":"addNote","text":format!("card {index}")}),
        )
        .unwrap();
        for (node, (id, x, y)) in canvas.nodes.iter().zip(before) {
            assert_eq!((&node.id, node.x, node.y), (&id, x, y));
        }
    }
    for (index, node) in canvas.nodes.iter().enumerate() {
        for other in &canvas.nodes[..index] {
            let overlaps = node.x < other.x + other.width
                && node.x + node.width > other.x
                && node.y < other.y + other.height
                && node.y + node.height > other.y;
            assert!(!overlaps, "overlapping cards: {index} and {}", other.id);
        }
    }
    assert_eq!((canvas.center_x, canvas.center_y), (120.0, 60.0));
}

#[test]
fn placement_respects_mixed_sizes_and_world_boundaries() {
    let mut canvas = Canvas::new("mixed");
    edit(&mut canvas, &json!({"op":"addNote","text":"large"})).unwrap();
    let first_id = canvas.nodes[0].id.clone();
    edit(
        &mut canvas,
        &json!({"op":"resize","id":first_id,"width":720.0,"height":720.0}),
    )
    .unwrap();
    edit(&mut canvas, &json!({"op":"addNote","text":"nearby"})).unwrap();
    let first = &canvas.nodes[0];
    let second = &canvas.nodes[1];
    assert!(
        second.x >= first.x + first.width
            || first.x >= second.x + second.width
            || second.y >= first.y + first.height
            || first.y >= second.y + second.height
    );
    canvas.center_x = 249_900.0;
    canvas.center_y = -249_900.0;
    edit(&mut canvas, &json!({"op":"addNote","text":"world edge"})).unwrap();
    let edge = canvas.nodes.last().unwrap();
    assert!(coordinate(edge.x) && coordinate(edge.y));
    assert!(coordinate(edge.x + edge.width) && coordinate(edge.y + edge.height));
}

#[test]
fn customer_cards_keep_name_and_details_after_reopen_and_edit() {
    let mut fixture = Fixture::new();
    let opened = fixture.open("customer-card");
    let created = apply(&opened, json!({"op":"addClient","name":"张先生","details":"待跟进：写作需求"}));
    assert_eq!(created["canvas"]["nodes"][0]["kind"], "client");
    assert_eq!(created["canvas"]["nodes"][0]["text"], "张先生\n待跟进：写作需求");
    let first_id = id(&created, 0).to_owned();
    let updated = apply(&created, json!({"op":"updateClient","id":first_id,"name":"张女士","details":"已有需求"}));
    let reopened = fixture.open("customer-card");
    assert_eq!(reopened["canvas"]["nodes"], updated["canvas"]["nodes"]);
    assert_eq!(reopened["canvas"]["nodes"][0]["text"], "张女士\n已有需求");
    let bad = command(updated["token"].as_str().unwrap(), &json!({"op":"updateClient","id":first_id,"name":"","details":"x"}).to_string());
    assert!(bad.is_err());
    assert_eq!(fixture.open("customer-card")["canvas"], updated["canvas"]);
}

#[test]
fn adding_referred_customer_is_atomic_and_rejects_ambiguous_parents() {
    let mut fixture = Fixture::new();
    let opened = fixture.open("customer-referral");
    let a = apply(&opened, json!({"op":"addClient","name":"甲","details":""}));
    let root = id(&a, 0).to_owned();
    let b = apply(&a, json!({"op":"addClient","name":"乙","details":"","parentId":root}));
    assert_eq!(b["canvas"]["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(b["canvas"]["edges"].as_array().unwrap().len(), 1);
    assert_eq!(b["canvas"]["edges"][0]["from"], root);
    assert_eq!(b["canvas"]["edges"][0]["to"], id(&b, 1));
    let child = id(&b, 1).to_owned();
    let c = apply(&b, json!({"op":"addClient","name":"丙","details":""}));
    let other = id(&c, 2).to_owned();
    assert!(command(c["token"].as_str().unwrap(), &json!({"op":"connect","from":other,"to":child,"relation":"referral"}).to_string()).is_err());
    assert!(command(c["token"].as_str().unwrap(), &json!({"op":"connect","from":child,"to":root,"relation":"referral"}).to_string()).is_err());
    assert!(command(c["token"].as_str().unwrap(), &json!({"op":"addClient","name":"丁","details":"","parentId":"missing"}).to_string()).is_err());
    let reopened = fixture.open("customer-referral");
    assert_eq!(reopened["canvas"], c["canvas"]);
}

#[test]
fn customer_tree_arrangement_respects_lineage_and_undo() {
    let mut fixture = Fixture::new();
    let opened = fixture.open("customer-layout");
    let a = apply(&opened, json!({"op":"addClient","name":"一级","details":""}));
    let root = id(&a, 0).to_owned();
    let b = apply(&a, json!({"op":"addClient","name":"二级","details":"","parentId":root}));
    let c = apply(&b, json!({"op":"addClient","name":"三级","details":"","parentId":id(&b, 1)}));
    let before = c["canvas"].clone();
    let laid_out = apply(&c, json!({"op":"arrangeTree"}));
    let nodes = laid_out["canvas"]["nodes"].as_array().unwrap();
    assert_eq!(nodes[0]["x"], nodes[1]["x"]);
    assert_eq!(nodes[1]["x"], nodes[2]["x"]);
    assert!(nodes[0]["y"].as_f64().unwrap() < nodes[1]["y"].as_f64().unwrap());
    assert!(nodes[1]["y"].as_f64().unwrap() < nodes[2]["y"].as_f64().unwrap());
    let undone = apply(&laid_out, json!({"op":"undo"}));
    assert_eq!(undone["canvas"], before);
}


#[test]
fn templates_are_opt_in_and_old_stores_remain_blank() {
    let mut fixture = Fixture::new();
    let original = fixture.open("canvas-template");
    assert_eq!(original["canvas"]["template"], "blank");
    let original_id = original["canvas"]["id"].as_str().unwrap().to_owned();
    let network = apply(&original, json!({"op":"new","title":"客户网络","template":"customer_network"}));
    assert_eq!(network["canvas"]["template"], "customer_network");
    assert_eq!(network["canvases"][0]["template"], "blank");
    assert_eq!(network["canvases"][1]["template"], "customer_network");
    let blank = apply(&network, json!({"op":"new","title":"项目计划"}));
    assert_eq!(blank["canvas"]["template"], "blank");
    assert_eq!(blank["canvas"]["nodes"], json!([]));
    let reopened = fixture.open("canvas-template");
    assert_eq!(reopened["canvas"], blank["canvas"]);
    let old = apply(&reopened, json!({"op":"switch","id":original_id}));
    assert_eq!(old["canvas"]["template"], "blank");
    assert_eq!(old["canvas"]["id"], original_id);

    let old_json = r#"{"id":"abc","title":"旧画布","centerX":0.0,"centerY":0.0,"zoom":1.0,"nodes":[],"edges":[]}"#;
    let decoded: Canvas = serde_json::from_str(old_json).unwrap();
    assert_eq!(decoded.template, "blank");
    let invalid = command(old["token"].as_str().unwrap(),
        &json!({"op":"new","title":"未知模板","template":"unsupported"}).to_string());
    assert!(invalid.is_err());
    assert_eq!(fixture.open("canvas-template")["canvas"], old["canvas"]);
}

#[test]
fn unrestricted_visual_lines_are_independent_of_validated_referrals() {
    let mut fixture = Fixture::new();
    let opened = fixture.open("universal-links");
    let a = apply(&opened, json!({"op":"addClient","name":"甲","details":""}));
    let a = apply(&a, json!({"op":"addClient","name":"乙","details":""}));
    let a = apply(&a, json!({"op":"addClient","name":"丙","details":""}));
    let x = id(&a, 0).to_owned();
    let y = id(&a, 1).to_owned();
    let z = id(&a, 2).to_owned();
    let a = apply(&a, json!({"op":"connect","from":x,"to":y,"relation":"referral"}));
    let a = apply(&a, json!({"op":"connect","from":y,"to":x,"relation":"link"}));
    let a = apply(&a, json!({"op":"connect","from":z,"to":y,"relation":"link"}));
    assert_eq!(a["canvas"]["edges"].as_array().unwrap().len(), 3);
    assert_eq!(a["canvas"]["edges"][1]["relation"], "link");
    assert_eq!(a["canvas"]["edges"][2]["relation"], "link");
    assert!(command(a["token"].as_str().unwrap(),
        &json!({"op":"connect","from":z,"to":y,"relation":"referral"}).to_string()).is_err());
    assert!(command(a["token"].as_str().unwrap(),
        &json!({"op":"connect","from":y,"to":x,"relation":"referral"}).to_string()).is_err());
    let arranged = apply(&a, json!({"op":"arrangeTree"}));
    assert_eq!(arranged["canvas"]["nodes"][0]["x"], arranged["canvas"]["nodes"][1]["x"]);
    assert!(arranged["canvas"]["nodes"][1]["y"].as_f64().unwrap() >
        arranged["canvas"]["nodes"][0]["y"].as_f64().unwrap());
    assert!(arranged["canvas"]["nodes"][2]["y"].as_f64().unwrap() >
        arranged["canvas"]["nodes"][1]["y"].as_f64().unwrap());
    let undo = apply(&arranged, json!({"op":"undo"}));
    assert_eq!(undo["canvas"], a["canvas"]);
    let reopened = fixture.open("universal-links");
    assert_eq!(reopened["canvas"], undo["canvas"]);
}

#[test]
fn generic_note_and_page_cards_restore_resizing_and_connection_management() {
    let mut fixture = Fixture::new();
    let opened = fixture.open("generic-tools");
    let a = apply(&opened, json!({"op":"addNote","text":"待处理\n记录"}));
    let a = apply(&a, json!({"op":"addPage","pageId":"existing-page-id"}));
    let a = apply(&a, json!({"op":"addNote","text":"另一条便签"}));
    let first = id(&a, 0).to_owned();
    let second = id(&a, 1).to_owned();
    let third = id(&a, 2).to_owned();
    let a = apply(&a, json!({"op":"connect","from":first,"to":second,"relation":"link"}));
    let a = apply(&a, json!({"op":"connect","from":second,"to":third}));
    let a = apply(&a, json!({"op":"resize","id":second,"width":384.0,"height":252.0}));
    assert_eq!(a["canvas"]["nodes"][1]["width"], 384.0);
    assert_eq!(a["canvas"]["edges"].as_array().unwrap().len(), 2);
    let removed = apply(&a, json!({"op":"removeEdge","id":a["canvas"]["edges"][0]["id"]}));
    assert_eq!(removed["canvas"]["edges"].as_array().unwrap().len(), 1);
    let restored = apply(&removed, json!({"op":"undo"}));
    assert_eq!(restored["canvas"]["edges"].as_array().unwrap().len(), 2);
    assert_eq!(restored["canvas"]["nodes"][1]["width"], 384.0);
}

#[test]
fn legacy_customer_connections_retain_referral_semantics() {
    let original = r#"{"id":"legacy","title":"客户画布","centerX":0,"centerY":0,"zoom":1,
        "nodes":[
            {"id":"c1","kind":"client","pageId":"","text":"甲\n","x":0,"y":0,"width":192,"height":132,"color":"blue"},
            {"id":"c2","kind":"client","pageId":"","text":"乙\n","x":316,"y":0,"width":192,"height":132,"color":"blue"}
        ],
        "edges":[{"id":"e1","from":"c1","to":"c2"}]}"#;
    let mut canvas: Canvas = serde_json::from_str(original).unwrap();
    canvas.validate().unwrap();
    assert!(is_referral(&canvas.edges[0],
        &canvas.nodes.iter().map(|n| n.id.as_str()).collect()));
    edit(&mut canvas, &json!({"op":"connect","from":"c2","to":"c1","relation":"link"})).unwrap();
    assert!(edit(&mut canvas,
        &json!({"op":"connect","from":"c2","to":"c1","relation":"referral"})).is_err());
    canvas.validate().unwrap();
}

#[test]
fn referral_forest_centers_parents_groups_networks_and_separates_singletons() {
    let mut fixture = Fixture::new();
    let mut state = fixture.open("compact-referral-forest");
    state = apply(&state, json!({"op":"addClient","name":"上级甲","details":""}));
    let root = id(&state, 0).to_owned();
    state = apply(&state, json!({"op":"addClient","name":"下级甲","details":"","parentId":root}));
    let first_child = id(&state, 1).to_owned();
    state = apply(&state, json!({"op":"addClient","name":"下级乙","details":"","parentId":root}));
    state = apply(&state, json!({"op":"addClient","name":"第三级","details":"","parentId":first_child}));
    state = apply(&state, json!({"op":"addClient","name":"上级乙","details":""}));
    let other_root = id(&state, 4).to_owned();
    state = apply(&state, json!({"op":"addClient","name":"下级丙","details":"","parentId":other_root}));
    state = apply(&state, json!({"op":"addClient","name":"无关系","details":""}));
    state = apply(&state, json!({"op":"addNote","text":"普通便签不参与客户布局"}));
    let old = state["canvas"].clone();
    let moved = apply(&state, json!({"op":"arrangeTree"}));
    let nodes = moved["canvas"]["nodes"].as_array().unwrap();
    let n = |i: usize, key: &str| nodes[i][key].as_f64().unwrap();
    assert_eq!(n(1,"y"), n(2,"y"), "direct referrals share a depth");
    assert!(n(1,"x") < n(2,"x"), "siblings retain a predictable order");
    assert!(n(0,"x") + n(0,"width") / 2.0 >
        n(1,"x") + n(1,"width") / 2.0);
    assert!(n(0,"x") + n(0,"width") / 2.0 <
        n(2,"x") + n(2,"width") / 2.0);
    assert!(n(3,"y") > n(1,"y"), "grandchildren are below their parent");
    assert_eq!(n(4,"y"), n(0,"y"), "independent trees share the first row");
    assert!(n(4,"x") > n(2,"x"), "independent trees do not interleave");
    assert!(n(6,"y") > n(3,"y"), "singletons are separate from connected networks");
    assert_eq!(nodes[7], old["nodes"][7], "ordinary note coordinates are preserved");
    for (i, node) in nodes[..7].iter().enumerate() {
        for (j, other) in nodes[..i].iter().enumerate() {
            let overlap = n(i,"x") < n(j,"x") + n(j,"width") &&
                n(i,"x") + n(i,"width") > n(j,"x") &&
                n(i,"y") < n(j,"y") + n(j,"height") &&
                n(i,"y") + n(i,"height") > n(j,"y");
            assert!(!overlap, "clients {i} and {j} overlap: {node:?} {other:?}");
        }
    }
    let again = apply(&moved, json!({"op":"arrangeTree"}));
    assert_eq!(again["canvas"], moved["canvas"], "repeat arrangement must be stable");
    let undone = apply(&again, json!({"op":"undo"}));
    assert_eq!(undone["canvas"], old, "the single explicit action can be undone");
}

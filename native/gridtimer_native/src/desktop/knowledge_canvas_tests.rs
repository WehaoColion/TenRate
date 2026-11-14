// Windows canvas management: preserve board identity and content across edits.

#[test]
fn windows_canvas_new_boards_have_distinct_names_and_identity() {
    let mut store = KnowledgeCanvasStore::default();
    assert!(store.new_canvas("知识画布"));
    assert!(store.new_canvas("新画布"));
    assert!(store.new_canvas("新画布"));

    assert_eq!(store.canvases.len(), 3);
    assert_eq!(store.active_canvas_id, store.canvases[2].id);
    let names = store
        .canvases
        .iter()
        .map(|canvas| canvas.title.as_str())
        .collect::<HashSet<_>>();
    let ids = store
        .canvases
        .iter()
        .map(|canvas| canvas.id.as_str())
        .collect::<HashSet<_>>();
    assert_eq!(
        names.len(),
        3,
        "the picker must distinguish newly created boards"
    );
    assert_eq!(ids.len(), 3);
    assert!(store.validate().is_ok());

    while store.canvases.len() < 128 {
        assert!(store.new_canvas("新画布"));
    }
    let before = store.clone();
    assert!(!store.new_canvas("新画布"));
    assert_eq!(
        store, before,
        "a rejected creation must leave the store intact"
    );
}

#[test]
fn windows_canvas_duplicate_rekeys_nodes_and_edges_without_losing_content() {
    let mut store = KnowledgeCanvasStore::default();
    assert!(store.new_canvas("研究"));
    let original_id = store.active_canvas_id.clone();
    let original = store.active_mut().unwrap();
    original.nodes = vec![
        KnowledgeCanvasNode {
            id: "source-a".into(),
            kind: KnowledgeCanvasNodeKind::Note,
            text: "第一条想法".into(),
            x: 32.0,
            y: 48.0,
            color: "amber".into(),
            ..Default::default()
        },
        KnowledgeCanvasNode {
            id: "source-b".into(),
            kind: KnowledgeCanvasNodeKind::Page,
            page_id: "document-1".into(),
            x: 240.0,
            y: 120.0,
            ..Default::default()
        },
    ];
    original.edges = vec![KnowledgeCanvasEdge {
        id: "edge-a".into(),
        from: "source-a".into(),
        to: "source-b".into(),
        relation: None,
    }];

    assert!(store.duplicate_active());
    assert_eq!(store.canvases.len(), 2);
    assert_ne!(store.active_canvas_id, original_id);
    let source = &store.canvases[0];
    let copy = store.active().unwrap();
    assert_ne!(source.title, copy.title);
    assert_eq!(source.nodes.len(), copy.nodes.len());
    assert_eq!(source.edges.len(), copy.edges.len());
    assert_eq!(copy.nodes[0].text, source.nodes[0].text);
    assert_eq!(copy.nodes[0].color, source.nodes[0].color);
    assert_eq!(copy.nodes[1].page_id, source.nodes[1].page_id);
    assert_eq!((copy.nodes[0].x, copy.nodes[1].y), (32.0, 120.0));
    assert_ne!(copy.nodes[0].id, source.nodes[0].id);
    assert_ne!(copy.nodes[1].id, source.nodes[1].id);
    assert_ne!(copy.edges[0].id, source.edges[0].id);
    assert_eq!(copy.edges[0].from, copy.nodes[0].id);
    assert_eq!(copy.edges[0].to, copy.nodes[1].id);
    assert!(store.validate().is_ok());
}

#[test]
fn windows_canvas_delete_active_chooses_survivor_and_keeps_last_board() {
    let mut store = KnowledgeCanvasStore::default();
    for title in ["一", "二", "三"] {
        assert!(store.new_canvas(title));
    }
    let middle_id = store.canvases[1].id.clone();
    let final_id = store.canvases[2].id.clone();
    store.active_canvas_id = middle_id.clone();
    assert!(store.delete_canvas(&middle_id));
    assert_eq!(store.canvases.len(), 2);
    assert!(
        store.active().is_some(),
        "deleting the active board must select a survivor"
    );
    assert_ne!(store.active_canvas_id, middle_id);
    assert!(store.canvases.iter().any(|canvas| canvas.id == final_id));
    assert!(store.validate().is_ok());

    let active_id = store.active_canvas_id.clone();
    assert!(store.delete_canvas(&active_id));
    assert_eq!(store.canvases.len(), 1);
    let last_id = store.active_canvas_id.clone();
    assert!(!store.delete_canvas(&last_id));
    assert_eq!(store.canvases.len(), 1);
    assert_eq!(store.active_canvas_id, last_id);
    assert!(store.validate().is_ok());
}

#[test]
fn windows_canvas_store_json_roundtrip_keeps_active_board_and_graph() {
    let mut store = KnowledgeCanvasStore::default();
    assert!(store.new_canvas("项目"));
    store.active_mut().unwrap().nodes.push(KnowledgeCanvasNode {
        id: "note-1".into(),
        kind: KnowledgeCanvasNodeKind::Note,
        text: "可编辑内容".into(),
        ..Default::default()
    });
    assert!(store.duplicate_active());
    let serialized = serde_json::to_string(&store).unwrap();
    let reloaded: KnowledgeCanvasStore = serde_json::from_str(&serialized).unwrap();
    assert_eq!(reloaded, store);
    assert!(reloaded.validate().is_ok());
    assert_eq!(reloaded.active().unwrap().nodes[0].text, "可编辑内容");
    assert!(knowledge_canvas_json_is_valid(&serialized).is_some());

    let mut invalid = reloaded;
    invalid.active_mut().unwrap().title = " \t ".into();
    assert!(
        invalid.validate().is_err(),
        "blank board names cannot be saved"
    );
}

#[test]
fn windows_canvas_legacy_blank_title_is_repaired_without_losing_cards() {
    let mut store = KnowledgeCanvasStore::default();
    assert!(store.new_canvas("旧画布"));
    store.active_mut().unwrap().nodes.push(KnowledgeCanvasNode {
        id: "legacy-card".into(),
        kind: KnowledgeCanvasNodeKind::Note,
        text: "历史内容".into(),
        ..Default::default()
    });
    store.active_mut().unwrap().title = " \t ".into();
    let legacy = serde_json::to_string(&store).unwrap();

    let repaired_json = knowledge_canvas_json_is_valid(&legacy)
        .expect("a legacy blank title should not discard its canvas");
    let repaired: KnowledgeCanvasStore = serde_json::from_str(&repaired_json).unwrap();
    assert!(repaired.validate().is_ok());
    assert_eq!(repaired.canvases.len(), 1);
    assert_eq!(repaired.active_canvas_id, store.active_canvas_id);
    assert!(!repaired.active().unwrap().title.trim().is_empty());
    assert_eq!(repaired.active().unwrap().nodes[0].id, "legacy-card");
    assert_eq!(repaired.active().unwrap().nodes[0].text, "历史内容");
}


#[test]
fn windows_customer_amounts_and_receipts_use_shared_rust_precision() {
    let customer = KnowledgeCanvasNode {
        id: "client-1".into(),
        kind: KnowledgeCanvasNodeKind::Client,
        text: "客户甲\\n备注".into(),
        ..Default::default()
    };
    let customer = knowledge_canvas_apply_profile(
        &customer, "客户甲", "备注", "560.00", "660.00",
    ).unwrap();
    let mut core = knowledge_canvas_customer_to_core(&customer);
    assert_eq!(gridtimer_native::android_canvas::checked_client_totals(&core).unwrap(), (56_000, 66_000));
    gridtimer_native::android_canvas::apply_customer_money_operation(
        &mut core,
        &json!({"op":"importClientHistory","id":"client-1",
            "chargeId":"charge-1","receiptId":"receipt-1",
            "title":"服务项目","serviceLocation":"服务地","dueOn":"",
            "receivedOn":"","location":"收款地","channel":"转账",
            "note":"","receiptNote":""}),
    ).unwrap();
    assert_eq!(core.charges.len(), 1);
    assert_eq!(core.charges[0].receipts[0].location, "收款地");
    assert_eq!(core.charges[0].receipts[0].amount_cents, 56_000);
    assert_eq!(gridtimer_native::android_canvas::checked_client_totals(&core).unwrap(), (56_000, 66_000));
    assert!(core.paid_cents.is_none() && core.pending_cents.is_none());

    let before = core.clone();
    assert!(gridtimer_native::android_canvas::apply_customer_money_operation(
        &mut core,
        &json!({"op":"addClientReceipt","id":"client-1",
            "chargeId":"charge-1","receiptId":"receipt-invalid",
            "amountYuan":"660.001","receivedOn":"2026-11-14",
            "location":"长沙","channel":"转账","note":""}),
    ).is_err());
    assert_eq!(core, before, "failed cents validation must never mutate a receipt");
}

#[test]
fn windows_referral_graph_rejects_cycles_and_multiple_direct_parents() {
    let mut store = KnowledgeCanvasStore::default();
    assert!(store.new_canvas("客户网络"));
    let canvas = store.active_mut().unwrap();
    canvas.template = "customer_network".into();
    for name in ["甲", "乙", "丙"] {
        canvas.nodes.push(KnowledgeCanvasNode {
            id: name.into(), kind: KnowledgeCanvasNodeKind::Client,
            text: format!("{name}\\n"), ..Default::default()
        });
    }
    canvas.edges.push(KnowledgeCanvasEdge {
        id:"e1".into(), from:"甲".into(), to:"乙".into(), relation:Some("referral".into())
    });
    canvas.edges.push(KnowledgeCanvasEdge {
        id:"e2".into(), from:"乙".into(), to:"丙".into(), relation:Some("referral".into())
    });
    assert!(store.validate().is_ok());
    store.active_mut().unwrap().edges.push(KnowledgeCanvasEdge {
        id:"e3".into(), from:"丙".into(), to:"甲".into(), relation:Some("referral".into())
    });
    assert!(store.validate().is_err(), "a cycle must be rejected");
    store.active_mut().unwrap().edges.pop();
    store.active_mut().unwrap().edges.push(KnowledgeCanvasEdge {
        id:"e4".into(), from:"甲".into(), to:"丙".into(), relation:Some("referral".into())
    });
    assert!(store.validate().is_err(), "only one direct introducer is permitted");
}

#[test]
fn windows_canvas_json_preserves_android_customer_fields_and_one_percent_zoom() {
    let mut store = KnowledgeCanvasStore::default();
    assert!(store.new_canvas("客户"));
    let canvas = store.active_mut().unwrap();
    canvas.template = "customer_network".into();
    canvas.zoom = 0.01;
    canvas.nodes.push(KnowledgeCanvasNode {
        id:"client-a".into(), kind: KnowledgeCanvasNodeKind::Client,
        text:"客户甲\\n说明".into(), paid_cents:Some(1), pending_cents:Some(99),
        ..Default::default()
    });
    let encoded = serde_json::to_string(&store).unwrap();
    let decoded: KnowledgeCanvasStore = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, store);
    assert_eq!(decoded.active().unwrap().zoom, 0.01);
    assert_eq!(decoded.active().unwrap().nodes[0].paid_cents, Some(1));
    assert!(knowledge_canvas_json_is_valid(&encoded).is_some());
    let old = r#"{"version":1,"activeCanvasId":"a","canvases":[{"id":"a","title":"旧画布","centerX":0,"centerY":0,"zoom":1,"nodes":[],"edges":[]}]}"#;
    assert!(knowledge_canvas_json_is_valid(old).is_some());
}

// Windows client customer graph and payment UI. All money transactions use the Android Rust rules.
// Windows canvas files are local to the desktop account; this code does not imply cross-device sync.

#[derive(Default)]
struct KnowledgeCanvasCustomerDraft {
    client_id: String,
    name: String,
    details: String,
    legacy_paid: String,
    legacy_pending: String,
    child_name: String,
    charge_title: String,
    charge_amount: String,
    service_location: String,
    due_on: String,
    charge_note: String,
    charge_edit_id: String,
    charge_remove_id: String,
    charge_id: String,
    receipt_amount: String,
    receipt_date: String,
    receipt_location: String,
    receipt_channel: String,
    receipt_note: String,
    receipt_edit_id: String,
    receipt_remove_id: String,
}

fn knowledge_canvas_customer_to_core(
    node: &KnowledgeCanvasNode,
) -> gridtimer_native::android_canvas::Node {
    gridtimer_native::android_canvas::Node {
        id: node.id.clone(),
        kind: match node.kind {
            KnowledgeCanvasNodeKind::Page => "page",
            KnowledgeCanvasNodeKind::Note => "note",
            KnowledgeCanvasNodeKind::Client => "client",
        }.into(),
        page_id: node.page_id.clone(),
        text: node.text.clone(),
        x: node.x,
        y: node.y,
        width: node.width,
        height: node.height,
        color: match node.color.as_str() {
            "sage" => "green", "rose" => "pink", other => other,
        }.into(),
        paid_cents: node.paid_cents,
        pending_cents: node.pending_cents,
        charges: node.charges.clone(),
    }
}

fn customer_money_label(cents: i64) -> String {
    format!("¥{}.{:02}", cents / 100, cents % 100)
}

fn customer_money_editable(cents: Option<i64>) -> String {
    cents.map(|v| format!("{}.{:02}", v / 100, v % 100)).unwrap_or_default()
}

fn knowledge_canvas_apply_profile(
    current: &KnowledgeCanvasNode,
    name: &str,
    details: &str,
    paid: &str,
    pending: &str,
) -> Result<KnowledgeCanvasNode, String> {
    if current.kind != KnowledgeCanvasNodeKind::Client {
        return Err("此卡片不是客户".into());
    }
    let name = name.trim();
    let details = details.trim();
    if name.is_empty() || name.chars().count() > 80 || details.chars().count() > 4000 {
        return Err("客户姓名或备注超过允许范围".into());
    }
    let mut next = current.clone();
    next.text = format!("{name}\n{details}");
    next.paid_cents = gridtimer_native::android_canvas::parse_client_yuan(paid)?;
    next.pending_cents = gridtimer_native::android_canvas::parse_client_yuan(pending)?;
    gridtimer_native::android_canvas::checked_client_totals(
        &knowledge_canvas_customer_to_core(&next),
    )?;
    Ok(next)
}

impl TimerWindowsClient {
    fn knowledge_canvas_add_customer_named(&mut self, name: &str, parent: Option<&str>) {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 80 {
            self.status = "请填写不超过 80 字的客户姓名".into();
            return;
        }
        let mut next = self.desktop_ui.knowledge_canvas.store.clone();
        let Some(canvas) = next.active_mut() else { return; };
        if canvas.nodes.len() >= KNOWLEDGE_CANVAS_NODE_LIMIT {
            self.status = "画布卡片已达到数量上限".into();
            return;
        }
        if parent.is_some_and(|id| !canvas.nodes.iter().any(|node|
            node.kind == KnowledgeCanvasNodeKind::Client && node.id == id))
        {
            self.status = "推荐人不存在或并非客户".into();
            return;
        }
        if parent.is_some() && canvas.edges.len() >= KNOWLEDGE_CANVAS_EDGE_LIMIT {
            self.status = "画布关系已达到数量上限".into();
            return;
        }
        let (x, y) = if let Some(parent_id) = parent {
            let parent_card = canvas.nodes.iter()
                .find(|node| node.id == parent_id)
                .expect("validated existing customer");
            let direct_children = canvas.edges.iter()
                .filter(|edge| edge.from == parent_id
                    && edge.relation.as_deref() == Some("referral")).count();
            (parent_card.x + 210.0,
                parent_card.y + direct_children as f32 * 162.0)
        } else if canvas.nodes.is_empty() {
            // The first customer must appear in the visible center,
            // not at the legacy -500 world-unit grid origin.
            (canvas.center_x - 96.0, canvas.center_y - 66.0)
        } else {
            knowledge_canvas_next_position(canvas)
        };
        let new_id = random_desktop_identifier("knowledge-canvas-client");
        canvas.nodes.push(KnowledgeCanvasNode {
            id: new_id.clone(),
            kind: KnowledgeCanvasNodeKind::Client,
            text: format!("{name}\n"),
            x, y, width: 192.0, height: 132.0,
            color: "blue".into(),
            ..Default::default()
        });
        if let Some(parent) = parent {
            canvas.edges.push(KnowledgeCanvasEdge {
                id: random_desktop_identifier("knowledge-canvas-referral"),
                from: parent.into(),
                to: new_id.clone(),
                relation: Some("referral".into()),
            });
        }
        if let Err(error) = next.validate() {
            self.status = error;
            return;
        }
        self.desktop_ui.knowledge_canvas.store = next;
        self.desktop_ui.knowledge_canvas.selected_nodes = vec![new_id];
        self.desktop_ui.knowledge_canvas.selected_edge = None;
        self.mark_knowledge_canvas_dirty();
    }

    fn knowledge_canvas_update_customer(
        &mut self, id: &str, request: Value,
    ) -> Result<(), String> {
        let mut next = self.desktop_ui.knowledge_canvas.store.clone();
        let canvas = next.active_mut().ok_or("画布不存在")?;
        let customer = canvas.nodes.iter_mut().find(|n| n.id == id &&
            n.kind == KnowledgeCanvasNodeKind::Client).ok_or("客户卡片不存在")?;
        if request["op"].as_str() == Some("saveProfile") {
            *customer = knowledge_canvas_apply_profile(
                customer,
                request["name"].as_str().unwrap_or(""),
                request["details"].as_str().unwrap_or(""),
                request["paidYuan"].as_str().unwrap_or(""),
                request["pendingYuan"].as_str().unwrap_or(""),
            )?;
        } else {
            let mut core = knowledge_canvas_customer_to_core(customer);
            gridtimer_native::android_canvas::apply_customer_money_operation(
                &mut core, &request,
            )?;
            customer.paid_cents = core.paid_cents;
            customer.pending_cents = core.pending_cents;
            customer.charges = core.charges;
        }
        next.validate()?;
        self.desktop_ui.knowledge_canvas.store = next;
        self.mark_knowledge_canvas_dirty();
        Ok(())
    }

    fn knowledge_canvas_arrange_referrals(&mut self) {
        let Some(source) = self.desktop_ui.knowledge_canvas.store.active() else { return; };
        let mut core = gridtimer_native::android_canvas::Canvas {
            id: source.id.clone(), title: source.title.clone(), template: source.template.clone(),
            center_x: source.center_x, center_y: source.center_y, zoom: source.zoom,
            nodes: source.nodes.iter().map(knowledge_canvas_customer_to_core)
                .collect::<Vec<_>>().into(),
            edges: source.edges.iter().map(|e| gridtimer_native::android_canvas::Edge {
                id: e.id.clone(), from: e.from.clone(), to: e.to.clone(),
                relation: e.relation.clone(),
            }).collect::<Vec<_>>().into(),
        };
        if let Err(error) = gridtimer_native::android_canvas::layout_customer_referrals(&mut core) {
            self.status = error;
            return;
        }
        let mut next = self.desktop_ui.knowledge_canvas.store.clone();
        if let Some(canvas) = next.active_mut() {
            for (dest, source) in canvas.nodes.iter_mut().zip(core.nodes.iter()) {
                if dest.kind == KnowledgeCanvasNodeKind::Client {
                    dest.x = source.x;
                    dest.y = source.y;
                }
            }
            canvas.center_x = core.center_x;
            canvas.center_y = core.center_y;
        }
        if let Err(error) = next.validate() {
            self.status = error;
            return;
        }
        self.desktop_ui.knowledge_canvas.store = next;
        self.mark_knowledge_canvas_dirty();
    }

    fn ui_knowledge_canvas_add_customer_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("＋ 客户", |ui| {
            ui.set_min_width(230.0);
            ui.label("创建客户卡片");
            ui.add(egui::TextEdit::singleline(
                &mut self.desktop_ui.knowledge_canvas.new_customer_name,
            ).hint_text("客户姓名"));
            let name = self.desktop_ui.knowledge_canvas.new_customer_name.trim().to_owned();
            if ui.add_enabled(!name.is_empty(), egui::Button::new("添加至画布")).clicked() {
                self.knowledge_canvas_add_customer_named(&name, None);
                self.desktop_ui.knowledge_canvas.new_customer_name.clear();
                ui.close_menu();
            }
        });
    }

    fn ui_knowledge_canvas_customer_inspector(&mut self, ctx: &egui::Context) {
        let selected = self.desktop_ui.knowledge_canvas.selected_nodes.first().cloned();
        let Some(node) = selected.and_then(|id| self.desktop_ui.knowledge_canvas.store
            .active().and_then(|canvas| canvas.nodes.iter().find(|n| n.id == id)).cloned())
            .filter(|node| node.kind == KnowledgeCanvasNodeKind::Client)
        else { return; };
        let draft = &mut self.desktop_ui.knowledge_canvas.customer_draft;
        if draft.client_id != node.id {
            *draft = KnowledgeCanvasCustomerDraft {
                client_id: node.id.clone(),
                name: node.text.lines().next().unwrap_or("").into(),
                details: node.text.split_once('\n').map_or("", |(_, rest)| rest).into(),
                legacy_paid: customer_money_editable(node.paid_cents),
                legacy_pending: customer_money_editable(node.pending_cents),
                ..Default::default()
            };
        }
        let mut action: Option<Value> = None;
        let mut new_child: Option<String> = None;
        let compact = ctx.screen_rect().width() < 900.0;
        let inspector_y = if ctx.screen_rect().height() < 650.0 { 72.0 } else { 212.0 };
        let inspector_height = (ctx.screen_rect().height() - inspector_y - 52.0).max(160.0);
        egui::Window::new("客户资料与收付款")
            .id(egui::Id::new(("knowledge_canvas_customer_inspector", &node.id)))
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-14.0, inspector_y))
            .default_width(if compact { 350.0 } else { 410.0 })
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().max_height(inspector_height).show(ui, |ui| {
                    let d = &mut self.desktop_ui.knowledge_canvas.customer_draft;
                    ui.label("客户姓名");
                    ui.text_edit_singleline(&mut d.name);
                    ui.label("备注");
                    ui.add(egui::TextEdit::multiline(&mut d.details).desired_rows(2));
                    ui.horizontal(|ui| {
                        ui.label("历史已收（元）");
                        ui.add(egui::TextEdit::singleline(&mut d.legacy_paid).desired_width(96.0));
                        ui.label("历史待收（元）");
                        ui.add(egui::TextEdit::singleline(&mut d.legacy_pending).desired_width(96.0));
                    });
                    if ui.button("保存客户资料与原始金额").clicked() {
                        action = Some(json!({"op":"saveProfile","name":d.name,"details":d.details,
                            "paidYuan":d.legacy_paid,"pendingYuan":d.legacy_pending}));
                    }
                    ui.separator();
                    if let Ok((paid, pending)) =
                        gridtimer_native::android_canvas::checked_client_totals(
                            &knowledge_canvas_customer_to_core(&node))
                    {
                        ui.horizontal(|ui| {
                            ui.strong(format!("累计已收 {}", customer_money_label(paid)));
                            ui.label(format!("尚待收 {}", customer_money_label(pending)));
                        });
                    }
                    ui.small("以下客户款项只保存在知识画布，不进入风控账本。");
                    ui.collapsing(
                        if d.charge_edit_id.is_empty() { "新增款项项目" } else { "正在修改款项项目" },
                        |ui| {
                            ui.label("款项名称");
                            ui.text_edit_singleline(&mut d.charge_title);
                            ui.horizontal(|ui| {
                                ui.label("应收金额（元）");
                                ui.add(egui::TextEdit::singleline(&mut d.charge_amount).desired_width(108.0));
                            });
                            ui.label("服务地点");
                            ui.text_edit_singleline(&mut d.service_location);
                            ui.label("预计付款日期（可留空，格式 YYYY-MM-DD）");
                            ui.text_edit_singleline(&mut d.due_on);
                            ui.label("款项备注");
                            ui.add(egui::TextEdit::multiline(&mut d.charge_note).desired_rows(2));
                            if d.charge_edit_id.is_empty() {
                                if ui.button("记录这笔款项").clicked() {
                                    action = Some(json!({"op":"addClientCharge", "id":node.id,
                                        "chargeId":random_desktop_identifier("customer-charge"),
                                        "title":d.charge_title,"totalYuan":d.charge_amount,
                                        "serviceLocation":d.service_location,
                                        "dueOn":d.due_on,"note":d.charge_note}));
                                }
                                if (node.paid_cents.unwrap_or(0) > 0 || node.pending_cents.unwrap_or(0) > 0)
                                    && ui.button("把原始历史金额整理为这笔款项").clicked()
                                {
                                    action = Some(json!({"op":"importClientHistory","id":node.id,
                                        "chargeId":random_desktop_identifier("customer-charge"),
                                        "receiptId":random_desktop_identifier("customer-receipt"),
                                        "title":d.charge_title, "serviceLocation":d.service_location,
                                        "dueOn":d.due_on, "receivedOn":"", "location":d.receipt_location,
                                        "channel":d.receipt_channel,"note":d.charge_note,"receiptNote":""}));
                                }
                            } else {
                                ui.horizontal(|ui| {
                                    if ui.button("保存款项修改").clicked() {
                                        action = Some(json!({"op":"updateClientCharge","id":node.id,
                                            "chargeId":d.charge_edit_id,"title":d.charge_title,
                                            "totalYuan":d.charge_amount,
                                            "serviceLocation":d.service_location,
                                            "dueOn":d.due_on,"note":d.charge_note}));
                                    }
                                    if ui.button("取消修改款项").clicked() {
                                        d.charge_edit_id.clear();
                                        d.charge_title.clear();
                                        d.charge_amount.clear();
                                        d.service_location.clear();
                                        d.due_on.clear();
                                        d.charge_note.clear();
                                    }
                                });
                            }
                        },
                    );
                    if !node.charges.is_empty() {
                        ui.separator();
                        ui.strong("款项与实际收款记录");
                        for charge in &node.charges {
                            let paid = charge.receipts.iter().map(|r| r.amount_cents).sum::<i64>();
                            ui.collapsing(format!("{} · {} / {}",
                                charge.title, customer_money_label(paid),
                                customer_money_label(charge.total_cents)), |ui| {
                                ui.label(format!("服务地点：{}",
                                    if charge.service_location.is_empty() {"未填写"} else {&charge.service_location}));
                                ui.label(format!("预计付款：{}",
                                    if charge.due_on.is_empty() {"未填写"} else {&charge.due_on}));
                                if !charge.note.is_empty() {
                                    ui.label(format!("款项备注：{}", charge.note));
                                }
                                ui.horizontal_wrapped(|ui| {
                                    if ui.small_button("修改款项").clicked() {
                                        d.charge_edit_id = charge.id.clone();
                                        d.charge_title = charge.title.clone();
                                        d.charge_amount = customer_money_editable(Some(charge.total_cents));
                                        d.service_location = charge.service_location.clone();
                                        d.due_on = charge.due_on.clone();
                                        d.charge_note = charge.note.clone();
                                    }
                                    if charge.receipts.is_empty() {
                                        if ui.small_button("删除款项…").clicked() {
                                            d.charge_remove_id = charge.id.clone();
                                        }
                                    } else {
                                        ui.small("已有收款，须先移除收款记录才能删除款项");
                                    }
                                });
                                if d.charge_remove_id == charge.id {
                                    ui.label("确认删除此款项？删除后客户待收汇总会重新计算。");
                                    ui.horizontal(|ui| {
                                        if ui.button("确认删除款项").clicked() {
                                            action = Some(json!({"op":"removeClientCharge","id":node.id,
                                                "chargeId":charge.id}));
                                        }
                                        if ui.button("取消").clicked() {
                                            d.charge_remove_id.clear();
                                        }
                                    });
                                }
                                for receipt in &charge.receipts {
                                    ui.separator();
                                    ui.label(format!("收款 {} · {} · {}",
                                        customer_money_label(receipt.amount_cents),
                                        if receipt.received_on.is_empty() {"历史日期未知"} else {&receipt.received_on},
                                        if receipt.location.is_empty() {"地点未填写"} else {&receipt.location}));
                                    ui.label(format!("收款方式：{}",
                                        if receipt.channel.is_empty() {"未填写"} else {&receipt.channel}));
                                    if !receipt.note.is_empty() {
                                        ui.label(format!("收款备注：{}", receipt.note));
                                    }
                                    ui.horizontal_wrapped(|ui| {
                                        if ui.small_button("修改本次收款").clicked() {
                                            d.charge_id = charge.id.clone();
                                            d.receipt_edit_id = receipt.id.clone();
                                            d.receipt_amount = customer_money_editable(Some(receipt.amount_cents));
                                            d.receipt_date = receipt.received_on.clone();
                                            d.receipt_location = receipt.location.clone();
                                            d.receipt_channel = receipt.channel.clone();
                                            d.receipt_note = receipt.note.clone();
                                        }
                                        if ui.small_button("删除本次收款…").clicked() {
                                            d.receipt_remove_id = receipt.id.clone();
                                        }
                                    });
                                    if d.receipt_remove_id == receipt.id {
                                        ui.label("确认删除本次收款？客户已收和待收汇总会同步重新计算。");
                                        ui.horizontal(|ui| {
                                            if ui.button("确认删除收款").clicked() {
                                                action = Some(json!({"op":"removeClientReceipt","id":node.id,
                                                    "chargeId":charge.id, "receiptId":receipt.id}));
                                            }
                                            if ui.button("取消").clicked() {
                                                d.receipt_remove_id.clear();
                                            }
                                        });
                                    }
                                }
                                if ui.button("记录这项款项的新收款").clicked() {
                                    d.charge_id = charge.id.clone();
                                    d.receipt_edit_id.clear();
                                    d.receipt_amount.clear();
                                    d.receipt_date.clear();
                                    d.receipt_location.clear();
                                    d.receipt_channel.clear();
                                    d.receipt_note.clear();
                                }
                            });
                        }
                    }
                    if !d.charge_id.is_empty() {
                        ui.separator();
                        ui.label(if d.receipt_edit_id.is_empty() {
                            "记录实际收款"
                        } else {
                            "修改实际收款"
                        });
                        ui.horizontal(|ui| {
                            ui.label("金额（元）");
                            ui.add(egui::TextEdit::singleline(&mut d.receipt_amount).desired_width(112.0));
                        });
                        ui.label("实际日期（YYYY-MM-DD；历史金额整理前未知的日期可以留空）");
                        ui.text_edit_singleline(&mut d.receipt_date);
                        ui.label("实际收款地点");
                        ui.text_edit_singleline(&mut d.receipt_location);
                        ui.label("收款方式");
                        ui.text_edit_singleline(&mut d.receipt_channel);
                        ui.label("收款备注");
                        ui.add(egui::TextEdit::multiline(&mut d.receipt_note).desired_rows(2));
                        ui.horizontal(|ui| {
                            if d.receipt_edit_id.is_empty() {
                                if ui.button("保存本次收款").clicked() {
                                    action = Some(json!({"op":"addClientReceipt","id":node.id,
                                        "chargeId":d.charge_id,
                                        "receiptId":random_desktop_identifier("customer-receipt"),
                                        "amountYuan":d.receipt_amount, "receivedOn":d.receipt_date,
                                        "location":d.receipt_location, "channel":d.receipt_channel,
                                        "note":d.receipt_note}));
                                }
                            } else if ui.button("保存收款修改").clicked() {
                                action = Some(json!({"op":"updateClientReceipt","id":node.id,
                                    "chargeId":d.charge_id,"receiptId":d.receipt_edit_id,
                                    "amountYuan":d.receipt_amount, "receivedOn":d.receipt_date,
                                    "location":d.receipt_location,"channel":d.receipt_channel,
                                    "note":d.receipt_note}));
                            }
                            if ui.button("关闭收款编辑").clicked() {
                                d.charge_id.clear();
                                d.receipt_edit_id.clear();
                                d.receipt_amount.clear();
                                d.receipt_date.clear();
                                d.receipt_location.clear();
                                d.receipt_channel.clear();
                                d.receipt_note.clear();
                            }
                        });
                    }
                    ui.separator();
                    ui.label("新增被此客户推荐的客户");
                    ui.text_edit_singleline(&mut d.child_name);
                    if ui.add_enabled(!d.child_name.trim().is_empty(),
                        egui::Button::new("新增被推荐客户")).clicked() {
                        new_child = Some(d.child_name.clone());
                    }
                });
            });
        if let Some(request) = action {
            match self.knowledge_canvas_update_customer(&node.id, request) {
                Ok(()) => {
                    self.status = "客户资料已更新，等待本地画布保存完成".into();
                    self.desktop_ui.knowledge_canvas.customer_draft.client_id.clear();
                },
                Err(error) => self.status = format!("客户更新未保存：{error}"),
            }
        }
        if let Some(name) = new_child {
            self.knowledge_canvas_add_customer_named(&name, Some(&node.id));
        }
    }
}

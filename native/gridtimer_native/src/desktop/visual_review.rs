// v2.22.48 - Inspect timer records, archive navigation and Android risk labels.
// v2.22.47 - Inspect Android parity pages and render PNG exports in a real native window.
// Opt-in visual inspection only; this is not a pixel snapshot assertion.
// 2026-11-16: Optional real-window Chinese-caption video frames and native mouse event receipts.

fn parity_visual_finance_profile() -> Value {
    let mut ledgers = serde_json::Map::new();
    for (month, days, income) in [("2026-08", 31, 9000), ("2026-09", 15, 10000)] {
        for day in 1..=days {
            let incomes = if day == 1 {
                json!([{"id":format!("income-{month}"),"kind":"ACTIVE","name":"工资","amount":income}])
            } else {
                json!([])
            };
            ledgers.insert(format!("{month}-{day:02}"), json!({"incomes":incomes,"expenses":[{"id":format!("expense-{month}-{day}"),"bucket":"FOOD","name":"日常餐饮","amount":if day==12 {300} else {60}}],"confirmedAtEpochMillis":1}));
        }
    }
    json!({"dailyLedgers":ledgers,"monthlySnapshots":{
        "2026-08":{"assets":[{"id":"cash-aug","kind":"CASH_RESERVE","name":"存款","amount":16000}],"liabilities":[],"confirmedAtEpochMillis":1},
        "2026-09":{"assets":[{"id":"cash-sep","kind":"CASH_RESERVE","name":"存款","amount":20000}],"liabilities":[{"id":"debt-sep","kind":"LIABILITY_BALANCE","name":"分期余额","amount":3000}],"confirmedAtEpochMillis":1}
    }})
}

#[cfg(target_os = "windows")]
#[test]
#[ignore = "opens a native window for visual review"]
fn native_window_visual_review() {
    use winit::platform::windows::EventLoopBuilderExtWindows;
    let output = PathBuf::from(
        std::env::var_os("TIMER_VISUAL_REVIEW_DIR").expect("review output directory"),
    );
    fs::create_dir_all(&output).unwrap();
    let video_dir = std::env::var_os("TIMER_CHINESE_VIDEO_FRAMES_DIR").map(PathBuf::from);
    if let Some(dir) = &video_dir { fs::create_dir_all(dir).unwrap(); }
    let profile = output.join(format!("profile-{}", std::process::id()));
    fs::create_dir_all(&profile).unwrap();
    let now = now_millis();
    let mut state: Value = serde_json::from_str(&knowledge_fixture_state()).unwrap();
    let timer_history: Value = serde_json::from_str(&history_test_state(now)).unwrap();
    state["sessions"] = timer_history["sessions"].clone();
    state["archivedTasks"] = timer_history["archivedTasks"].clone();
    for (index, (title, note, elapsed)) in [
        (
            "完成产品方案",
            "梳理用户流程，整理本周的改进清单",
            5_042_000,
        ),
        (
            "阅读与摘录",
            "读完第三章，记下值得继续思考的问题",
            2_640_000,
        ),
        ("学习 Rust", "练习所有权与借用", 1_815_000),
        ("整理项目资料", "", 915_000),
        ("英语听力", "", 0),
        ("晚间复盘", "", 0),
    ]
    .into_iter()
    .enumerate()
    {
        let slot = &mut state["slots"][index];
        slot["title"] = json!(title);
        slot["note"] = json!(note);
        slot["accumulatedMillis"] = json!(elapsed);
    }
    if std::env::var_os("TIMER_VISUAL_FINANCE_DATA").is_some() {
        state["financeProfile"] = parity_visual_finance_profile();
    }
    let raw = app_data::sanitize_app_data_json(&state.to_string(), now).unwrap();
    let mut client = test_client_for_account_scope(&profile, "visual-review", raw);
    client.sync.token.clear();
    client.sync.server_url.clear();
    client.settings.timer_bell_enabled = false;
    client.settings.language = "zh".into();
    ACTIVE_DESKTOP_LANGUAGE.store(desktop_language_code("zh"), AtomicOrdering::Relaxed);
    client.status = "本地数据已就绪".into();
    struct ReviewApp {
        client: TimerWindowsClient,
        output: PathBuf,
        scene: usize,
        frames: usize,
        started: Instant,
        video_dir: Option<PathBuf>,
        video_index: usize,
        clicked_positions: usize,
        attempted_clicks: usize,
        last_click: Option<(egui::Pos2, usize)>,
    }
    #[cfg(target_os = "windows")]
    fn send_native_demo_click(ctx: &egui::Context, scene: &str) -> bool {
        #[repr(C)]
        struct NativePoint { x: i32, y: i32 }
        #[link(name = "user32")]
        extern "system" {
            fn FindWindowW(class: *const u16, title: *const u16) -> isize;
            fn ClientToScreen(window: isize, point: *mut NativePoint) -> i32;
            fn SetCursorPos(x: i32, y: i32) -> i32;
            fn SetForegroundWindow(window: isize) -> i32;
            fn mouse_event(flags: u32, x: u32, y: u32, data: u32, extra: usize);
        }
        let (x, y) = match scene {
            "board_light" | "board_running" => (273.0, 474.0),
            "notes_light" => (260.0, 97.0),
            "knowledge_light" => (282.0, 84.0),
            "canvas_customer" => (551.0, 491.0),
            "finance_light" => (258.0, 151.0),
            "finance_cockpit" => (331.0, 151.0),
            "account_light" => (900.0, 136.0),
            _ => return false,
        };
        let title = "十倍率 · 界面检查".encode_utf16().chain(std::iter::once(0)).collect::<Vec<_>>();
        let window = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
        if window == 0 { return false; }
        let scale = ctx.pixels_per_point();
        let mut point = NativePoint { x: (x * scale) as i32, y: (y * scale) as i32 };
        if unsafe { ClientToScreen(window, &mut point) } == 0 { return false; }
        unsafe {
            SetForegroundWindow(window);
            if SetCursorPos(point.x, point.y) == 0 { return false; }
            mouse_event(0x0002, 0, 0, 0, 0);
            mouse_event(0x0004, 0, 0, 0, 0);
        }
        true
    }
    fn scene_caption(scene: &str) -> &'static str {
        match scene {
            "board_light" => "计时：查看任务、点击开始",
            "board_running" => "计时：运行中的任务、点击暂停",
            "board_dark" => "计时：深色模式",
            "board_compact" => "计时：紧凑窗口",
            "board_minimum" => "计时：最小窗口",
            "timer_editor" => "计时：任务详情与编辑",
            "board_empty" => "计时：搜索无结果",
            "notes_light" => "便签：列表与内容",
            "notes_dark" => "便签：深色模式",
            "notes_compact" => "便签：紧凑窗口",
            "notes_minimum" => "便签：最小窗口",
            "knowledge_light" => "知识：页面浏览",
            "knowledge_dark" => "知识：深色模式",
            "canvas_customer" => "知识：客户画布与收付款记录",
            "canvas_customer_zoom" => "知识：客户画布缩放",
            "history_light" => "计时历史：记录列表",
            "history_minimum" => "计时历史：最小窗口",
            "history_empty" => "计时历史：搜索无结果",
            "history_detail_return" => "计时历史：从任务详情返回",
            "archive_light" => "计时历史：归档任务",
            "finance_light" => "风控：财务与交易记录",
            "finance_dark" => "风控：深色模式",
            "account_light" => "我的：账户与设置",
            "knowledge_compact" => "知识：紧凑窗口",
            "knowledge_minimum" => "知识：最小窗口",
            "finance_compact" => "风控：紧凑窗口",
            "finance_minimum" => "风控：最小窗口",
            "account_oled" => "我的：深黑主题",
            "finance_cockpit" => "风控：总览",
            "finance_budgets" => "风控：预算",
            "finance_forecast" => "风控：预测",
            "finance_health" => "风控：健康评估",
            "finance_snapshot" => "风控：财务快照",
            "finance_trend" => "风控：趋势",
            _ => "十倍率 Windows 客户端",
        }
    }
    const SCENES: &[(&str, AppTab, u8, f32, f32)] = &[
        ("board_light", AppTab::Board, 1, 1240.0, 800.0),
        ("board_running", AppTab::Board, 1, 1240.0, 800.0),
        ("board_dark", AppTab::Board, 2, 1240.0, 800.0),
        ("board_compact", AppTab::Board, 1, 760.0, 560.0),
        ("board_minimum", AppTab::Board, 1, 760.0, 480.0),
        ("timer_editor", AppTab::Board, 1, 1000.0, 720.0),
        ("board_empty", AppTab::Board, 1, 1240.0, 800.0),
        ("notes_light", AppTab::Notes, 1, 1240.0, 800.0),
        ("notes_dark", AppTab::Notes, 2, 1240.0, 800.0),
        ("notes_compact", AppTab::Notes, 1, 760.0, 560.0),
        ("notes_minimum", AppTab::Notes, 1, 760.0, 480.0),
        ("knowledge_light", AppTab::Knowledge, 1, 1240.0, 800.0),
        ("knowledge_dark", AppTab::Knowledge, 2, 1240.0, 800.0),
        ("canvas_customer", AppTab::Knowledge, 1, 1240.0, 800.0),
        ("canvas_customer_zoom", AppTab::Knowledge, 1, 1240.0, 800.0),
        ("history_light", AppTab::History, 1, 1240.0, 800.0),
        ("history_minimum", AppTab::History, 1, 760.0, 480.0),
        ("history_empty", AppTab::History, 1, 760.0, 480.0),
        ("history_detail_return", AppTab::History, 1, 1000.0, 720.0),
        ("archive_light", AppTab::History, 1, 1240.0, 800.0),
        ("finance_light", AppTab::Finance, 1, 1240.0, 800.0),
        ("finance_dark", AppTab::Finance, 2, 1240.0, 800.0),
        ("account_light", AppTab::My, 1, 1240.0, 800.0),
        ("knowledge_compact", AppTab::Knowledge, 1, 760.0, 560.0),
        ("knowledge_minimum", AppTab::Knowledge, 1, 760.0, 480.0),
        ("finance_compact", AppTab::Finance, 1, 760.0, 560.0),
        ("finance_minimum", AppTab::Finance, 1, 760.0, 480.0),
        ("account_oled", AppTab::My, 3, 1000.0, 720.0),
        ("finance_cockpit", AppTab::Finance, 1, 1240.0, 800.0),
        ("finance_budgets", AppTab::Finance, 1, 1240.0, 800.0),
        ("finance_forecast", AppTab::Finance, 1, 1240.0, 800.0),
        ("finance_health", AppTab::Finance, 1, 1240.0, 800.0),
        ("finance_snapshot", AppTab::Finance, 1, 1240.0, 800.0),
        ("finance_trend", AppTab::Finance, 1, 760.0, 560.0),
    ];
    impl eframe::App for ReviewApp {
        fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
            if self.scene >= SCENES.len() {
                self.client.poll_note_image_export(ctx, frame);
                if self.client.desktop_ui.parity.image_export.busy() {
                    ctx.request_repaint_after(Duration::from_millis(100));
                    return;
                }
                fs::write(self.output.join("export_status.json"),json!({"status":self.client.status,"path":self.client.transfers.last_note_export}).to_string()).unwrap();
                if self.video_dir.is_some() {
                    fs::write(self.output.join("video_receipts.json"), json!({
                        "videoMode": true,
                        "frames": self.video_index,
                        "scenes": SCENES.len(),
                        "nativeClickAttempts": self.attempted_clicks,
                        "nativeClickEventsObserved": self.clicked_positions,
                        "note": "Scenes other than native click receipts are programmatic visual states, not interactive coverage."
                    }).to_string()).unwrap();
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
            assert!(
                self.started.elapsed() < Duration::from_secs(if self.video_dir.is_some() { 300 } else { 150 }),
                "native capture timeout"
            );
            if self.frames > 0 {
                let images = ctx.input(|i| {
                    i.events
                        .iter()
                        .filter_map(|e| match e {
                            egui::Event::Screenshot { image, .. } => Some(image.clone()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                });
                for captured in images {
                    let pixels: Vec<u8> =
                        captured.pixels.iter().flat_map(|p| p.to_array()).collect();
                    if let Some(video_dir) = &self.video_dir {
                        let width = captured.width() as u32;
                        let height = captured.height() as u32;
                        let native = image::RgbaImage::from_raw(width, height, pixels.clone()).unwrap();
                        let factor = f64::min(1280.0 / width as f64, 840.0 / height as f64);
                        let target_w = (width as f64 * factor).round() as u32;
                        let target_h = (height as f64 * factor).round() as u32;
                        let scaled = image::imageops::resize(
                            &native, target_w, target_h, image::imageops::FilterType::Triangle
                        );
                        let mut canvas = image::RgbaImage::from_pixel(
                            1280, 840, image::Rgba([239, 243, 249, 255])
                        );
                        image::imageops::overlay(&mut canvas, &scaled,
                            ((1280 - target_w) / 2) as i64, ((840 - target_h) / 2) as i64
                        );
                        canvas.save(video_dir.join(format!("{:05}.png", self.video_index))).unwrap();
                        self.video_index += 1;
                        if self.frames < 24 { continue; }
                    }
                    image::save_buffer(
                        self.output.join(format!("{}.png", SCENES[self.scene].0)),
                        &pixels,
                        captured.width() as u32,
                        captured.height() as u32,
                        image::ColorType::Rgba8,
                    )
                    .unwrap();
                    self.scene += 1;
                    self.frames = 0;
                    self.last_click = None;
                    if self.scene == SCENES.len() {
                        ctx.request_repaint();
                        return;
                    }
                }
            }
            let (_, tab, theme, width, height) = SCENES[self.scene];
            if self.frames == 0 {
                ACTIVE_DESKTOP_THEME.store(theme, AtomicOrdering::Relaxed);
                self.client.data.theme_mode = if theme == 1 { "LIGHT" } else { "DARK" }.into();
                self.client.data.oled_theme_enabled = theme == 3;
                install_ui_style(ctx);
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(width, height)));
                self.client.switch_tab(tab);
                self.client.data.note_preferences.selected_folder_id = None;
                self.client.view_cache.data_version = u64::MAX;
                self.client.desktop_ui.slot_editor_open = false;
                self.client.desktop_ui.board_query.clear();
                if tab == AppTab::History {
                    self.client.switch_tab(AppTab::Board);
                    let detail = SCENES[self.scene].0 == "history_detail_return";
                    if detail {
                        self.client.open_slot_editor(1);
                    }
                    assert!(self.client.open_timer_history(
                        detail.then_some(1),
                        SCENES[self.scene].0 == "archive_light",
                    ));
                    if SCENES[self.scene].0 == "history_empty" {
                        self.client.desktop_ui.history_query = "不存在的记录".into();
                    }
                }
                if SCENES[self.scene].0 == "board_running" {
                    self.client
                        .toggle_slot(&self.client.selected_slot().unwrap(), ctx);
                }
                if SCENES[self.scene].0 == "timer_editor" {
                    self.client.open_slot_editor(1);
                }
                if SCENES[self.scene].0 == "board_empty" {
                    self.client.desktop_ui.board_query = "找不到的任务".into();
                }
                self.client.rebuild_view_cache();
                if tab == AppTab::Notes {
                    self.client.select_note_by_id("sticky-a");
                    if SCENES[self.scene].0 == "notes_light" {
                        let note = self.client.note_sharing_snapshot().unwrap();
                        let html = self.client.note_sharing_html(&note).unwrap();
                        self.client.desktop_ui.parity.image_export = NoteImageExport {
                            page: Some(note_image_page(&html)),
                            path: Some(self.output.join("note_export.png")),
                            workspace: self.client.background_job_workspace_fingerprint(),
                            started: Some(Instant::now()),
                            ..Default::default()
                        };
                    }
                }
                if tab == AppTab::Knowledge {
                    if SCENES[self.scene].0.starts_with("canvas_customer") {
                        self.client.return_to_document_list();
                        self.client.knowledge_view = KnowledgeCollectionView::Canvas;
                        assert!(self.client.ensure_knowledge_canvas_loaded());
                        if !self.client.desktop_ui.knowledge_canvas.store.active()
                            .is_some_and(|canvas| canvas.nodes.iter()
                                .any(|node| node.kind == KnowledgeCanvasNodeKind::Client))
                        {
                            self.client.knowledge_canvas_add_customer_named("演示客户", None);
                            let selected = self.client.desktop_ui.knowledge_canvas
                                .selected_nodes[0].clone();
                            self.client.knowledge_canvas_update_customer(&selected,
                                json!({"op":"saveProfile","name":"演示客户",
                                    "details":"独立知识画布客户记录","paidYuan":"560.00",
                                    "pendingYuan":"660.00"})).unwrap();
                        }
                        if let Some(canvas) = self.client.desktop_ui.knowledge_canvas.store.active_mut() {
                            canvas.zoom = if SCENES[self.scene].0 == "canvas_customer_zoom" {
                                0.01
                            } else {
                                1.0
                            };
                        }
                    } else {
                        self.client.select_note_by_id("doc-a");
                    }
                }
                if let Some(page) = [
                    "finance_cockpit",
                    "finance_budgets",
                    "finance_forecast",
                    "finance_health",
                    "finance_snapshot",
                    "finance_trend",
                ]
                .iter()
                .position(|name| *name == SCENES[self.scene].0)
                {
                    self.client.finance_workbench.tab = 5;
                    self.client.finance_workbench.overview.page = page;
                    self.client.finance_workbench.overview.date = "2026-09-15".into();
                    self.client.finance_workbench.overview.month = "2026-09".into();
                    self.client.finance_workbench.overview.year = "2026".into();
                }
            }
            self.client.poll_note_image_export(ctx, frame);
            self.client.refresh_timer_projection(now_millis());
            if self.video_dir.is_none() {
                ctx.input_mut(|i| i.pointer = Default::default());
            }
            if self.video_dir.is_some() && self.frames == 7 &&
                send_native_demo_click(ctx, SCENES[self.scene].0) {
                self.attempted_clicks += 1;
            }
            if self.video_dir.is_some() {
                if let Some(pos) = ctx.input(|i| {
                    if i.pointer.any_pressed() { i.pointer.interact_pos() } else { None }
                }) {
                    self.clicked_positions += 1;
                    self.last_click = Some((pos, self.frames));
                }
            }
            self.client.ui_workspace(ctx);
            if self.video_dir.is_some() {
                let screen = ctx.screen_rect();
                let painter = ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Foreground, egui::Id::new("chinese_video_caption")
                ));
                let footer = egui::Rect::from_min_max(
                    egui::pos2(205.0, screen.bottom() - 51.0),
                    egui::pos2(screen.right() - 10.0, screen.bottom() - 9.0)
                );
                painter.rect_filled(footer, 6.0, egui::Color32::from_rgba_unmultiplied(15, 32, 57, 240));
                painter.text(footer.center(), egui::Align2::CENTER_CENTER,
                    scene_caption(SCENES[self.scene].0),
                    egui::FontId::proportional(19.0), egui::Color32::WHITE);
                if let Some((pos, at)) = self.last_click {
                    if self.frames.saturating_sub(at) < 10 {
                        painter.circle_filled(pos, 12.0, egui::Color32::from_rgba_unmultiplied(252, 176, 37, 75));
                        painter.circle_stroke(pos, 15.0, egui::Stroke::new(3.0, egui::Color32::from_rgb(255, 172, 41)));
                    }
                }
            }
            self.frames += 1;
            if (self.video_dir.is_some() && self.frames % 2 == 0 && self.frames <= 24)
                || (self.video_dir.is_none() && self.frames == 8) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
            }
            ctx.request_repaint_after(Duration::from_millis(80));
        }
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1240.0, 800.0])
            .with_position([40.0, 40.0]),
        event_loop_builder: Some(Box::new(|builder| {
            builder.with_any_thread(true);
        })),
        persist_window: false,
        ..Default::default()
    };
    let expected_output = output.clone();
    eframe::run_native(
        "十倍率 · 界面检查",
        options,
        Box::new(move |cc| {
            install_ui_fonts(&cc.egui_ctx);
            Box::new(ReviewApp {
                client,
                output,
                scene: 0,
                frames: 0,
                started: Instant::now(),
                video_dir,
                video_index: 0,
                clicked_positions: 0,
                attempted_clicks: 0,
                last_click: None,
            })
        }),
    )
    .unwrap();
    assert!(SCENES
        .iter()
        .all(|s| expected_output.join(format!("{}.png", s.0)).is_file()));
    let exported = image::open(expected_output.join("note_export.png"))
        .expect("native WebView PNG export must decode");
    assert_eq!(1080, exported.width());
    assert!(exported.height() > 128);
}

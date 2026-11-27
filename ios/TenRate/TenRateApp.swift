import SwiftUI

@main
struct TenRateApp: App {
    @StateObject private var store = WorkspaceStore()
    var body: some Scene {
        WindowGroup {
            Group {
                if store.ready {
                    TabView {
                        TimerHome().tabItem { Label("计时", systemImage: "square.grid.2x2") }
                        NotesHome().tabItem { Label("便签", systemImage: "note.text") }
                        HistoryHome().tabItem { Label("记录", systemImage: "clock.arrow.circlepath") }
                        SettingsHome().tabItem { Label("我的", systemImage: "person.crop.circle") }
                    }
                } else {
                    ContentUnavailableView {
                        Label("TenRate", systemImage: "timer")
                    } description: {
                        Text(store.error == nil ? "正在打开工作区…" : "工作区未打开，原有文件已保留。")
                    } actions: {
                        if store.error != nil { Button("重试") { Task { await store.load() } } }
                        else { ProgressView() }
                    }
                }
            }
            .environmentObject(store)
            .tint(.indigo)
            .task { await store.load() }
            .alert("未能完成", isPresented: Binding(get: { store.error != nil }, set: { if !$0 { store.error = nil } })) {
                Button("知道了", role: .cancel) { store.error = nil }
            } message: { Text(store.error ?? "") }
        }
    }
}

struct TimerHome: View {
    @EnvironmentObject private var store: WorkspaceStore
    @State private var selected: TimerView?
    var body: some View {
        NavigationStack {
            TimelineView(.periodic(from: .now, by: 1)) { context in
                ScrollView {
                    VStack(alignment: .leading, spacing: 20) {
                        VStack(alignment: .leading, spacing: 6) {
                            Text(Date.now, format: .dateTime.month(.wide).day().weekday(.wide)).font(.subheadline).foregroundStyle(.secondary)
                            Text("把时间，留给重要的事。").font(.title2.bold())
                        }.padding(.top, 8)
                        LazyVGrid(columns: [GridItem(.adaptive(minimum: 280), spacing: 16)], spacing: 16) {
                            ForEach(store.timers(at: context.date)) { timer in
                                TimerCard(timer: timer, open: { selected = timer })
                            }
                        }
                    }.padding(20)
                }.background(Color(uiColor: .systemGroupedBackground))
            }
            .navigationTitle("计时")
            .sheet(item: $selected) { timer in SlotSheet(slotId: timer.id) }
        }
    }
}

struct TimerCard: View {
    @EnvironmentObject private var store: WorkspaceStore
    let timer: TimerView
    let open: () -> Void
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Button(action: open) {
                VStack(alignment: .leading, spacing: 12) {
                    HStack {
                        Text(store.categoryName(timer.categoryId)).font(.caption.weight(.medium)).foregroundStyle(.indigo)
                        Spacer()
                        Text(String(format: "%02d", timer.id)).font(.caption.monospacedDigit()).foregroundStyle(.secondary)
                        Image(systemName: "ellipsis").foregroundStyle(.secondary)
                    }
                    Text(timer.title.isEmpty ? "格子 \(timer.id)" : timer.title)
                        .font(.title3.weight(.semibold)).foregroundStyle(.primary).multilineTextAlignment(.leading)
                    Text(Core.duration(timer.accumulatedMillis)).font(.system(.largeTitle, design: .rounded).monospacedDigit().weight(.medium))
                        .foregroundStyle(.primary).contentTransition(.numericText()).minimumScaleFactor(0.6).lineLimit(1)
                }.frame(maxWidth: .infinity, alignment: .leading).contentShape(Rectangle())
            }.buttonStyle(.plain).accessibilityHint("打开格子详情和快捷操作")
            HStack {
                Label(timer.isRunning ? (timer.microBreakPhase == "BREAK" ? "短休息" : "专注中") : "准备开始", systemImage: timer.isRunning ? "circle.fill" : "circle")
                    .font(.caption).foregroundStyle(timer.isRunning ? .green : .secondary)
                Spacer()
                Button {
                    Task { await store.mutate(["action": timer.isRunning ? "pause" : "start", "slotId": timer.id]) }
                } label: {
                    Label(timer.isRunning ? "暂停" : "开始", systemImage: timer.isRunning ? "pause.fill" : "play.fill")
                        .font(.subheadline.weight(.semibold)).padding(.horizontal, 8).padding(.vertical, 5)
                }.buttonStyle(.borderedProminent).disabled(store.saving)
                    .accessibilityLabel("\(timer.title.isEmpty ? "格子 \(timer.id)" : timer.title)，\(timer.isRunning ? "暂停" : "开始")")
            }
        }
        .padding(20).background(.background, in: RoundedRectangle(cornerRadius: 24))
        .overlay(RoundedRectangle(cornerRadius: 24).strokeBorder(timer.isRunning ? Color.indigo.opacity(0.35) : Color.primary.opacity(0.05)))
    }
}

struct SlotSheet: View {
    @EnvironmentObject private var store: WorkspaceStore
    @Environment(\.dismiss) private var dismiss
    let slotId: Int
    @State private var title = ""
    @State private var confirmReset = false
    @State private var confirmArchive = false
    var body: some View {
        NavigationStack {
            TimelineView(.periodic(from: .now, by: 1)) { context in
                if let timer = store.timers(at: context.date).first(where: { $0.id == slotId }) {
                    Form {
                        Section {
                            Text(Core.duration(timer.accumulatedMillis)).font(.system(.largeTitle, design: .rounded).monospacedDigit()).frame(maxWidth: .infinity).padding(.vertical, 10)
                            TextField("任务名称", text: $title).accessibilityLabel("任务名称")
                            Button("保存名称") { Task { await store.mutate(["action": "title", "slotId": slotId, "text": title]) } }
                            Picker("分类", selection: Binding(get: { timer.categoryId ?? "" }, set: { value in
                                Task { await store.mutate(["action": "category", "slotId": slotId, "text": value]) }
                            })) {
                                Text("未分类").tag("")
                                ForEach(store.categories) { Text($0.name).tag($0.id) }
                            }
                        }
                        Section {
                            Button { Task { await store.mutate(["action": timer.isRunning ? "pause" : "start", "slotId": slotId]) } } label: {
                                Label(timer.isRunning ? "暂停计时" : "开始计时", systemImage: timer.isRunning ? "pause.fill" : "play.fill")
                            }
                            Button { confirmArchive = true } label: { Label("归档任务", systemImage: "archivebox") }.disabled(timer.isRunning)
                            Button(role: .destructive) { confirmReset = true } label: { Label("重置计时", systemImage: "arrow.counterclockwise") }.disabled(timer.isRunning)
                        } footer: { Text("归档或重置前请先暂停计时。历史记录会保留。") }
                    }.disabled(store.saving)
                }
            }
            .navigationTitle(String(format: "格子 %02d", slotId)).navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("完成", systemImage: "xmark") { dismiss() }.accessibilityLabel("关闭快捷操作") } }
            .task { title = store.timers(at: .now).first { $0.id == slotId }?.title ?? "" }
            .confirmationDialog("重置本格子的累计时间？历史记录会保留。", isPresented: $confirmReset, titleVisibility: .visible) {
                Button("重置计时", role: .destructive) { Task { await store.mutate(["action": "reset", "slotId": slotId]) } }
            }
            .confirmationDialog("归档后可在记录页恢复任务。", isPresented: $confirmArchive, titleVisibility: .visible) {
                Button("归档任务") { Task { if await store.mutate(["action": "archive", "slotId": slotId, "text": UUID().uuidString]) { dismiss() } } }
            }
        }.presentationDragIndicator(.visible).presentationDetents([.large])
    }
}

struct HistoryHome: View {
    @EnvironmentObject private var store: WorkspaceStore
    var body: some View {
        NavigationStack {
            List {
                if !store.archives.isEmpty {
                    Section("已归档") {
                        ForEach(store.archives) { task in
                            VStack(alignment: .leading, spacing: 8) {
                                Text(task.title.isEmpty ? "未命名任务" : task.title).font(.headline)
                                Text(Core.duration(task.accumulatedMillis)).monospacedDigit().foregroundStyle(.secondary)
                                Button("恢复到空格子") { Task { await store.mutate(["action": "restoreArchive", "text": task.id]) } }.disabled(store.saving)
                            }.padding(.vertical, 6)
                        }
                    }
                }
                Section("计时记录") {
                    ForEach(store.sessions) { session in
                        VStack(alignment: .leading, spacing: 6) {
                            Text(session.slotTitle.isEmpty ? "未命名任务" : session.slotTitle).font(.headline)
                            Text(Core.duration(session.durationMillis)).monospacedDigit()
                            Text(Date(timeIntervalSince1970: Double(session.endedAtEpochMillis) / 1000), format: .dateTime.month().day().hour().minute()).font(.caption).foregroundStyle(.secondary)
                        }.padding(.vertical, 6)
                    }
                }
            }.overlay {
                if store.sessions.isEmpty && store.archives.isEmpty { ContentUnavailableView("还没有记录", systemImage: "clock", description: Text("开始一次计时，暂停后会自动记录。")) }
            }.navigationTitle("记录")
        }
    }
}

struct NotesHome: View {
    @EnvironmentObject private var store: WorkspaceStore
    @State private var search = ""
    @State private var selected: Note?
    @State private var trash = false
    var body: some View {
        NavigationStack {
            let notes = store.notes.filter { $0.deleted == trash && (search.isEmpty || ($0.title + $0.content).localizedCaseInsensitiveContains(search)) }
            List {
                ForEach(notes) { note in
                    Button { selected = note } label: {
                        VStack(alignment: .leading, spacing: 8) {
                            Text(note.title.isEmpty ? "未命名便签" : note.title).font(.headline).foregroundStyle(.primary)
                            Text(note.editable ? note.content : "此文档暂不支持在 iOS 编辑").font(.subheadline).foregroundStyle(.secondary).lineLimit(3)
                        }.padding(.vertical, 8).frame(maxWidth: .infinity, alignment: .leading)
                    }
                    .swipeActions {
                        if trash {
                            Button("恢复", systemImage: "arrow.uturn.backward") { Task { await store.mutate(["action": "restoreNote", "text": note.id]) } }.tint(.green)
                        } else {
                            Button("移到回收站", systemImage: "trash", role: .destructive) { Task { await store.mutate(["action": "deleteNote", "text": note.id]) } }
                        }
                    }.disabled(store.saving)
                }
            }
            .overlay { if notes.isEmpty { ContentUnavailableView(search.isEmpty ? "这里还没有便签" : "没有匹配的便签", systemImage: "note.text", description: Text("用右上角的加号记录一个想法。")) } }
            .navigationTitle(trash ? "回收站" : "便签").searchable(text: $search, prompt: "搜索标题和正文")
            .toolbar {
                ToolbarItem(placement: .topBarLeading) { Button(trash ? "返回便签" : "回收站", systemImage: trash ? "note.text" : "trash") { trash.toggle() } }
                ToolbarItem(placement: .topBarTrailing) {
                    Button("新建便签", systemImage: "plus") { selected = Note(value: ["id": UUID().uuidString, "kind": "STICKY", "createdAtEpochMillis": Core.now]) }.disabled(trash)
                }
            }.sheet(item: $selected) { NoteEditor(note: $0) }
        }
    }
}

struct NoteEditor: View {
    @EnvironmentObject private var store: WorkspaceStore
    @Environment(\.dismiss) private var dismiss
    let note: Note
    @State private var title = ""
    @State private var content = ""
    @State private var discard = false
    private var changed: Bool { title != note.title || content != note.content }
    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 12) {
                if !note.editable || note.deleted {
                    ContentUnavailableView("暂不可编辑", systemImage: "lock.doc", description: Text(note.deleted ? "请先从回收站恢复此便签。" : "结构化文档、富文本及加密内容请继续使用现有客户端。"))
                } else {
                    TextField("标题", text: $title, axis: .vertical).font(.title2.bold()).padding(.horizontal, 20).padding(.top, 20)
                    Divider().padding(.horizontal, 20)
                    TextEditor(text: $content).padding(.horizontal, 16).accessibilityLabel("便签正文").scrollDismissesKeyboard(.interactively)
                }
            }
            .navigationTitle("便签").navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("关闭") { if changed { discard = true } else { dismiss() } }.disabled(store.saving) }
                ToolbarItem(placement: .confirmationAction) {
                    Button("保存") {
                        var value = note.value
                        value["title"] = title; value["content"] = content
                        Task { if await store.mutate(["action": "saveNote", "note": value]) { dismiss() } }
                    }.disabled(store.saving || !note.editable || note.deleted)
                }
            }.task { title = note.title; content = note.content }
            .interactiveDismissDisabled(changed || store.saving)
            .confirmationDialog("放弃未保存的修改？", isPresented: $discard, titleVisibility: .visible) {
                Button("放弃修改", role: .destructive) { dismiss() }
            }
        }
    }
}

struct SettingsHome: View {
    @EnvironmentObject private var store: WorkspaceStore
    var body: some View {
        NavigationStack {
            List {
                Section {
                    Label("TenRate", systemImage: "timer").font(.title2.bold())
                    LabeledContent("iOS 首版", value: "1.0.0.1")
                    Text("计时与便签保存在此 iPhone。本版支持计时、分类、历史、任务归档，以及纯文本便签。")
                }
                Section("备份") {
                    ShareLink(item: BackupDocument(raw: store.raw), preview: SharePreview("TenRate 工作区备份")) {
                        Label("导出工作区 JSON", systemImage: "square.and.arrow.up")
                    }
                    Text("备份包含便签正文，请保存到你信任的位置。").font(.footnote).foregroundStyle(.secondary)
                }
                Section("功能进度") {
                    Text("账户登录、跨端同步、财务、AI、知识画布和附件尚未接入。本版不会连接或修改现有账户。")
                }
            }.navigationTitle("我的")
        }
    }
}

import UniformTypeIdentifiers
struct BackupDocument: Transferable {
    let raw: String
    static var transferRepresentation: some TransferRepresentation {
        DataRepresentation(exportedContentType: .json) { Data($0.raw.utf8) }
            .suggestedFileName("TenRate-workspace.json")
    }
}

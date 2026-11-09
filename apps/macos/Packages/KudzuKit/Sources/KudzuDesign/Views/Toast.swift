// The toast stack (sonner, as the web configures it: `<Toaster position="bottom-center" />` with
// popover colours and the 10 pt radius). There is no native in-window toast, so this is custom:
// bottom-centre, 24 pt from the edge, 356 pt wide, newest in front; older ones tuck behind, lifted
// 14 pt and scaled down 5% each, at most three visible; hovering expands the stack and pauses the
// timers; 4 s default lifetime; action and cancel buttons dismiss after running; swipe down to dismiss.
//
// `toast("Deleted “Draft”", action: .init("Undo") { restore() }, duration: 6)`, `toast.success(…)`.

import AppKit
import SwiftUI

/// The status indicated by a toast.
public enum ToastKind: Sendable { case normal, success, info, warning, error, loading }

/// A labelled action in a toast.
public struct ToastButton: Sendable {
    /// The user-visible label.
    public var label: String
    /// The main-actor action to run.
    public var action: @MainActor @Sendable () -> Void
    /// Creates ToastButton with the supplied content and styling.
    public init(_ label: String, action: @escaping @MainActor @Sendable () -> Void) {
        self.label = label
        self.action = action
    }
}

/// The content and lifetime of an in-window toast.
public struct ToastItem: Identifiable, Sendable {
    /// Stable identity for this value.
    public var id: String
    /// The toast’s status treatment.
    public var kind: ToastKind
    /// The user-visible title.
    public var title: String
    /// Supporting text below the title.
    public var description: String?
    /// The main-actor action to run.
    public var action: ToastButton?
    /// Optional secondary toast action.
    public var cancel: ToastButton?
    /// Seconds; `.infinity` stays until dismissed. Loading toasts never time out.
    public var duration: TimeInterval
}

@MainActor
@Observable
/// Owns toast presentation and pausable dismissal deadlines.
public final class ToastCenter {
    /// The shared presentation coordinator.
    public static let shared = ToastCenter()

    /// sonner's constants.
    public static let defaultDuration: TimeInterval = 4
    /// The `visibleCount` design-system value.
    public static let visibleCount = 3
    /// The `gap` design-system value.
    public static let gap: CGFloat = 14
    /// The `width` design-system value.
    public static let width: CGFloat = 356
    /// The `offset` design-system value.
    public static let offset: CGFloat = 24

    /// Newest first.
    public private(set) var toasts: [ToastItem] = []
    /// Whether the anchored content or toast stack is expanded.
    public var expanded = false { didSet { updatePause() } }
    /// Pointer presence pauses even a single toast, preserving time to reach Undo.
    public var hovered = false { didSet { updatePause() } }
    var appHidden = false { didSet { updatePause() } }
    private var paused: Bool { expanded || hovered || appHidden }

    private func updatePause() { paused ? pauseAll() : resumeAll() }

    private var timers: [String: DispatchWorkItem] = [:]
    private var remaining: [String: TimeInterval] = [:]
    private var started: [String: Date] = [:]
    private var counter = 0

    /// Creates ToastCenter with the supplied content and styling.
    public init() {}

    @discardableResult
    /// Shows or replaces a toast and resets its dismissal deadline.
    public func show(_ title: String, kind: ToastKind = .normal, description: String? = nil, action: ToastButton? = nil, cancel: ToastButton? = nil, duration: TimeInterval? = nil, id: String? = nil) -> String {
        counter += 1
        let item = ToastItem(id: id ?? "toast-\(counter)", kind: kind, title: title, description: description, action: action, cancel: cancel,
                             duration: kind == .loading ? .infinity : (duration ?? Self.defaultDuration))
        if let i = toasts.firstIndex(where: { $0.id == item.id }) {
            toasts[i] = item
        } else {
            toasts.insert(item, at: 0)
        }
        timers[item.id]?.cancel()
        timers[item.id] = nil
        started[item.id] = nil
        remaining[item.id] = item.duration
        if !paused { start(item.id) }
        return item.id
    }

    /// Dismisses one toast, or all when `id` is nil.
    public func dismiss(_ id: String? = nil) {
        let ids = id.map { [$0] } ?? toasts.map(\.id)
        for id in ids {
            timers[id]?.cancel()
            timers[id] = nil
            remaining[id] = nil
            started[id] = nil
        }
        toasts.removeAll { ids.contains($0.id) }
        if toasts.count <= 1 { expanded = false }
    }

    private func start(_ id: String) {
        timers[id]?.cancel()
        timers[id] = nil
        guard let left = remaining[id], left.isFinite else { return }
        started[id] = Date()
        let work = DispatchWorkItem { [weak self] in
            MainActor.assumeIsolated { self?.dismiss(id) }
        }
        timers[id] = work
        DispatchQueue.main.asyncAfter(deadline: .now() + max(0, left), execute: work)
    }

    private func pauseAll() {
        for (id, work) in timers {
            work.cancel()
            if let s = started[id], let left = remaining[id] { remaining[id] = left - Date().timeIntervalSince(s) }
        }
        timers.removeAll()
    }

    private func resumeAll() {
        for t in toasts where timers[t.id] == nil { start(t.id) }
    }
}

/// `toast(…)`, `toast.success(…)`, `toast.error(…)`, `toast.dismiss(id)`.
public struct ToastAPI: Sendable {
    @MainActor @discardableResult
    /// Shows a toast through the shared toast center.
    public func callAsFunction(_ title: String, description: String? = nil, action: ToastButton? = nil, cancel: ToastButton? = nil, duration: TimeInterval? = nil, id: String? = nil) -> String {
        ToastCenter.shared.show(title, description: description, action: action, cancel: cancel, duration: duration, id: id)
    }
    @MainActor @discardableResult
    /// Shows a success toast.
    public func success(_ title: String, description: String? = nil, action: ToastButton? = nil, duration: TimeInterval? = nil) -> String {
        ToastCenter.shared.show(title, kind: .success, description: description, action: action, duration: duration)
    }
    @MainActor @discardableResult
    /// Shows an error toast.
    public func error(_ title: String, description: String? = nil, action: ToastButton? = nil, duration: TimeInterval? = nil) -> String {
        ToastCenter.shared.show(title, kind: .error, description: description, action: action, duration: duration)
    }
    @MainActor @discardableResult
    /// Shows an informational toast.
    public func info(_ title: String, description: String? = nil, duration: TimeInterval? = nil) -> String {
        ToastCenter.shared.show(title, kind: .info, description: description, duration: duration)
    }
    @MainActor @discardableResult
    /// Shows a warning toast.
    public func warning(_ title: String, description: String? = nil, duration: TimeInterval? = nil) -> String {
        ToastCenter.shared.show(title, kind: .warning, description: description, duration: duration)
    }
    @MainActor @discardableResult
    /// Shows a persistent loading toast.
    public func loading(_ title: String, description: String? = nil, id: String? = nil) -> String {
        ToastCenter.shared.show(title, kind: .loading, description: description, id: id)
    }
    @MainActor
    /// Applies the dismiss design-system operation.
    public func dismiss(_ id: String? = nil) { ToastCenter.shared.dismiss(id) }
}

/// Convenience entry point for showing and dismissing shared toasts.
public let toast = ToastAPI()

// MARK: Views

extension View {
    /// Shows the toast stack over this view (install once per window, at its root).
    public func kzToaster(_ center: ToastCenter = .shared) -> some View {
        overlay(alignment: .bottom) { Toaster(center: center) }
    }
}

/// An in-window stack of toasts with hover expansion and dismissal.
public struct Toaster: View {
    var center: ToastCenter
    @Environment(\.accessibilityReduceMotion) private var reduce
    @State private var heights: [String: CGFloat] = [:]

    /// Creates Toaster with the supplied content and styling.
    public init(center: ToastCenter = .shared) { self.center = center }

    /// The composed native view.
    public var body: some View {
        let toasts = center.toasts
        let frontHeight = toasts.first.flatMap { heights[$0.id] } ?? 0
        ZStack(alignment: .bottom) {
            ForEach(Array(toasts.enumerated()), id: \.element.id) { index, item in
                let before = toasts.prefix(index).reduce(CGFloat(0)) { $0 + (heights[$1.id] ?? 0) }
                let lift = center.expanded ? before + CGFloat(index) * ToastCenter.gap : CGFloat(index) * ToastCenter.gap
                ToastView(item: item, center: center, front: index == 0, collapsedBehind: !center.expanded && index > 0)
                    .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { heights[item.id] = $0 }
                    .frame(height: !center.expanded && index > 0 && frontHeight > 0 ? frontHeight : nil, alignment: .bottom)
                    .scaleEffect(center.expanded ? 1 : 1 - 0.05 * CGFloat(index), anchor: .top)
                    .offset(y: -lift)
                    .opacity(index < ToastCenter.visibleCount ? 1 : 0)
                    .zIndex(Double(toasts.count - index))
                    .transition(.asymmetric(insertion: .move(edge: .bottom).combined(with: .opacity), removal: .move(edge: .bottom).combined(with: .opacity)))
            }
        }
        .frame(width: ToastCenter.width)
        .padding(.bottom, ToastCenter.offset)
        .animation(.kz.honoring(reduce, .kz.ease(duration: 0.4)), value: toasts.map(\.id))
        .animation(.kz.honoring(reduce, .kz.ease(duration: 0.4)), value: center.expanded)
        .onHover { center.hovered = $0; center.expanded = $0 && toasts.count > 1 }
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.didHideNotification)) { _ in center.appHidden = true }
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.didUnhideNotification)) { _ in center.appHidden = false }
    }
}

struct ToastView: View {
    var item: ToastItem
    var center: ToastCenter
    var front: Bool
    var collapsedBehind: Bool
    @Environment(\.accessibilityReduceMotion) private var reduce
    @State private var drag: CGFloat = 0
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let t = KZTheme(scheme)
        let shape = RoundedRectangle(cornerRadius: Radius.lg)
        HStack(spacing: 6) {
            if item.kind == .loading {
                ProgressView().controlSize(.small)
            } else if let icon {
                icon.icon(16).padding(.leading, -3).padding(.trailing, 4)
            }
            VStack(alignment: .leading, spacing: 2) {
                Text(item.title).font(.system(size: 13, weight: .medium))
                if let d = item.description {
                    Text(d).font(.system(size: 13))
                        .foregroundStyle(t.dark ? Color(.sRGB, white: 0.91) : Color(.sRGB, red: 0x3f / 255, green: 0x3f / 255, blue: 0x3f / 255))
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if let cancel = item.cancel {
                toastButton(cancel, fill: t.dark ? Color.white.opacity(0.3) : Color.black.opacity(0.08), text: t[.popoverForeground])
            }
            if let action = item.action {
                toastButton(action, fill: t[.popoverForeground], text: t[.popover])
            }
        }
        .opacity(collapsedBehind ? 0 : 1)
        .foregroundStyle(t[.popoverForeground])
        .padding(16)
        .frame(width: ToastCenter.width)
        .background(t[.popover], in: shape)
        .overlay { shape.strokeBorder(t[.border], lineWidth: 1) }
        .clipShape(shape)
        .shadow(color: .black.opacity(0.1), radius: 6, y: 4)
        .offset(y: max(0, drag))
        .gesture(
            DragGesture(minimumDistance: 4)
                .onChanged { drag = $0.translation.height }
                .onEnded { value in
                    // sonner's SWIPE_THRESHOLD: 45 pt down dismisses.
                    if value.translation.height > 45 { center.dismiss(item.id) } else { withKZAnimation(.kz.ease(duration: 0.2), reduce: reduce) { drag = 0 } }
                })
    }

    private var icon: Image? {
        switch item.kind {
        case .normal: nil
        case .success: Icon.circleCheck
        case .info: Icon.info
        case .warning: Icon.triangleAlert
        case .error: Icon.octagonX
        case .loading: Icon.loader2
        }
    }

    private func toastButton(_ b: ToastButton, fill: Color, text: Color) -> some View {
        Button {
            b.action()
            center.dismiss(item.id)
        } label: {
            Text(b.label)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(text)
                .padding(.horizontal, 8)
                .frame(height: 24)
                .background(fill, in: RoundedRectangle(cornerRadius: 4))
        }
        .buttonStyle(.plain)
    }
}

// `flex flex-wrap items-center gap-*`: children left to right, wrapping onto new lines, each line
// centred vertically. (KudzuEditor has an internal twin for its toolbar; this one serves the
// trigger chips, app chips and offset presets.)

import SwiftUI

struct FlowLayout: Layout {
    var spacing: CGFloat = 6
    var lineSpacing: CGFloat = 6

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let lines = arrange(width: proposal.width ?? .infinity, subviews: subviews)
        let width = lines.map(\.width).max() ?? 0
        let height = lines.map(\.height).reduce(0, +) + lineSpacing * CGFloat(max(0, lines.count - 1))
        let fitted: CGFloat = if let w = proposal.width, w.isFinite { w } else { width }
        return CGSize(width: fitted, height: height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var y = bounds.minY
        for line in arrange(width: bounds.width, subviews: subviews) {
            var x = bounds.minX
            for item in line.items {
                let size = item.size
                subviews[item.index].place(at: CGPoint(x: x, y: y + (line.height - size.height) / 2), proposal: ProposedViewSize(size))
                x += size.width + spacing
            }
            y += line.height + lineSpacing
        }
    }

    private struct Line {
        var items: [(index: Int, size: CGSize)] = []
        var width: CGFloat = 0
        var height: CGFloat = 0
    }

    private func arrange(width maxWidth: CGFloat, subviews: Subviews) -> [Line] {
        var lines: [Line] = []
        var line = Line()
        for index in subviews.indices {
            var size = subviews[index].sizeThatFits(.unspecified)
            // A flexible child (the trigger text field) takes the rest of its line, at least its own width.
            let flexible = subviews[index].priority > 0
            if !line.items.isEmpty, line.width + spacing + size.width > maxWidth {
                lines.append(line)
                line = Line()
            }
            if flexible, maxWidth.isFinite {
                let used = line.items.isEmpty ? 0 : line.width + spacing
                size.width = max(size.width, maxWidth - used)
            }
            line.width += (line.items.isEmpty ? 0 : spacing) + size.width
            line.height = max(line.height, size.height)
            line.items.append((index, size))
        }
        if !line.items.isEmpty { lines.append(line) }
        return lines
    }
}

import CoreGraphics
import Foundation

/// Parses SVG path data (`d` attributes) into Core Graphics paths.
///
/// Supports every path command (M L H V C S Q T A Z, absolute and relative, implicit repeats)
/// and the compact number syntax design tools emit (`1.5-1.5`, `.71.4`, `1e-3`).
public enum KzSVGPath {
    /// A parse failure, with the offending position.
    public struct ParseError: Error, CustomStringConvertible, Sendable {
        /// What went wrong.
        public let message: String
        /// Character offset in the path data.
        public let offset: Int
        /// Human-readable description.
        public var description: String { "SVG path error at \(offset): \(message)" }
    }

    /// Parses path data into a `CGPath` in the path's own coordinate space.
    public static func parse(_ data: String) throws -> CGPath {
        var scanner = Scanner(data)
        let path = CGMutablePath()
        var command: Character?
        var current = CGPoint.zero
        var subpathStart = CGPoint.zero
        var lastControl: CGPoint? // for S/T reflection
        var lastWasCubic = false
        var lastWasQuad = false

        while true {
            scanner.skipSeparators()
            guard !scanner.atEnd else { break }
            if let c = scanner.peekCommand() {
                scanner.advance()
                command = c
            } else if command == nil {
                throw ParseError(message: "path must start with a command", offset: scanner.offset)
            }
            guard let cmd = command else { break }
            let relative = cmd.isLowercase
            let origin = relative ? current : .zero
            func point() throws -> CGPoint {
                let x = try scanner.number()
                let y = try scanner.number()
                return CGPoint(x: origin.x + x, y: origin.y + y)
            }
            var isCubic = false
            var isQuad = false

            switch cmd.uppercased() {
            case "M":
                let p = try point()
                path.move(to: p)
                current = p
                subpathStart = p
                // Further pairs after a moveto are implicit linetos.
                command = relative ? "l" : "L"
            case "L":
                let p = try point()
                path.addLine(to: p)
                current = p
            case "H":
                let x = try scanner.number()
                current = CGPoint(x: relative ? current.x + x : x, y: current.y)
                path.addLine(to: current)
            case "V":
                let y = try scanner.number()
                current = CGPoint(x: current.x, y: relative ? current.y + y : y)
                path.addLine(to: current)
            case "C":
                let c1 = try point(), c2 = try point(), p = try point()
                path.addCurve(to: p, control1: c1, control2: c2)
                lastControl = c2
                current = p
                isCubic = true
            case "S":
                let c1 = lastWasCubic ? reflect(lastControl ?? current, about: current) : current
                let c2 = try point(), p = try point()
                path.addCurve(to: p, control1: c1, control2: c2)
                lastControl = c2
                current = p
                isCubic = true
            case "Q":
                let c = try point(), p = try point()
                path.addQuadCurve(to: p, control: c)
                lastControl = c
                current = p
                isQuad = true
            case "T":
                let c = lastWasQuad ? reflect(lastControl ?? current, about: current) : current
                let p = try point()
                path.addQuadCurve(to: p, control: c)
                lastControl = c
                current = p
                isQuad = true
            case "A":
                let rx = try scanner.number(), ry = try scanner.number()
                let rotation = try scanner.number()
                let largeArc = try scanner.flag(), sweep = try scanner.flag()
                let p = try point()
                addArc(to: path, from: current, to: p, rx: rx, ry: ry, rotationDegrees: rotation, largeArc: largeArc, sweep: sweep)
                current = p
            case "Z":
                path.closeSubpath()
                current = subpathStart
            default:
                throw ParseError(message: "unknown command \(cmd)", offset: scanner.offset)
            }
            lastWasCubic = isCubic
            lastWasQuad = isQuad
        }
        return path
    }

    private static func reflect(_ p: CGPoint, about c: CGPoint) -> CGPoint {
        CGPoint(x: 2 * c.x - p.x, y: 2 * c.y - p.y)
    }

    /// Endpoint-parameterised elliptical arc → cubic Béziers (SVG 1.1 implementation notes, F.6).
    private static func addArc(to path: CGMutablePath, from p0: CGPoint, to p1: CGPoint, rx rxIn: CGFloat, ry ryIn: CGFloat, rotationDegrees: CGFloat, largeArc: Bool, sweep: Bool) {
        if p0 == p1 { return }
        var rx = abs(rxIn), ry = abs(ryIn)
        if rx == 0 || ry == 0 {
            path.addLine(to: p1)
            return
        }
        let phi = rotationDegrees * .pi / 180
        let cosPhi = cos(phi), sinPhi = sin(phi)
        let dx = (p0.x - p1.x) / 2, dy = (p0.y - p1.y) / 2
        let x1p = cosPhi * dx + sinPhi * dy
        let y1p = -sinPhi * dx + cosPhi * dy
        // Scale radii up if they can't span the endpoints.
        let lambda = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry)
        if lambda > 1 {
            let s = lambda.squareRoot()
            rx *= s
            ry *= s
        }
        let num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p
        let den = rx * rx * y1p * y1p + ry * ry * x1p * x1p
        var coef = den == 0 ? 0 : (max(0, num) / den).squareRoot()
        if largeArc == sweep { coef = -coef }
        let cxp = coef * (rx * y1p / ry)
        let cyp = coef * -(ry * x1p / rx)
        let cx = cosPhi * cxp - sinPhi * cyp + (p0.x + p1.x) / 2
        let cy = sinPhi * cxp + cosPhi * cyp + (p0.y + p1.y) / 2

        func angle(_ ux: CGFloat, _ uy: CGFloat, _ vx: CGFloat, _ vy: CGFloat) -> CGFloat {
            let a = atan2(ux * vy - uy * vx, ux * vx + uy * vy)
            return a
        }
        let theta1 = angle(1, 0, (x1p - cxp) / rx, (y1p - cyp) / ry)
        var delta = angle((x1p - cxp) / rx, (y1p - cyp) / ry, (-x1p - cxp) / rx, (-y1p - cyp) / ry)
        if !sweep && delta > 0 { delta -= 2 * .pi }
        if sweep && delta < 0 { delta += 2 * .pi }

        let segments = max(1, Int(ceil(abs(delta) / (.pi / 2) - 1e-9)))
        let step = delta / CGFloat(segments)
        let k = 4.0 / 3.0 * tan(step / 4)
        var t = theta1
        for _ in 0..<segments {
            let t2 = t + step
            let (c1, s1, c2, s2) = (cos(t), sin(t), cos(t2), sin(t2))
            // Unit-circle control points, then scale/rotate/translate.
            func map(_ x: CGFloat, _ y: CGFloat) -> CGPoint {
                let ex = rx * x, ey = ry * y
                return CGPoint(x: cosPhi * ex - sinPhi * ey + cx, y: sinPhi * ex + cosPhi * ey + cy)
            }
            let cp1 = map(c1 - k * s1, s1 + k * c1)
            let cp2 = map(c2 + k * s2, s2 - k * c2)
            let end = map(c2, s2)
            path.addCurve(to: end, control1: cp1, control2: cp2)
            t = t2
        }
    }

    private struct Scanner {
        let chars: [Character]
        var offset = 0

        init(_ text: String) { chars = Array(text) }

        var atEnd: Bool { offset >= chars.count }

        mutating func advance() { offset += 1 }

        mutating func skipSeparators() {
            while offset < chars.count, chars[offset].isWhitespace || chars[offset] == "," { offset += 1 }
        }

        func peekCommand() -> Character? {
            guard offset < chars.count else { return nil }
            let c = chars[offset]
            return "MmLlHhVvCcSsQqTtAaZz".contains(c) ? c : nil
        }

        mutating func flag() throws -> Bool {
            skipSeparators()
            guard offset < chars.count, chars[offset] == "0" || chars[offset] == "1" else {
                throw ParseError(message: "expected an arc flag", offset: offset)
            }
            defer { offset += 1 }
            return chars[offset] == "1"
        }

        mutating func number() throws -> CGFloat {
            skipSeparators()
            let start = offset
            if offset < chars.count, chars[offset] == "-" || chars[offset] == "+" { offset += 1 }
            var sawDigit = false, sawDot = false
            while offset < chars.count {
                let c = chars[offset]
                if c.isASCII, c.isNumber {
                    sawDigit = true
                } else if c == ".", !sawDot {
                    sawDot = true
                } else {
                    break
                }
                offset += 1
            }
            if sawDigit, offset < chars.count, chars[offset] == "e" || chars[offset] == "E" {
                var look = offset + 1
                if look < chars.count, chars[look] == "-" || chars[look] == "+" { look += 1 }
                if look < chars.count, chars[look].isASCII, chars[look].isNumber {
                    offset = look
                    while offset < chars.count, chars[offset].isASCII, chars[offset].isNumber { offset += 1 }
                }
            }
            guard sawDigit, let value = Double(String(chars[start..<offset])) else {
                throw ParseError(message: "expected a number", offset: start)
            }
            return CGFloat(value)
        }
    }
}

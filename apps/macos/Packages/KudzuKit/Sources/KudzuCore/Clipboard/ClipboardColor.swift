import Foundation

extension Clipboard {
    public struct RGBA: Equatable, Sendable {
        public var r: Double; public var g: Double; public var b: Double; public var a: Double
        public init(_ r: Double, _ g: Double, _ b: Double, _ a: Double = 1) { self.r = r; self.g = g; self.b = b; self.a = a }
    }
    public struct ColorFormats: Equatable, Sendable {
        public struct Value: Equatable, Sendable { public var label: String; public var value: String }
        public var values: [Value]; public var onWhite: String; public var onBlack: String; public var light: Bool
    }
    static func srgbToLinear(_ c: Double) -> Double { c <= 0.04045 ? c/12.92 : pow((c+0.055)/1.055,2.4) }
    static func linearToSrgb(_ c: Double) -> Double { c <= 0.0031308 ? 12.92*c : 1.055*pow(c,1/2.4)-0.055 }
    /// Parses the CSS colour families classified by `parseColor`, into canvas-style 8-bit sRGB.
    public static func cssToRGB(_ color: String) -> RGBA? {
        let s = color.trimmingCharacters(in:.whitespacesAndNewlines).lowercased()
        func quantize(_ c: RGBA) -> RGBA { func q(_ n: Double) -> Double { (max(0,min(1,n))*255).rounded()/255 }; return RGBA(q(c.r),q(c.g),q(c.b),q(c.a)) }
        if s.hasPrefix("#") {
            var h = String(s.dropFirst()); if h.count == 3 || h.count == 4 { h = h.map { String(repeating:String($0),count:2) }.joined() }
            guard h.count == 6 || h.count == 8, h.allSatisfy(\.isHexDigit), let n = UInt64(h,radix:16) else { return nil }
            let alpha = h.count == 8, bits = alpha ? n >> 8 : n
            return RGBA(Double((bits>>16)&255)/255,Double((bits>>8)&255)/255,Double(bits&255)/255,alpha ? Double(n&255)/255 : 1)
        }
        guard let open = s.firstIndex(of:"("), s.last == ")" else { return nil }
        let name = String(s[..<open]), args = String(s[s.index(after:open)..<s.index(before:s.endIndex)]).split { $0.isWhitespace || $0 == "," || $0 == "/" }.map(String.init)
        guard (3...4).contains(args.count) else { return nil }
        func number(_ i: Int, _ percent: Double = 1) -> Double? { let v = args[i]; return Double(v.hasSuffix("%") ? String(v.dropLast()) : v).map { v.hasSuffix("%") ? $0*percent/100 : $0 } }
        func hue(_ s: String) -> Double? {
            if s.hasSuffix("grad") { return Double(s.dropLast(4)).map { $0*0.9 } }
            if s.hasSuffix("rad") { return Double(s.dropLast(3)).map { $0*180 / .pi } }
            if s.hasSuffix("turn") { return Double(s.dropLast(4)).map { $0*360 } }
            return Double(s.hasSuffix("deg") ? String(s.dropLast(3)) : s)
        }
        let alpha = args.count == 4 ? number(3) ?? 1 : 1
        guard let x = number(0,1) ?? hue(args[0]), let y = number(1,1), let z = number(2,1) ?? hue(args[2]) else { return nil }
        func hsl(_ h: Double, _ sat: Double, _ light: Double) -> RGBA {
            let h = (h.truncatingRemainder(dividingBy:360)+360).truncatingRemainder(dividingBy:360)/60, c = (1-abs(2*light-1))*sat, m = light-(1-abs(2*light-1))*sat/2
            let x = c*(1-abs(h.truncatingRemainder(dividingBy:2)-1)); let p: (Double,Double,Double)
            switch h { case ..<1: p = (c,x,0); case ..<2: p = (x,c,0); case ..<3: p = (0,c,x); case ..<4: p = (0,x,c); case ..<5: p = (x,0,c); default: p = (c,0,x) }
            return RGBA(p.0+m,p.1+m,p.2+m,alpha)
        }
        switch name {
        case "rgb","rgba": guard let r = number(0,255), let g = number(1,255), let b = number(2,255) else { return nil }; return quantize(RGBA(r/255,g/255,b/255,alpha))
        case "hsl","hsla": guard let h = hue(args[0]) else { return nil }; return quantize(hsl(h,y,z))
        case "hwb":
            guard let h = hue(args[0]) else { return nil }; if y+z >= 1 { let v = y/(y+z); return quantize(RGBA(v,v,v,alpha)) }
            let base = hsl(h,1,0.5), f = 1-y-z; return quantize(RGBA(base.r*f+y,base.g*f+y,base.b*f+y,alpha))
        case "oklab","oklch":
            let a: Double, b: Double
            if name == "oklch" { guard let h = hue(args[2]) else { return nil }; let c = number(1,0.4) ?? y; a = c*cos(h * .pi/180); b = c*sin(h * .pi/180) }
            else { a = number(1,0.4) ?? y; b = number(2,0.4) ?? z }
            let l = pow(x+0.3963377774*a+0.2158037573*b,3), m = pow(x-0.1055613458*a-0.0638541728*b,3), t = pow(x-0.0894841775*a-1.291485548*b,3)
            return quantize(RGBA(linearToSrgb(4.0767416621*l-3.3077115913*m+0.2309699292*t),linearToSrgb(-1.2684380046*l+2.6097574011*m-0.3413193965*t),linearToSrgb(-0.0041960863*l-0.7034186147*m+1.707614701*t),alpha))
        case "lab","lch":
            let light = number(0,100) ?? x, a: Double, b: Double
            if name == "lch" { guard let h = hue(args[2]) else { return nil }; let c = number(1,150) ?? y; a = c*cos(h * .pi/180); b = c*sin(h * .pi/180) }
            else { a = number(1,125) ?? y; b = number(2,125) ?? z }
            let fy = (light+16)/116, fx = fy+a/500, fz = fy-b/200
            func inv(_ t: Double) -> Double { t > 6/29 ? pow(t,3) : 3*pow(6/29,2)*(t-4/29) }
            let xx = inv(fx)*0.96422, yy = inv(fy), zz = inv(fz)*0.82521
            let X = 0.9555766*xx-0.0230393*yy+0.0631636*zz, Y = -0.0282895*xx+1.0099416*yy+0.0210077*zz, Z = 0.0122982*xx-0.020483*yy+1.3299098*zz
            return quantize(RGBA(linearToSrgb(3.2404542*X-1.5371385*Y-0.4985314*Z),linearToSrgb(-0.969266*X+1.8760108*Y+0.041556*Z),linearToSrgb(0.0556434*X-0.2040259*Y+1.0572252*Z),alpha))
        default: return nil
        }
    }
    /// HEX/RGB/HSL/OKLCH and contrast, using Björn Ottosson's matrices from clip-content.tsx.
    public static func formats(_ color: RGBA) -> ColorFormats {
        let r = color.r, g = color.g, b = color.b, a = color.a
        func rounded(_ n: Double) -> Int { Int(floor(n+0.5)) }
        let hex = "#" + [r,g,b].map { String(format:"%02X",rounded($0*255)) }.joined() + (a < 1 ? String(format:"%02X",rounded(a*255)) : "")
        let maxC = max(r,g,b), minC = min(r,g,b), light = (maxC+minC)/2, d = maxC-minC, sat = d == 0 ? 0 : d/(1-abs(2*light-1))
        var hue = 0.0
        if d != 0 { hue = maxC == r ? ((g-b)/d).truncatingRemainder(dividingBy:6) : maxC == g ? (b-r)/d+2 : (r-g)/d+4 }
        let h = rounded(hue*60+360)%360, lr = srgbToLinear(r), lg = srgbToLinear(g), lb = srgbToLinear(b)
        let L = cbrt(0.4122214708*lr+0.5363325363*lg+0.0514459929*lb), M = cbrt(0.2119034982*lr+0.6806995451*lg+0.1073969566*lb), S = cbrt(0.0883024619*lr+0.2817188376*lg+0.6299787005*lb)
        let okL = 0.2104542553*L+0.793617785*M-0.0040720468*S, okA = 1.9779984951*L-2.428592205*M+0.4505937099*S, okB = 0.0259040371*L+0.7827717662*M-0.808675766*S
        let C = hypot(okA,okB), H = (atan2(okB,okA)*180 / .pi+360).truncatingRemainder(dividingBy:360), alpha = a < 1 ? " / \(rounded(a*100))%" : "", lum = 0.2126*lr+0.7152*lg+0.0722*lb
        let values: [(String,String)] = [("HEX",hex),("RGB","rgb(\(rounded(r*255)) \(rounded(g*255)) \(rounded(b*255))\(alpha))"),("HSL","hsl(\(h) \(rounded(sat*100))% \(rounded(light*100))%\(alpha))"),("OKLCH","oklch(\(toFixed(okL,3)) \(toFixed(C,3)) \(C < 0.0005 ? "0" : toFixed(H,1))\(alpha))")]
        return ColorFormats(values:values.map { .init(label:$0.0,value:$0.1) },onWhite:toFixed(1.05/(lum+0.05),2),onBlack:toFixed((lum+0.05)/0.05,2),light:lum > 0.4)
    }
    /// What the colour view shows for a CSS colour: the TS draws it on a canvas pre-filled with `#000`,
    /// so a colour the browser rejects reads as black.
    public static func colorFormats(_ css: String) -> ColorFormats { formats(cssToRGB(css) ?? RGBA(0,0,0)) }
    /// JavaScript `Number.prototype.toFixed`: an exact tie rounds away from zero (printf rounds it to
    /// even), and `-0` prints without a sign.
    static func toFixed(_ x: Double, _ digits: Int) -> String {
        let scaled = abs(x) * Double(1 << (digits + 1))
        if scaled < 9e15, scaled == scaled.rounded(), scaled.truncatingRemainder(dividingBy:2) == 1 {
            var s = String(Int64((abs(x) * pow(10,Double(digits))).rounded(.up)))
            if digits > 0 { while s.count <= digits { s = "0"+s }; s.insert(".",at:s.index(s.endIndex,offsetBy:-digits)) }
            return (x < 0 ? "-" : "") + s
        }
        return String(format:"%.\(digits)f",x == 0 ? 0 : x)
    }
}

import CoreGraphics

private let verbMove: UInt8 = 0
private let verbLine: UInt8 = 1
private let verbQuad: UInt8 = 2
private let verbCubic: UInt8 = 3
private let verbClose: UInt8 = 4

extension CGPath {
    static func from(_ data: PathData) -> CGPath {
        let path = CGMutablePath()
        let coords = data.coords
        var i = 0
        func point() -> CGPoint {
            defer { i += 2 }
            return CGPoint(x: CGFloat(coords[i]), y: CGFloat(coords[i + 1]))
        }
        for verb in data.verbs {
            switch verb {
            case verbMove:
                path.move(to: point())
            case verbLine:
                path.addLine(to: point())
            case verbQuad:
                let control = point()
                path.addQuadCurve(to: point(), control: control)
            case verbCubic:
                let control1 = point()
                let control2 = point()
                path.addCurve(to: point(), control1: control1, control2: control2)
            case verbClose:
                path.closeSubpath()
            default:
                continue
            }
        }
        return path
    }
}

extension CGAffineTransform {
    init(_ affine: Affine) {
        self.init(
            a: CGFloat(affine.a),
            b: CGFloat(affine.b),
            c: CGFloat(affine.c),
            d: CGFloat(affine.d),
            tx: CGFloat(affine.e),
            ty: CGFloat(affine.f)
        )
    }
}

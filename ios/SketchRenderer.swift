import CoreGraphics
import Foundation
import ImageIO

/// Draws the core's draw ops with Core Graphics. Callers set the context's transform so op
/// coordinates land in view space; gradients use pad spread (reflect and repeat fall back to pad).
enum SketchRenderer {
    static let decodedImages = NSCache<NSData, CGImage>()

    private static let sRGB = CGColorSpace(name: CGColorSpace.sRGB)!

    static func draw(_ ops: [DrawOp], in context: CGContext) {
        for op in ops {
            switch op {
            case let .fillPath(path, paint, rule, opacity, transform):
                context.saveGState()
                context.concatenate(CGAffineTransform(transform))
                fill(CGPath.from(path), paint: paint, rule: rule, opacity: opacity, in: context)
                context.restoreGState()
            case let .strokePath(path, paint, params, opacity, transform):
                context.saveGState()
                context.concatenate(CGAffineTransform(transform))
                stroke(CGPath.from(path), paint: paint, params: params, opacity: opacity, in: context)
                context.restoreGState()
            case let .pushClip(path, rule, transform):
                context.saveGState()
                var placement = CGAffineTransform(transform)
                context.addPath(CGPath.from(path).copy(using: &placement) ?? CGPath.from(path))
                context.clip(using: rule.cgFillRule)
            case .popClip:
                context.restoreGState()
            case let .pushLayer(opacity):
                context.saveGState()
                context.setAlpha(CGFloat(opacity))
                context.beginTransparencyLayer(auxiliaryInfo: nil)
                context.setAlpha(1)
            case .popLayer:
                context.endTransparencyLayer()
                context.restoreGState()
            case let .image(data, _, x, y, width, height, transform):
                guard let image = decodedImage(data) else { continue }
                context.saveGState()
                context.concatenate(CGAffineTransform(transform))
                context.translateBy(x: CGFloat(x), y: CGFloat(y + height))
                context.scaleBy(x: 1, y: -1)
                context.draw(image, in: CGRect(x: 0, y: 0, width: CGFloat(width), height: CGFloat(height)))
                context.restoreGState()
            }
        }
    }

    static func drawStrokes(_ strokes: [CommittedStroke], in context: CGContext) {
        for stroke in strokes {
            drawFill(stroke.path, color: stroke.color, in: context)
        }
    }

    static func drawFill(_ path: PathData, color: Color, in context: CGContext) {
        context.setFillColor(cgColor(color))
        context.addPath(CGPath.from(path))
        context.fillPath(using: .winding)
    }

    static func cgColor(_ color: Color, opacity: Float = 1) -> CGColor {
        CGColor(
            colorSpace: sRGB,
            components: [
                CGFloat(color.r) / 255,
                CGFloat(color.g) / 255,
                CGFloat(color.b) / 255,
                CGFloat(color.a) / 255 * CGFloat(opacity)
            ]
        )!
    }

    private static func fill(_ path: CGPath, paint: Paint, rule: FillRule, opacity: Float, in context: CGContext) {
        context.addPath(path)
        if case let .solid(color) = paint {
            context.setFillColor(cgColor(color, opacity: opacity))
            context.fillPath(using: rule.cgFillRule)
        } else {
            context.clip(using: rule.cgFillRule)
            drawGradient(paint, opacity: opacity, in: context)
        }
    }

    private static func stroke(
        _ path: CGPath,
        paint: Paint,
        params: StrokeParams,
        opacity: Float,
        in context: CGContext
    ) {
        context.setLineWidth(CGFloat(params.width))
        context.setLineCap(params.cap.cgLineCap)
        context.setLineJoin(params.join.cgLineJoin)
        context.setMiterLimit(CGFloat(params.miterLimit))
        if !params.dash.isEmpty {
            context.setLineDash(phase: CGFloat(params.dashOffset), lengths: params.dash.map { CGFloat($0) })
        }
        context.addPath(path)
        if case let .solid(color) = paint {
            context.setStrokeColor(cgColor(color, opacity: opacity))
            context.strokePath()
        } else {
            context.replacePathWithStrokedPath()
            context.clip()
            drawGradient(paint, opacity: opacity, in: context)
        }
    }

    /// Fills the current clip with a gradient paint.
    private static func drawGradient(_ paint: Paint, opacity: Float, in context: CGContext) {
        let extend: CGGradientDrawingOptions = [.drawsBeforeStartLocation, .drawsAfterEndLocation]
        context.setAlpha(CGFloat(opacity))
        switch paint {
        case .solid:
            return
        case let .linearGradient(x1, y1, x2, y2, stops, _, transform):
            guard let gradient = cgGradient(stops) else { return }
            context.concatenate(CGAffineTransform(transform))
            context.drawLinearGradient(
                gradient,
                start: CGPoint(x: CGFloat(x1), y: CGFloat(y1)),
                end: CGPoint(x: CGFloat(x2), y: CGFloat(y2)),
                options: extend
            )
        case let .radialGradient(cx, cy, radius, fx, fy, stops, _, transform):
            guard let gradient = cgGradient(stops) else { return }
            context.concatenate(CGAffineTransform(transform))
            context.drawRadialGradient(
                gradient,
                startCenter: CGPoint(x: CGFloat(fx), y: CGFloat(fy)),
                startRadius: 0,
                endCenter: CGPoint(x: CGFloat(cx), y: CGFloat(cy)),
                endRadius: CGFloat(radius),
                options: extend
            )
        }
    }

    private static func cgGradient(_ stops: [GradientStop]) -> CGGradient? {
        CGGradient(
            colorsSpace: sRGB,
            colors: stops.map { cgColor($0.color) } as CFArray,
            locations: stops.map { CGFloat($0.offset) }
        )
    }

    private static func decodedImage(_ data: Data) -> CGImage? {
        let key = data as NSData
        if let cached = decodedImages.object(forKey: key) {
            return cached
        }
        guard let source = CGImageSourceCreateWithData(key, nil),
              let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else {
            return nil
        }
        decodedImages.setObject(image, forKey: key)
        return image
    }
}

private extension FillRule {
    var cgFillRule: CGPathFillRule {
        switch self {
        case .nonZero: return .winding
        case .evenOdd: return .evenOdd
        }
    }
}

private extension LineCap {
    var cgLineCap: CGLineCap {
        switch self {
        case .butt: return .butt
        case .round: return .round
        case .square: return .square
        }
    }
}

private extension LineJoin {
    var cgLineJoin: CGLineJoin {
        switch self {
        case .miter: return .miter
        case .round: return .round
        case .bevel: return .bevel
        }
    }
}

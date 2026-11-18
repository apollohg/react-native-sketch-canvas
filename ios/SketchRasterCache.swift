import UIKit

/// Bitmaps behind the canvas in view points at screen scale: committed content (fill, background,
/// strokes) and the live stroke's settled pieces, which are drawn opaque and composited with the
/// stroke's alpha once so overlapping pieces never darken.
final class SketchRasterCache {
    private var committed: CGContext?
    private var live: CGContext?
    private var committedImage: UIImage?
    private var liveImage: UIImage?
    private var scale: CGFloat = 1

    func rebuild(
        size: CGSize,
        scale: CGFloat,
        fill: CGColor?,
        background: Background?,
        backgroundTransform: CGAffineTransform,
        strokes: [CommittedStroke],
        strokeTransform: CGAffineTransform
    ) {
        self.scale = scale
        committed = Self.makeContext(size: size, scale: scale)
        live = Self.makeContext(size: size, scale: scale)
        committedImage = nil
        liveImage = nil
        guard let committed else { return }
        if let fill {
            committed.setFillColor(fill)
            committed.fill(CGRect(origin: .zero, size: size))
        }
        if let background {
            committed.saveGState()
            committed.concatenate(backgroundTransform)
            SketchRenderer.draw(background.ops, in: committed)
            committed.restoreGState()
        }
        committed.saveGState()
        committed.concatenate(strokeTransform)
        SketchRenderer.drawStrokes(strokes, in: committed)
        committed.restoreGState()
    }

    func composite(_ path: PathData, color: Color, transform: CGAffineTransform) {
        guard let committed else { return }
        committed.saveGState()
        committed.concatenate(transform)
        SketchRenderer.drawFill(path, color: color, in: committed)
        committed.restoreGState()
        committedImage = nil
    }

    func appendLive(_ path: PathData, color: Color, transform: CGAffineTransform) {
        guard let live, !path.verbs.isEmpty else { return }
        live.saveGState()
        live.concatenate(transform)
        SketchRenderer.drawFill(path, color: Color(r: color.r, g: color.g, b: color.b, a: 255), in: live)
        live.restoreGState()
        liveImage = nil
    }

    func clearLive() {
        guard let live else { return }
        live.clear(CGRect(x: 0, y: 0, width: live.width, height: live.height))
        liveImage = nil
    }

    /// Draws committed content, then the live layer plus `tail` (in `liveTransform` space) at `liveColor`'s alpha.
    func draw(in context: CGContext, bounds: CGRect, tail: CGPath?, liveColor: Color?, liveTransform: CGAffineTransform) {
        image(of: committed, cached: &committedImage)?.draw(in: bounds)
        guard let liveColor else { return }
        context.saveGState()
        context.setAlpha(CGFloat(liveColor.a) / 255)
        context.beginTransparencyLayer(auxiliaryInfo: nil)
        context.setAlpha(1)
        image(of: live, cached: &liveImage)?.draw(in: bounds)
        if let tail {
            context.concatenate(liveTransform)
            context.setFillColor(SketchRenderer.cgColor(Color(r: liveColor.r, g: liveColor.g, b: liveColor.b, a: 255)))
            context.addPath(tail)
            context.fillPath(using: .winding)
        }
        context.endTransparencyLayer()
        context.restoreGState()
    }

    private func image(of context: CGContext?, cached: inout UIImage?) -> UIImage? {
        if let cached {
            return cached
        }
        guard let cgImage = context?.makeImage() else { return nil }
        let image = UIImage(cgImage: cgImage, scale: scale, orientation: .up)
        cached = image
        return image
    }

    private static func makeContext(size: CGSize, scale: CGFloat) -> CGContext? {
        let width = Int((size.width * scale).rounded(.up))
        let height = Int((size.height * scale).rounded(.up))
        guard width > 0, height > 0, let context = CGContext(
            data: nil,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: 0,
            space: CGColorSpace(name: CGColorSpace.sRGB)!,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else {
            return nil
        }
        context.translateBy(x: 0, y: CGFloat(height))
        context.scaleBy(x: scale, y: -scale)
        return context
    }
}

import UIKit
import XCTest

/// RGBA8 premultiplied sRGB bitmaps for comparing native rendering with the Rust-rendered goldens.
struct RGBABitmap {
    static let tolerance: Int = 48
    static let requiredMatchFraction = 0.99
    static let maxMeanDifference = 3.0

    let width: Int
    let height: Int
    var pixels: [UInt8]

    init(width: Int, height: Int) {
        self.width = width
        self.height = height
        pixels = [UInt8](repeating: 0, count: width * height * 4)
    }

    init(png url: URL) throws {
        let image = try XCTUnwrap(UIImage(contentsOfFile: url.path)?.cgImage, "cannot decode \(url.lastPathComponent)")
        self.init(width: image.width, height: image.height)
        draw { context in
            context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        }
    }

    /// Draws with a y-down coordinate system in points equal to pixels, like UIKit.
    mutating func drawFlipped(_ body: (CGContext) -> Void) {
        let height = CGFloat(self.height)
        draw { context in
            context.translateBy(x: 0, y: height)
            context.scaleBy(x: 1, y: -1)
            body(context)
        }
    }

    private mutating func draw(_ body: (CGContext) -> Void) {
        let (width, height) = (self.width, self.height)
        pixels.withUnsafeMutableBytes { buffer in
            guard let context = CGContext(
                data: buffer.baseAddress,
                width: width,
                height: height,
                bitsPerComponent: 8,
                bytesPerRow: width * 4,
                space: CGColorSpace(name: CGColorSpace.sRGB)!,
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
            ) else { return }
            body(context)
        }
    }

    func image() -> UIImage {
        var copy = pixels
        let cgImage = copy.withUnsafeMutableBytes { buffer -> CGImage? in
            CGContext(
                data: buffer.baseAddress,
                width: width,
                height: height,
                bitsPerComponent: 8,
                bytesPerRow: width * 4,
                space: CGColorSpace(name: CGColorSpace.sRGB)!,
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
            )?.makeImage()
        }
        return UIImage(cgImage: cgImage!)
    }

    struct Comparison {
        let matchFraction: Double
        let meanDifference: Double
        let diff: RGBABitmap

        var passes: Bool {
            matchFraction >= RGBABitmap.requiredMatchFraction && meanDifference <= RGBABitmap.maxMeanDifference
        }
    }

    func compare(to expected: RGBABitmap) -> Comparison {
        var diff = RGBABitmap(width: width, height: height)
        var matching = 0
        var total = 0
        for pixel in 0..<(width * height) {
            var worst = 0
            for channel in 0..<4 {
                let index = pixel * 4 + channel
                let delta = abs(Int(pixels[index]) - Int(expected.pixels[index]))
                worst = max(worst, delta)
                total += delta
            }
            if worst <= Self.tolerance {
                matching += 1
            }
            let shade = UInt8(min(255, worst * 4))
            diff.pixels[pixel * 4] = shade
            diff.pixels[pixel * 4 + 3] = 255
        }
        let count = Double(width * height)
        return Comparison(
            matchFraction: Double(matching) / count,
            meanDifference: Double(total) / (count * 4),
            diff: diff
        )
    }
}

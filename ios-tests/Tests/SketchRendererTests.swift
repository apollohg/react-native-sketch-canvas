import UIKit
import XCTest

final class SketchRendererTests: XCTestCase {
    private static let scenes = [
        "strokes-only", "paths-bg", "nested-bg", "clip-bg", "gradient-bg", "raster-bg", "alpha-strokes"
    ]

    private struct Scene: Decodable {
        let width: Int
        let height: Int
        let backgroundColor: String?
        let background: String?
    }

    func testRendererMatchesGoldens() throws {
        for name in Self.scenes {
            try assertSceneMatchesGolden(name)
        }
    }

    /// `#RRGGBB` scene colours; the library itself takes colours as UIColor props.
    private static func color(hex: String) -> CGColor? {
        guard hex.count == 7, let value = UInt32(hex.dropFirst(), radix: 16) else { return nil }
        let channel = { (shift: UInt32) in CGFloat((value >> shift) & 0xFF) / 255 }
        return CGColor(srgbRed: channel(16), green: channel(8), blue: channel(0), alpha: 1)
    }

    private func resource(_ folder: String, _ file: String) throws -> URL {
        let bundle = Bundle(for: Self.self)
        return try XCTUnwrap(bundle.resourceURL?.appendingPathComponent(folder).appendingPathComponent(file))
    }

    private func assertSceneMatchesGolden(_ name: String) throws {
        let sceneData = try Data(contentsOf: resource("goldens", "\(name).json"))
        let scene = try JSONDecoder().decode(Scene.self, from: sceneData)
        let sceneObject = try XCTUnwrap(JSONSerialization.jsonObject(with: sceneData) as? [String: Any])
        let documentData = try JSONSerialization.data(withJSONObject: try XCTUnwrap(sceneObject["document"]))
        let session = try SketchSession.fromJson(json: try XCTUnwrap(String(bytes: documentData, encoding: .utf8)))
        let background = try scene.background.map {
            try parseBackground(svg: String(contentsOf: resource("backgrounds", $0), encoding: .utf8))
        }

        let fill = try scene.backgroundColor.map { try XCTUnwrap(Self.color(hex: $0), "bad scene colour \($0)") }
        var actual = RGBABitmap(width: scene.width, height: scene.height)
        actual.drawFlipped { context in
            if let fill {
                context.setFillColor(fill)
                context.fill(CGRect(x: 0, y: 0, width: scene.width, height: scene.height))
            }
            let size = session.documentSize()
            let docToView = CGAffineTransform(
                fitTransform(
                    contentWidth: size.width,
                    contentHeight: size.height,
                    boxWidth: Double(scene.width),
                    boxHeight: Double(scene.height)
                )
            )
            if let background {
                let bgToDoc = CGAffineTransform(
                    fitTransform(
                        contentWidth: background.width,
                        contentHeight: background.height,
                        boxWidth: size.width,
                        boxHeight: size.height
                    )
                )
                context.saveGState()
                context.concatenate(bgToDoc.concatenating(docToView))
                SketchRenderer.draw(background.ops, in: context)
                context.restoreGState()
            }
            context.saveGState()
            context.concatenate(docToView)
            SketchRenderer.drawStrokes(session.committedPaths(), in: context)
            context.restoreGState()
        }

        let expected = try RGBABitmap(png: resource("goldens", "\(name).png"))
        let comparison = actual.compare(to: expected)
        if !comparison.passes {
            for (label, bitmap) in [("actual", actual), ("diff", comparison.diff)] {
                let attachment = XCTAttachment(image: bitmap.image())
                attachment.name = "\(name)-\(label)"
                attachment.lifetime = .keepAlways
                add(attachment)
            }
        }
        XCTAssertTrue(
            comparison.passes,
            String(
                format: "%@: %.4f of pixels within %d (need %.2f), mean difference %.3f (max %.1f)",
                name,
                comparison.matchFraction,
                RGBABitmap.tolerance,
                RGBABitmap.requiredMatchFraction,
                comparison.meanDifference,
                RGBABitmap.maxMeanDifference
            )
        )
    }
}

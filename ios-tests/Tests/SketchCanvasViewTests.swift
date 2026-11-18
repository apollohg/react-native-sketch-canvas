import UIKit
import XCTest

@MainActor
final class SketchCanvasViewTests: XCTestCase {
    private static let backgroundSvg = """
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 300"><rect width="400" height="300" fill="#00FF00"/></svg>
    """
    private static let blackPen = (color: UIColor.black, min: 1.0, max: 3.0)

    private var changes: [[String: Any]] = []
    private var strokeEnds = 0

    private func makeView(
        size: CGSize = CGSize(width: 200, height: 200),
        configure: (SketchCanvasView) -> Void = { _ in }
    ) -> SketchCanvasView {
        let view = SketchCanvasView(appContext: nil)
        view.onSketchChange.onEventSent = { [weak self] payload in self?.changes.append(payload) }
        view.onStrokeEnd.onEventSent = { [weak self] _ in self?.strokeEnds += 1 }
        view.pen = Pen(color: Color(Self.blackPen.color), minWidth: Self.blackPen.min, maxWidth: Self.blackPen.max)
        configure(view)
        view.didUpdateProps()
        view.frame = CGRect(origin: .zero, size: size)
        view.layoutIfNeeded()
        return view
    }

    private func sample(_ x: CGFloat, _ y: CGFloat, at time: TimeInterval, stylus: Bool = false) -> SketchPointerSample {
        SketchPointerSample(location: CGPoint(x: x, y: y), timestamp: time, force: stylus ? 0.5 : nil, isStylus: stylus)
    }

    private func drawLine(on view: SketchCanvasView, pointer: Int, from start: CGFloat, to end: CGFloat, y: CGFloat = 100) {
        view.pointerBegan(pointer, sample(start, y, at: 0))
        let steps = 10
        for step in 1...steps {
            let x = start + (end - start) * CGFloat(step) / CGFloat(steps)
            view.pointerMoved(pointer, samples: [sample(x, y, at: Double(step) * 0.016)], predicted: [])
        }
        view.pointerEnded(pointer, nil)
    }

    private func document(of view: SketchCanvasView, file: StaticString = #filePath, line: UInt = #line) throws -> [String: Any] {
        let done = expectation(description: "getDocument")
        var result: Result<String, Error>?
        view.getDocument { result = $0; done.fulfill() }
        wait(for: [done], timeout: 5)
        let json = try XCTUnwrap(result, file: file, line: line).get()
        return try XCTUnwrap(JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: Any], file: file, line: line)
    }

    private func strokes(of view: SketchCanvasView) throws -> [[String: Any]] {
        try XCTUnwrap(try document(of: view)["strokes"] as? [[String: Any]])
    }

    private func load(_ json: String, into view: SketchCanvasView) -> Result<Void, Error> {
        let done = expectation(description: "loadDocument")
        var result: Result<Void, Error> = .success(())
        view.loadDocument(json) { result = $0; done.fulfill() }
        wait(for: [done], timeout: 5)
        return result
    }

    func testStrokeCommitsThroughTouches() throws {
        let view = makeView()
        drawLine(on: view, pointer: 1, from: 20, to: 180)

        XCTAssertEqual(try strokes(of: view).count, 1)
        XCTAssertEqual(changes.last?["strokeCount"] as? Int, 1, "changes: \(changes)")
        XCTAssertEqual(strokeEnds, 1)
    }

    func testInputIgnoredBeforeBackgroundParses() throws {
        let loaded = expectation(description: "background parsed")
        let view = makeView { view in
            view.onBackgroundLoad.onEventSent = { _ in loaded.fulfill() }
            view.setBackgroundSvg(Self.backgroundSvg)
        }
        drawLine(on: view, pointer: 1, from: 20, to: 180)
        wait(for: [loaded], timeout: 5)
        drawLine(on: view, pointer: 2, from: 20, to: 180)

        let doc = try document(of: view)
        XCTAssertEqual(doc["width"] as? Double, 400, "document should take the background's size: \(doc)")
        XCTAssertEqual((doc["strokes"] as? [Any])?.count, 1, "the stroke drawn before the background parsed must be ignored")
    }

    func testPencilIgnoresConcurrentFinger() throws {
        let view = makeView()
        view.pointerBegan(1, sample(20, 50, at: 0, stylus: true))
        view.pointerBegan(2, sample(150, 150, at: 0.01))
        view.pointerMoved(2, samples: [sample(160, 160, at: 0.02)], predicted: [])
        view.pointerEnded(2, nil)
        view.pointerMoved(1, samples: [sample(60, 50, at: 0.03, stylus: true), sample(100, 50, at: 0.05, stylus: true)], predicted: [])
        view.pointerEnded(1, nil)

        let strokes = try strokes(of: view)
        XCTAssertEqual(strokes.count, 1)
        let ys = (strokes.first?["points"] as? [[Double]] ?? []).map { $0[1] }
        XCTAssertFalse(ys.isEmpty)
        XCTAssertTrue(ys.allSatisfy { abs($0 - ys[0]) < 1 }, "finger samples leaked into the pencil stroke: \(ys)")
    }

    func testSecondFingerCancelsFingerStroke() throws {
        let view = makeView()
        view.pointerBegan(1, sample(20, 100, at: 0))
        view.pointerMoved(1, samples: [sample(60, 100, at: 0.016)], predicted: [])
        view.pointerBegan(2, sample(150, 150, at: 0.02))
        XCTAssertFalse(view.hasLiveStroke)
        view.pointerMoved(1, samples: [sample(100, 100, at: 0.032)], predicted: [])
        view.pointerEnded(1, nil)
        view.pointerEnded(2, nil)

        XCTAssertEqual(try strokes(of: view).count, 0)
        XCTAssertEqual(strokeEnds, 0)
    }

    func testNotEditablePassesTouchesThrough() {
        let view = makeView()
        view.setEditable(false)
        let centre = CGPoint(x: 100, y: 100)

        XCTAssertFalse(view.point(inside: centre, with: nil))
        XCTAssertNil(view.hitTest(centre, with: nil))
    }

    func testExportPngSize() throws {
        let view = makeView { view in
            view.setInitialDocument(#"{"version":1,"width":300,"height":150,"strokes":[]}"#)
        }
        let dataUrl = try view.exportPng(scale: 2, includeBackground: true)
        let prefix = "data:image/png;base64,"
        XCTAssertTrue(dataUrl.hasPrefix(prefix))
        let data = try XCTUnwrap(Data(base64Encoded: String(dataUrl.dropFirst(prefix.count))))
        let image = try XCTUnwrap(UIImage(data: data)?.cgImage)
        XCTAssertEqual(image.width, 400)
        XCTAssertEqual(image.height, 200)
    }

    func testLoadDocumentRejectsAndKeeps() throws {
        let view = makeView()
        let valid = ##"{"version":1,"width":100,"height":50,"strokes":[{"color":"#FF0000","points":[[10,10,2]]}]}"##
        XCTAssertNoThrow(try load(valid, into: view).get())
        let before = try document(of: view)

        let result = load(#"{"version":1,"width":0,"height":50,"strokes":[]}"#, into: view)
        guard case let .failure(error as SketchCanvasError) = result else {
            return XCTFail("expected an invalidDocument failure, got \(result)")
        }
        XCTAssertEqual(error.code, "invalidDocument")
        XCTAssertEqual(try document(of: view) as NSDictionary, before as NSDictionary)
    }

    func testParentScrollPanDoesNotBegin() {
        let scrollView = UIScrollView(frame: CGRect(x: 0, y: 0, width: 300, height: 600))
        let view = makeView()
        scrollView.addSubview(view)
        view.pointerBegan(1, sample(100, 100, at: 0))

        XCTAssertFalse(view.gestureRecognizerShouldBegin(scrollView.panGestureRecognizer))
        view.pointerCancelled(1)
        view.setEditable(false)
        XCTAssertTrue(view.gestureRecognizerShouldBegin(scrollView.panGestureRecognizer))
    }

    func testResizeCancelsLiveStroke() throws {
        let view = makeView()
        drawLine(on: view, pointer: 1, from: 20, to: 120)
        view.pointerBegan(2, sample(20, 150, at: 1))
        view.pointerMoved(2, samples: [sample(80, 150, at: 1.016)], predicted: [])
        XCTAssertTrue(view.hasLiveStroke)

        view.frame = CGRect(x: 0, y: 0, width: 320, height: 180)
        view.layoutIfNeeded()

        XCTAssertFalse(view.hasLiveStroke)
        view.pointerEnded(2, nil)
        XCTAssertEqual(try strokes(of: view).count, 1)
    }
}

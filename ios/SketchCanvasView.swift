import ExpoModulesCore
import UIKit

enum SketchCanvasError: Error, Equatable {
    case invalidDocument(String)
    case invalidBackground(String)
    case exportFailed(String)

    init(_ error: SketchError) {
        switch error {
        case let .InvalidDocument(message): self = .invalidDocument(message)
        case let .InvalidBackground(message): self = .invalidBackground(message)
        }
    }

    var code: String {
        switch self {
        case .invalidDocument: return "invalidDocument"
        case .invalidBackground: return "invalidBackground"
        case .exportFailed: return "exportFailed"
        }
    }

    var message: String {
        switch self {
        case let .invalidDocument(message), let .invalidBackground(message), let .exportFailed(message):
            return message
        }
    }
}

final class SketchCanvasView: ExpoView {
    private static let dirtyMargin: CGFloat = 2
    private static let pngDataUrlPrefix = "data:image/png;base64,"
    private static let noDocumentMessage = "The canvas has no document yet"
    /// One serial queue for all canvases, so background work completes in call order.
    private static let worker = DispatchQueue(label: "com.apollohg.sketchcanvas.worker", qos: .userInitiated)

    let onSketchChange = EventDispatcher()
    let onStrokeStart = EventDispatcher()
    let onStrokeEnd = EventDispatcher()
    let onBackgroundLoad = EventDispatcher()
    let onSketchError = EventDispatcher()

    var pen = Pen(color: Color(r: 0, g: 0, b: 0, a: 255), minWidth: 1, maxWidth: 3)

    private var session: SketchSession?
    private var editable = true
    private var canvasBackgroundColor: UIColor?
    private var backgroundSvg: String?
    private var background: Background?
    private var backgroundPending = false
    private var initialDocument: String?
    private var initialDocumentConsumed = false
    private var lastEmittedRevision: UInt64?
    /// Added to each session's own revision so revisions keep increasing when a session is replaced.
    private var revisionBase: UInt64 = 0
    private var lastLayoutSize = CGSize.zero

    private var tracker = SketchPointerTracker()
    private let cache = SketchRasterCache()
    private var liveColor: Color?
    private var liveTail: CGPath?

    required init(appContext: AppContext? = nil) {
        super.init(appContext: appContext)
        isOpaque = false
        contentMode = .redraw
        isMultipleTouchEnabled = true
        isAccessibilityElement = true
        accessibilityTraits = .image
    }

    var hasLiveStroke: Bool {
        session?.hasLiveStroke() ?? false
    }

    // MARK: - Props

    func setEditable(_ editable: Bool) {
        self.editable = editable
        if !editable {
            cancelLiveStroke()
        }
    }

    func setCanvasBackgroundColor(_ color: UIColor?) {
        canvasBackgroundColor = color
        rebuildCache()
    }

    func setBackgroundSvg(_ svg: String?) {
        guard svg != backgroundSvg else { return }
        backgroundSvg = svg
        background = nil
        backgroundPending = svg != nil
        rebuildCache()
        guard let svg else { return }
        Self.worker.async { [weak self] in
            let result = Result { try parseBackground(svg: svg) }
            DispatchQueue.main.async {
                self?.backgroundParsed(svg, result)
            }
        }
    }

    /// Read once, when the canvas first creates its document.
    func setInitialDocument(_ json: String?) {
        guard !initialDocumentConsumed else { return }
        initialDocument = json
    }

    func didUpdateProps() {
        ensureSession()
    }

    private func backgroundParsed(_ svg: String, _ result: Result<Background, Error>) {
        guard svg == backgroundSvg else { return }
        backgroundPending = false
        switch result {
        case let .success(parsed):
            background = parsed
            onBackgroundLoad(["warnings": parsed.warnings.map(\.eventName)])
        case let .failure(error):
            emitError(error, fallback: SketchCanvasError.invalidBackground)
        }
        ensureSession()
        rebuildCache()
    }

    // MARK: - Session

    private func ensureSession() {
        guard session == nil else { return }
        if !initialDocumentConsumed, let json = initialDocument {
            initialDocumentConsumed = true
            do {
                return adopt(try SketchSession.fromJson(json: json))
            } catch {
                emitError(error, fallback: SketchCanvasError.invalidDocument)
            }
        }
        if backgroundPending {
            return
        }
        if let background {
            return adopt(try? SketchSession.newEmpty(width: background.width, height: background.height))
        }
        guard bounds.width > 0, bounds.height > 0 else { return }
        adopt(try? SketchSession.newEmpty(width: Double(bounds.width), height: Double(bounds.height)))
    }

    private func adopt(_ newSession: SketchSession?) {
        guard let newSession else { return }
        initialDocumentConsumed = true
        session = newSession
        if let lastEmittedRevision {
            revisionBase = lastEmittedRevision + 1
        }
        rebuildCache()
        emitChange(newSession.state())
    }

    private var docToView: CGAffineTransform {
        guard let size = session?.documentSize(), bounds.width > 0, bounds.height > 0 else { return .identity }
        return CGAffineTransform(
            fitTransform(
                contentWidth: size.width,
                contentHeight: size.height,
                boxWidth: Double(bounds.width),
                boxHeight: Double(bounds.height)
            )
        )
    }

    private var backgroundToDocument: CGAffineTransform {
        guard let background, let size = session?.documentSize() else { return .identity }
        return CGAffineTransform(
            fitTransform(
                contentWidth: background.width,
                contentHeight: background.height,
                boxWidth: size.width,
                boxHeight: size.height
            )
        )
    }

    private var displayScale: CGFloat {
        max(window?.screen.scale ?? traitCollection.displayScale, 1)
    }

    // MARK: - Layout and drawing

    override func layoutSubviews() {
        super.layoutSubviews()
        guard bounds.size != lastLayoutSize else { return }
        lastLayoutSize = bounds.size
        cancelLiveStroke()
        ensureSession()
        rebuildCache()
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        rebuildCache()
    }

    /// Rebuilding drops the live layer, so a stroke in progress is cancelled first.
    private func rebuildCache() {
        guard bounds.width > 0, bounds.height > 0 else { return }
        if hasLiveStroke {
            cancelLiveStroke()
        }
        let docToView = self.docToView
        cache.rebuild(
            size: bounds.size,
            scale: displayScale,
            fill: canvasBackgroundColor?.cgColor,
            background: background,
            backgroundTransform: backgroundToDocument.concatenating(docToView),
            strokes: session?.committedPaths() ?? [],
            strokeTransform: docToView
        )
        setNeedsDisplay()
    }

    override func draw(_ rect: CGRect) {
        guard let context = UIGraphicsGetCurrentContext() else { return }
        cache.draw(in: context, bounds: bounds, tail: liveTail, liveColor: liveColor, liveTransform: docToView)
    }

    private func invalidate(documentPath path: CGPath?) {
        guard let path, !path.isEmpty else { return }
        let rect = path.boundingBoxOfPath.applying(docToView).insetBy(dx: -Self.dirtyMargin, dy: -Self.dirtyMargin)
        setNeedsDisplay(rect)
    }

    // MARK: - Input

    override func point(inside point: CGPoint, with event: UIEvent?) -> Bool {
        editable && super.point(inside: point, with: event)
    }

    /// Parent scroll views and sheets must not pan while a stroke can start inside the canvas.
    override func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
        guard editable, gestureRecognizer is UIPanGestureRecognizer, gestureRecognizer.view !== self else {
            return super.gestureRecognizerShouldBegin(gestureRecognizer)
        }
        return !tracker.hasPointersDown && !bounds.contains(gestureRecognizer.location(in: self))
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches {
            pointerBegan(ObjectIdentifier(touch), SketchPointerSample(touch: touch, in: self))
        }
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches {
            let coalesced = event?.coalescedTouches(for: touch) ?? [touch]
            let predicted = event?.predictedTouches(for: touch) ?? []
            pointerMoved(
                ObjectIdentifier(touch),
                samples: coalesced.map { SketchPointerSample(touch: $0, in: self) },
                predicted: predicted.map { SketchPointerSample(touch: $0, in: self) }
            )
        }
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches {
            pointerEnded(ObjectIdentifier(touch), SketchPointerSample(touch: touch, in: self))
        }
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches {
            pointerCancelled(ObjectIdentifier(touch))
        }
    }

    func pointerBegan(_ pointer: AnyHashable, _ sample: SketchPointerSample) {
        guard editable else { return }
        switch tracker.began(pointer, isStylus: sample.isStylus) {
        case .ignore:
            return
        case .cancel:
            cancelLiveStroke()
        case .cancelThenBegin:
            cancelLiveStroke()
            beginStroke(sample)
        case .begin:
            beginStroke(sample)
        }
    }

    func pointerMoved(_ pointer: AnyHashable, samples: [SketchPointerSample], predicted: [SketchPointerSample]) {
        guard tracker.isDrawing(pointer), let session, session.hasLiveStroke() else { return }
        apply(session.extendStroke(samples: samples.map(\.inputSample), predicted: predicted.map(\.inputSample)))
    }

    func pointerEnded(_ pointer: AnyHashable, _ final: SketchPointerSample?) {
        if let final {
            pointerMoved(pointer, samples: [final], predicted: [])
        }
        guard tracker.ended(pointer), let color = liveColor else { return }
        // The core discards a stroke that would take the document past the format's limits.
        guard let commit = session?.endStroke() else { return cancelLiveStroke() }
        cache.composite(commit.path, color: color, transform: docToView)
        clearLive()
        invalidate(documentPath: CGPath.from(commit.path))
        emitChange(commit.state)
        onStrokeEnd()
    }

    func pointerCancelled(_ pointer: AnyHashable) {
        if tracker.ended(pointer) {
            cancelLiveStroke()
        }
    }

    private func beginStroke(_ sample: SketchPointerSample) {
        guard let session else { return }
        let viewToDoc = docToView.inverted()
        let update = session.beginStroke(
            pen: pen,
            viewToDoc: Affine(
                a: Double(viewToDoc.a),
                b: Double(viewToDoc.b),
                c: Double(viewToDoc.c),
                d: Double(viewToDoc.d),
                e: Double(viewToDoc.tx),
                f: Double(viewToDoc.ty)
            ),
            sample: sample.inputSample
        )
        cache.clearLive()
        liveColor = pen.color
        apply(update)
        onStrokeStart()
    }

    private func apply(_ update: LiveUpdate) {
        guard let color = liveColor else { return }
        cache.appendLive(update.settled, color: color, transform: docToView)
        invalidate(documentPath: CGPath.from(update.settled))
        invalidate(documentPath: liveTail)
        liveTail = CGPath.from(update.tail)
        invalidate(documentPath: liveTail)
    }

    private func cancelLiveStroke() {
        tracker.abandonDrawing()
        session?.cancelStroke()
        clearLive()
        setNeedsDisplay()
    }

    private func clearLive() {
        cache.clearLive()
        liveColor = nil
        liveTail = nil
    }

    // MARK: - Commands

    func undo() {
        applyCommand { $0.undo() }
    }

    func redo() {
        applyCommand { $0.redo() }
    }

    func clear() {
        applyCommand { $0.clear() }
    }

    private func applyCommand(_ command: (SketchSession) -> SessionState) {
        guard let session else { return }
        tracker.abandonDrawing()
        clearLive()
        let state = command(session)
        rebuildCache()
        emitChange(state)
    }

    /// Serialises off the main thread; excludes any stroke in progress.
    func getDocument(completion: @escaping (Result<String, Error>) -> Void) {
        guard let session else {
            return completion(.failure(SketchCanvasError.exportFailed(Self.noDocumentMessage)))
        }
        Self.worker.async {
            let json = session.toJson()
            DispatchQueue.main.async { completion(.success(json)) }
        }
    }

    /// Parses off the main thread; an invalid document fails and leaves the canvas unchanged.
    func loadDocument(_ json: String, completion: @escaping (Result<Void, Error>) -> Void) {
        let existing = session
        Self.worker.async { [weak self] in
            let result = Result { () -> SketchSession in
                guard let existing else { return try SketchSession.fromJson(json: json) }
                _ = try existing.loadJson(json: json)
                return existing
            }.mapError { error in (error as? SketchError).map(SketchCanvasError.init) ?? error }
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                switch result {
                case let .success(loaded):
                    self.tracker.abandonDrawing()
                    self.clearLive()
                    if self.session !== loaded {
                        self.session = nil
                        self.adopt(loaded)
                    } else {
                        self.rebuildCache()
                        self.emitChange(loaded.state())
                    }
                    completion(.success(()))
                case let .failure(error):
                    completion(.failure(error))
                }
            }
        }
    }

    func exportSvg(includeBackground: Bool, completion: @escaping (Result<String, Error>) -> Void) {
        guard let session else {
            return completion(.failure(SketchCanvasError.exportFailed(Self.noDocumentMessage)))
        }
        let svg = includeBackground ? backgroundSvg : nil
        Self.worker.async {
            let result = Result { try session.exportSvg(background: svg) }
                .mapError { error in SketchCanvasError.exportFailed((error as? SketchError).map(SketchCanvasError.init)?.message ?? "\(error)") }
            DispatchQueue.main.async { completion(result.mapError { $0 }) }
        }
    }

    /// A PNG data URL of the document at its on-screen size times `scale` (default: the screen's).
    func exportPng(scale: Double?, includeBackground: Bool) throws -> String {
        guard let session else { throw SketchCanvasError.exportFailed(Self.noDocumentMessage) }
        let size = session.documentSize()
        let fitScale = bounds.width > 0 && bounds.height > 0 ? Double(docToView.a) : 1
        let pixelScale = CGFloat(fitScale * (scale ?? Double(displayScale)))
        let pixelSize = CGSize(
            width: (CGFloat(size.width) * pixelScale).rounded(),
            height: (CGFloat(size.height) * pixelScale).rounded()
        )
        guard pixelSize.width >= 1, pixelSize.height >= 1 else {
            throw SketchCanvasError.exportFailed("Export size \(pixelSize) is empty")
        }
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1
        format.opaque = false
        let image = UIGraphicsImageRenderer(size: pixelSize, format: format).image { rendererContext in
            let context = rendererContext.cgContext
            context.scaleBy(x: pixelScale, y: pixelScale)
            if let fill = canvasBackgroundColor {
                context.setFillColor(fill.cgColor)
                context.fill(CGRect(x: 0, y: 0, width: size.width, height: size.height))
            }
            if includeBackground, let background {
                context.saveGState()
                context.concatenate(backgroundToDocument)
                SketchRenderer.draw(background.ops, in: context)
                context.restoreGState()
            }
            SketchRenderer.drawStrokes(session.committedPaths(), in: context)
        }
        guard let png = image.pngData() else { throw SketchCanvasError.exportFailed("PNG encoding failed") }
        return Self.pngDataUrlPrefix + png.base64EncodedString()
    }

    // MARK: - Events

    private func emitChange(_ state: SessionState) {
        let revision = revisionBase + state.revision
        guard revision != lastEmittedRevision else { return }
        lastEmittedRevision = revision
        onSketchChange([
            "strokeCount": Int(state.strokeCount),
            "canUndo": state.canUndo,
            "canRedo": state.canRedo,
            "revision": String(revision)
        ])
    }

    private func emitError(_ error: Error, fallback: (String) -> SketchCanvasError) {
        let canvasError = (error as? SketchError).map(SketchCanvasError.init) ?? fallback("\(error)")
        onSketchError(["code": canvasError.code, "message": canvasError.message])
    }
}

extension Color {
    init(_ color: UIColor) {
        var red: CGFloat = 0
        var green: CGFloat = 0
        var blue: CGFloat = 0
        var alpha: CGFloat = 0
        color.getRed(&red, green: &green, blue: &blue, alpha: &alpha)
        let channel = { (value: CGFloat) in UInt8((min(max(value, 0), 1) * 255).rounded()) }
        self.init(r: channel(red), g: channel(green), b: channel(blue), a: channel(alpha))
    }
}

private extension BackgroundWarning {
    var eventName: String {
        switch self {
        case .unsupportedText: return "unsupportedText"
        case .unsupportedFilter: return "unsupportedFilter"
        case .unsupportedMask: return "unsupportedMask"
        case .unsupportedPattern: return "unsupportedPattern"
        case .unsupportedExternalImage: return "unsupportedExternalImage"
        }
    }
}

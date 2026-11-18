import ExpoModulesCore

struct ExportPngOptions: Record {
    @Field var scale: Double?
    @Field var includeBackground: Bool = true
}

struct ExportSvgOptions: Record {
    @Field var includeBackground: Bool = true
}

final class SketchCanvasException: GenericException<SketchCanvasError>, @unchecked Sendable {
    override var code: String {
        param.code
    }

    override var reason: String {
        param.message
    }
}

private func settle<Value>(_ promise: Promise, _ result: Result<Value, Error>) {
    switch result {
    case let .success(value):
        promise.resolve(value is Void ? nil : value)
    case let .failure(error as SketchCanvasError):
        promise.reject(SketchCanvasException(error))
    case let .failure(error):
        promise.reject(SketchCanvasException(.exportFailed("\(error)")))
    }
}

/// `definition()` is main-actor isolated because view functions take `SketchCanvasView`, whose
/// `AnyArgument` conformance is main-actor isolated; it only builds the definition.
public class SketchCanvasModule: BaseModule, @preconcurrency AnyModule {
    @MainActor
    public func definition() -> ModuleDefinition {
        Name("SketchCanvas")

        View(SketchCanvasView.self) {
            Events("onSketchChange", "onStrokeStart", "onStrokeEnd", "onBackgroundLoad", "onSketchError")

            Prop("background") { (view: SketchCanvasView, svg: String?) in
                view.setBackgroundSvg(svg)
            }
            Prop("canvasBackgroundColor") { (view: SketchCanvasView, color: UIColor?) in
                view.setCanvasBackgroundColor(color)
            }
            Prop("penColor") { (view: SketchCanvasView, color: UIColor) in
                view.pen.color = Color(color)
            }
            Prop("penMinWidth") { (view: SketchCanvasView, width: Double) in
                view.pen.minWidth = width
            }
            Prop("penMaxWidth") { (view: SketchCanvasView, width: Double) in
                view.pen.maxWidth = width
            }
            Prop("initialDocument") { (view: SketchCanvasView, json: String?) in
                view.setInitialDocument(json)
            }
            Prop("editable") { (view: SketchCanvasView, editable: Bool) in
                view.setEditable(editable)
            }
            OnViewDidUpdateProps { (view: SketchCanvasView) in
                view.didUpdateProps()
            }

            AsyncFunction("undo") { (view: SketchCanvasView) in
                view.undo()
            }.runOnQueue(.main)
            AsyncFunction("redo") { (view: SketchCanvasView) in
                view.redo()
            }.runOnQueue(.main)
            AsyncFunction("clear") { (view: SketchCanvasView) in
                view.clear()
            }.runOnQueue(.main)
            AsyncFunction("getDocument") { (view: SketchCanvasView, promise: Promise) in
                view.getDocument { settle(promise, $0) }
            }.runOnQueue(.main)
            AsyncFunction("loadDocument") { (view: SketchCanvasView, json: String, promise: Promise) in
                view.loadDocument(json) { settle(promise, $0) }
            }.runOnQueue(.main)
            AsyncFunction("exportPng") { (view: SketchCanvasView, options: ExportPngOptions) throws -> String in
                do {
                    return try view.exportPng(scale: options.scale, includeBackground: options.includeBackground)
                } catch let error as SketchCanvasError {
                    throw SketchCanvasException(error)
                }
            }.runOnQueue(.main)
            AsyncFunction("exportSvg") { (view: SketchCanvasView, options: ExportSvgOptions, promise: Promise) in
                view.exportSvg(includeBackground: options.includeBackground) { settle(promise, $0) }
            }.runOnQueue(.main)
        }
    }
}

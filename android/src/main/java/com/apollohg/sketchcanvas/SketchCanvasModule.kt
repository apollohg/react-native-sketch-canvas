package com.apollohg.sketchcanvas

import expo.modules.kotlin.Promise
import expo.modules.kotlin.exception.CodedException
import expo.modules.kotlin.functions.Queues
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition
import expo.modules.kotlin.records.Field
import expo.modules.kotlin.records.Record
import uniffi.sketch_core.Color as SketchColor

class ExportPngOptions : Record {
    @Field val scale: Double? = null

    @Field val includeBackground: Boolean = true
}

class ExportSvgOptions : Record {
    @Field val includeBackground: Boolean = true
}

class SketchCanvasException(error: SketchCanvasError) :
    CodedException(error.code, error.message, error)

private fun <T> Promise.settle(result: Result<T>) {
    result
        .onSuccess { resolve(if (it == Unit) null else it) }
        .onFailure {
            reject(
                SketchCanvasException(
                    SketchCanvasError.from(it) { m ->
                        SketchCanvasError.ExportFailed(m)
                    }
                )
            )
        }
}

private fun android.graphics.Color.toSketchColor() = SketchColor(
    (red() * CHANNEL_MAX).toInt().toUByte(),
    (green() * CHANNEL_MAX).toInt().toUByte(),
    (blue() * CHANNEL_MAX).toInt().toUByte(),
    (alpha() * CHANNEL_MAX).toInt().toUByte()
)

private const val CHANNEL_MAX = 255f

class SketchCanvasModule : Module() {
    override fun definition() = ModuleDefinition {
        Name("SketchCanvas")

        View(SketchCanvasView::class) {
            Events(
                "onSketchChange",
                "onStrokeStart",
                "onStrokeEnd",
                "onBackgroundLoad",
                "onSketchError"
            )

            Prop("background") { view: SketchCanvasView, svg: String? ->
                view.setBackgroundSvg(svg)
            }
            Prop("canvasBackgroundColor") {
                    view: SketchCanvasView,
                    color: android.graphics.Color?
                ->
                view.setCanvasBackgroundColor(color?.toArgb())
            }
            Prop("penColor") { view: SketchCanvasView, color: android.graphics.Color ->
                view.pen = view.pen.copy(color = color.toSketchColor())
            }
            Prop("penMinWidth") { view: SketchCanvasView, width: Double ->
                view.pen =
                    view.pen.copy(minWidth = width)
            }
            Prop("penMaxWidth") { view: SketchCanvasView, width: Double ->
                view.pen =
                    view.pen.copy(maxWidth = width)
            }
            Prop("initialDocument") { view: SketchCanvasView, json: String? ->
                view.setInitialDocument(json)
            }
            Prop("editable") { view: SketchCanvasView, editable: Boolean ->
                view.setEditable(editable)
            }
            OnViewDidUpdateProps { view: SketchCanvasView -> view.didUpdateProps() }

            AsyncFunction("undo") { view: SketchCanvasView ->
                view.undo()
            }.runOnQueue(Queues.MAIN)
            AsyncFunction("redo") { view: SketchCanvasView ->
                view.redo()
            }.runOnQueue(Queues.MAIN)
            AsyncFunction("clear") { view: SketchCanvasView ->
                view.clear()
            }.runOnQueue(Queues.MAIN)
            AsyncFunction("getDocument") { view: SketchCanvasView, promise: Promise ->
                view.getDocument { promise.settle(it) }
            }.runOnQueue(Queues.MAIN)
            AsyncFunction("loadDocument") {
                    view: SketchCanvasView,
                    json: String,
                    promise: Promise
                ->
                view.loadDocument(json) { promise.settle(it) }
            }.runOnQueue(Queues.MAIN)
            AsyncFunction("exportPng") { view: SketchCanvasView, options: ExportPngOptions ->
                try {
                    view.exportPng(options.scale, options.includeBackground)
                } catch (error: SketchCanvasError) {
                    throw SketchCanvasException(error)
                }
            }.runOnQueue(Queues.MAIN)
            AsyncFunction("exportSvg") {
                    view: SketchCanvasView,
                    options: ExportSvgOptions,
                    promise: Promise
                ->
                view.exportSvg(options.includeBackground) { promise.settle(it) }
            }.runOnQueue(Queues.MAIN)
        }
    }
}

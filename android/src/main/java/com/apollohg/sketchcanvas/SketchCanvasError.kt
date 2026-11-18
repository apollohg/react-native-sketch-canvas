package com.apollohg.sketchcanvas

import uniffi.sketch_core.SketchException

sealed class SketchCanvasError(val code: String, message: String) : Exception(message) {
    class InvalidDocument(message: String) : SketchCanvasError("invalidDocument", message)

    class InvalidBackground(message: String) : SketchCanvasError("invalidBackground", message)

    class ExportFailed(message: String) : SketchCanvasError("exportFailed", message)

    companion object {
        fun from(error: Throwable, fallback: (String) -> SketchCanvasError): SketchCanvasError =
            when (error) {
                is SketchCanvasError -> error
                is SketchException.InvalidDocument -> InvalidDocument(error.message.orEmpty())
                is SketchException.InvalidBackground -> InvalidBackground(error.message.orEmpty())
                else -> fallback(error.message ?: error.toString())
            }
    }
}

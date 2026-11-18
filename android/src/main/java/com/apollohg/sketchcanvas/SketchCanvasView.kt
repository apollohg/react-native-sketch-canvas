package com.apollohg.sketchcanvas

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Matrix
import android.graphics.Path
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Base64
import android.view.MotionEvent
import expo.modules.kotlin.AppContext
import expo.modules.kotlin.viewevent.EventDispatcher
import expo.modules.kotlin.views.ExpoView
import java.io.ByteArrayOutputStream
import java.util.concurrent.Executors
import kotlin.math.roundToInt
import uniffi.sketch_core.Affine
import uniffi.sketch_core.Background
import uniffi.sketch_core.BackgroundWarning
import uniffi.sketch_core.Color as SketchColor
import uniffi.sketch_core.LiveUpdate
import uniffi.sketch_core.Pen
import uniffi.sketch_core.SessionState
import uniffi.sketch_core.SketchSession
import uniffi.sketch_core.fitTransform
import uniffi.sketch_core.parseBackground

@SuppressLint("ViewConstructor")
class SketchCanvasView(context: Context, appContext: AppContext) : ExpoView(context, appContext) {
    private val onSketchChange by EventDispatcher<Map<String, Any>>()
    private val onStrokeStart by EventDispatcher<Map<String, Any>>()
    private val onStrokeEnd by EventDispatcher<Map<String, Any>>()
    private val onBackgroundLoad by EventDispatcher<Map<String, Any>>()
    private val onSketchError by EventDispatcher<Map<String, Any>>()

    /** Receives events instead of the Expo dispatchers, which need a React host. */
    internal var eventObserverForTesting: ((String, Map<String, Any>) -> Unit)? = null

    var pen = Pen(SketchColor(0u, 0u, 0u, OPAQUE), DEFAULT_MIN_WIDTH, DEFAULT_MAX_WIDTH)

    private var session: SketchSession? = null
    private var editable = true
    private var canvasBackgroundColor: Int? = null
    private var backgroundSvg: String? = null
    private var background: Background? = null
    private var backgroundPending = false
    private var initialDocument: String? = null
    private var initialDocumentConsumed = false
    private var lastEmittedRevision: ULong? = null

    /** Added to each session's own revision so revisions keep increasing when a session is replaced. */
    private var revisionBase: ULong = 0u

    private val tracker = SketchPointerTracker()
    private val cache = SketchRasterCache()
    private val mainHandler = Handler(Looper.getMainLooper())
    private var liveColor: SketchColor? = null
    private var liveTail: Path? = null

    init {
        setWillNotDraw(false)
        importantForAccessibility = IMPORTANT_FOR_ACCESSIBILITY_YES
    }

    val hasLiveStroke: Boolean
        get() = session?.hasLiveStroke() ?: false

    private val density: Float
        get() = resources.displayMetrics.density

    // region Props

    fun setEditable(editable: Boolean) {
        this.editable = editable
        if (!editable) cancelLiveStroke()
    }

    fun setCanvasBackgroundColor(color: Int?) {
        canvasBackgroundColor = color
        rebuildCache()
    }

    fun setBackgroundSvg(svg: String?) {
        if (svg == backgroundSvg) return
        backgroundSvg = svg
        background = null
        backgroundPending = svg != null
        rebuildCache()
        if (svg == null) return
        worker.execute {
            val result = runCatching { parseBackground(svg) }
            mainHandler.post { backgroundParsed(svg, result) }
        }
    }

    /** Read once, when the canvas first creates its document. */
    fun setInitialDocument(json: String?) {
        if (!initialDocumentConsumed) initialDocument = json
    }

    fun didUpdateProps() = ensureSession()

    private fun backgroundParsed(svg: String, result: Result<Background>) {
        if (svg != backgroundSvg) return
        backgroundPending = false
        result
            .onSuccess {
                background = it
                emit(
                    EVENT_BACKGROUND_LOAD,
                    mapOf(
                        "warnings" to it.warnings.map { warning ->
                            warning.eventName()
                        }
                    )
                )
            }.onFailure {
                emitError(
                    SketchCanvasError.from(it) { message ->
                        SketchCanvasError.InvalidBackground(message)
                    }
                )
            }
        ensureSession()
        rebuildCache()
    }

    // endregion

    // region Session

    private fun ensureSession() {
        if (session != null) return
        val json = initialDocument
        if (!initialDocumentConsumed && json != null) {
            initialDocumentConsumed = true
            runCatching { SketchSession.fromJson(json) }
                .onSuccess { return adopt(it) }
                .onFailure {
                    emitError(
                        SketchCanvasError.from(it) { message ->
                            SketchCanvasError.InvalidDocument(message)
                        }
                    )
                }
        }
        if (backgroundPending) return
        background?.let { return adopt(SketchSession.newEmpty(it.width, it.height)) }
        if (width <= 0 || height <= 0) return
        adopt(SketchSession.newEmpty((width / density).toDouble(), (height / density).toDouble()))
    }

    private fun adopt(newSession: SketchSession) {
        initialDocumentConsumed = true
        session = newSession
        lastEmittedRevision?.let { revisionBase = it + 1u }
        rebuildCache()
        emitChange(newSession.state())
    }

    /** Document units to view points. */
    private fun docToView(): Affine {
        val size = session?.documentSize()
        if (size == null || width <= 0 || height <= 0) return IDENTITY
        return fitTransform(
            size.width,
            size.height,
            (width / density).toDouble(),
            (height / density).toDouble()
        )
    }

    private fun backgroundToDocument(): Affine {
        val bg = background ?: return IDENTITY
        val size = session?.documentSize() ?: return IDENTITY
        return fitTransform(bg.width, bg.height, size.width, size.height)
    }

    // endregion

    // region Layout and drawing

    override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
        super.onSizeChanged(w, h, oldw, oldh)
        cancelLiveStroke()
        ensureSession()
        rebuildCache()
    }

    /** Rebuilding drops the live layer, so a stroke in progress is cancelled first. */
    private fun rebuildCache() {
        if (width <= 0 || height <= 0) return
        if (hasLiveStroke) cancelLiveStroke()
        val docToView = docToView().toMatrix()
        cache.rebuild(
            width,
            height,
            density,
            canvasBackgroundColor,
            background,
            Matrix(docToView).apply { preConcat(backgroundToDocument().toMatrix()) },
            session?.committedPaths().orEmpty(),
            docToView
        )
        invalidate()
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        cache.draw(canvas, liveTail, liveColor)
    }

    // endregion

    // region Input

    override fun onTouchEvent(event: MotionEvent): Boolean {
        if (!editable) return false
        val index = event.actionIndex
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                parent?.requestDisallowInterceptTouchEvent(true)
                pointerBegan(
                    event.getPointerId(index),
                    SketchPointerSample.current(event, index, density)
                )
            }

            MotionEvent.ACTION_POINTER_DOWN ->
                pointerBegan(
                    event.getPointerId(index),
                    SketchPointerSample.current(event, index, density)
                )

            MotionEvent.ACTION_MOVE ->
                for (pointerIndex in 0 until event.pointerCount) {
                    pointerMoved(
                        event.getPointerId(pointerIndex),
                        SketchPointerSample.batch(event, pointerIndex, density)
                    )
                }

            MotionEvent.ACTION_POINTER_UP, MotionEvent.ACTION_UP -> {
                val pointer = event.getPointerId(index)
                if (isPalmCancel(event)) {
                    pointerCancelled(pointer)
                } else {
                    pointerEnded(pointer, SketchPointerSample.current(event, index, density))
                }
            }

            MotionEvent.ACTION_CANCEL ->
                for (pointerIndex in 0 until event.pointerCount) {
                    pointerCancelled(
                        event.getPointerId(pointerIndex)
                    )
                }
        }
        return true
    }

    private fun isPalmCancel(event: MotionEvent) =
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            event.flags and MotionEvent.FLAG_CANCELED != 0

    private fun pointerBegan(pointer: Int, sample: SketchPointerSample) {
        when (tracker.began(pointer, sample.isStylus)) {
            SketchPointerTracker.Decision.IGNORE -> Unit

            SketchPointerTracker.Decision.CANCEL -> cancelLiveStroke()

            SketchPointerTracker.Decision.CANCEL_THEN_BEGIN -> {
                cancelLiveStroke()
                beginStroke(sample)
            }

            SketchPointerTracker.Decision.BEGIN -> beginStroke(sample)
        }
    }

    private fun pointerMoved(pointer: Int, samples: List<SketchPointerSample>) {
        val session = session ?: return
        if (!tracker.isDrawing(pointer) || !session.hasLiveStroke()) return
        apply(session.extendStroke(samples.map { it.inputSample() }, emptyList()))
    }

    private fun pointerEnded(pointer: Int, final: SketchPointerSample) {
        pointerMoved(pointer, listOf(final))
        if (!tracker.ended(pointer)) return
        val color = liveColor ?: return
        // The core discards a stroke that would take the document past the format's limits.
        val commit = session?.endStroke() ?: return cancelLiveStroke()
        cache.composite(commit.path, color, docToView().toMatrix())
        clearLive()
        invalidate()
        emitChange(commit.state)
        emit(EVENT_STROKE_END, emptyMap())
    }

    private fun pointerCancelled(pointer: Int) {
        if (tracker.ended(pointer)) cancelLiveStroke()
    }

    private fun beginStroke(sample: SketchPointerSample) {
        val session = session ?: return
        val viewToDoc = Matrix().also { docToView().toMatrix().invert(it) }.toAffine()
        val update = session.beginStroke(pen, viewToDoc, sample.inputSample())
        cache.clearLive()
        liveColor = pen.color
        apply(update)
        emit(EVENT_STROKE_START, emptyMap())
    }

    private fun apply(update: LiveUpdate) {
        val color = liveColor ?: return
        val matrix = docToView().toMatrix()
        cache.appendLive(update.settled, matrix, color)
        liveTail = update.tail.toAndroidPath().apply { transform(matrix) }
        invalidate()
    }

    private fun cancelLiveStroke() {
        tracker.abandonDrawing()
        session?.cancelStroke()
        clearLive()
        invalidate()
    }

    private fun clearLive() {
        cache.clearLive()
        liveColor = null
        liveTail = null
    }

    // endregion

    // region Commands

    fun undo() = applyCommand { it.undo() }

    fun redo() = applyCommand { it.redo() }

    fun clear() = applyCommand { it.clear() }

    private fun applyCommand(command: (SketchSession) -> SessionState) {
        val session = session ?: return
        tracker.abandonDrawing()
        clearLive()
        val state = command(session)
        rebuildCache()
        emitChange(state)
    }

    /** Serialises off the main thread; excludes any stroke in progress. */
    fun getDocument(completion: (Result<String>) -> Unit) {
        val session =
            session
                ?: return completion(Result.failure(SketchCanvasError.ExportFailed(NO_DOCUMENT)))
        worker.execute {
            val json = session.toJson()
            mainHandler.post { completion(Result.success(json)) }
        }
    }

    /** Parses off the main thread; an invalid document fails and leaves the canvas unchanged. */
    fun loadDocument(json: String, completion: (Result<Unit>) -> Unit) {
        val existing = session
        worker.execute {
            val result =
                runCatching {
                    existing?.also { it.loadJson(json) } ?: SketchSession.fromJson(json)
                }.recoverCatching {
                    throw SketchCanvasError.from(it) { message ->
                        SketchCanvasError.InvalidDocument(message)
                    }
                }
            mainHandler.post {
                result
                    .onSuccess { loaded ->
                        tracker.abandonDrawing()
                        clearLive()
                        if (session !== loaded) {
                            session = null
                            adopt(loaded)
                        } else {
                            rebuildCache()
                            emitChange(loaded.state())
                        }
                        completion(Result.success(Unit))
                    }.onFailure { completion(Result.failure(it)) }
            }
        }
    }

    fun exportSvg(includeBackground: Boolean, completion: (Result<String>) -> Unit) {
        val session =
            session
                ?: return completion(Result.failure(SketchCanvasError.ExportFailed(NO_DOCUMENT)))
        val svg = if (includeBackground) backgroundSvg else null
        worker.execute {
            val result =
                runCatching { session.exportSvg(svg) }
                    .recoverCatching {
                        throw SketchCanvasError.ExportFailed(
                            SketchCanvasError.from(it) { m ->
                                SketchCanvasError.ExportFailed(m)
                            }.message.orEmpty()
                        )
                    }
            mainHandler.post { completion(result) }
        }
    }

    /** A PNG data URL of the document at its on-screen size times `scale` (default: screen density). */
    fun exportPng(scale: Double?, includeBackground: Boolean): String {
        val session = session ?: throw SketchCanvasError.ExportFailed(NO_DOCUMENT)
        val size = session.documentSize()
        val fitScale = if (width > 0 && height > 0) docToView().a else 1.0
        val pixelScale = (fitScale * (scale ?: density.toDouble())).toFloat()
        val pixelWidth = (size.width * pixelScale).roundToInt()
        val pixelHeight = (size.height * pixelScale).roundToInt()
        if (pixelWidth < 1 || pixelHeight < 1) {
            throw SketchCanvasError.ExportFailed("Export size ${pixelWidth}x$pixelHeight is empty")
        }
        val bitmap = Bitmap.createBitmap(pixelWidth, pixelHeight, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bitmap).apply { scale(pixelScale, pixelScale) }
        canvasBackgroundColor?.let { canvas.drawColor(it) }
        val bg = background
        if (includeBackground && bg != null) {
            canvas.save()
            canvas.concat(backgroundToDocument().toMatrix())
            SketchRenderer.draw(bg.ops, canvas)
            canvas.restore()
        }
        SketchRenderer.drawStrokes(session.committedPaths(), canvas)
        val png = ByteArrayOutputStream()
        if (!bitmap.compress(Bitmap.CompressFormat.PNG, PNG_QUALITY, png)) {
            throw SketchCanvasError.ExportFailed("PNG encoding failed")
        }
        return PNG_DATA_URL_PREFIX + Base64.encodeToString(png.toByteArray(), Base64.NO_WRAP)
    }

    // endregion

    // region Events

    private fun emitChange(state: SessionState) {
        val revision = revisionBase + state.revision
        if (revision == lastEmittedRevision) return
        lastEmittedRevision = revision
        emit(
            EVENT_CHANGE,
            mapOf(
                "strokeCount" to state.strokeCount.toInt(),
                "canUndo" to state.canUndo,
                "canRedo" to state.canRedo,
                "revision" to revision.toString()
            )
        )
    }

    private fun emitError(error: SketchCanvasError) {
        emit(EVENT_ERROR, mapOf("code" to error.code, "message" to error.message.orEmpty()))
    }

    private fun emit(name: String, payload: Map<String, Any>) {
        eventObserverForTesting?.let { observer ->
            observer(name, payload)
            return
        }
        when (name) {
            EVENT_CHANGE -> onSketchChange(payload)
            EVENT_STROKE_START -> onStrokeStart(payload)
            EVENT_STROKE_END -> onStrokeEnd(payload)
            EVENT_BACKGROUND_LOAD -> onBackgroundLoad(payload)
            EVENT_ERROR -> onSketchError(payload)
        }
    }

    // endregion

    private companion object {
        const val OPAQUE: UByte = 255u
        const val DEFAULT_MIN_WIDTH = 1.0
        const val DEFAULT_MAX_WIDTH = 3.0
        const val PNG_QUALITY = 100
        const val PNG_DATA_URL_PREFIX = "data:image/png;base64,"
        const val NO_DOCUMENT = "The canvas has no document yet"
        const val EVENT_CHANGE = "onSketchChange"
        const val EVENT_STROKE_START = "onStrokeStart"
        const val EVENT_STROKE_END = "onStrokeEnd"
        const val EVENT_BACKGROUND_LOAD = "onBackgroundLoad"
        const val EVENT_ERROR = "onSketchError"
        val IDENTITY = Affine(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)

        /** Background parsing and document serialisation, off the main thread. */
        val worker =
            Executors.newSingleThreadExecutor { runnable ->
                Thread(runnable, "SketchCanvasWorker").apply { isDaemon = true }
            }

        fun Matrix.toAffine(): Affine {
            val v = FloatArray(MATRIX_VALUES).also { getValues(it) }
            return Affine(
                v[Matrix.MSCALE_X].toDouble(),
                v[Matrix.MSKEW_Y].toDouble(),
                v[Matrix.MSKEW_X].toDouble(),
                v[Matrix.MSCALE_Y].toDouble(),
                v[Matrix.MTRANS_X].toDouble(),
                v[Matrix.MTRANS_Y].toDouble()
            )
        }

        const val MATRIX_VALUES = 9

        fun BackgroundWarning.eventName() = when (this) {
            BackgroundWarning.UNSUPPORTED_TEXT -> "unsupportedText"
            BackgroundWarning.UNSUPPORTED_FILTER -> "unsupportedFilter"
            BackgroundWarning.UNSUPPORTED_MASK -> "unsupportedMask"
            BackgroundWarning.UNSUPPORTED_PATTERN -> "unsupportedPattern"
            BackgroundWarning.UNSUPPORTED_EXTERNAL_IMAGE -> "unsupportedExternalImage"
        }
    }
}

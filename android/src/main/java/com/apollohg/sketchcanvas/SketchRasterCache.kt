package com.apollohg.sketchcanvas

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Matrix
import android.graphics.Path
import android.graphics.PorterDuff
import uniffi.sketch_core.Background
import uniffi.sketch_core.Color as SketchColor
import uniffi.sketch_core.CommittedStroke
import uniffi.sketch_core.PathData

/**
 * Bitmaps behind the canvas at pixel size: committed content (fill, background, strokes) and the
 * live stroke. Settled live pieces gather in a path that is flushed to the live bitmap in chunks,
 * so each frame draws a bounded amount; live ink is drawn opaque and composited once at the
 * pen's alpha so overlapping pieces never darken.
 */
class SketchRasterCache {
    private var committed: Bitmap? = null
    private var committedCanvas: Canvas? = null
    private var live: Bitmap? = null
    private var liveCanvas: Canvas? = null
    private val pendingLive = Path()
    private var pendingLiveVerbs = 0
    private var density = 1f

    fun rebuild(
        widthPx: Int,
        heightPx: Int,
        density: Float,
        fill: Int?,
        background: Background?,
        backgroundMatrix: Matrix,
        strokes: List<CommittedStroke>,
        strokeMatrix: Matrix
    ) {
        this.density = density
        resetLive()
        if (widthPx <= 0 || heightPx <= 0) {
            committed = null
            committedCanvas = null
            live = null
            liveCanvas = null
            return
        }
        val committedBitmap = Bitmap.createBitmap(widthPx, heightPx, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(committedBitmap).apply { scale(density, density) }
        fill?.let { canvas.drawColor(it) }
        if (background != null) {
            canvas.save()
            canvas.concat(backgroundMatrix)
            SketchRenderer.draw(background.ops, canvas)
            canvas.restore()
        }
        canvas.save()
        canvas.concat(strokeMatrix)
        SketchRenderer.drawStrokes(strokes, canvas)
        canvas.restore()
        committed = committedBitmap
        committedCanvas = canvas
        live = Bitmap.createBitmap(widthPx, heightPx, Bitmap.Config.ARGB_8888)
        liveCanvas = Canvas(live!!).apply { scale(density, density) }
    }

    fun composite(path: PathData, color: SketchColor, matrix: Matrix) {
        val canvas = committedCanvas ?: return
        canvas.save()
        canvas.concat(matrix)
        SketchRenderer.drawFill(path, color, canvas)
        canvas.restore()
    }

    fun appendLive(path: PathData, matrix: Matrix, color: SketchColor) {
        if (path.verbs.isEmpty()) return
        pendingLive.addPath(path.toAndroidPath().apply { transform(matrix) })
        pendingLiveVerbs += path.verbs.size
        if (pendingLiveVerbs >= LIVE_FLUSH_VERBS) {
            liveCanvas?.drawPath(pendingLive, SketchRenderer.fillPaint(color.opaque()))
            pendingLive.reset()
            pendingLiveVerbs = 0
        }
    }

    fun clearLive() {
        resetLive()
        liveCanvas?.drawColor(Color.TRANSPARENT, PorterDuff.Mode.CLEAR)
    }

    /** Draws committed content, then the live layer and `tail` (view-point coordinates). */
    fun draw(canvas: Canvas, tail: Path?, liveColor: SketchColor?) {
        committed?.let { canvas.drawBitmap(it, 0f, 0f, null) }
        if (liveColor == null) return
        val restore = canvas.saveLayerAlpha(null, liveColor.a.toInt())
        live?.let { canvas.drawBitmap(it, 0f, 0f, null) }
        canvas.scale(density, density)
        val paint = SketchRenderer.fillPaint(liveColor.opaque())
        canvas.drawPath(pendingLive, paint)
        tail?.let { canvas.drawPath(it, paint) }
        canvas.restoreToCount(restore)
    }

    private fun resetLive() {
        pendingLive.reset()
        pendingLiveVerbs = 0
    }

    private companion object {
        const val LIVE_FLUSH_VERBS = 2048
        const val OPAQUE: UByte = 255u

        fun SketchColor.opaque() = SketchColor(r, g, b, OPAQUE)
    }
}

package com.apollohg.sketchcanvas

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.DashPathEffect
import android.graphics.LinearGradient
import android.graphics.Paint as AndroidPaint
import android.graphics.Path
import android.graphics.RadialGradient
import android.graphics.RectF
import android.graphics.Shader
import android.os.Build
import android.util.LruCache
import kotlin.math.roundToInt
import uniffi.sketch_core.Color as SketchColor
import uniffi.sketch_core.CommittedStroke
import uniffi.sketch_core.DrawOp
import uniffi.sketch_core.FillRule
import uniffi.sketch_core.GradientStop
import uniffi.sketch_core.LineCap
import uniffi.sketch_core.LineJoin
import uniffi.sketch_core.Paint as SketchPaint
import uniffi.sketch_core.PathData
import uniffi.sketch_core.Spread
import uniffi.sketch_core.StrokeParams

/** Draws the core's draw ops on an Android canvas whose matrix maps op coordinates to view space. */
object SketchRenderer {
    private const val DECODED_IMAGE_CACHE_BYTES = 16 * 1024 * 1024
    private const val OPAQUE = 255

    private class ImageKey(val bytes: ByteArray) {
        override fun equals(other: Any?) = other is ImageKey && other.bytes.contentEquals(bytes)

        override fun hashCode() = bytes.contentHashCode()
    }

    private val decodedImages =
        object : LruCache<ImageKey, Bitmap>(DECODED_IMAGE_CACHE_BYTES) {
            override fun sizeOf(key: ImageKey, value: Bitmap) = value.byteCount
        }

    fun draw(ops: List<DrawOp>, canvas: Canvas) {
        for (op in ops) {
            when (op) {
                is DrawOp.FillPath -> {
                    canvas.save()
                    canvas.concat(op.transform.toMatrix())
                    val path = op.path.toAndroidPath().apply { fillType = op.rule.toFillType() }
                    canvas.drawPath(path, paintFor(op.paint, op.opacity, AndroidPaint.Style.FILL))
                    canvas.restore()
                }

                is DrawOp.StrokePath -> {
                    canvas.save()
                    canvas.concat(op.transform.toMatrix())
                    val paint = paintFor(op.paint, op.opacity, AndroidPaint.Style.STROKE).apply {
                        applyStroke(op.params)
                    }
                    canvas.drawPath(op.path.toAndroidPath(), paint)
                    canvas.restore()
                }

                is DrawOp.PushClip -> {
                    canvas.save()
                    val clip =
                        op.path.toAndroidPath().apply {
                            fillType = op.rule.toFillType()
                            transform(op.transform.toMatrix())
                        }
                    canvas.clipPath(clip)
                }

                DrawOp.PopClip -> canvas.restore()

                is DrawOp.PushLayer -> canvas.saveLayerAlpha(
                    null,
                    (op.opacity * OPAQUE).roundToInt()
                )

                DrawOp.PopLayer -> canvas.restore()

                is DrawOp.Image -> {
                    val bitmap = decodedImage(op.data) ?: continue
                    canvas.save()
                    canvas.concat(op.transform.toMatrix())
                    canvas.drawBitmap(
                        bitmap,
                        null,
                        RectF(op.x, op.y, op.x + op.width, op.y + op.height),
                        AndroidPaint(
                            AndroidPaint.FILTER_BITMAP_FLAG or AndroidPaint.ANTI_ALIAS_FLAG
                        )
                    )
                    canvas.restore()
                }
            }
        }
    }

    fun drawStrokes(strokes: List<CommittedStroke>, canvas: Canvas) {
        for (stroke in strokes) {
            drawFill(stroke.path, stroke.color, canvas)
        }
    }

    fun drawFill(path: PathData, color: SketchColor, canvas: Canvas) {
        canvas.drawPath(
            path.toAndroidPath().apply {
                fillType = Path.FillType.WINDING
            },
            fillPaint(color)
        )
    }

    fun fillPaint(color: SketchColor) = AndroidPaint(AndroidPaint.ANTI_ALIAS_FLAG).apply {
        style = AndroidPaint.Style.FILL
        this.color = color.toArgb()
    }

    private fun paintFor(paint: SketchPaint, opacity: Float, paintStyle: AndroidPaint.Style) =
        AndroidPaint(AndroidPaint.ANTI_ALIAS_FLAG).apply {
            style = paintStyle
            when (paint) {
                is SketchPaint.Solid -> color = paint.v1.toArgb(opacity)

                is SketchPaint.LinearGradient -> {
                    shader =
                        LinearGradient(
                            paint.x1,
                            paint.y1,
                            paint.x2,
                            paint.y2,
                            paint.stops.colors(),
                            paint.stops.positions(),
                            paint.spread.toTileMode()
                        ).apply { setLocalMatrix(paint.transform.toMatrix()) }
                    alpha = (opacity * OPAQUE).roundToInt()
                }

                is SketchPaint.RadialGradient -> {
                    shader =
                        radialGradient(paint).apply { setLocalMatrix(paint.transform.toMatrix()) }
                    alpha = (opacity * OPAQUE).roundToInt()
                }
            }
        }

    private fun radialGradient(paint: SketchPaint.RadialGradient): Shader {
        val focused = paint.fx != paint.cx || paint.fy != paint.cy
        if (focused && Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            return RadialGradient(
                paint.fx,
                paint.fy,
                0f,
                paint.cx,
                paint.cy,
                paint.r,
                paint.stops.map { android.graphics.Color.pack(it.color.toArgb()) }.toLongArray(),
                paint.stops.positions(),
                paint.spread.toTileMode()
            )
        }
        return RadialGradient(
            paint.cx,
            paint.cy,
            paint.r,
            paint.stops.colors(),
            paint.stops.positions(),
            paint.spread.toTileMode()
        )
    }

    private fun AndroidPaint.applyStroke(params: StrokeParams) {
        strokeWidth = params.width
        strokeMiter = params.miterLimit
        strokeCap =
            when (params.cap) {
                LineCap.BUTT -> AndroidPaint.Cap.BUTT
                LineCap.ROUND -> AndroidPaint.Cap.ROUND
                LineCap.SQUARE -> AndroidPaint.Cap.SQUARE
            }
        strokeJoin =
            when (params.join) {
                LineJoin.MITER -> AndroidPaint.Join.MITER
                LineJoin.ROUND -> AndroidPaint.Join.ROUND
                LineJoin.BEVEL -> AndroidPaint.Join.BEVEL
            }
        if (params.dash.isNotEmpty()) {
            // DashPathEffect needs an even count; SVG repeats an odd dash list to make one.
            val dash = if (params.dash.size % 2 == 0) params.dash else params.dash + params.dash
            pathEffect = DashPathEffect(dash.toFloatArray(), params.dashOffset)
        }
    }

    private fun decodedImage(data: ByteArray): Bitmap? {
        val key = ImageKey(data)
        decodedImages.get(key)?.let { return it }
        val bitmap = BitmapFactory.decodeByteArray(data, 0, data.size) ?: return null
        decodedImages.put(key, bitmap)
        return bitmap
    }
}

fun SketchColor.toArgb(opacity: Float = 1f): Int =
    android.graphics.Color.argb((a.toInt() * opacity).roundToInt(), r.toInt(), g.toInt(), b.toInt())

private fun List<GradientStop>.colors() = map { it.color.toArgb() }.toIntArray()

private fun List<GradientStop>.positions() = map { it.offset }.toFloatArray()

private fun Spread.toTileMode() = when (this) {
    Spread.PAD -> Shader.TileMode.CLAMP
    Spread.REFLECT -> Shader.TileMode.MIRROR
    Spread.REPEAT -> Shader.TileMode.REPEAT
}

private fun FillRule.toFillType() = when (this) {
    FillRule.NON_ZERO -> Path.FillType.WINDING
    FillRule.EVEN_ODD -> Path.FillType.EVEN_ODD
}

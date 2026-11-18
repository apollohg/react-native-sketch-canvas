package com.apollohg.sketchcanvas

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.Matrix
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import kotlin.math.abs
import kotlin.math.max
import org.json.JSONObject
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.sketch_core.SketchSession
import uniffi.sketch_core.fitTransform
import uniffi.sketch_core.parseBackground

@RunWith(AndroidJUnit4::class)
class SketchRendererGoldenTest {
    private val assets = InstrumentationRegistry.getInstrumentation().context.assets

    @Test
    fun rendererMatchesGoldens() {
        val failures = SCENES.mapNotNull { scene -> compareScene(scene) }
        assertTrue(failures.joinToString("\n"), failures.isEmpty())
    }

    private fun compareScene(name: String): String? {
        val scene = JSONObject(assets.open("goldens/$name.json").bufferedReader().readText())
        val width = scene.getInt("width")
        val height = scene.getInt("height")
        val session = SketchSession.fromJson(scene.getJSONObject("document").toString())
        val background = scene.optString("background").takeIf {
            it.isNotEmpty() && it != "null"
        }?.let {
            parseBackground(assets.open("backgrounds/$it").bufferedReader().readText())
        }

        val actual = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(actual)
        scene.optString("backgroundColor").takeIf { it.startsWith("#") }?.let {
            canvas.drawColor(android.graphics.Color.parseColor(it))
        }
        val size = session.documentSize()
        val docToView = fitTransform(
            size.width,
            size.height,
            width.toDouble(),
            height.toDouble()
        ).toMatrix()
        if (background != null) {
            val bgToDoc = fitTransform(
                background.width,
                background.height,
                size.width,
                size.height
            ).toMatrix()
            canvas.save()
            canvas.concat(Matrix(docToView).apply { preConcat(bgToDoc) })
            SketchRenderer.draw(background.ops, canvas)
            canvas.restore()
        }
        canvas.save()
        canvas.concat(docToView)
        SketchRenderer.drawStrokes(session.committedPaths(), canvas)
        canvas.restore()

        val expected = assets.open("goldens/$name.png").use { BitmapFactory.decodeStream(it) }
        var matching = 0
        var total = 0L
        val diff = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        for (y in 0 until height) {
            for (x in 0 until width) {
                val a = actual.getPixel(x, y)
                val e = expected.getPixel(x, y)
                var worst = 0
                for (shift in intArrayOf(0, 8, 16, 24)) {
                    val delta = abs(((a ushr shift) and 0xFF) - ((e ushr shift) and 0xFF))
                    worst = max(worst, delta)
                    total += delta
                }
                if (worst <= TOLERANCE) matching++
                diff.setPixel(x, y, android.graphics.Color.argb(255, minOf(255, worst * 4), 0, 0))
            }
        }
        val pixels = (width * height).toDouble()
        val matchFraction = matching / pixels
        val meanDifference = total / (pixels * 4)
        if (matchFraction >= REQUIRED_MATCH_FRACTION &&
            meanDifference <= MAX_MEAN_DIFFERENCE
        ) {
            return null
        }
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val outputDir = context.getExternalFilesDir(FAILURE_DIRECTORY)
        listOf("actual" to actual, "diff" to diff).forEach { (label, bitmap) ->
            File(outputDir, "$name-$label.png").outputStream().use {
                bitmap.compress(Bitmap.CompressFormat.PNG, 100, it)
            }
        }
        return FAILURE_FORMAT.format(
            name,
            matchFraction,
            TOLERANCE,
            REQUIRED_MATCH_FRACTION,
            meanDifference,
            MAX_MEAN_DIFFERENCE,
            outputDir
        )
    }

    private companion object {
        val SCENES =
            listOf(
                "strokes-only",
                "paths-bg",
                "nested-bg",
                "clip-bg",
                "gradient-bg",
                "raster-bg",
                "alpha-strokes"
            )
        const val TOLERANCE = 48
        const val REQUIRED_MATCH_FRACTION = 0.99
        const val MAX_MEAN_DIFFERENCE = 3.0
        const val FAILURE_DIRECTORY = "golden-failures"
        const val FAILURE_FORMAT =
            "%s: %.4f of pixels within %d (need %.2f), mean difference %.3f (max %.1f); images in %s"
    }
}

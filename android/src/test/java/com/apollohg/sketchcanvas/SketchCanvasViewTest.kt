package com.apollohg.sketchcanvas

import android.graphics.BitmapFactory
import android.os.Looper
import android.os.SystemClock
import android.util.Base64
import android.view.MotionEvent
import android.view.View
import android.widget.FrameLayout
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** The newest Robolectric API level that runs on the Java 17 test JVM, as in the editor. */
private const val ROBOLECTRIC_SDK = 34

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [ROBOLECTRIC_SDK])
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class SketchCanvasViewTest {
    private val events = mutableListOf<Pair<String, Map<String, Any>>>()

    private fun makeView(
        width: Int = 200,
        height: Int = 200,
        configure: (SketchCanvasView) -> Unit = {}
    ): SketchCanvasView {
        val expo = testExpoContext(RuntimeEnvironment.getApplication())
        val view = SketchCanvasView(expo.context, expo.appContext)
        view.eventObserverForTesting = { name, payload -> events += name to payload }
        configure(view)
        view.didUpdateProps()
        layout(view, width, height)
        return view
    }

    private fun layout(view: View, width: Int, height: Int) {
        view.measure(
            View.MeasureSpec.makeMeasureSpec(width, View.MeasureSpec.EXACTLY),
            View.MeasureSpec.makeMeasureSpec(height, View.MeasureSpec.EXACTLY)
        )
        view.layout(0, 0, width, height)
    }

    private fun motion(
        action: Int,
        pointers: List<Triple<Int, Float, Float>>,
        stylusIds: Set<Int> = emptySet(),
        actionIndex: Int = 0,
        time: Long = SystemClock.uptimeMillis()
    ): MotionEvent {
        val properties =
            pointers.map { (id, _, _) ->
                MotionEvent.PointerProperties().apply {
                    this.id = id
                    toolType =
                        if (id in
                            stylusIds
                        ) {
                            MotionEvent.TOOL_TYPE_STYLUS
                        } else {
                            MotionEvent.TOOL_TYPE_FINGER
                        }
                }
            }
        val coords =
            pointers.map { (_, x, y) ->
                MotionEvent.PointerCoords().apply {
                    this.x = x
                    this.y = y
                    pressure = HALF_PRESSURE
                    size = 1f
                }
            }
        val masked =
            if (action == MotionEvent.ACTION_POINTER_DOWN ||
                action == MotionEvent.ACTION_POINTER_UP
            ) {
                action or (actionIndex shl MotionEvent.ACTION_POINTER_INDEX_SHIFT)
            } else {
                action
            }
        return MotionEvent.obtain(
            0L,
            time,
            masked,
            pointers.size,
            properties.toTypedArray(),
            coords.toTypedArray(),
            0,
            0,
            1f,
            1f,
            0,
            0,
            0,
            0
        )
    }

    private fun drawLine(view: SketchCanvasView, from: Float, to: Float, y: Float = 100f) {
        view.dispatchTouchEvent(
            motion(MotionEvent.ACTION_DOWN, listOf(Triple(0, from, y)), time = 0)
        )
        for (step in 1..STEPS) {
            val x = from + (to - from) * step / STEPS
            view.dispatchTouchEvent(
                motion(
                    MotionEvent.ACTION_MOVE,
                    listOf(Triple(0, x, y)),
                    time =
                        step * FRAME_MS
                )
            )
        }
        view.dispatchTouchEvent(
            motion(
                MotionEvent.ACTION_UP,
                listOf(Triple(0, to, y)),
                time =
                    (STEPS + 1) * FRAME_MS
            )
        )
    }

    private fun <T> awaitResult(start: ((Result<T>) -> Unit) -> Unit): Result<T> {
        var result: Result<T>? = null
        start { result = it }
        val deadline = System.currentTimeMillis() + TIMEOUT_MS
        while (result == null && System.currentTimeMillis() < deadline) {
            shadowOf(Looper.getMainLooper()).idle()
            Thread.sleep(POLL_MS)
        }
        return result ?: fail("timed out").let { error("unreachable") }
    }

    private fun document(view: SketchCanvasView) = JSONObject(
        awaitResult<String> {
            view.getDocument(it)
        }.getOrThrow()
    )

    private fun strokeCount(view: SketchCanvasView) =
        document(view).getJSONArray("strokes").length()

    @Test
    fun strokeCommitsThroughMotionEvents() {
        val view = makeView()
        drawLine(view, 20f, 180f)

        assertEquals(1, strokeCount(view))
        val lastChange = events.last { it.first == "onSketchChange" }.second
        assertEquals("change events: $events", 1, lastChange["strokeCount"])
        assertEquals(1, events.count { it.first == "onStrokeEnd" })
    }

    @Test
    fun inputIgnoredBeforeBackgroundParses() {
        val view = makeView { it.setBackgroundSvg(BACKGROUND_SVG) }
        drawLine(view, 20f, 180f)
        val deadline = System.currentTimeMillis() + TIMEOUT_MS
        while (events.none { it.first == "onBackgroundLoad" } &&
            System.currentTimeMillis() < deadline
        ) {
            shadowOf(Looper.getMainLooper()).idle()
            Thread.sleep(POLL_MS)
        }
        assertTrue(
            "background never parsed: $events",
            events.any {
                it.first == "onBackgroundLoad"
            }
        )
        drawLine(view, 20f, 180f)

        val doc = document(view)
        assertEquals(
            "document should take the background's size: $doc",
            400.0,
            doc.getDouble("width"),
            0.0
        )
        assertEquals(
            "the stroke drawn before parsing must be ignored",
            1,
            doc.getJSONArray("strokes").length()
        )
    }

    @Test
    fun stylusIgnoresConcurrentFinger() {
        val view = makeView()
        val stylus = setOf(0)
        view.dispatchTouchEvent(
            motion(MotionEvent.ACTION_DOWN, listOf(Triple(0, 20f, 50f)), stylus, time = 0)
        )
        val both = listOf(Triple(0, 20f, 50f), Triple(1, 150f, 150f))
        view.dispatchTouchEvent(
            motion(MotionEvent.ACTION_POINTER_DOWN, both, stylus, actionIndex = 1, time = 10)
        )
        for (step in 1..STEPS) {
            val moved = listOf(Triple(0, 20f + step * 8f, 50f), Triple(1, 150f + step, 150f + step))
            view.dispatchTouchEvent(
                motion(
                    MotionEvent.ACTION_MOVE,
                    moved,
                    stylus,
                    time =
                        10 + step * FRAME_MS
                )
            )
        }
        val end = listOf(Triple(0, 100f, 50f), Triple(1, 160f, 160f))
        view.dispatchTouchEvent(
            motion(MotionEvent.ACTION_POINTER_UP, end, stylus, actionIndex = 1, time = 300)
        )
        view.dispatchTouchEvent(
            motion(MotionEvent.ACTION_UP, listOf(Triple(0, 100f, 50f)), stylus, time = 310)
        )

        val strokes = document(view).getJSONArray("strokes")
        assertEquals(1, strokes.length())
        val points = strokes.getJSONObject(0).getJSONArray("points")
        val ys = (0 until points.length()).map { points.getJSONArray(it).getDouble(1) }
        assertTrue(
            "finger samples leaked into the stylus stroke: $ys",
            ys.all {
                kotlin.math.abs(it - ys[0]) <
                    1
            }
        )
    }

    @Test
    fun secondFingerCancelsFingerStroke() {
        val view = makeView()
        view.dispatchTouchEvent(
            motion(MotionEvent.ACTION_DOWN, listOf(Triple(0, 20f, 100f)), time = 0)
        )
        view.dispatchTouchEvent(
            motion(MotionEvent.ACTION_MOVE, listOf(Triple(0, 60f, 100f)), time = FRAME_MS)
        )
        val both = listOf(Triple(0, 60f, 100f), Triple(1, 150f, 150f))
        view.dispatchTouchEvent(
            motion(MotionEvent.ACTION_POINTER_DOWN, both, actionIndex = 1, time = 20)
        )
        assertFalse(view.hasLiveStroke)
        val moved = listOf(Triple(0, 100f, 100f), Triple(1, 150f, 150f))
        view.dispatchTouchEvent(motion(MotionEvent.ACTION_MOVE, moved, time = 2 * FRAME_MS))
        view.dispatchTouchEvent(
            motion(MotionEvent.ACTION_POINTER_UP, moved, actionIndex = 1, time = 40)
        )
        view.dispatchTouchEvent(
            motion(MotionEvent.ACTION_UP, listOf(Triple(0, 100f, 100f)), time = 50)
        )

        assertEquals(0, strokeCount(view))
        assertEquals(0, events.count { it.first == "onStrokeEnd" })
    }

    @Test
    fun notEditablePassesTouchesThrough() {
        val view = makeView()
        view.setEditable(false)

        assertFalse(
            view.onTouchEvent(motion(MotionEvent.ACTION_DOWN, listOf(Triple(0, 100f, 100f))))
        )
    }

    @Test
    fun exportPngSize() {
        val view =
            makeView {
                it.setInitialDocument("""{"version":1,"width":300,"height":150,"strokes":[]}""")
            }
        val dataUrl = view.exportPng(2.0, true)
        val prefix = "data:image/png;base64,"
        assertTrue(dataUrl.startsWith(prefix))
        val bytes = Base64.decode(dataUrl.removePrefix(prefix), Base64.DEFAULT)
        val bitmap = BitmapFactory.decodeByteArray(bytes, 0, bytes.size)
        val density = view.resources.displayMetrics.density
        assertEquals(Math.round(300 * (200 / density / 300) * 2).toInt(), bitmap.width)
        assertEquals(Math.round(150 * (200 / density / 300) * 2).toInt(), bitmap.height)
    }

    @Test
    fun loadDocumentRejectsAndKeeps() {
        val view = makeView()
        val valid = """{"version":1,"width":100,"height":50,"strokes":[$RED_DOT]}"""
        awaitResult<Unit> { view.loadDocument(valid, it) }.getOrThrow()
        val before = document(view).toString()

        val result =
            awaitResult<Unit> {
                view.loadDocument("""{"version":1,"width":0,"height":50,"strokes":[]}""", it)
            }
        val error = result.exceptionOrNull() as? SketchCanvasError
        assertEquals("expected invalidDocument, got $result", "invalidDocument", error?.code)
        assertEquals(before, document(view).toString())
    }

    private class RecordingParent(context: android.content.Context) : FrameLayout(context) {
        val requests = mutableListOf<Boolean>()

        override fun requestDisallowInterceptTouchEvent(disallowIntercept: Boolean) {
            requests += disallowIntercept
            super.requestDisallowInterceptTouchEvent(disallowIntercept)
        }
    }

    @Test
    fun downDisallowsParentIntercept() {
        val view = makeView()
        val parent = RecordingParent(view.context)
        parent.addView(view)
        view.dispatchTouchEvent(motion(MotionEvent.ACTION_DOWN, listOf(Triple(0, 100f, 100f))))
        assertEquals(listOf(true), parent.requests)

        view.dispatchTouchEvent(motion(MotionEvent.ACTION_CANCEL, listOf(Triple(0, 100f, 100f))))
        parent.requests.clear()
        view.setEditable(false)
        view.dispatchTouchEvent(motion(MotionEvent.ACTION_DOWN, listOf(Triple(0, 100f, 100f))))
        assertEquals(emptyList<Boolean>(), parent.requests)
    }

    @Test
    fun resizeCancelsLiveStroke() {
        val view = makeView()
        drawLine(view, 20f, 120f)
        view.dispatchTouchEvent(
            motion(MotionEvent.ACTION_DOWN, listOf(Triple(0, 20f, 150f)), time = 1000)
        )
        view.dispatchTouchEvent(
            motion(MotionEvent.ACTION_MOVE, listOf(Triple(0, 80f, 150f)), time = 1016)
        )
        assertTrue(view.hasLiveStroke)

        layout(view, 320, 180)

        assertFalse(view.hasLiveStroke)
        view.dispatchTouchEvent(
            motion(MotionEvent.ACTION_UP, listOf(Triple(0, 80f, 150f)), time = 1032)
        )
        assertEquals(1, strokeCount(view))
    }

    private companion object {
        const val STEPS = 10
        const val FRAME_MS = 16L
        const val TIMEOUT_MS = 5_000L
        const val POLL_MS = 5L
        const val HALF_PRESSURE = 0.5f
        const val RED_DOT = """{"color":"#FF0000","points":[[10,10,2]]}"""
        const val BACKGROUND_SVG =
            """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 300"><rect width="400" height="300"/></svg>"""
    }
}

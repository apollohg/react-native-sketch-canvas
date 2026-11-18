package com.apollohg.sketchcanvas

import android.view.MotionEvent
import uniffi.sketch_core.InputSample
import uniffi.sketch_core.PointerKind

/** One pointer sample in view points (dp); `pressure` is set for a stylus only. */
data class SketchPointerSample(
    val x: Float,
    val y: Float,
    val timeMs: Long,
    val pressure: Float?,
    val isStylus: Boolean
) {
    fun inputSample() = InputSample(
        x = x.toDouble(),
        y = y.toDouble(),
        timeMs = timeMs.toDouble(),
        pressure = pressure?.toDouble(),
        kind = if (isStylus) PointerKind.STYLUS else PointerKind.TOUCH
    )

    companion object {
        fun current(event: MotionEvent, index: Int, density: Float) = from(
            event,
            index,
            event.getX(index),
            event.getY(index),
            event.eventTime,
            event.getPressure(index),
            density
        )

        /** Historical samples first, then the current one. */
        fun batch(event: MotionEvent, index: Int, density: Float): List<SketchPointerSample> =
            (0 until event.historySize).map { position ->
                from(
                    event,
                    index,
                    event.getHistoricalX(index, position),
                    event.getHistoricalY(index, position),
                    event.getHistoricalEventTime(position),
                    event.getHistoricalPressure(index, position),
                    density
                )
            } + current(event, index, density)

        private fun from(
            event: MotionEvent,
            index: Int,
            x: Float,
            y: Float,
            timeMs: Long,
            pressure: Float,
            density: Float
        ): SketchPointerSample {
            val isStylus = event.getToolType(index) == MotionEvent.TOOL_TYPE_STYLUS
            return SketchPointerSample(
                x = x / density,
                y = y / density,
                timeMs = timeMs,
                pressure = if (isStylus) pressure.coerceIn(0f, 1f) else null,
                isStylus = isStylus
            )
        }
    }
}

/**
 * Decides which pointer draws: one at a time, a stylus beats fingers (palm rejection), and a
 * second finger cancels a finger stroke and keeps fingers ignored until they all lift.
 */
class SketchPointerTracker {
    enum class Decision { BEGIN, CANCEL_THEN_BEGIN, CANCEL, IGNORE }

    var drawing: Int? = null
        private set
    private var drawingIsStylus = false
    private val down = mutableMapOf<Int, Boolean>()
    private var fingersSuppressed = false

    fun began(pointer: Int, isStylus: Boolean): Decision {
        val stylusDown = down.containsValue(true)
        down[pointer] = isStylus
        if (isStylus) {
            val fingerWasDrawing = drawing != null && !drawingIsStylus
            drawing = pointer
            drawingIsStylus = true
            return if (fingerWasDrawing) Decision.CANCEL_THEN_BEGIN else Decision.BEGIN
        }
        if (stylusDown || fingersSuppressed) return Decision.IGNORE
        if (drawing != null) {
            drawing = null
            fingersSuppressed = true
            return Decision.CANCEL
        }
        drawing = pointer
        drawingIsStylus = false
        return Decision.BEGIN
    }

    fun isDrawing(pointer: Int) = drawing == pointer

    /** Stops the current stroke without lifting its pointer, e.g. when the view resizes. */
    fun abandonDrawing() {
        drawing = null
    }

    /** Returns whether the lifted pointer was the one drawing. */
    fun ended(pointer: Int): Boolean {
        down.remove(pointer)
        if (!down.containsValue(false)) fingersSuppressed = false
        if (drawing != pointer) return false
        drawing = null
        return true
    }
}

package com.apollohg.sketchcanvas

import android.graphics.Matrix
import android.graphics.Path
import uniffi.sketch_core.Affine
import uniffi.sketch_core.PathData

private const val VERB_MOVE: Byte = 0
private const val VERB_LINE: Byte = 1
private const val VERB_QUAD: Byte = 2
private const val VERB_CUBIC: Byte = 3
private const val VERB_CLOSE: Byte = 4

fun PathData.toAndroidPath(): Path {
    val path = Path()
    val c = coords
    var i = 0
    for (verb in verbs) {
        when (verb) {
            VERB_MOVE -> path.moveTo(c[i], c[i + 1]).also { i += 2 }

            VERB_LINE -> path.lineTo(c[i], c[i + 1]).also { i += 2 }

            VERB_QUAD -> path.quadTo(c[i], c[i + 1], c[i + 2], c[i + 3]).also { i += 4 }

            VERB_CUBIC -> path.cubicTo(
                c[i],
                c[i + 1],
                c[i + 2],
                c[i + 3],
                c[i + 4],
                c[i + 5]
            ).also {
                i +=
                    6
            }

            VERB_CLOSE -> path.close()
        }
    }
    return path
}

fun Affine.toMatrix(): Matrix = Matrix().apply {
    setValues(
        floatArrayOf(
            a.toFloat(), c.toFloat(), e.toFloat(),
            b.toFloat(), d.toFloat(), f.toFloat(),
            0f, 0f, 1f
        )
    )
}

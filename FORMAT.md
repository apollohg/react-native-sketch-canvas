# Sketch document format, version 1

A sketch document is a JSON object holding strokes in its own coordinate space. This file is the complete contract: any implementation that follows it reads, validates and renders a document exactly as this library does.

## Schema

```json
{
  "version": 1,
  "width": 300,
  "height": 150,
  "strokes": [
    { "color": "#FF0000", "points": [[10.13, 20.5, 2.75], [40.33, 60.12, 3.1]] }
  ]
}
```

| Field | Meaning |
|---|---|
| `version` | Always `1` for this format. |
| `width`, `height` | The document's coordinate space. Fixed once the document exists. |
| `strokes` | Drawing order, first stroke at the bottom. |
| `color` | `#RRGGBB` or `#RRGGBBAA`, hex digits in either case. |
| `points` | `[x, y, w]` triples in document units: the centre of the stroke and its full width there. |

Coordinates may lie outside `0…width` and `0…height`. A document never names or embeds a background image; the host application records that separately.

## Validation

A reader rejects the whole document, and changes nothing, when any of these holds:

- it is not a JSON object, or has a field other than the four above (strokes likewise accept only `color` and `points`);
- `version` is not `1`;
- `width` or `height` is not a finite number greater than zero;
- a `color` is not `#` followed by exactly 6 or 8 hex digits;
- a stroke has no points;
- a point is not exactly three numbers, any of them is not finite, or `w` is not greater than zero.

## Writing

Writers emit numbers rounded to 2 decimal places with trailing zeros removed (`2.0` → `2`), widths no smaller than `0.01`, and colours in uppercase, as `#RRGGBB` when fully opaque and `#RRGGBBAA` otherwise.

## Rendering

Each stroke becomes one closed outline path, filled with its colour using the **non-zero** winding rule. Strokes are composited in order with ordinary source-over blending. This algorithm is frozen for version 1; a different algorithm means a new format version.

### Outline algorithm

Input: the points `p[0..n)` of one stroke, with radius `r[i] = w[i] / 2`. All arithmetic is IEEE 754 double precision.

1. **Centre line.** Build the sample list `S` of discs (centre, radius):
   - `n == 1`: `S = [p0]`.
   - `n == 2`: `S = [p0, p1]`.
   - `n >= 3`: let `m[i]` be the midpoint of `p[i]` and `p[i+1]`, with radius `(r[i] + r[i+1]) / 2`. The centre line runs
     - straight from `p0` to `m[0]`,
     - then, for `i` in `1 ..= n-2`, along the quadratic Bézier from `m[i-1]` with control `p[i]` to `m[i]`,
     - then straight to `p[n-1]`.

     The radius along each quadratic is the same quadratic Bézier applied to `(r(m[i-1]), r[i], r(m[i]))`.
     Flatten each quadratic uniformly in `t` with

     ```text
     tol = max(0.02 · max(r(m[i-1]), r[i], r(m[i])), 0.01)
     k   = clamp(ceil(sqrt(|m[i-1] − 2·p[i] + m[i]| / (4 · tol))), 1, 64)
     ```

     The cap of 64 only matters for degenerate input; it bounds the work any document can cause. Append the samples at `t = j / k` for `j = 1 ..= k`. Straight parts contribute only their end samples, so `S` starts `[p0, m[0], …]` and ends `[…, m[n-2], p[n-1]]`.
   - While building `S`, skip any sample whose centre lies within `1e-9` of the previous sample's centre.

2. **Pieces.** If `S` has one sample, emit its circle. Otherwise, for each consecutive pair of discs `A`, `B` with centre distance `d`:
   - If `d <= |rA − rB|`, one disc contains the other: emit a circle for the larger (for `A` when the radii are equal).
   - Otherwise emit the convex hull of the two discs (a "capsule"):

     ```text
     θ  = atan2(B.y − A.y, B.x − A.x)
     β  = acos((rA − rB) / d)
     a1 = A + rA·(cos(θ+β), sin(θ+β))     b1 = B + rB·(cos(θ+β), sin(θ+β))
     a2 = A + rA·(cos(θ−β), sin(θ−β))

     M a1
     L b1
     arc around B, radius rB, from angle θ+β to θ−β        (sweeps through θ)
     L a2
     arc around A, radius rA, from angle θ−β to θ+β−2π     (sweeps through θ+π)
     Z
     ```

     ```text
                 a1 ________________ b1
                  /                  \
          θ+π  ( A        →  θ        B )
                  \__________________/
                 a2                   b2
     ```

3. **Circles** start at angle 0 and run from angle `0` to `−2π`: `M (c.x + r, c.y)`, the arc, `Z`.

4. **Arcs become cubics.** An arc from angle `a` to `b` with `Δ = b − a` is split into `ceil(|Δ| / (π/2))` equal parts (at least 1). A part from `a0` to `a1` (step `δ`) is the cubic

   ```text
   h  = r · 4/3 · tan(δ / 4)
   P0 = c + r·(cos a0, sin a0)            P3 = c + r·(cos a1, sin a1)
   P1 = P0 + h·(−sin a0, cos a0)          P2 = P3 − h·(−sin a1, cos a1)
   ```

5. **Orientation.** Every arc sweeps toward decreasing angles, so each piece's signed shoelace area, taken over its on-curve and control points in order, is negative in y-down coordinates. All pieces wind the same way, and the non-zero fill of their union has no holes.

The outline is the concatenation of the pieces in order. Output coordinates are single precision.

### Incremental rendering

Appending points to a stroke only changes the samples after `m[n-2]`. A renderer drawing a stroke as it is captured may keep the pieces whose discs both come before that point and redraw only the rest.

## Fitting into a box

A document, or a background into a document, is drawn **aspect-fit and centred**: for content `cw × ch` in a box `bw × bh`,

```text
s  = min(bw / cw, bh / ch)
tx = (bw − cw·s) / 2
ty = (bh − ch·s) / 2
(x, y) → (s·x + tx, s·y + ty)
```

## Conformance suite

`fixtures/conformance/` holds input documents (`<name>.json`) and their expected outlines (`<name>.outline.json`):

```json
{ "strokes": [ { "verbs": [0, 1, 3, 3, 1, 3, 3, 3, 4], "coords": [ … ] } ] }
```

Verbs are `0` move (2 coords), `1` line (2), `2` quadratic (4), `3` cubic (6) and `4` close (0). An implementation conforms when, for every input, it emits exactly the expected verbs for each stroke and every coordinate within `1e-3` of the expected value.

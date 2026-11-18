// Exercises the built web package (npm run build:wasm) through its published ES module.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { before, describe, test } from 'node:test';
import { fileURLToPath, pathToFileURL } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const packageDir = path.join(repoRoot, 'rust/wasm');
const conformanceDir = path.join(repoRoot, 'fixtures/conformance');
const twoStrokesFixture = path.join(repoRoot, 'fixtures/documents/two-strokes.json');
const outlineSuffix = '.outline.json';
const conformanceTolerance = 1e-3;
const documentWidth = 5536.89;
const documentHeight = 5000;
const viewWidth = 820;
const viewHeight = 600;
const pen = { color: '#1A2B3C', minWidth: 1, maxWidth: 3 };
const conformanceNames = readdirSync(conformanceDir)
    .filter(file => file.endsWith('.json') && !file.endsWith(outlineSuffix))
    .map(file => file.slice(0, -'.json'.length))
    .sort();

let sketch;

function coordsPerVerb(verb) {
    const counts = new Map([
        [sketch.PathVerb.Move, 2],
        [sketch.PathVerb.Line, 2],
        [sketch.PathVerb.Quad, 4],
        [sketch.PathVerb.Cubic, 6],
        [sketch.PathVerb.Close, 0],
    ]);
    assert.ok(counts.has(verb), `unknown verb ${verb}`);
    return counts.get(verb);
}

function assertPathData(path, label) {
    assert.ok(path.verbs instanceof Uint8Array, `${label}: verbs is ${Object.prototype.toString.call(path.verbs)}`);
    assert.ok(path.coords instanceof Float32Array, `${label}: coords is ${Object.prototype.toString.call(path.coords)}`);
    const expected = [...path.verbs].reduce((sum, verb) => sum + coordsPerVerb(verb), 0);
    assert.equal(path.coords.length, expected, `${label}: ${path.coords.length} coords for verbs [${path.verbs}]`);
}

function assertLiveUpdate(update, label) {
    assert.deepEqual(Object.keys(update).sort(), ['settled', 'tail'], `${label}: keys`);
    assertPathData(update.settled, `${label}.settled`);
    assertPathData(update.tail, `${label}.tail`);
}

function bounds(path) {
    const xs = path.coords.filter((_, index) => index % 2 === 0);
    const ys = path.coords.filter((_, index) => index % 2 === 1);
    return { minX: Math.min(...xs), maxX: Math.max(...xs), minY: Math.min(...ys), maxY: Math.max(...ys) };
}

function invert({ a, b, c, d, e, f }) {
    const det = a * d - b * c;
    return { a: d / det, b: -b / det, c: -c / det, d: a / det, e: (c * f - d * e) / det, f: (b * e - a * f) / det };
}

function apply({ a, b, c, d, e, f }, x, y) {
    return [a * x + c * y + e, b * x + d * y + f];
}

function viewToDoc() {
    return invert(sketch.fitTransform(documentWidth, documentHeight, viewWidth, viewHeight));
}

function sample(x, y, timeMs, kind = 'touch', pressure = undefined) {
    return pressure === undefined ? { x, y, timeMs, kind } : { x, y, timeMs, kind, pressure };
}

function drawStroke(session, points, { kind = 'touch', strokePen = pen } = {}) {
    const samples = points.map(([x, y, pressure], index) => sample(x, y, index * 8, kind, pressure));
    assertLiveUpdate(session.beginStroke(strokePen, viewToDoc(), samples[0]), 'beginStroke');
    assert.equal(session.hasLiveStroke(), true, 'live stroke after beginStroke');
    for (let index = 1; index < samples.length; index += 2) {
        const batch = samples.slice(index, index + 2);
        const predicted = [{ ...batch.at(-1), x: batch.at(-1).x + 4, timeMs: batch.at(-1).timeMs + 8 }];
        assertLiveUpdate(session.extendStroke(batch, predicted), `extendStroke at sample ${index}`);
    }
    const commit = session.endStroke();
    assert.notEqual(commit, undefined, 'endStroke committed nothing');
    assert.equal(session.hasLiveStroke(), false, 'live stroke after endStroke');
    assertPathData(commit.path, 'endStroke.path');
    return commit;
}

function withSession(session, body) {
    try {
        return body(session);
    } finally {
        session.free();
    }
}

function assertThrowsMessage(action, message, label) {
    assert.throws(action, error => {
        assert.ok(error instanceof Error, `${label}: threw ${Object.prototype.toString.call(error)}, not an Error`);
        assert.equal(error.message, message, `${label}: message`);
        return true;
    }, `${label}: did not throw`);
}

function pathsAsArrays(session) {
    return session.committedPaths().map(({ path, color }) => ({ color, verbs: [...path.verbs], coords: [...path.coords] }));
}

before(async () => {
    const modulePath = path.join(packageDir, 'sketch_core_wasm.js');
    if (!existsSync(modulePath)) {
        throw new Error(`${modulePath} is missing; run npm run build:wasm first`);
    }
    sketch = await import(pathToFileURL(modulePath).href);
    await sketch.default({ module_or_path: readFileSync(path.join(packageDir, 'sketch_core_wasm_bg.wasm')) });
});

describe('sketch-core web package', () => {
    test('exports the path verbs', () => {
        assert.deepEqual(
            { Move: sketch.PathVerb.Move, Line: sketch.PathVerb.Line, Quad: sketch.PathVerb.Quad, Cubic: sketch.PathVerb.Cubic, Close: sketch.PathVerb.Close },
            { Move: 0, Line: 1, Quad: 2, Cubic: 3, Close: 4 },
        );
    });

    test('fitTransform aspect-fits and centres', () => {
        const fit = sketch.fitTransform(documentWidth, documentHeight, viewWidth, viewHeight);
        const scale = Math.min(viewWidth / documentWidth, viewHeight / documentHeight);
        assert.deepEqual(fit, {
            a: scale, b: 0, c: 0, d: scale,
            e: (viewWidth - documentWidth * scale) / 2, f: (viewHeight - documentHeight * scale) / 2,
        });
    });

    test('strokes land in document space through the inverse fit', () => {
        withSession(sketch.SketchSession.newEmpty(documentWidth, documentHeight), session => {
            assert.deepEqual(session.documentSize(), { width: documentWidth, height: documentHeight });
            assert.deepEqual(session.state(), { strokeCount: 0, canUndo: false, canRedo: false, revision: 0 });
            const points = Array.from({ length: 21 }, (_, index) => [100 + index * 10, 200 + Math.sin(index / 3) * 20]);
            const commit = drawStroke(session, points);
            assert.deepEqual(commit.state, { strokeCount: 1, canUndo: true, canRedo: false, revision: 1 });

            const toDoc = viewToDoc();
            const [startX] = apply(toDoc, 100, 0);
            const [endX] = apply(toDoc, 300, 0);
            const [, topY] = apply(toDoc, 0, 180);
            const [, bottomY] = apply(toDoc, 0, 220);
            const halfWidth = (pen.maxWidth / 2) * toDoc.a;
            const outline = bounds(commit.path);
            const expected = { minX: startX - halfWidth, maxX: endX + halfWidth, minY: topY - halfWidth, maxY: bottomY + halfWidth };
            const slack = 1e-3 * documentWidth;
            assert.ok(
                outline.minX >= expected.minX - slack && outline.maxX <= expected.maxX + slack
                    && outline.minY >= expected.minY - slack && outline.maxY <= expected.maxY + slack
                    && outline.maxX - outline.minX > (endX - startX) * 0.99,
                `outline ${JSON.stringify(outline)} is not the view stroke mapped into ${JSON.stringify(expected)}`,
            );
        });
    });

    test('undo, redo and an undoable clear', () => {
        withSession(sketch.SketchSession.newEmpty(documentWidth, documentHeight), session => {
            drawStroke(session, [[50, 50], [120, 90], [200, 60], [260, 140]]);
            drawStroke(session, [[400, 300, 0.2], [420, 330, 0.6], [450, 350, 1], [480, 360, 0.4]], { kind: 'stylus' });
            assert.deepEqual(session.state(), { strokeCount: 2, canUndo: true, canRedo: false, revision: 2 });
            const drawn = pathsAsArrays(session);

            assert.deepEqual(session.undo(), { strokeCount: 1, canUndo: true, canRedo: true, revision: 3 });
            assert.deepEqual(session.redo(), { strokeCount: 2, canUndo: true, canRedo: false, revision: 4 });
            assert.deepEqual(session.clear(), { strokeCount: 0, canUndo: true, canRedo: false, revision: 5 });
            assert.deepEqual(session.committedPaths(), [], 'committed paths after clear');
            assert.deepEqual(session.undo(), { strokeCount: 2, canUndo: true, canRedo: true, revision: 6 });
            assert.deepEqual(pathsAsArrays(session), drawn, 'undoing clear restores the strokes');
            assert.deepEqual(session.redo(), { strokeCount: 0, canUndo: true, canRedo: false, revision: 7 });
        });
    });

    test('live strokes cancel and only commit once', () => {
        withSession(sketch.SketchSession.newEmpty(documentWidth, documentHeight), session => {
            assert.equal(session.endStroke(), undefined, 'endStroke without a live stroke');
            assertLiveUpdate(session.extendStroke([sample(10, 10, 0)], []), 'extendStroke without a live stroke');
            session.beginStroke(pen, viewToDoc(), sample(10, 10, 0));
            session.extendStroke([sample(40, 10, 16), sample(80, 10, 32)], []);
            session.cancelStroke();
            assert.equal(session.hasLiveStroke(), false, 'live stroke after cancelStroke');
            assert.equal(session.endStroke(), undefined, 'endStroke after cancelStroke');
            assert.deepEqual(session.state(), { strokeCount: 0, canUndo: false, canRedo: false, revision: 0 });
        });
    });

    test('committed paths carry each stroke colour as hex', () => {
        withSession(sketch.SketchSession.newEmpty(documentWidth, documentHeight), session => {
            drawStroke(session, [[10, 10], [60, 60]], { strokePen: { ...pen, color: '#abcdef' } });
            drawStroke(session, [[10, 100], [60, 160]], { strokePen: { ...pen, color: '#11223380' } });
            drawStroke(session, [[10, 200], [60, 260]], { strokePen: { ...pen, color: '#445566FF' } });
            const paths = session.committedPaths();
            assert.deepEqual(paths.map(({ color }) => color), ['#ABCDEF', '#11223380', '#445566']);
            paths.forEach(({ path }, index) => assertPathData(path, `committedPaths[${index}]`));
        });
    });

    test('JSON round-trips through toJson and fromJson', () => {
        withSession(sketch.SketchSession.newEmpty(documentWidth, documentHeight), session => {
            drawStroke(session, Array.from({ length: 30 }, (_, index) => [300 + 120 * Math.cos(index / 4), 300 + 120 * Math.sin(index / 4)]));
            drawStroke(session, [[500, 100, 0.1], [520, 140, 0.9], [560, 150, 0.5]], { kind: 'stylus' });
            const json = session.toJson();
            const parsed = JSON.parse(json);
            assert.equal(parsed.version, 1);
            assert.equal(parsed.width, documentWidth);
            assert.equal(parsed.strokes.length, 2);
            withSession(sketch.SketchSession.fromJson(json), loaded => {
                assert.equal(loaded.toJson(), json, 'fromJson(toJson()) re-serialises differently');
                assert.deepEqual(loaded.state(), { strokeCount: 2, canUndo: false, canRedo: false, revision: 0 });
                assert.deepEqual(loaded.documentSize(), { width: documentWidth, height: documentHeight });
            });
            withSession(sketch.SketchSession.newEmpty(1, 1), replaced => {
                assert.deepEqual(replaced.loadJson(json), { strokeCount: 2, canUndo: false, canRedo: false, revision: 1 });
                assert.equal(replaced.toJson(), json, 'loadJson(toJson()) re-serialises differently');
            });
        });
    });

    test('invalid documents throw the sketch-core message and change nothing', () => {
        assertThrowsMessage(() => sketch.SketchSession.fromJson('{}'), 'malformed document: missing field `version`', 'fromJson({})');
        assertThrowsMessage(
            () => sketch.SketchSession.newEmpty(0, 10),
            'document width and height must be finite, positive and at most 1,000,000',
            'newEmpty(0, 10)',
        );
        withSession(sketch.SketchSession.fromJson(readFileSync(twoStrokesFixture, 'utf8')), session => {
            const before = session.toJson();
            const state = session.state();
            assertThrowsMessage(
                () => session.loadJson('{"version":1,"width":10,"height":10,"strokes":[{"color":"blue","points":[[1,1,1]]}]}'),
                'stroke 0 has an invalid color',
                'loadJson with an invalid stroke colour',
            );
            assert.equal(session.toJson(), before, 'document changed after a rejected loadJson');
            assert.deepEqual(session.state(), state, 'state changed after a rejected loadJson');
        });
    });

    test('invalid pen colours and pointer kinds throw instead of drawing', () => {
        withSession(sketch.SketchSession.newEmpty(documentWidth, documentHeight), session => {
            for (const color of ['red', '#12345', '#1234567', '#GG0000', '000000', '']) {
                assertThrowsMessage(
                    () => session.beginStroke({ ...pen, color }, viewToDoc(), sample(10, 10, 0)),
                    `invalid color ${JSON.stringify(color)}: expected #RRGGBB or #RRGGBBAA`,
                    `pen colour ${JSON.stringify(color)}`,
                );
                assert.equal(session.hasLiveStroke(), false, `live stroke after pen colour ${JSON.stringify(color)}`);
            }
            assertThrowsMessage(
                () => session.beginStroke(pen, viewToDoc(), sample(10, 10, 0, 'mouse')),
                'invalid pointer kind "mouse": expected "touch" or "stylus"',
                'pointer kind mouse',
            );
            session.beginStroke(pen, viewToDoc(), sample(10, 10, 0));
            assertThrowsMessage(
                () => session.extendStroke([sample(40, 10, 16)], [sample(50, 10, 24, 'pen')]),
                'invalid pointer kind "pen": expected "touch" or "stylus"',
                'predicted pointer kind pen',
            );
        });
    });

    test('exportSvg writes the strokes and rejects an invalid background', () => {
        withSession(sketch.SketchSession.fromJson(readFileSync(twoStrokesFixture, 'utf8')), session => {
            const svg = session.exportSvg();
            assert.match(svg, /^<svg[\s>]/, `exportSvg() starts ${JSON.stringify(svg.slice(0, 40))}`);
            assert.equal((svg.match(/<path\b/g) ?? []).length, session.state().strokeCount, `paths in ${svg}`);
            assert.throws(() => session.exportSvg('<svg'), error => error instanceof Error && error.message.startsWith('invalid background svg:'));
        });
    });

    test('every conformance input has one expected outline', () => {
        const outlines = readdirSync(conformanceDir).filter(file => file.endsWith(outlineSuffix)).map(file => file.slice(0, -outlineSuffix.length)).sort();
        assert.ok(conformanceNames.length > 0, `no conformance inputs in ${conformanceDir}`);
        assert.deepEqual(conformanceNames, outlines, `inputs and ${outlineSuffix} files in ${conformanceDir}`);
    });

    for (const name of conformanceNames) {
        test(`committed outlines match conformance/${name}`, () => {
            const input = JSON.parse(readFileSync(path.join(conformanceDir, `${name}.json`), 'utf8'));
            const expected = JSON.parse(readFileSync(path.join(conformanceDir, name + outlineSuffix), 'utf8')).strokes;
            withSession(sketch.SketchSession.fromJson(JSON.stringify(input)), session => {
                const actual = session.committedPaths();
                assert.equal(actual.length, expected.length, `${name}: stroke count`);
                actual.forEach(({ path, color }, index) => {
                    const label = `${name} stroke ${index}`;
                    assertPathData(path, label);
                    assert.equal(color, input.strokes[index].color.toUpperCase().replace(/^(#[0-9A-F]{6})FF$/, '$1'), `${label}: colour`);
                    const verbs = [...path.verbs];
                    const firstDifference = verbs.findIndex((verb, verbIndex) => verb !== expected[index].verbs[verbIndex]);
                    assert.deepEqual(
                        verbs,
                        expected[index].verbs,
                        `${label}: ${verbs.length} verbs, expected ${expected[index].verbs.length}; first difference at verb ${firstDifference}: `
                            + `actual [${verbs.slice(firstDifference, firstDifference + 8)}], expected [${expected[index].verbs.slice(firstDifference, firstDifference + 8)}]`,
                    );
                    assert.equal(path.coords.length, expected[index].coords.length, `${label}: coord count`);
                    path.coords.forEach((coord, coordIndex) => {
                        const want = expected[index].coords[coordIndex];
                        assert.ok(
                            Math.abs(coord - want) <= conformanceTolerance,
                            `${label} coord ${coordIndex}: actual ${coord}, expected ${want}`,
                        );
                    });
                });
            });
        });
    }
});

# @apollohg/react-native-sketch-canvas

A native drawing canvas for React Native, built on a Rust core shared by iOS and Android. Use it for signatures, annotations and sketches over an SVG background.

- Variable-width ink from finger speed or stylus pressure, drawn as one smooth filled outline per stroke.
- Any SVG as a background, flattened in Rust and drawn natively with Core Graphics and `android.graphics`.
- Undo, redo and clear, with clear undoable.
- A small, versioned JSON document format that any platform can read and render ([FORMAT.md](FORMAT.md)).
- PNG and SVG export.

## Requirements

- Expo SDK 58 or later, with React Native's New Architecture.
- iOS 16.4 or later; Android API 24 or later.
- No Rust toolchain in your app: the package ships a prebuilt `SketchCore.xcframework` and `.so` libraries for `arm64-v8a`, `armeabi-v7a`, `x86` and `x86_64`.

## Install

```sh
npx expo install @apollohg/react-native-sketch-canvas
npx expo prebuild
```

The component renders `null` on web.

## Usage

```tsx
import { SketchCanvas, type SketchCanvasRef } from '@apollohg/react-native-sketch-canvas';
import { useRef } from 'react';

export function SignaturePad() {
    const canvas = useRef<SketchCanvasRef>(null);

    return (
        <SketchCanvas
            ref={canvas}
            pen={{ color: '#000000', minWidth: 1, maxWidth: 3 }}
            backgroundColor={'#FFFFFF'}
            accessibilityLabel={'Signature'}
            onChange={({ strokeCount }) => console.log(strokeCount)}
            style={{ aspectRatio: 2 }}
        />
    );
}
```

Save with `await canvas.current.getDocument()` and restore with `initialDocument` or `loadDocument`.

## Props

| Prop | Type | Meaning |
|---|---|---|
| `pen` | `{ color, minWidth, maxWidth }` | Applies to the next stroke. Widths are in points. Equal widths give a fixed-width pen. |
| `background` | `string` | SVG markup drawn under the ink. Changing it re-parses the SVG and keeps the strokes. |
| `backgroundColor` | `string` | Fill behind everything. Transparent by default. |
| `initialDocument` | `string` | Document JSON, read once when the canvas first creates its document. |
| `editable` | `boolean` | `true` by default. When `false`, the canvas draws nothing new and touches pass through to its parent. |
| `accessibilityLabel` | `string` | The canvas is an image to assistive technology. |
| `style` | `ViewStyle` | Must give the view a non-zero size. |

| Event | Payload |
|---|---|
| `onChange` | `{ strokeCount, canUndo, canRedo, revision }` when the document is created or changes. `revision` is a decimal string. |
| `onStrokeStart`, `onStrokeEnd` | None. A stroke ends when it is committed; cancelled strokes do not end. |
| `onBackgroundLoad` | `{ warnings }`, listing the background features that were skipped. |
| `onError` | `{ code, message }`, with `code` one of `invalidDocument`, `invalidBackground`, `exportFailed`. |

## Ref methods

| Method | Result |
|---|---|
| `undo()`, `redo()`, `clear()` | `Promise<void>` |
| `getDocument()` | `Promise<string>`: the document JSON, without any stroke in progress. |
| `loadDocument(json)` | `Promise<void>`. Rejects with `invalidDocument` and leaves the canvas unchanged when the JSON is invalid. |
| `exportPng({ scale?, includeBackground? })` | `Promise<string>`: a `data:image/png;base64,…` URL. The image is the document at its on-screen size times `scale` (the screen's scale by default). |
| `exportSvg({ includeBackground? })` | `Promise<string>`: a standalone SVG. |

`includeBackground` defaults to `true`. Methods called before the canvas has a document reject with `exportFailed`, except `loadDocument`, which creates it.

## Sizing and coordinates

A document has its own coordinate space, fixed when it is created:

- from `initialDocument` or `loadDocument`, the document's own `width` and `height`;
- otherwise, with a `background`, the SVG's size; input is ignored until the SVG has parsed;
- otherwise, the view's size in points at first layout.

The document is drawn aspect-fit and centred in the view, and a background is aspect-fit and centred into the document. Resizing the view rescales the drawing without changing stored points. A resize or rotation during a stroke cancels that stroke.

## Input

- One pointer draws at a time. While a stylus (Apple Pencil, `TOOL_TYPE_STYLUS`) is down, finger touches are ignored, so a resting palm does not draw.
- A second finger during a finger stroke cancels the stroke.
- The canvas owns its touches, so vertical drags draw instead of scrolling an enclosing `ScrollView` or dragging a bottom sheet.

## Document format

```json
{ "version": 1, "width": 300, "height": 150, "strokes": [{ "color": "#FF0000", "points": [[10.5, 20, 2.75]] }] }
```

Points are `[x, y, width]` in document units. The way points become outlines is fixed for each format version and documented, with a conformance suite, in [FORMAT.md](FORMAT.md). A document never stores its background; record which background it was drawn on in your own data.

## SVG background support

Backgrounds are flattened by [usvg](https://github.com/linebender/resvg): groups, transforms, shapes, paths, `<use>`, clip paths, opacity, linear and radial gradients, raster images and nested SVG images from `data:` URLs.

These are skipped and reported in `onBackgroundLoad`'s warnings:

| Warning | Feature |
|---|---|
| `unsupportedText` | `<text>` |
| `unsupportedFilter` | Filters; the filtered content is drawn without the filter. |
| `unsupportedMask` | Masks; the masked content is drawn unmasked. |
| `unsupportedPattern` | Pattern fills and strokes. |
| `unsupportedExternalImage` | Images referenced by file path or URL. Only `data:` URLs are read; the canvas never touches the file system or network for a background. |

Other limits:

- On iOS, gradients with `spreadMethod="reflect"` or `"repeat"` draw as `pad`.
- On Android before API 29, radial gradients ignore their focal point.
- A background is rejected with `invalidBackground` if it flattens to more than 50,000 drawing operations, 4 million path coordinates, 32 MB of embedded images or 32 megapixels of decoded images.

## Release builds

The Android library ships consumer ProGuard rules that keep the UniFFI and JNA classes; no app configuration is needed.

## Using the core outside React Native

`rust/sketch-core` is an ordinary Rust crate with no mobile dependencies in its default features. It parses and validates documents, renders strokes to outlines, flattens SVG backgrounds and exports SVG, for example on a server that renders saved sketches into a PDF.

## Development

```sh
npm install
npm run build:rust     # bindings, iOS xcframework and Android libraries (needs rust/toolchain.sh's toolchain and cargo-ndk)
npm run test:rust
npm test
npm run test:ios       # after npm run prepare:example:native and npm run install:ios-test-pods
npm run test:android   # Robolectric; npm run test:android:device runs the golden tests on a device or emulator
```

The example app in `example/` has a signature screen and a sketch-over-SVG screen.

## License

Apache-2.0. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for bundled dependencies.

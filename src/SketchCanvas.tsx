import { requireNativeView } from 'expo';
import { forwardRef, type Ref, useImperativeHandle, useRef } from 'react';
import { type NativeSyntheticEvent, type StyleProp, type ViewStyle } from 'react-native';

import {
    type SketchBackgroundLoad,
    type SketchCanvasChange,
    type SketchCanvasError,
    type SketchCanvasProps,
    type SketchCanvasRef,
} from './types';

interface NativeSketchCanvasProps {
    ref?: Ref<SketchCanvasRef>;
    background?: string;
    canvasBackgroundColor?: string;
    penColor: string;
    penMinWidth: number;
    penMaxWidth: number;
    initialDocument?: string;
    editable: boolean;
    accessibilityLabel?: string;
    style?: StyleProp<ViewStyle>;
    onSketchChange?: (event: NativeSyntheticEvent<SketchCanvasChange>) => void;
    onStrokeStart?: () => void;
    onStrokeEnd?: () => void;
    onBackgroundLoad?: (event: NativeSyntheticEvent<SketchBackgroundLoad>) => void;
    onSketchError?: (event: NativeSyntheticEvent<SketchCanvasError>) => void;
}

const NativeSketchCanvas = requireNativeView<NativeSketchCanvasProps>('SketchCanvas');

const NOT_MOUNTED_MESSAGE = 'SketchCanvas is not mounted';

export const SketchCanvas = forwardRef<SketchCanvasRef, SketchCanvasProps>((
    {
        pen, backgroundColor, editable = true, onChange, onStrokeStart, onStrokeEnd, onBackgroundLoad, onError, ...props
    },
    ref,
) => {
    const nativeRef = useRef<SketchCanvasRef>(null);

    useImperativeHandle(ref, () => {
        const native = (): SketchCanvasRef => {
            if (!nativeRef.current) {
                throw new Error(NOT_MOUNTED_MESSAGE);
            }

            return nativeRef.current;
        };

        return {
            undo: async() => native().undo(),
            redo: async() => native().redo(),
            clear: async() => native().clear(),
            getDocument: async() => native().getDocument(),
            loadDocument: async json => native().loadDocument(json),
            exportPng: async(options = {}) => native().exportPng(options),
            exportSvg: async(options = {}) => native().exportSvg(options),
        };
    }, []);

    return (
        <NativeSketchCanvas
            {...props}
            ref={nativeRef}
            canvasBackgroundColor={backgroundColor}
            penColor={pen.color}
            penMinWidth={pen.minWidth}
            penMaxWidth={pen.maxWidth}
            editable={editable}
            onSketchChange={onChange && (event => onChange(event.nativeEvent))}
            onStrokeStart={onStrokeStart}
            onStrokeEnd={onStrokeEnd}
            onBackgroundLoad={onBackgroundLoad && (event => onBackgroundLoad(event.nativeEvent))}
            onSketchError={onError && (event => onError(event.nativeEvent))}
        />
    );
});

import { forwardRef, useImperativeHandle } from 'react';

import { type SketchCanvasProps, type SketchCanvasRef } from './types';

const UNSUPPORTED_MESSAGE = 'SketchCanvas is not supported on web';

const unsupported = (): Promise<never> => Promise.reject(new Error(UNSUPPORTED_MESSAGE));

export const SketchCanvas = forwardRef<SketchCanvasRef, SketchCanvasProps>((_props, ref) => {
    useImperativeHandle(ref, () => ({
        undo: unsupported,
        redo: unsupported,
        clear: unsupported,
        getDocument: unsupported,
        loadDocument: unsupported,
        exportPng: unsupported,
        exportSvg: unsupported,
    }), []);

    return null;
});

import { render } from '@testing-library/react-native';
import { createRef } from 'react';

import { SketchCanvas, type SketchCanvasRef } from '..';

const mockNativeMethods = {
    undo: jest.fn(() => Promise.resolve()),
    redo: jest.fn(() => Promise.resolve()),
    clear: jest.fn(() => Promise.resolve()),
    getDocument: jest.fn(() => Promise.resolve('{}')),
    loadDocument: jest.fn(() => Promise.resolve()),
    exportPng: jest.fn(() => Promise.resolve('data:image/png;base64,')),
    exportSvg: jest.fn(() => Promise.resolve('<svg/>')),
};

const mockNativeProps: { current: Record<string, unknown> } = { current: {} };

jest.mock('expo', () => {
    const { forwardRef, useImperativeHandle } = jest.requireActual<typeof import('react')>('react');

    return {
        requireNativeView: () => forwardRef<typeof mockNativeMethods, Record<string, unknown>>((props, ref) => {
            mockNativeProps.current = props;
            useImperativeHandle(ref, () => mockNativeMethods);

            return null;
        }),
    };
});

const PEN = { color: '#000000', minWidth: 1, maxWidth: 3 };

describe('SketchCanvas', () => {
    beforeEach(() => {
        jest.clearAllMocks();
    });

    it('flattens pen into native props', () => {
        render(<SketchCanvas pen={PEN} />);

        expect(mockNativeProps.current).toMatchObject({ penColor: '#000000', penMinWidth: 1, penMaxWidth: 3 });
    });

    it('defaults editable to true', () => {
        render(<SketchCanvas pen={PEN} />);

        expect(mockNativeProps.current.editable).toBe(true);
    });

    it('unwraps change events', () => {
        const onChange = jest.fn();
        render(<SketchCanvas pen={PEN} onChange={onChange} />);
        const change = { strokeCount: 2, canUndo: true, canRedo: false, revision: '7' };

        (mockNativeProps.current.onSketchChange as (event: { nativeEvent: unknown }) => void)({ nativeEvent: change });

        expect(onChange).toHaveBeenCalledWith(change);
    });

    it('forwards ref methods', async() => {
        const ref = createRef<SketchCanvasRef>();
        render(<SketchCanvas ref={ref} pen={PEN} />);

        await ref.current?.exportPng({ scale: 2 });

        expect(mockNativeMethods.exportPng).toHaveBeenCalledWith({ scale: 2 });
    });
});

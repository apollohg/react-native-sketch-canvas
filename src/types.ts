import { type StyleProp, type ViewStyle } from 'react-native';

export interface SketchPen {
    color: string;
    minWidth: number;
    maxWidth: number;
}

export interface SketchCanvasChange {
    strokeCount: number;
    canUndo: boolean;
    canRedo: boolean;
    revision: string;
}

export type SketchCanvasErrorCode = 'invalidDocument' | 'invalidBackground' | 'exportFailed';

export type SketchBackgroundWarning =
    | 'unsupportedText'
    | 'unsupportedFilter'
    | 'unsupportedMask'
    | 'unsupportedPattern'
    | 'unsupportedExternalImage';

export interface SketchCanvasError {
    code: SketchCanvasErrorCode;
    message: string;
}

export interface SketchBackgroundLoad {
    warnings: SketchBackgroundWarning[];
}

export interface SketchCanvasProps {
    background?: string;
    backgroundColor?: string;
    pen: SketchPen;
    initialDocument?: string;
    editable?: boolean;
    accessibilityLabel?: string;
    style?: StyleProp<ViewStyle>;
    onChange?: (change: SketchCanvasChange) => void;
    onStrokeStart?: () => void;
    onStrokeEnd?: () => void;
    onBackgroundLoad?: (result: SketchBackgroundLoad) => void;
    onError?: (error: SketchCanvasError) => void;
}

export interface SketchExportPngOptions {
    scale?: number;
    includeBackground?: boolean;
}

export interface SketchExportSvgOptions {
    includeBackground?: boolean;
}

export interface SketchCanvasRef {
    undo(): Promise<void>;
    redo(): Promise<void>;
    clear(): Promise<void>;
    getDocument(): Promise<string>;
    loadDocument(json: string): Promise<void>;
    exportPng(options?: SketchExportPngOptions): Promise<string>;
    exportSvg(options?: SketchExportSvgOptions): Promise<string>;
}

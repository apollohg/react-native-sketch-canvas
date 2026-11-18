import {
    SketchCanvas,
    type SketchCanvasChange,
    type SketchCanvasRef,
} from '@apollohg/react-native-sketch-canvas';
import { useRef, useState } from 'react';
import { Pressable, ScrollView, StyleSheet, Switch, Text, View } from 'react-native';

import { Button } from '../components/Button';
import { Filler } from '../components/Filler';
import { SAMPLE_BACKGROUND } from '../sampleBackground';
import { colors, PEN_COLORS } from '../theme';

const PEN_WIDTH = 2;

export function SketchScreen() {
    const canvas = useRef<SketchCanvasRef>(null);
    const [ penColor, setPenColor ] = useState<string>(PEN_COLORS[0]);
    const [ editable, setEditable ] = useState(true);
    const [ saved, setSaved ] = useState<string>();
    const [ status, setStatus ] = useState<SketchCanvasChange>();
    const [ svg, setSvg ] = useState<string>();
    const [ error, setError ] = useState<string>();

    return (
        <ScrollView contentContainerStyle={styles.content}>
            <Filler text={'Scroll down to the sketch. With editing off, drags over the canvas scroll the page.'} />
            <View style={styles.palette}>
                {PEN_COLORS.map(color => (
                    <Pressable
                        key={color}
                        accessibilityRole={'radio'}
                        accessibilityState={{ selected: penColor === color }}
                        accessibilityLabel={`Pen ${color}`}
                        onPress={() => setPenColor(color)}
                        style={[ styles.swatch, { backgroundColor: color }, penColor === color && styles.swatchSelected ]}
                    />
                ))}
            </View>
            <SketchCanvas
                ref={canvas}
                pen={{ color: penColor, minWidth: PEN_WIDTH, maxWidth: PEN_WIDTH }}
                background={SAMPLE_BACKGROUND}
                backgroundColor={colors.canvas}
                editable={editable}
                accessibilityLabel={'Sketch'}
                onChange={setStatus}
                onError={({ code, message }) => setError(`${code}: ${message}`)}
                style={styles.canvas}
            />
            {error ? <Text style={styles.status}>{error}</Text> : null}
            <Text style={styles.status}>
                {status ? `${status.strokeCount} strokes · revision ${status.revision}` : 'Waiting for the background'}
            </Text>
            <View style={styles.actions}>
                <Button label={'Undo'} onPress={() => void canvas.current?.undo()} />
                <Button label={'Redo'} onPress={() => void canvas.current?.redo()} />
                <Button label={'Clear'} onPress={() => void canvas.current?.clear()} />
                <Button label={'Save JSON'} onPress={() => void canvas.current?.getDocument().then(setSaved)} />
                <Button label={'Load JSON'} onPress={() => saved && void canvas.current?.loadDocument(saved)} />
                <Button label={'Export SVG'} onPress={() => void canvas.current?.exportSvg().then(setSvg)} />
            </View>
            <View style={styles.toggle}>
                <Text style={styles.status}>{'Editable'}</Text>
                <Switch value={editable} onValueChange={setEditable} />
            </View>
            {svg ? <Text selectable={true} style={styles.svg}>{svg}</Text> : null}
            <Filler text={'More content below the canvas.'} />
        </ScrollView>
    );
}

const styles = StyleSheet.create({
    content: {
        padding: 16,
    },
    palette: {
        flexDirection: 'row',
        gap: 8,
        marginBottom: 12,
    },
    swatch: {
        width: 28,
        height: 28,
        borderRadius: 14,
        borderWidth: 2,
        borderColor: 'transparent',
    },
    swatchSelected: {
        borderColor: colors.text,
        borderWidth: 3,
    },
    canvas: {
        aspectRatio: 1.2,
        borderWidth: StyleSheet.hairlineWidth,
        borderColor: colors.border,
    },
    status: {
        marginTop: 8,
        color: colors.muted,
    },
    actions: {
        flexDirection: 'row',
        flexWrap: 'wrap',
        gap: 8,
        marginTop: 12,
    },
    toggle: {
        flexDirection: 'row',
        alignItems: 'center',
        justifyContent: 'space-between',
        marginTop: 12,
    },
    svg: {
        marginTop: 12,
        fontFamily: 'Courier',
        fontSize: 10,
        color: colors.text,
    },
});

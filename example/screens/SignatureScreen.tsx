import { SketchCanvas, type SketchCanvasRef } from '@apollohg/react-native-sketch-canvas';
import { useRef, useState } from 'react';
import { Image, ScrollView, StyleSheet, View } from 'react-native';

import { Button } from '../components/Button';
import { Filler } from '../components/Filler';
import { colors } from '../theme';

const PEN = { color: '#000000', minWidth: 1, maxWidth: 3 };

export function SignatureScreen() {
    const canvas = useRef<SketchCanvasRef>(null);
    const [ preview, setPreview ] = useState<string>();

    return (
        <ScrollView contentContainerStyle={styles.content}>
            <Filler text={'Scroll past this text. Drag vertically on the pad below: it should draw, not scroll.'} />
            <SketchCanvas
                ref={canvas}
                pen={PEN}
                backgroundColor={colors.canvas}
                accessibilityLabel={'Signature pad'}
                style={styles.canvas}
            />
            <View style={styles.actions}>
                <Button label={'Clear'} onPress={() => void canvas.current?.clear()} />
                <Button label={'Undo'} onPress={() => void canvas.current?.undo()} />
                <Button label={'Redo'} onPress={() => void canvas.current?.redo()} />
                <Button label={'Export PNG'} onPress={() => void canvas.current?.exportPng().then(setPreview)} />
            </View>
            {preview ? <Image source={{ uri: preview }} resizeMode={'contain'} style={styles.preview} /> : null}
            <Filler text={'More content below the canvas.'} />
        </ScrollView>
    );
}

const styles = StyleSheet.create({
    content: {
        padding: 16,
    },
    canvas: {
        aspectRatio: 2,
        borderWidth: StyleSheet.hairlineWidth,
        borderColor: colors.border,
        borderRadius: 12,
        overflow: 'hidden',
    },
    actions: {
        flexDirection: 'row',
        flexWrap: 'wrap',
        gap: 8,
        marginTop: 12,
    },
    preview: {
        marginTop: 12,
        aspectRatio: 2,
        borderWidth: StyleSheet.hairlineWidth,
        borderColor: colors.border,
    },
});

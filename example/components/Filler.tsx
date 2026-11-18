import { StyleSheet, Text } from 'react-native';

import { colors } from '../theme';

/** Content above and below the canvas so the screen scrolls: vertical drags on the canvas must still draw. */
export function Filler({ text }: { text: string }) {
    return <Text style={styles.text}>{text}</Text>;
}

const styles = StyleSheet.create({
    text: {
        color: colors.muted,
        lineHeight: 22,
        paddingVertical: 160,
    },
});

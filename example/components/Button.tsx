import { Pressable, StyleSheet, Text } from 'react-native';

import { colors } from '../theme';

export function Button({ label, onPress }: { label: string; onPress: () => void }) {
    return (
        <Pressable accessibilityRole={'button'} onPress={onPress} style={styles.button}>
            <Text style={styles.label}>{label}</Text>
        </Pressable>
    );
}

const styles = StyleSheet.create({
    button: {
        paddingHorizontal: 14,
        paddingVertical: 8,
        borderRadius: 8,
        backgroundColor: colors.control,
    },
    label: {
        color: colors.text,
        fontWeight: '500',
    },
});

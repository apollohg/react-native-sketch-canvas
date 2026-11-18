import { StatusBar } from 'expo-status-bar';
import { useState } from 'react';
import { Pressable, StyleSheet, Text, View } from 'react-native';
import { SafeAreaProvider, SafeAreaView } from 'react-native-safe-area-context';

import { SignatureScreen } from './screens/SignatureScreen';
import { SketchScreen } from './screens/SketchScreen';
import { colors } from './theme';

const SCREENS = [
    { key: 'signature', title: 'Signature' },
    { key: 'sketch', title: 'Sketch over SVG' },
] as const;

type ScreenKey = typeof SCREENS[number]['key'];

export default function App() {
    const [ screen, setScreen ] = useState<ScreenKey>('signature');

    return (
        <SafeAreaProvider>
            <SafeAreaView style={styles.screen}>
                <View style={styles.tabs}>
                    {SCREENS.map(({ key, title }) => (
                        <Pressable
                            key={key}
                            accessibilityRole={'tab'}
                            accessibilityState={{ selected: screen === key }}
                            onPress={() => setScreen(key)}
                            style={[ styles.tab, screen === key && styles.tabSelected ]}
                        >
                            <Text style={styles.tabText}>{title}</Text>
                        </Pressable>
                    ))}
                </View>
                {screen === 'signature' ? <SignatureScreen /> : <SketchScreen />}
                <StatusBar style={'dark'} />
            </SafeAreaView>
        </SafeAreaProvider>
    );
}

const styles = StyleSheet.create({
    screen: {
        flex: 1,
        backgroundColor: colors.page,
    },
    tabs: {
        flexDirection: 'row',
        padding: 8,
        gap: 8,
    },
    tab: {
        flex: 1,
        paddingVertical: 10,
        borderRadius: 8,
        alignItems: 'center',
        backgroundColor: colors.control,
    },
    tabSelected: {
        backgroundColor: colors.accent,
    },
    tabText: {
        fontWeight: '600',
        color: colors.text,
    },
});

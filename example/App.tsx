import { SketchCanvas } from '@apollohg/react-native-sketch-canvas';
import { StatusBar } from 'expo-status-bar';
import { StyleSheet, View } from 'react-native';

const PEN = { color: '#000000', minWidth: 1, maxWidth: 3 };

export default function App() {
    return (
        <View style={styles.screen}>
            <SketchCanvas pen={PEN} backgroundColor={'#FFFFFF'} style={styles.canvas} />
            <StatusBar style={'dark'} />
        </View>
    );
}

const styles = StyleSheet.create({
    screen: {
        flex: 1,
        backgroundColor: '#FFFFFF',
    },
    canvas: {
        flex: 1,
    },
});

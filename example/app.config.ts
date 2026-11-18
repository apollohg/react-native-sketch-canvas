const rootPackage = require('../package.json');

export default {
    expo: {
        name: 'Sketch Canvas Example',
        slug: 'sketch-canvas-example',
        version: rootPackage.version,
        plugins: [ 'expo-status-bar' ],
        orientation: 'default',
        ios: {
            supportsTablet: true,
            bundleIdentifier: 'com.apollohg.sketchcanvasexample',
        },
        android: {
            package: 'com.apollohg.sketchcanvasexample',
        },
    },
};

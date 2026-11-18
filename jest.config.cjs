module.exports = {
    preset: 'jest-expo',
    testMatch: ['<rootDir>/src/__tests__/**/*.test.ts', '<rootDir>/src/__tests__/**/*.test.tsx'],
    modulePathIgnorePatterns: ['<rootDir>/android/build/', '<rootDir>/ios-tests/build/', '<rootDir>/example/'],
};

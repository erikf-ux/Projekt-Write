import js from '@eslint/js';
import reactHooks from 'eslint-plugin-react-hooks';
import globals from 'globals';
import tseslint from 'typescript-eslint';

export default tseslint.config(
  {
    ignores: [
      'dist/',
      'dist-isolation/',
      'src-tauri/',
      'tools/spell-compare/target/',
      'src/types/generated/',
      'node_modules/',
      'tests/e2e/.tmp/',
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.strictTypeChecked,
  {
    languageOptions: {
      parserOptions: {
        project: ['./tsconfig.json', './tsconfig.node.json'],
        tsconfigRootDir: import.meta.dirname,
      },
    },
  },
  {
    files: ['src/**/*.{ts,tsx}'],
    languageOptions: {
      globals: globals.browser,
    },
    plugins: {
      'react-hooks': reactHooks,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      // The frontend never talks to Tauri directly: all calls go through src/services.
      'no-restricted-imports': [
        'error',
        {
          paths: [
            {
              name: '@tauri-apps/api/core',
              message: 'Use the typed command wrappers in src/services instead.',
            },
          ],
          patterns: [
            {
              group: ['@tauri-apps/plugin-*'],
              message: 'Tauri plugins are not available to the frontend (see docs/architecture.md).',
            },
          ],
        },
      ],
    },
  },
  {
    files: ['src/services/ipc.ts', 'src/**/*.test.{ts,tsx}'],
    rules: {
      'no-restricted-imports': 'off',
    },
  },
  {
    // Tests simulate Tauri, which rejects with plain values instead of Error objects.
    files: ['src/**/*.test.{ts,tsx}'],
    rules: {
      '@typescript-eslint/only-throw-error': 'off',
    },
  },
  {
    files: ['**/*.js', '**/*.mjs'],
    ...tseslint.configs.disableTypeChecked,
  },
  {
    files: ['tests/e2e/**/*.ts', 'scripts/**/*.{ts,mjs}', '*.config.{ts,js}'],
    languageOptions: {
      globals: { ...globals.node, ...globals.mocha },
    },
  },
);

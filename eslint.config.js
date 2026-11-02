import js from '@eslint/js';
import reactHooks from 'eslint-plugin-react-hooks';
import globals from 'globals';
import tseslint from 'typescript-eslint';

export default tseslint.config(
  {
    ignores: ['dist/', 'dist-isolation/', 'src-tauri/', 'src/types/generated/', 'node_modules/', 'tests/e2e/.tmp/'],
  },
  js.configs.recommended,
  ...tseslint.configs.strictTypeChecked,
  {
    languageOptions: {
      parserOptions: {
        projectService: {
          allowDefaultProject: ['eslint.config.js'],
        },
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
    files: ['tests/e2e/**/*.ts', '*.config.{ts,js}'],
    languageOptions: {
      globals: { ...globals.node, ...globals.mocha },
    },
  },
);

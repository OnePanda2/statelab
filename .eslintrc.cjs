/**
 * ESLint config (§9). Enforces `strict` TS with no `any`, and the FROZEN
 * import-boundary rule: visualization files may not import from an
 * engine-computation path (the frontend mirror of the Rust `engine.rs` /
 * `systems/` isolation). See the `no-restricted-imports` rule below.
 */
module.exports = {
  root: true,
  env: { browser: true, es2022: true },
  extends: [
    'eslint:recommended',
    'plugin:@typescript-eslint/recommended',
    'plugin:@typescript-eslint/recommended-requiring-type-checking',
  ],
  parser: '@typescript-eslint/parser',
  parserOptions: { project: ['./tsconfig.json', './tsconfig.node.json'] },
  plugins: ['@typescript-eslint', 'react-hooks'],
  ignorePatterns: ['dist', 'node_modules', '.eslintrc.cjs', 'src-tauri'],
  rules: {
    '@typescript-eslint/no-explicit-any': 'error',
    'react-hooks/rules-of-hooks': 'error',
    'react-hooks/exhaustive-deps': 'warn',
  },
  overrides: [
    {
      // Visualizations are pure consumers of already-computed data (Principle #3).
      files: ['src/visualizations/**/*.{ts,tsx}'],
      rules: {
        'no-restricted-imports': [
          'error',
          {
            patterns: [
              {
                group: ['@/controllers/*', '**/engine/*', '**/*engine*'],
                message:
                  'Visualizations must not import engine-computation paths; render from the immutable Trajectory only (§5.2, Principle #3).',
              },
            ],
          },
        ],
      },
    },
  ],
};

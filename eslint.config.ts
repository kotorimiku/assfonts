// import vueI18n from '@intlify/eslint-plugin-vue-i18n';
// import type { TSESLint } from '@typescript-eslint/utils';
import {
  defineConfigWithVueTs,
  vueTsConfigs,
} from '@vue/eslint-config-typescript';
import { configureVueProject } from '@vue/eslint-config-typescript';
import skipFormatting from 'eslint-config-prettier/flat';
import pluginOxlint from 'eslint-plugin-oxlint';
import pluginVue from 'eslint-plugin-vue';
import { globalIgnores } from 'eslint/config';

import autoImportGlobals from './.eslintrc-auto-import.json' with { type: 'json' };

configureVueProject({ scriptLangs: ['ts', 'tsx'] });

// const vueI18nRecommended = vueI18n.configs
//   .recommended as TSESLint.FlatConfig.Config[];

export default defineConfigWithVueTs(
  {
    name: 'app/files-to-lint',
    files: ['**/*.{vue,ts,mts,tsx}'],
    languageOptions: {
      globals: { ...autoImportGlobals.globals },
    },
  },

  globalIgnores([
    '**/dist/**',
    '**/dist-ssr/**',
    '**/coverage/**',
    'src/bindings.ts',
    'src-tauri/',
    '.github/',
    'src/auto-imports.d.ts',
    'src/components.d.ts',
  ]),

  ...pluginVue.configs['flat/essential'],
  vueTsConfigs.recommended,

  ...pluginOxlint.buildFromOxlintConfigFile('.oxlintrc.json'),

  skipFormatting,

  {
    rules: {
      '@typescript-eslint/no-unused-vars': ["warn", { "varsIgnorePattern": "^_" }],
      'vue/multi-word-component-names': 'warn',
    },
  },

  // ...vueI18nRecommended,
  {
    rules: {
      // Optional.
      '@intlify/vue-i18n/no-dynamic-keys': 'off',
      '@intlify/vue-i18n/no-missing-keys': 'error',
      // '@intlify/vue-i18n/no-unused-keys': [
      //   'error',
      //   {
      //     extensions: ['.js', '.vue', '.ts'],
      //   },
      // ],
    },
    settings: {
      'vue-i18n': {
        localeDir: './src/locales/*.{json,json5,yaml,yml}', // extension is glob formatting!
        // or
        // localeDir: {
        //   pattern: './path/to/locales/*.{json,json5,yaml,yml}', // extension is glob formatting!
        //   localeKey: 'file' // or 'path' or 'key'
        // }
        // or
        // localeDir: [
        //   {
        //     // 'file' case
        //     pattern: './path/to/locales1/*.{json,json5,yaml,yml}',
        //     localeKey: 'file'
        //   },
        //   {
        //     // 'path' case
        //     pattern: './path/to/locales2/*.{json,json5,yaml,yml}',
        //     localePattern: /^.*\/(?<locale>[A-Za-z0-9-_]+)\/.*\.(json5?|ya?ml)$/,
        //     localeKey: 'path'
        //   },
        //   {
        //     // 'key' case
        //     pattern: './path/to/locales3/*.{json,json5,yaml,yml}',
        //     localeKey: 'key'
        //   },
        // ]

        // Specify the version of `vue-i18n` you are using.
        // If not specified, the message will be parsed twice.
        messageSyntaxVersion: '^11.0.0',
      },
    },
  }
);

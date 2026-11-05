import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

const frontendRoot = fileURLToPath(new URL('.', import.meta.url));
const projectRoot = resolve(frontendRoot, '..');

export default defineConfig(({ command }) => ({
  root: frontendRoot,
  plugins: [
    {
      name: 'openchat-development-favicon',
      apply: 'serve',
      transformIndexHtml(html) {
        return html.replace('href="/favicon.svg"', 'href="/favicon-dev.svg"');
      },
    },
    react(),
    tailwindcss(),
  ],
  resolve: {
    alias: {
      '@': resolve(frontendRoot, 'src'),
      '@assets': resolve(projectRoot, 'assets'),
      '@app': resolve(frontendRoot, 'src/app'),
      '@features': resolve(frontendRoot, 'src/features'),
      '@entities': resolve(frontendRoot, 'src/entities'),
      '@widgets': resolve(frontendRoot, 'src/widgets'),
      '@shared': resolve(frontendRoot, 'src/shared'),
    },
  },
  json: {
    // Named exports duplicate the locale dictionaries in Vite dev; source JSON imports use defaults.
    namedExports: command !== 'serve',
    // Keep large development locale modules as object literals instead of escaped JSON.parse strings.
    stringify: command === 'serve' ? false : 'auto',
  },
  clearScreen: false,
  server: {
    host: '127.0.0.1',
    port: 5173,
    strictPort: true,
    cors: {
      origin: 'http://127.0.0.1:5173',
    },
    hmr: {
      overlay: false,
    },
    fs: {
      allow: [frontendRoot, resolve(projectRoot, 'assets')],
    },
  },
  build: {
    target: 'es2022',
    emptyOutDir: true,
    // The syntax highlighter engine and language grammars are lazy-loaded; the largest measured chunk is 797 kB.
    chunkSizeWarningLimit: 1024,
    rollupOptions: {
      output: {
        manualChunks(id) {
          const moduleId = id.replaceAll('\\', '/');
          const shikiLanguageModule = moduleId.match(/\/node_modules\/@shikijs\/langs\/dist\/([^/]+)\.mjs$/u);
          if (shikiLanguageModule) {
            const languageName = shikiLanguageModule[1] === 'bash' ? 'shellscript' : shikiLanguageModule[1];
            return `shiki-language-${languageName}`;
          }
          if (moduleId.includes('node_modules/react-dom') || moduleId.includes('node_modules/react/')) {
            return 'react-vendor';
          }
          if (moduleId.includes('node_modules/lucide-react')) {
            return 'icons-vendor';
          }
          if (moduleId.includes('node_modules/@radix-ui')) {
            return 'radix-vendor';
          }
          return undefined;
        },
      },
    },
  },
}));

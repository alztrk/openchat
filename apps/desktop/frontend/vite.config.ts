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
    warmup: {
      // Prime the two dynamic imports that gate the first app shell commit.
      clientFiles: ['./src/bootstrap/App.tsx', './src/widgets/application-error/index.ts', './src/index.css'],
    },
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
    // Target chunk budget for lazy-loaded modules and vendor bundles.
    chunkSizeWarningLimit: 1024,
    rollupOptions: {
      output: {
        manualChunks(id) {
          const moduleId = id.replaceAll('\\', '/');
          if (moduleId.includes('node_modules/react-dom') || moduleId.includes('node_modules/react/')) {
            return 'react-vendor';
          }
          if (moduleId.includes('node_modules/@radix-ui') || moduleId.includes('node_modules/@floating-ui')) {
            return 'ui-vendor';
          }
          if (moduleId.includes('node_modules/lucide-react') || moduleId.includes('node_modules/@icons-pack')) {
            return 'icons-vendor';
          }
          if (moduleId.includes('node_modules/i18next') || moduleId.includes('node_modules/react-i18next')) {
            return 'i18n-vendor';
          }
          if (moduleId.includes('node_modules/@tanstack')) {
            return 'tanstack-vendor';
          }
          return undefined;
        },
      },
    },
  },
}));

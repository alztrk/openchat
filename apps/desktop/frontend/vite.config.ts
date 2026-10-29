import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

const frontendRoot = fileURLToPath(new URL('.', import.meta.url));
const projectRoot = resolve(frontendRoot, '..');

export default defineConfig({
  root: frontendRoot,
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': resolve(frontendRoot, 'src'),
      '@assets': resolve(projectRoot, 'assets'),
    },
  },
  clearScreen: false,
  server: {
    host: '127.0.0.1',
    port: 5173,
    strictPort: true,
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
    // Shiki's single C++ grammar is 501 kB minified and cannot be split further by Rollup.
    chunkSizeWarningLimit: 512,
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
          if (moduleId.includes('node_modules/shiki') || moduleId.includes('node_modules/@shikijs')) {
            return 'shiki-vendor';
          }
          return undefined;
        },
      },
    },
  },
});

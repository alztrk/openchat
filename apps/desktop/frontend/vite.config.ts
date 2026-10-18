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
    fs: {
      allow: [frontendRoot, resolve(projectRoot, 'assets')],
    },
  },
  build: {
    target: 'es2022',
    emptyOutDir: true,
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes('node_modules/lucide-react')) return 'lucide';
          if (id.includes('node_modules/react-dom') || id.includes('node_modules/react/')) {
            return 'react-vendor';
          }
          return undefined;
        },
      },
    },
  },
});

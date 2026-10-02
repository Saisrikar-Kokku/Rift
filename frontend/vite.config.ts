import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'path';
import fs from 'fs';

// Custom plugin to ensure widget.html, screens, shared, and assets are copied into dist/
function copyLegacySupportPlugin() {
  function copyDirRecursive(src: string, dest: string) {
    if (!fs.existsSync(src)) return;
    if (!fs.existsSync(dest)) fs.mkdirSync(dest, { recursive: true });
    const entries = fs.readdirSync(src, { withFileTypes: true });
    for (const entry of entries) {
      const srcPath = path.join(src, entry.name);
      const destPath = path.join(dest, entry.name);
      if (entry.isDirectory()) {
        copyDirRecursive(srcPath, destPath);
      } else {
        fs.copyFileSync(srcPath, destPath);
      }
    }
  }

  return {
    name: 'copy-legacy-support',
    closeBundle() {
      const widgetSrc = path.resolve(__dirname, '../ui/widget/widget.html');
      const distWidgetDir = path.resolve(__dirname, '../ui/dist/widget');
      const distWidgetFile = path.resolve(distWidgetDir, 'widget.html');

      if (fs.existsSync(widgetSrc)) {
        if (!fs.existsSync(distWidgetDir)) {
          fs.mkdirSync(distWidgetDir, { recursive: true });
        }
        fs.copyFileSync(widgetSrc, distWidgetFile);
        console.log('[Vite] Synced standalone widget.html -> dist/widget/widget.html');
      }

      copyDirRecursive(path.resolve(__dirname, '../ui/screens'), path.resolve(__dirname, '../ui/dist/screens'));
      copyDirRecursive(path.resolve(__dirname, '../ui/shared'), path.resolve(__dirname, '../ui/dist/shared'));
      copyDirRecursive(path.resolve(__dirname, '../ui/assets'), path.resolve(__dirname, '../ui/dist/assets'));
      console.log('[Vite] Synced screens, shared, and assets -> dist/');
    }
  };
}

export default defineConfig({
  plugins: [react(), copyLegacySupportPlugin()],
  base: './', // Essential for desktop Tauri webview file resolution
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  build: {
    outDir: '../ui/dist',
    emptyOutDir: true,
    target: 'es2020',
    chunkSizeWarningLimit: 1200,
    rollupOptions: {
      output: {
        manualChunks: {
          vendor_three: ['three'],
          vendor_radix: ['@radix-ui/react-tooltip', '@radix-ui/react-dialog', '@radix-ui/react-switch', '@radix-ui/react-tabs'],
        },
      },
    },
  },
  server: {
    port: 1420,
    strictPort: true,
  }
});

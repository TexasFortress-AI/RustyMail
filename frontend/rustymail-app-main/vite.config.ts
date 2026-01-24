// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

import { defineConfig } from "vite";
import react from "@vitejs/plugin-react-swc";
import path from "path";
import { componentTagger } from "lovable-tagger";

// https://vitejs.dev/config/
export default defineConfig(({ mode }) => {
  const dashboardPort = process.env.DASHBOARD_PORT;
  const restPort = process.env.REST_PORT;

  if (!dashboardPort) {
    throw new Error('DASHBOARD_PORT environment variable is required');
  }
  if (!restPort) {
    throw new Error('REST_PORT environment variable is required');
  }

  return {
    envDir: path.resolve(__dirname, '../../'),
    server: {
      host: process.env.DASHBOARD_HOST || "0.0.0.0",
      port: parseInt(dashboardPort),
      strictPort: true,
      proxy: {
        '/api': {
          target: `http://localhost:${restPort}`,
          changeOrigin: true,
          secure: false,
        }
      }
    },
    plugins: [
      react(),
      mode === 'development' &&
      componentTagger(),
    ].filter(Boolean),
    resolve: {
      alias: {
        "@": path.resolve(__dirname, "./src"),
      },
    },
  };
});

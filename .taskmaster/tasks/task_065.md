# Task ID: 65

**Title:** Upgrade Vite from 6.x to 7.x in frontend

**Status:** deferred

**Dependencies:** 50 ✓

**Priority:** medium

**Description:** Upgrade Vite dependency from version 6.x to 7.x in the frontend to address GHSA-67mh-4wv8-2f99 (esbuild dev server request vulnerability, moderate severity), ensuring vite.config.ts compatibility with any breaking changes.

**Details:**

1. **Update package.json dependencies**:
```json
{
  "devDependencies": {
    "vite": "^7.0.0",
    "@vitejs/plugin-react": "^5.0.0"  // or @sveltejs/vite-plugin-svelte if using Svelte
  }
}
```

2. **Review Vite 7.x breaking changes**:
   - Check for deprecated config options in vite.config.ts
   - Update any plugin configurations that may have changed APIs
   - Review build.rollupOptions if custom Rollup config is used
   - Verify server.proxy configurations still work as expected
   - Check for any changes to CSS handling or preprocessor options

3. **Common migration updates needed**:
   ```typescript
   // vite.config.ts
   import { defineConfig } from 'vite'
   
   export default defineConfig({
     // If using legacy options, update them:
     // Old: build.polyfillDynamicImport (removed in v7)
     // New: Use @vitejs/plugin-legacy if needed
     
     // Check server configuration
     server: {
       port: 3000,
       // Verify proxy still works
       proxy: {
         '/api': 'http://localhost:8080'
       }
     },
     
     // Update any deprecated build options
     build: {
       // target: 'esnext' is now the default
       // cssCodeSplit: true is now the default
     }
   })
   ```

4. **Update npm scripts if needed**:
   - Vite 7 may have new CLI options or changed defaults
   - Review package.json scripts for any deprecated flags

5. **Clear caches and reinstall**:
   ```bash
   rm -rf node_modules package-lock.json
   npm install
   ```

6. **Note**: This is a dev-only tooling upgrade that does not affect the shipped production code. The vulnerability GHSA-67mh-4wv8-2f99 is in the esbuild dev server and only affects development environments.

**Test Strategy:**

1. **Verify clean installation**:
   ```bash
   cd webui
   rm -rf node_modules package-lock.json
   npm install
   npm list vite  # Confirm version 7.x is installed
   ```

2. **Test development server**:
   ```bash
   npm run dev
   ```
   - Verify the dev server starts without errors
   - Check that HMR (Hot Module Replacement) works correctly
   - Test proxy configurations to backend API endpoints
   - Ensure no console warnings about deprecated options

3. **Test production build**:
   ```bash
   npm run build
   npm run preview
   ```
   - Verify build completes without errors
   - Check build output size hasn't significantly changed
   - Test the preview server to ensure built app works correctly

4. **Regression testing**:
   - Navigate through all major UI routes
   - Test OAuth flow (Task 50 functionality)
   - Verify all API calls work through the proxy
   - Check that CSS and assets load correctly
   - Test any dynamic imports or code splitting

5. **Security verification**:
   - Run `npm audit` to confirm GHSA-67mh-4wv8-2f99 is resolved
   - Verify no new vulnerabilities were introduced

6. **Plugin compatibility**:
   - If using Svelte, verify @sveltejs/vite-plugin-svelte works with Vite 7
   - Test any other Vite plugins for compatibility issues

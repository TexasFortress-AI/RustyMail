# Task ID: 35

**Title:** Build WebUI sampler configuration panel

**Status:** done

**Dependencies:** 33 ✓, 34 ✓

**Priority:** high

**Description:** Create a React component in the dashboard settings page that allows users to configure sampler settings for each AI provider/model with provider-specific field visibility and test functionality.

**Details:**

Create src/dashboard/components/settings/SamplerConfigPanel.tsx with the following implementation:

1. **Component Structure**:
   ```tsx
   interface SamplerConfig {
     provider: string;
     modelName: string;
     temperature: number;
     topP: number;
     topK?: number;
     minP?: number;
     repeatPenalty: number;
     numCtx: number;
     thinkMode: boolean;
     stopSequences: string[];
   }
   ```

2. **Main Component Features**:
   - Provider/Model selector dropdown that loads available models from the backend
   - Temperature slider (0-2, step 0.1) with numeric display
   - Top_p slider (0-1, step 0.01) with numeric display
   - Top_k number input (optional, min 1)
   - Min_p slider (0-1, step 0.001) - only shown for llama.cpp provider
   - Repeat_penalty slider (0-2, step 0.1)
   - Context window size (num_ctx) number input with provider-specific limits
   - Think mode toggle switch
   - Stop sequences text area with ability to add/remove sequences (one per line)

3. **Provider-Specific Field Visibility**:
   ```tsx
   const providerFields = {
     'ollama': ['temperature', 'topP', 'topK', 'repeatPenalty', 'numCtx', 'stopSequences'],
     'llamacpp': ['temperature', 'topP', 'topK', 'minP', 'repeatPenalty', 'numCtx', 'stopSequences'],
     'openai': ['temperature', 'topP', 'stopSequences', 'thinkMode']
   };
   ```

4. **API Integration**:
   - GET /api/sampler-configs/:provider/:model to load existing config
   - POST /api/sampler-configs to save configuration
   - GET /api/models to load available models per provider
   - POST /api/sampler-configs/test to test configuration

5. **Reset to Defaults Button**:
   ```tsx
   const defaultConfigs = {
     'ollama': { temperature: 0.7, topP: 1.0, repeatPenalty: 1.0, numCtx: 2048 },
     'llamacpp': { temperature: 0.7, topP: 1.0, minP: 0.01, repeatPenalty: 1.0, numCtx: 2048 },
     'openai': { temperature: 0.7, topP: 1.0, thinkMode: false }
   };
   ```

6. **Test Configuration Feature**:
   - Modal dialog with test prompt input
   - Sends request to backend with current config + test prompt
   - Displays response and timing information
   - Shows any errors or warnings

7. **UI Components**:
   - Use Material-UI or existing design system components
   - Tooltips for each setting explaining its purpose
   - Visual feedback for saving (loading spinner, success toast)
   - Validation feedback (red borders for invalid values)

8. **State Management**:
   - Use React hooks (useState, useEffect) for local state
   - Debounce slider changes to avoid excessive API calls
   - Show unsaved changes indicator
   - Confirm navigation away with unsaved changes

**Test Strategy:**

1. **Component Rendering Tests**:
   - Verify component renders without errors
   - Check all form fields are present for default provider
   - Verify provider-specific fields show/hide correctly when switching providers
   - Test that switching models loads the correct configuration

2. **Field Interaction Tests**:
   - Test temperature slider updates value correctly (0-2 range)
   - Test top_p slider updates value correctly (0-1 range)
   - Verify min_p field only appears for llama.cpp provider
   - Test stop sequences can be added/removed
   - Verify number inputs reject invalid values

3. **API Integration Tests**:
   - Mock GET /api/sampler-configs/:provider/:model and verify config loads
   - Mock POST /api/sampler-configs and verify save functionality
   - Test error handling for failed API calls
   - Verify loading states display correctly

4. **Reset Functionality Test**:
   - Click "Reset to Defaults" and verify all fields update
   - Ensure provider-specific defaults are applied correctly
   - Verify unsaved changes indicator appears after reset

5. **Test Configuration Feature**:
   - Click "Test Configuration" and verify modal opens
   - Enter test prompt and submit
   - Mock POST /api/sampler-configs/test endpoint
   - Verify response displays in modal
   - Test error handling for failed test requests

6. **Validation Tests**:
   - Enter invalid values (negative numbers, out of range)
   - Verify validation messages appear
   - Ensure save is disabled with invalid values
   - Test required field validation

7. **Integration Test**:
   - Load existing configuration
   - Modify multiple fields
   - Save configuration
   - Reload page and verify changes persisted
   - Test configuration with actual prompt

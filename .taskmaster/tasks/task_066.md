# Task ID: 66

**Title:** Add IMAP connection status indicator to Email tab

**Status:** done

**Dependencies:** 49 ✓

**Priority:** medium

**Description:** Implement a compact connection status badge on the Email tab that displays IMAP connection health, particularly for OAuth accounts, with a warning icon and clickable link for failed connections that navigates to re-authorization.

**Details:**

Integrate the existing ConnectionStatusIndicator component into the EmailList view to provide real-time connection status visibility:

1. **Import and integrate ConnectionStatusIndicator**:
   ```tsx
   // In EmailList.tsx
   import { ConnectionStatusIndicator } from './ConnectionStatusIndicator';
   ```

2. **Add connection status next to account selector**:
   ```tsx
   // Inside EmailList component, near the account selector/folder dropdown
   <div className="flex items-center gap-2">
     <AccountSelector />
     {currentAccount && (
       <ConnectionStatusIndicator
         accountId={currentAccount.id}
         compact={true}
         onReauthorize={() => {
           // Navigate to Accounts tab or trigger re-auth
           navigate('/dashboard/accounts');
           // Or directly open edit dialog with re-auth
         }}
       />
     )}
   </div>
   ```

3. **Utilize existing backend endpoint**:
   - The component should use `GET /api/dashboard/accounts/{id}/connection-status`
   - Poll this endpoint periodically (e.g., every 30 seconds) when the Email tab is active
   - Stop polling when tab is inactive to save resources

4. **Handle OAuth-specific failures**:
   ```tsx
   // In ConnectionStatusIndicator, enhance to show OAuth-specific messages
   if (connectionStatus.error?.includes('OAuth') || connectionStatus.error?.includes('401')) {
     return (
       <Tooltip content="OAuth token expired. Click to re-authorize.">
         <button
           onClick={onReauthorize}
           className="flex items-center gap-1 text-amber-600 hover:text-amber-700"
         >
           <ExclamationTriangleIcon className="h-4 w-4" />
           <span className="text-xs">Reconnect</span>
         </button>
       </Tooltip>
     );
   }
   ```

5. **Leverage currentAccount context**:
   - Use `currentAccount.connection_status` field for initial state
   - Update this field when polling returns new status
   - Ensure status updates trigger re-renders appropriately

6. **Navigation and re-authorization flow**:
   - On click, either navigate to `/dashboard/accounts` tab
   - Or better: directly open the account edit dialog with focus on re-authorization
   - Pass account ID in navigation state for direct dialog opening

7. **Visual design considerations**:
   - Keep indicator compact to not clutter the UI
   - Use color coding: green (connected), amber (warning/expired), red (error)
   - Include subtle animation for "checking" state
   - Ensure accessibility with proper ARIA labels

**Test Strategy:**

1. **Visual verification**:
   - Verify indicator appears next to account selector on Email tab
   - Check that it displays correctly in compact mode
   - Ensure proper spacing and alignment with existing UI elements

2. **Connection status testing**:
   - Test with a working IMAP connection - should show green/connected state
   - Manually expire OAuth token in database and verify amber warning appears
   - Disconnect network and verify red error state displays
   - Test that status updates within 30 seconds of connection change

3. **OAuth-specific scenarios**:
   - Create test account with expired OAuth token
   - Verify warning icon appears with appropriate tooltip message
   - Click indicator and confirm navigation to Accounts tab
   - Test that re-authorization flow can be triggered from the indicator

4. **Performance testing**:
   - Verify polling stops when navigating away from Email tab
   - Check that polling resumes when returning to Email tab
   - Monitor network requests to ensure no excessive API calls
   - Test with multiple accounts to ensure correct status per account

5. **Edge cases**:
   - Test with no current account selected
   - Verify graceful handling of API endpoint failures
   - Test rapid account switching and status updates
   - Verify component cleanup on unmount

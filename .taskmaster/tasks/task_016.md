# Task ID: 16

**Title:** Fix compose dialog appearing on hard refresh

**Status:** done

**Dependencies:** 4 ✓

**Priority:** medium

**Description:** Prevent the SendMailDialog from automatically opening when the web UI is hard-refreshed (F5 or Ctrl+F5)

**Details:**

SPECIFIC BUG IDENTIFIED: The dialog is appearing on page load with `data-state="open"` in the DOM. The `composeDialogOpen` state is correctly initialized as `false` in EmailList.tsx:71, but something is triggering it to become `true` during component mount. Debug logging has been added at EmailList.tsx:74-76 to track state changes. The visual confusion is compounded by placeholder text in input fields (recipient@example.com, cc@example.com, etc.) that appears gray but looks like actual values. ROOT CAUSE INVESTIGATION NEEDED: 1) Trace what's calling setComposeDialogOpen(true) during initialization - check browser dev tools console for debug logs, 2) Verify if any useEffect hooks or props are triggering dialog open on mount, 3) Check if Radix UI Dialog component has any default behavior causing auto-open, 4) Investigate if URL parameters, localStorage, or sessionStorage are influencing initial state, 5) Ensure the Dialog component's `open` prop is properly controlled by the `composeDialogOpen` state variable

**Test Strategy:**

Test by: 1) Adding more granular debug logging to track exactly when and why setComposeDialogOpen(true) is called, 2) Performing a hard refresh (F5 or Ctrl+F5) and checking browser console for debug output, 3) Verifying dialog does not appear on hard refresh after fix, 4) Testing soft refresh and normal navigation to ensure functionality still works, 5) Testing actual compose dialog triggers (Compose button, Reply, Forward) to ensure they still work correctly, 6) Cross-browser testing in Chrome, Firefox, Safari to ensure consistent behavior, 7) Verify placeholder text styling doesn't create visual confusion about empty vs filled fields

## Subtasks

### 16.1. Add comprehensive debug logging to track dialog state changes

**Status:** done  
**Dependencies:** None  

Implement detailed logging to identify what triggers setComposeDialogOpen(true) during component initialization

**Details:**

Add console.log statements at all locations where setComposeDialogOpen is called, including stack traces. Log component mount/unmount cycles and prop changes. Add logging in useEffect hooks that might influence dialog state. Check EmailList.tsx:162 (handleComposeRequest) and EmailList.tsx:590 (Compose button click) for unexpected calls.

### 16.2. Investigate Radix UI Dialog component behavior on mount

**Status:** done  
**Dependencies:** 16.1  

Check if Radix UI Dialog has any default open behavior or hydration issues

**Details:**

Examine the Dialog component in components/ui/dialog.tsx and its usage in SendMailDialog.tsx:211. Verify the `open` prop is properly bound to composeDialogOpen state. Check if DialogPrimitive.Root has any default state that could cause auto-opening. Review Radix UI documentation for known hydration or SSR issues that might cause initial open state.

### 16.3. Add hard refresh detection to prevent unwanted dialog opening

**Status:** done  
**Dependencies:** 16.1, 16.2  

Implement logic to detect hard refresh and ensure dialog remains closed

**Details:**

Add a useEffect hook in EmailList component that detects if the page was loaded fresh (hard refresh) vs navigated to. Use performance.navigation.type or window.performance.getEntriesByType('navigation') to detect refresh. Set a flag to prevent dialog from opening on fresh page loads. Ensure this doesn't interfere with legitimate compose dialog triggers.

### 16.4. Fix placeholder text styling to reduce visual confusion

**Status:** done  
**Dependencies:** None  

Update input placeholder styling to be more clearly distinguishable from actual values

**Details:**

Modify placeholder text in SendMailDialog.tsx:235, 250, 261 to be more obviously placeholders. Consider using lighter gray color, italic styling, or different placeholder text that's clearly not a real email address. Update CSS classes if needed to make placeholders more visually distinct from user input.

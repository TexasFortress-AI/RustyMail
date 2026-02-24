# Task ID: 18

**Title:** Add 'Show Images' button to email viewer for privacy protection

**Status:** done

**Dependencies:** 17 ✓

**Priority:** medium

**Description:** Implement a privacy-focused image loading control in the email viewer with an optional button similar to Thunderbird, where images are blocked by default to prevent tracking.

**Details:**

Based on the current EmailBody.tsx implementation that only displays plain text (line 303 uses whitespace-pre-wrap on email.body_text), add image privacy controls when displaying HTML emails: 1) Add a state variable `showImages` (default false) to control image display, 2) When rendering HTML content using email.html_body (which is already available from the backend as seen in cache.rs), implement a two-stage rendering approach: first render HTML with all img src attributes stripped/blocked, 3) Add a 'Show Images' button (using existing Button component and Eye/EyeOff icons from lucide-react) that appears when HTML content contains images, 4) When clicked, re-render the HTML with images enabled, 5) Use DOMParser to safely detect and modify img tags before dangerouslySetInnerHTML rendering, 6) Add user preference persistence via localStorage to remember the choice per sender/domain, 7) Style the button consistently with existing Reply/Forward buttons in the header area (lines 262-284), 8) Ensure the feature works with the existing HTML/text toggle functionality mentioned in Task 17

**Test Strategy:**

Test by: 1) Sending HTML emails with embedded images and tracking pixels to test accounts, 2) Verify images are blocked by default and 'Show Images' button appears, 3) Test button functionality enables images properly, 4) Test localStorage persistence remembers preference, 5) Verify no external requests are made when images are blocked (check network tab), 6) Test with various email clients (Gmail, Outlook, etc.) to ensure compatibility, 7) Test the feature works alongside Task 17's HTML rendering improvements, 8) Verify button styling matches existing UI components

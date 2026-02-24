# Task ID: 50

**Title:** Implement Frontend Microsoft OAuth UI

**Status:** done

**Dependencies:** 45 ✓

**Priority:** medium

**Description:** Implement R7: Add OAuth button and UI updates to Svelte WebUI

**Details:**

In `webui/src/routes/accounts/+page.svelte`:

```svelte
<script>
    let authUrl = '';
    async function getMicrosoftAuthUrl() {
        const response = await fetch('/api/oauth/microsoft/authorize?state=' + crypto.randomUUID());
        const data = await response.json();
        authUrl = data.url;
        window.location.href = authUrl;
    }
</script>

<button class="microsoft-oauth-btn" on:click={getMicrosoftAuthUrl}>
    🔐 Sign in with Microsoft
</button>

{#if account.oauth_provider === 'microsoft'}
    <div class="oauth-badge">Microsoft OAuth ✓</div>
    <button class="re-auth-btn">Re-authorize (tokens expired)</button>
{/if}

<style>
    .microsoft-oauth-btn { background: #0078D4; color: white; }
</style>
```

**Test Strategy:**

Cypress/Playwright E2E tests: click OAuth button redirects to Microsoft, callback returns to accounts page with new account, verify badge display

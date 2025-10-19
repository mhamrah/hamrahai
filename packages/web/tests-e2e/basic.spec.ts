import { test, expect } from '@playwright/test';

test.describe('Basic Navigation', () => {
  test('should redirect to login page when accessing home page unauthenticated', async ({ page }) => {
    // Go to home page
    await page.goto('/');
    
    // Should be redirected to login page
    await expect(page).toHaveURL(/\/auth\/login\/?/);
    
    // Should see login button indicating unauthenticated state
    await expect(page.locator('[data-testid="login-button"]')).toBeVisible();
    
    // Should see Google sign-in button
    await expect(page.locator('[data-testid="google-signin-button"]')).toBeVisible();
  });
});
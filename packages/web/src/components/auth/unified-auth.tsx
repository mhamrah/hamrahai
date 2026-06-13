import { component$, useSignal, $, type QRL } from "@builder.io/qwik";
import { authenticateWithDiscoverablePasskey } from "~/lib/auth/webauthn";

interface UnifiedAuthProps {
  onSuccess?: QRL<(user: any) => void>;
  onError?: QRL<(error: string) => void>;
  redirectUrl?: string;
  initialError?: string;
}

/**
 * UnifiedAuth
 *
 * Simplified authentication component:
 * - Single explicit passkey sign-in button (discoverable credentials; forces platform prompt)
 * - Google & Apple OAuth buttons
 *
 * All email-driven / conditional UI / signup subflows have been removed.
 */
export const UnifiedAuth = component$<UnifiedAuthProps>((props) => {
  const isLoading = useSignal(false);
  const error = useSignal<string>(props.initialError || "");
  const success = useSignal<string>("");

  // Telemetry signals (basic in-memory counters; replace with real analytics pipeline later)
  const passkeyAttemptCount = useSignal(0);
  const passkeySuccessCount = useSignal(0);
  const passkeyFailureCount = useSignal(0);

  const redirectUrl = props.redirectUrl;

  const handleExplicitPasskeyAuth = $(async () => {
    if (isLoading.value) return;
    isLoading.value = true;
    error.value = "";

    // Telemetry: record attempt
    passkeyAttemptCount.value++;
    try {
      const start = performance.now();
      const result = await authenticateWithDiscoverablePasskey();
      const durationMs = Math.round(performance.now() - start);

      if (result.success && result.user) {
        passkeySuccessCount.value++;

        // Fire a lightweight custom event for any global listener (analytics hook)
        globalThis.dispatchEvent(
          new CustomEvent("telemetry:passkey-auth", {
            detail: {
              outcome: "success",
              attempts: passkeyAttemptCount.value,
              successes: passkeySuccessCount.value,
              failures: passkeyFailureCount.value,
              durationMs,
            },
          }),
        );

        success.value = "Signed in with passkey!";
        await props.onSuccess?.(result.user);
        const target = redirectUrl || "/";
        // Redirect shortly to allow state updates to flush
        setTimeout(() => {
          window.location.href = target;
        }, 10);
      } else {
        passkeyFailureCount.value++;
        const msg = result.error || "Authentication failed";

        globalThis.dispatchEvent(
          new CustomEvent("telemetry:passkey-auth", {
            detail: {
              outcome: "failure",
              reason: result.error || "unknown",
              attempts: passkeyAttemptCount.value,
              successes: passkeySuccessCount.value,
              failures: passkeyFailureCount.value,
              durationMs,
            },
          }),
        );

        error.value = msg;
        await props.onError?.(msg);
      }
    } catch (e) {
      passkeyFailureCount.value++;
      const msg = e instanceof Error ? e.message : "Authentication failed";

      globalThis.dispatchEvent(
        new CustomEvent("telemetry:passkey-auth", {
          detail: {
            outcome: "error",
            reason: msg,
            attempts: passkeyAttemptCount.value,
            successes: passkeySuccessCount.value,
            failures: passkeyFailureCount.value,
          },
        }),
      );

      error.value = msg;
      await props.onError?.(msg);
    } finally {
      isLoading.value = false;
    }
  });

  const handleGoogleAuth = $(() => {
    if (isLoading.value) return;
    isLoading.value = true;
    error.value = "";
    window.location.href = redirectUrl
      ? `/auth/google?redirect=${encodeURIComponent(redirectUrl)}`
      : "/auth/google";
  });

  const handleAppleAuth = $(() => {
    if (isLoading.value) return;
    isLoading.value = true;
    error.value = "";
    window.location.href = redirectUrl
      ? `/auth/apple?redirect=${encodeURIComponent(redirectUrl)}`
      : "/auth/apple";
  });

  return (
    <div class="space-y-6">
      <div>
        <p class="text-sm font-medium text-cambridge-blue-700">Secure sign in</p>
        <h2 class="mt-2 text-3xl font-semibold tracking-tight text-gray-950">
          Welcome to Hamrah
        </h2>
        <p class="mt-3 text-sm leading-6 text-gray-600">
          Use a passkey for the fastest, safest sign-in. Google and Apple are
          available if this device does not have a passkey yet.
        </p>
      </div>

      {success.value && (
        <div
          data-testid="success-message"
          class="rounded-lg border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-800"
        >
          {success.value}
        </div>
      )}

      {error.value && (
        <div
          data-testid="error-message"
          class="rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700"
        >
          {error.value}
        </div>
      )}

      <div class="space-y-4">
        <button
          type="button"
          data-testid="passkey-signin-button"
          onClick$={handleExplicitPasskeyAuth}
          disabled={isLoading.value}
          class="flex w-full items-center justify-center rounded-lg bg-gray-950 px-4 py-3 text-sm font-semibold text-white shadow-sm transition hover:bg-gray-800 focus:outline-2 disabled:cursor-not-allowed disabled:opacity-60"
        >
          {isLoading.value ? (
            <div class="flex items-center space-x-2">
              <div class="h-4 w-4 animate-spin rounded-full border-2 border-white border-t-transparent"></div>
              <span>Checking passkey...</span>
            </div>
          ) : (
            <div class="flex items-center space-x-2">
              <svg
                class="h-5 w-5"
                fill="none"
                viewBox="0 0 24 24"
                stroke-width={1.5}
                stroke="currentColor"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  d="M16.5 10.5V6.75a4.5 4.5 0 10-9 0v3.75m-.75 11.25h10.5A2.25 2.25 0 0019.5 19.5v-8.25A2.25 2.25 0 0017.25 9H6.75A2.25 2.25 0 004.5 11.25v8.25A2.25 2.25 0 006.75 21.75z"
                />
              </svg>
              <span>Continue with passkey</span>
            </div>
          )}
        </button>

        {/* Divider */}
        <div class="relative">
          <div class="absolute inset-0 flex items-center">
            <div class="w-full border-t border-gray-300" />
          </div>
          <div class="relative flex justify-center text-sm">
            <span class="bg-white px-3 text-xs font-medium uppercase tracking-wide text-gray-500">
              Or continue with
            </span>
          </div>
        </div>

        {/* OAuth Buttons */}
        <div class="grid grid-cols-2 gap-3">
          <button
            type="button"
            data-testid="google-signin-button"
            onClick$={handleGoogleAuth}
            disabled={isLoading.value}
            class="inline-flex w-full items-center justify-center rounded-lg border border-gray-200 bg-white px-4 py-2.5 text-sm font-medium text-gray-700 shadow-sm transition hover:border-gray-300 hover:bg-gray-50 focus:outline-2 disabled:cursor-not-allowed disabled:opacity-60"
          >
            <svg class="mr-2 h-5 w-5" viewBox="0 0 24 24">
              <path
                fill="#4285F4"
                d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"
              />
              <path
                fill="#34A853"
                d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"
              />
              <path
                fill="#FBBC05"
                d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"
              />
              <path
                fill="#EA4335"
                d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"
              />
            </svg>
            Google
          </button>

          <button
            type="button"
            onClick$={handleAppleAuth}
            disabled={isLoading.value}
            class="inline-flex w-full items-center justify-center rounded-lg border border-gray-200 bg-white px-4 py-2.5 text-sm font-medium text-gray-700 shadow-sm transition hover:border-gray-300 hover:bg-gray-50 focus:outline-2 disabled:cursor-not-allowed disabled:opacity-60"
          >
            <svg class="mr-2 h-5 w-5" fill="currentColor" viewBox="0 0 24 24">
              <path d="M18.71 19.5c-.83 1.24-1.71 2.45-3.05 2.47-1.34.03-1.77-.79-3.29-.79-1.53 0-2 .77-3.27.82-1.31.05-2.3-1.32-3.14-2.53C4.25 17 2.94 12.45 4.7 9.39c.87-1.52 2.43-2.48 4.12-2.51 1.28-.02 2.5.87 3.29.87.78 0 2.26-1.07 3.81-.91.65.03 2.47.26 3.64 1.98-.09.06-2.17 1.28-2.15 3.81.03 3.02 2.65 4.03 2.68 4.04-.03.07-.42 1.44-1.38 2.83M13 3.5c.73-.83 1.94-1.46 2.94-1.5.13 1.17-.34 2.35-1.04 3.19-.69.85-1.83 1.51-2.95 1.42-.15-1.15.41-2.35 1.05-3.11z" />
            </svg>
            Apple
          </button>
        </div>
      </div>

      <div class="rounded-lg bg-gray-50 px-4 py-3 text-xs leading-5 text-gray-600">
        Passkeys use your device biometrics or screen lock. Hamrah never sees
        or stores your biometric data.
      </div>
    </div>
  );
});

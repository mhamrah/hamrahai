import {
  component$,
  useSignal,
  useVisibleTask$,
  $,
  type QRL,
} from "@builder.io/qwik";
import {
  webauthnClient,
  WebAuthnClient,
  type WebAuthnCredential,
} from "~/lib/auth/webauthn";

interface PasskeyManagementProps {
  userId: string;
  userEmail?: string; // optional email to associate with newly created passkeys
  onError?: QRL<(error: string) => void>;
  class?: string;
}

/**
 * PasskeyManagement
 *
 * Simplified management component that:
 * - Lists existing passkeys for the authenticated user
 * - Adds new passkeys for supported browsers
 * - Allows renaming a passkey
 * - Allows deleting a passkey
 */
export const PasskeyManagement = component$<PasskeyManagementProps>((props) => {
  const passkeys = useSignal<WebAuthnCredential[]>([]);
  const isLoading = useSignal(true);
  const error = useSignal<string>("");
  const success = useSignal<string>("");
  const editingId = useSignal<string | null>(null);
  const editingName = useSignal<string>("");
  const isAdding = useSignal(false);

  const loadPasskeys = $(async () => {
    isLoading.value = true;
    error.value = "";
    success.value = "";
    try {
      const list = await webauthnClient.getUserPasskeys(props.userId);
      passkeys.value = list;
    } catch (err) {
      const msg =
        err instanceof Error ? err.message : "Failed to load passkeys";
      error.value = msg;
      await props.onError?.(msg);
    } finally {
      isLoading.value = false;
    }
  });

  const addPasskey = $(async () => {
    error.value = "";
    success.value = "";
    if (isAdding.value) return;
    if (!props.userId) {
      error.value = "Missing user id";
      return;
    }
    if (!props.userEmail) {
      error.value = "Missing user email (required to create passkey)";
      return;
    }
    if (!WebAuthnClient.isSupported()) {
      error.value = "WebAuthn not supported in this browser";
      return;
    }
    try {
      isAdding.value = true;
      const result = await webauthnClient.addPasskey(
        { id: props.userId, email: props.userEmail, name: props.userEmail },
        {},
      );
      if (result.success) {
        success.value = "Passkey added";
        await loadPasskeys();
      } else {
        error.value = result.error || "Failed to add passkey";
      }
    } catch (e: any) {
      error.value = e?.message || "Unexpected error adding passkey";
    } finally {
      isAdding.value = false;
    }
  });

  const startEditing = $((cred: WebAuthnCredential) => {
    editingId.value = cred.id;
    editingName.value = cred.name || "Passkey";
  });

  const cancelEditing = $(() => {
    editingId.value = null;
    editingName.value = "";
  });

  const saveName = $(async (credentialId: string) => {
    if (!editingName.value.trim()) {
      error.value = "Name cannot be empty";
      return;
    }
    error.value = "";
    success.value = "";
    try {
      const ok = await webauthnClient.renamePasskey(
        credentialId,
        editingName.value.trim(),
      );
      if (ok) {
        success.value = "Passkey renamed";
        editingId.value = null;
        editingName.value = "";
        await loadPasskeys();
      } else {
        error.value = "Failed to rename passkey";
      }
    } catch (err) {
      error.value =
        err instanceof Error ? err.message : "Failed to rename passkey";
    }
  });

  const deletePasskey = $(async (credentialId: string) => {
    if (!confirm("Delete this passkey? This action cannot be undone.")) return;
    error.value = "";
    success.value = "";
    try {
      const ok = await webauthnClient.deletePasskey(credentialId);
      if (ok) {
        success.value = "Passkey deleted";
        await loadPasskeys();
      } else {
        error.value = "Failed to delete passkey";
      }
    } catch (err) {
      error.value =
        err instanceof Error ? err.message : "Failed to delete passkey";
    }
  });

  const formatDate = (ts: number | string) =>
    new Date(ts).toLocaleDateString("en-US", {
      year: "numeric",
      month: "short",
      day: "numeric",
    });

  // Initial load
  // eslint-disable-next-line qwik/no-use-visible-task
  useVisibleTask$(async () => {
    await loadPasskeys();
  });

  return (
    <div class={props.class || "space-y-6"}>
      <div class="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
        <div>
          <h3 class="text-lg font-semibold text-gray-950">Passkeys</h3>
          <p class="mt-1 text-sm leading-6 text-gray-600">
            Manage the device passkeys that can access your account.
          </p>
        </div>
        <div class="flex items-center gap-2">
          <button
            type="button"
            onClick$={addPasskey}
            disabled={isLoading.value || isAdding.value}
            class={[
              "rounded-lg px-3 py-2 text-sm font-semibold text-white shadow-sm transition",
              isLoading.value || isAdding.value
                ? "cursor-not-allowed bg-gray-400"
                : "bg-gray-950 hover:bg-gray-800",
            ].join(" ")}
          >
            {isAdding.value ? (
              <span class="flex items-center gap-2">
                <span class="h-4 w-4 animate-spin rounded-full border-2 border-white border-t-transparent" />
                Adding...
              </span>
            ) : (
              "Add Passkey"
            )}
          </button>
        </div>
      </div>

      {error.value && (
        <div class="rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700">
          {error.value}
        </div>
      )}
      {success.value && (
        <div class="rounded-lg border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-800">
          {success.value}
        </div>
      )}

      {isLoading.value ? (
        <div class="flex items-center justify-center py-10">
          <div class="h-6 w-6 animate-spin rounded-full border-2 border-gray-950 border-t-transparent" />
        </div>
      ) : passkeys.value.length === 0 ? (
        <div class="rounded-lg border border-dashed border-gray-300 bg-gray-50 p-8 text-center">
          <svg
            class="mx-auto h-12 w-12 text-gray-400"
            fill="none"
            viewBox="0 0 24 24"
            stroke-width={1.5}
            stroke="currentColor"
          >
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              d="M15.75 5.25a3 3 0 013 3m3 0a6 6 0 01-7.029 5.912c-.563-.097-1.159.026-1.563.43L10.5 17.25H8.25v2.25H6v2.25H2.25v-2.818c0-.597.237-1.17.659-1.591l6.499-6.499c.404-.404.527-1 .43-1.563A6 6 0 1121.75 8.25z"
            />
          </svg>
          <h4 class="mt-4 text-lg font-medium text-gray-900">No passkeys</h4>
          <p class="mt-2 text-sm leading-6 text-gray-600">
            Add one now so future sign-ins can use Face ID, Touch ID, Windows
            Hello, or your device screen lock.
          </p>
        </div>
      ) : (
        <div class="space-y-3">
          {passkeys.value.map((cred) => {
            const isEditing = editingId.value === cred.id;
            return (
              <div
                key={cred.id}
                class="rounded-lg border border-gray-200 bg-white p-4 transition-colors hover:border-gray-300"
              >
                <div class="flex items-start justify-between gap-4">
                  <div class="flex items-center gap-3">
                    <div class="flex h-10 w-10 items-center justify-center rounded-full bg-gray-100">
                      <svg
                        class="h-5 w-5 text-gray-700"
                        fill="none"
                        viewBox="0 0 24 24"
                        stroke-width={1.5}
                        stroke="currentColor"
                      >
                        <path
                          stroke-linecap="round"
                          stroke-linejoin="round"
                          d="M15.75 5.25a3 3 0 013 3m3 0a6 6 0 01-7.029 5.912c-.563-.097-1.159.026-1.563.43L10.5 17.25H8.25v2.25H6v2.25H2.25v-2.818c0-.597.237-1.17.659-1.591l6.499-6.499c.404-.404.527-1 .43-1.563A6 6 0 1121.75 8.25z"
                        />
                      </svg>
                    </div>
                    <div>
                      {isEditing ? (
                        <div class="flex items-center gap-2">
                          <input
                            type="text"
                            value={editingName.value}
                            onInput$={(e) => {
                              editingName.value = (
                                e.target as HTMLInputElement
                              ).value;
                            }}
                            class="w-40 rounded-lg border border-gray-300 px-2 py-1 text-sm focus:border-gray-500 focus:outline-2"
                          />
                          <button
                            type="button"
                            onClick$={() => saveName(cred.id)}
                            class="rounded-lg bg-gray-950 px-2 py-1 text-xs font-medium text-white hover:bg-gray-800"
                          >
                            Save
                          </button>
                          <button
                            type="button"
                            onClick$={cancelEditing}
                            class="rounded-lg bg-gray-100 px-2 py-1 text-xs font-medium text-gray-700 hover:bg-gray-200"
                          >
                            Cancel
                          </button>
                        </div>
                      ) : (
                        <>
                          <h4 class="font-medium text-gray-900">
                            {cred.name || "Passkey"}
                          </h4>
                          <p class="text-xs text-gray-500">
                            Created {formatDate(cred.created_at)}
                            {cred.last_used
                              ? ` - Last used ${formatDate(cred.last_used)}`
                              : ""}
                          </p>
                        </>
                      )}
                    </div>
                  </div>
                  {!isEditing && (
                    <div class="flex items-center gap-1">
                      <button
                        type="button"
                        onClick$={() => startEditing(cred)}
                        class="rounded p-1 text-gray-400 hover:text-gray-600"
                        title="Rename"
                      >
                        <svg
                          class="h-4 w-4"
                          fill="none"
                          viewBox="0 0 24 24"
                          stroke-width={1.5}
                          stroke="currentColor"
                        >
                          <path
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            d="m16.862 4.487 1.687-1.688a1.875 1.875 0 1 1 2.652 2.652L10.582 16.07a4.5 4.5 0 0 1-1.897 1.13L6 18l.8-2.685a4.5 4.5 0 0 1 1.13-1.897l8.932-8.931Zm0 0L19.5 7.125M18 14v4.75A2.25 2.25 0 0 1 15.75 21H5.25A2.25 2.25 0 0 1 3 18.75V8.25A2.25 2.25 0 0 1 5.25 6H10"
                          />
                        </svg>
                      </button>
                      <button
                        type="button"
                        onClick$={() => deletePasskey(cred.id)}
                        class="rounded p-1 text-red-400 hover:text-red-600"
                        title="Delete"
                      >
                        <svg
                          class="h-4 w-4"
                          fill="none"
                          viewBox="0 0 24 24"
                          stroke-width={1.5}
                          stroke="currentColor"
                        >
                          <path
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            d="m14.74 9-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 0 1-2.244 2.077H8.084a2.25 2.25 0 0 1-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 0 0-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 0 1 3.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 0 0-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 0 0-7.5 0"
                          />
                        </svg>
                      </button>
                    </div>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}

      <div class="rounded-lg border border-cambridge-blue-200 bg-cambridge-blue-900/5 px-4 py-3 text-xs leading-relaxed text-cambridge-blue-900">
        <strong class="font-medium">About passkeys:</strong> Passkeys let you
        sign in using secure device biometrics (Face ID, Touch ID, Windows
        Hello) instead of passwords. If you delete all passkeys, you will need
        to sign in again with an alternative method before adding a new one in a
        future update.
      </div>
    </div>
  );
});

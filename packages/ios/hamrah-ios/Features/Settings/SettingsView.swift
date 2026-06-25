import AuthenticationServices
import LocalAuthentication
import SwiftData
import SwiftUI

#if os(macOS)
    import AppKit
#endif

// MARK: - Settings View

struct SettingsView: View {
    @Environment(\.modelContext) private var modelContext
    @EnvironmentObject var authManager: NativeAuthManager
    @EnvironmentObject var biometricManager: BiometricAuthManager
    @EnvironmentObject var syncEngine: SyncEngine

    // Backing store (SwiftData) for a single UserPrefs instance
    @Query private var prefsQuery: [UserPrefs]

    // UI State
    @State private var isLoading = false
    @State private var isSaving = false
    @State private var errorMessage: String?
    @State private var infoMessage: String?
    @State private var passkeysLoadMessage: String?
    @State private var showErrorAlert = false
    @State private var showConfirmDialog = false
    @State private var credentialToDelete: PasskeyCredential?
    @State private var showAddPasskey = false
    @State private var showBiometricSettings = false
    @State private var showLogoutConfirm = false

    // Editable preferences
    @State private var defaultModel: String = SettingsModelCatalog.defaultModelId
    @State private var preferredModels: Set<String> = []

    // Model catalog
    @State private var availableModels: [AIModelDTO] = SettingsModelCatalog.defaultModels
    @State private var isFetchingModels = false

    @State private var passkeys: [PasskeyCredential] = []

    private var availableModelIds: [String] {
        availableModels.filter(\.isAvailable).map(\.id)
    }

    var body: some View {
        Form {
            userInfoSection
            authProvidersSection
            passkeysSection
            biometricSection
            serverSyncSection
            modelsSection
            syncEngineSection
            advancedSection
        }
        .navigationTitle("Settings")
        .task {
            seedLocalFromStore()
            await fetchModelCatalog()
            await loadFromServerIfEmpty()
            loadPasskeys(showAlertOnFailure: false)
        }
        .alert(
            "Error", isPresented: .constant(errorMessage != nil),
            actions: {
                Button("OK") { errorMessage = nil }
            },
            message: {
                Text(errorMessage ?? "")
            }
        )
        .alert("Remove Passkey", isPresented: $showConfirmDialog) {
            Button("Cancel", role: .cancel) {
                credentialToDelete = nil
            }
            Button("Remove", role: .destructive) {
                if let credential = credentialToDelete {
                    removePasskey(credential)
                }
            }
        } message: {
            Text("Are you sure you want to remove this passkey? This action cannot be undone.")
        }
        .alert("Sign Out?", isPresented: $showLogoutConfirm) {
            Button("Cancel", role: .cancel) {}
            Button("Sign Out", role: .destructive) {
                authManager.logout()
                infoMessage = "Signed out."
            }
        } message: {
            Text("You will be signed out of this device.")
        }
    }

    // MARK: - View Components

    private var userInfoSection: some View {
        Section("Account Information") {
            if let user = authManager.currentUser {
                LabeledContent("User ID", value: user.id)
                LabeledContent("Email", value: user.email)
                LabeledContent("Name", value: user.name ?? "Not provided")
                LabeledContent("Last Sign-In", value: user.authMethod.capitalized)
                LabeledContent("Member Since", value: formatDate(user.createdAt ?? ""))

                // Sign Out action moved here for visibility
                Button(role: .destructive) {
                    showLogoutConfirm = true
                } label: {
                    Text("Sign Out")
                }
            } else {
                Text("Not logged in.")
            }
        }
    }

    private var authProvidersSection: some View {
        Section("Authentication Providers") {
            if let user = authManager.currentUser {
                ForEach(linkedAuthProviders(for: user), id: \.self) { provider in
                    HStack {
                        Image(systemName: authProviderIcon(provider))
                            .foregroundColor(authProviderColor(provider))
                            .frame(width: 24)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(provider.capitalized)
                                .font(.subheadline)
                            Text(provider == user.authMethod ? "Last used to sign in" : "Linked")
                                .font(.caption)
                                .foregroundColor(.secondary)
                        }
                    }
                }

                if !isAuthProviderLinked("google", for: user) {
                    Button(action: {
                        addGoogleSignIn()
                    }) {
                        HStack {
                            Image(systemName: "g.circle.fill")
                                .foregroundColor(.red)
                                .frame(width: 24)
                            Text("Add Google Sign-In")
                                .font(.subheadline)
                        }
                    }
                    .disabled(!authManager.googleSignInStatus.isAvailable)
                    if let message = authManager.googleSignInStatus.message {
                        Text(message)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }

                if !isAuthProviderLinked("apple", for: user) {
                    Button(action: {
                        Task { await authManager.signInWithApple() }
                    }) {
                        HStack {
                            Image(systemName: "applelogo")
                                .foregroundColor(.primary)
                                .frame(width: 24)
                            Text("Add Apple Sign-In")
                                .font(.subheadline)
                        }
                    }
                }
            } else {
                Text("Sign in to manage authentication providers.")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private var biometricSection: some View {
        Section("Security") {
            Button(action: {
                showBiometricSettings = true
            }) {
                HStack {
                    Image(systemName: biometricIconName)
                        .foregroundColor(.blue)
                        .frame(width: 24)

                    VStack(alignment: .leading, spacing: 2) {
                        Text(biometricManager.biometricTypeString)
                            .font(.subheadline)
                            .foregroundColor(.primary)

                        Text(biometricStatusText)
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }

                    Spacer()

                    Image(systemName: "chevron.right")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
            }
            .buttonStyle(PlainButtonStyle())
        }
        .sheet(isPresented: $showBiometricSettings) {
            #if os(macOS)
                NavigationView {
                    BiometricSettingsView()
                        .environmentObject(biometricManager)
                        .toolbar {
                            ToolbarItem(placement: .cancellationAction) {
                                Button("Done") { showBiometricSettings = false }
                            }
                        }
                }
                .frame(minWidth: 520, minHeight: 560)
            #else
                NavigationView {
                    BiometricSettingsView()
                        .environmentObject(biometricManager)
                        .navigationBarItems(
                            trailing: Button("Done") {
                                showBiometricSettings = false
                            })
                }
            #endif
        }
    }

    private var biometricIconName: String {
        switch biometricManager.biometricType {
        case .faceID:
            return "faceid"
        case .touchID:
            return "touchid"
        case .opticID:
            return "opticid"
        case .none:
            return "lock.slash"
        @unknown default:
            return "questionmark"
        }
    }

    private var biometricStatusText: String {
        if !biometricManager.isAvailable {
            return "Not available on this device"
        } else if biometricManager.isBiometricEnabled {
            return "Enabled"
        } else {
            return "Tap to set up"
        }
    }

    private var passkeysSection: some View {
        Section("Passkeys") {
            if isLoading {
                ProgressView("Loading passkeys...")
                    .frame(maxWidth: .infinity)
                    .padding()
            } else if let passkeysLoadMessage {
                Label(passkeysLoadMessage, systemImage: "exclamationmark.triangle.fill")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.vertical, 8)
            } else if passkeys.isEmpty {
                VStack(spacing: 8) {
                    Image(systemName: "key.fill")
                        .font(.title2)
                        .foregroundColor(.gray)
                    Text("No passkeys found")
                        .font(.caption)
                        .foregroundColor(.gray)
                    Text("Add a passkey for secure authentication")
                        .font(.caption2)
                        .foregroundColor(.gray)
                        .multilineTextAlignment(.center)
                }
                .frame(maxWidth: .infinity)
                .padding()
            } else {
                ForEach(passkeys) { passkey in
                    PasskeyRow(
                        passkey: passkey,
                        onRemove: { credential in
                            credentialToDelete = credential
                            showConfirmDialog = true
                        }
                    )
                }
            }
            Button("Add Passkey") {
                showAddPasskey = true
            }
            .font(.caption)
            .disabled(
                isLoading || authManager.currentUser == nil || authManager.accessToken == nil)
        }
        .sheet(isPresented: $showAddPasskey) {
            AddPasskeyView(onPasskeyAdded: {
                loadPasskeys(showAlertOnFailure: false)
            })
            .environmentObject(authManager)
            #if os(macOS)
                .frame(minWidth: 460, minHeight: 520)
            #endif
        }
    }

    @ViewBuilder
    private var serverSyncSection: some View {
        Section("Server Sync") {
            HStack {
                if isLoading {
                    ProgressView().controlSize(.small)
                }
                Button("Load From Server") { Task { await loadFromServer() } }
                    .disabled(isLoading || accessToken() == nil)
                Button("Save To Server") { Task { await saveToServer() } }
                    .disabled(isSaving || accessToken() == nil)
            }
            .buttonStyle(.bordered)

            if let info = infoMessage {
                Text(info).font(.footnote).foregroundStyle(.secondary)
            }

            if accessToken() == nil {
                Text("Sign in to sync preferences with your account.")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }
        }
    }

    @ViewBuilder
    private var modelsSection: some View {
        Section("AI Models") {
            // Default model picker
            Picker("Default Model", selection: $defaultModel) {
                ForEach(availableModels.filter(\.isAvailable)) { model in
                    Text(model.display_name).tag(model.id)
                }
            }
            .onChange(of: defaultModel) { _, _ in debounceAutosave() }

            // Preferred models multi-selection
            VStack(alignment: .leading, spacing: 8) {
                Text("Preferred Models (\(preferredModels.count) selected)")
                    .font(.subheadline)
                    .fontWeight(.medium)

                LazyVGrid(columns: [GridItem(.adaptive(minimum: 120))], spacing: 8) {
                    ForEach(availableModels.filter(\.isAvailable)) { model in
                        Button(action: {
                            if preferredModels.contains(model.id) {
                                preferredModels.remove(model.id)
                            } else {
                                preferredModels.insert(model.id)
                            }
                            debounceAutosave()
                        }) {
                            Text(model.display_name)
                                .font(.caption)
                                .padding(.horizontal, 8)
                                .padding(.vertical, 4)
                                .background(
                                    preferredModels.contains(model.id)
                                        ? Color.accentColor : Color.secondary.opacity(0.2)
                                )
                                .foregroundColor(
                                    preferredModels.contains(model.id) ? .white : .primary
                                )
                                .cornerRadius(8)
                        }
                        .buttonStyle(.plain)
                    }
                }
            }

            // Model management
            if isFetchingModels {
                HStack {
                    ProgressView().controlSize(.small)
                    Text("Fetching available models…")
                }
            } else {
                Text("Available: \(availableModels.count) models")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }

            HStack {
                Button("Refresh Models") { Task { await fetchModelCatalog() } }
                Spacer()
                Button("Clear Preferred") {
                    preferredModels = []
                    Task { await saveToServer() }
                }
            }
            .buttonStyle(.bordered)
        }
    }

    @ViewBuilder
    private var syncEngineSection: some View {
        Section("Sync Engine") {
            Button {
                Task { await runHealthSync() }
            } label: {
                if syncEngine.isSyncingNow {
                    Label("Syncing", systemImage: "arrow.triangle.2.circlepath")
                } else {
                    Label("Sync Now", systemImage: "arrow.clockwise")
                }
            }
            .buttonStyle(.borderedProminent)
            .disabled(syncEngine.isSyncingNow || accessToken() == nil)

            LabeledContent("Queued", value: "\(syncEngine.queuedLinkCount)")
            if let lastSyncAt = syncEngine.lastSyncAt {
                LabeledContent(
                    "Last Sync",
                    value: lastSyncAt.formatted(date: .abbreviated, time: .shortened))
            }
            if let lastSyncError = syncEngine.lastSyncError {
                Text(lastSyncError)
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }
        }
    }

    @ViewBuilder
    private var advancedSection: some View {
        Section("Advanced") {
            NavigationLink {
                APIConfigurationView()
            } label: {
                Label("API Environment", systemImage: "globe")
            }

            #if DEBUG
                Button {
                    copyAPIPromptToClipboard()
                } label: {
                    Label("Copy Settings API Prompt", systemImage: "doc.on.doc")
                }
                .buttonStyle(.bordered)

                Button(role: .destructive) {
                    AppAttestationManager.shared.resetAttestation()
                    infoMessage = "App Attestation will be recreated automatically."
                } label: {
                    Label("Reset App Attestation", systemImage: "shield.slash")
                }
                .buttonStyle(.bordered)
            #endif
        }
    }

    // Deprecated: logoutSection removed; Sign Out moved to Account Information section

    // MARK: - Local Store Helpers

    private func seedLocalFromStore() {
        let prefs = fetchOrCreatePrefs()
        defaultModel = prefs.defaultModel
        preferredModels = Set(prefs.preferredModels)
    }

    private func applyToStore() {
        let prefs = fetchOrCreatePrefs()
        prefs.defaultModel = defaultModel
        prefs.preferredModels = Array(preferredModels)
        prefs.lastUpdatedAt = Date()

        do { try modelContext.save() } catch {
            errorMessage = "Failed to save preferences locally: \(error.localizedDescription)"
        }
    }

    private func fetchOrCreatePrefs() -> UserPrefs {
        if let existing = prefsQuery.first {
            return existing
        }
        let created = UserPrefs(
            defaultModel: defaultModel,
            preferredModels: Array(preferredModels)
        )
        modelContext.insert(created)
        return created
    }

    // MARK: - Server Sync

    private func loadFromServerIfEmpty() async {
        // Load once from server if we have no server values yet
        if accessToken() == nil { return }
        if !preferredModels.isEmpty || defaultModel != SettingsModelCatalog.defaultModelId { return }
        await loadFromServer()
    }

    private func loadFromServer() async {
        guard let token = accessToken() else {
            infoMessage = "Not signed in."
            return
        }
        isLoading = true
        defer { isLoading = false }
        do {
            let dto: UserPrefsDTO = try await SecureAPIService.shared.get(
                endpoint: "/v1/user/prefs",
                accessToken: token,
                responseType: UserPrefsDTO.self
            )
            await MainActor.run {
                mapDTOToState(dto)
                applyToStore()
                infoMessage =
                    "Loaded from server at \(Date().formatted(date: .omitted, time: .shortened))."
            }
        } catch {
            await MainActor.run {
                errorMessage = "Failed to load from server: \(error.localizedDescription)"
            }
        }
    }

    private func saveToServer() async {
        guard let token = accessToken() else {
            infoMessage = "Not signed in."
            return
        }
        isSaving = true
        defer { isSaving = false }
        do {
            let dto = makeDTOFromState()
            let _: UserPrefsDTO = try await SecureAPIService.shared.put(
                endpoint: "/v1/user/prefs",
                body: dto.asJSON(),
                accessToken: token,
                responseType: UserPrefsDTO.self
            )
            await MainActor.run {
                applyToStore()
                infoMessage =
                    "Saved to server at \(Date().formatted(date: .omitted, time: .shortened))."
            }
        } catch {
            await MainActor.run {
                errorMessage = "Failed to save to server: \(error.localizedDescription)"
            }
        }
    }

    // MARK: - Models Catalog

    private func fetchModelCatalog() async {
        isFetchingModels = true
        defer { isFetchingModels = false }
        // Try a best-effort fetch. If missing, keep local defaults.
        do {
            struct CatalogResponse: Codable { let models: [AIModelDTO] }
            let token = accessToken()
            let resp: CatalogResponse = try await SecureAPIService.shared.get(
                endpoint: "/v1/models",
                accessToken: token,
                responseType: CatalogResponse.self
            )
            await MainActor.run {
                availableModels = resp.models.isEmpty ? SettingsModelCatalog.defaultModels : resp.models
                pruneUnavailableModels(showMessage: true)
            }
        } catch {
            // Silently fall back to defaults if the endpoint is not implemented
            await MainActor.run {
                availableModels = SettingsModelCatalog.defaultModels
                pruneUnavailableModels(showMessage: false)
            }
        }
    }

    // MARK: - Actions

    private func copyAPIPromptToClipboard() {
        #if os(iOS)
            PlatformBridge.copyToClipboard(SettingsAPIPrompt.prompt)
        #elseif os(macOS)
            PlatformBridge.copyToClipboard(SettingsAPIPrompt.prompt)
        #endif
        infoMessage = "API prompt copied to clipboard."
    }

    private func accessToken() -> String? {
        KeychainManager.shared.retrieveString(for: "hamrah_access_token")
    }

    private func mapDTOToState(_ dto: UserPrefsDTO) {
        defaultModel = dto.default_model
        preferredModels = Set(dto.preferred_models)
        pruneUnavailableModels(showMessage: false)
    }

    private func makeDTOFromState() -> UserPrefsDTO {
        UserPrefsDTO(
            default_model: defaultModel,
            preferred_models: Array(preferredModels)
        )
    }

    private func formatDate(_ dateString: String) -> String {
        let formatter = ISO8601DateFormatter()
        if let date = formatter.date(from: dateString) {
            let displayFormatter = DateFormatter()
            displayFormatter.dateStyle = .medium
            return displayFormatter.string(from: date)
        }
        return dateString
    }

    private func authProviderIcon(_ method: String) -> String {
        switch method.lowercased() {
        case "google": return "g.circle.fill"
        case "apple": return "applelogo"
        case "passkey": return "key.fill"
        default: return "person.circle.fill"
        }
    }

    private func authProviderColor(_ method: String) -> Color {
        switch method.lowercased() {
        case "google": return .red
        case "apple": return .primary
        case "passkey": return .green
        default: return .blue
        }
    }

    private func linkedAuthProviders(for user: NativeAuthManager.HamrahUser) -> [String] {
        let providers = user.authProviders.isEmpty ? [user.authMethod] : user.authProviders
        let withPasskey = passkeys.isEmpty ? providers : providers + ["passkey"]
        return Array(Set(withPasskey.map { $0.lowercased() })).sorted()
    }

    private func isAuthProviderLinked(
        _ provider: String,
        for user: NativeAuthManager.HamrahUser
    ) -> Bool {
        linkedAuthProviders(for: user).contains(provider.lowercased())
    }

    private func addGoogleSignIn() {
        errorMessage = nil
        guard authManager.googleSignInStatus.isAvailable else {
            errorMessage = authManager.googleSignInStatus.message
            return
        }
        Task {
            await authManager.signInWithGoogle()
            await MainActor.run {
                if let message = authManager.errorMessage {
                    self.errorMessage = message
                }
            }
        }
    }

    private func loadPasskeys(showAlertOnFailure: Bool = false) {
        // Debug authentication state
        print("🔍 MyAccountView Authentication Debug:")
        print("  Current User present: \(authManager.currentUser != nil)")
        print("  Access Token: \(accessToken() != nil ? "present" : "nil")")
        print("  Is Authenticated: \(authManager.isAuthenticated)")

        guard let accessToken = accessToken() else {
            passkeysLoadMessage = "Sign in to view passkeys."
            if showAlertOnFailure {
                errorMessage = "Not authenticated. Please sign in again."
                showErrorAlert = true
            }
            return
        }

        isLoading = true
        passkeysLoadMessage = nil
        if showAlertOnFailure {
            errorMessage = nil
        }

        Task {
            do {
                let credentials = try await fetchPasskeys(accessToken: accessToken)
                await MainActor.run {
                    self.passkeys = credentials
                    self.passkeysLoadMessage = nil
                    self.isLoading = false
                }
            } catch {
                await MainActor.run {
                    self.passkeys = []
                    self.passkeysLoadMessage = "Passkeys couldn't be loaded right now."
                    if showAlertOnFailure {
                        self.errorMessage = "Failed to load passkeys: \(error.localizedDescription)"
                        self.showErrorAlert = true
                    }
                    self.isLoading = false
                }
            }
        }
    }

    private func removePasskey(_ credential: PasskeyCredential) {
        guard let accessToken = accessToken() else { return }

        Task {
            do {
                try await deletePasskey(credentialId: credential.id, accessToken: accessToken)
                await MainActor.run {
                    self.passkeys.removeAll { $0.id == credential.id }
                    self.credentialToDelete = nil
                }
            } catch {
                await MainActor.run {
                    self.errorMessage = "Failed to remove passkey: \(error.localizedDescription)"
                    self.showErrorAlert = true
                    self.credentialToDelete = nil
                }
            }
        }
    }

    private func fetchPasskeys(accessToken: String) async throws -> [PasskeyCredential] {
        guard let userId = authManager.currentUser?.id else {
            throw NSError(
                domain: "API",
                code: -1,
                userInfo: [NSLocalizedDescriptionKey: "User ID not available"])
        }
        let response: PasskeyListResponse = try await SecureAPIService.shared.get(
            endpoint: "/api/webauthn/users/\(userId)/credentials",
            accessToken: accessToken,
            responseType: PasskeyListResponse.self
        )
        return response.credentials
    }

    private func deletePasskey(credentialId: String, accessToken: String) async throws {
        // Updated to use path parameter style delete (no body)
        _ = try await SecureAPIService.shared.delete(
            endpoint: "/api/webauthn/credentials/\(credentialId)",
            accessToken: accessToken,
            responseType: APIResponse.self
        )
    }

    // MARK: - Autosave

    @State private var debounceTask: Task<Void, Never>?

    private func debounceAutosave() {
        debounceTask?.cancel()
        debounceTask = Task {
            try? await Task.sleep(for: .seconds(1))
            if !Task.isCancelled {
                await MainActor.run { applyToStore() }
            }
        }
    }

    private func pruneUnavailableModels(showMessage: Bool) {
        let validIds = Set(availableModelIds)
        let originalDefault = defaultModel
        let originalPreferred = preferredModels

        if !validIds.contains(defaultModel) {
            defaultModel = availableModelIds.first ?? SettingsModelCatalog.defaultModelId
        }
        preferredModels = preferredModels.intersection(validIds)

        if showMessage && (originalDefault != defaultModel || originalPreferred != preferredModels) {
            infoMessage = "Models updated. Deprecated selections were removed."
            applyToStore()
        }
    }

    private func runHealthSync() async {
        if authManager.isTokenExpiringSoon() {
            _ = await authManager.refreshToken()
        }
        if let token = authManager.accessToken ?? accessToken() {
            await SecureAPIService.shared.initializeAttestation(accessToken: token)
        }
        await syncEngine.runSyncNow(reason: "settings_manual_sync")
        await fetchModelCatalog()
        await loadFromServer()
    }
}

struct PasskeyRow: View {
    let passkey: PasskeyCredential
    let onRemove: (PasskeyCredential) -> Void

    var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 4) {
                HStack {
                    Image(systemName: "key.fill")
                        .font(.caption)
                        .foregroundColor(.green)

                    Text(passkey.name)
                        .font(.caption)
                        .fontWeight(.medium)
                }

                Text("Created \(formatDate(passkey.createdAt))")
                    .font(.caption2)
                    .foregroundColor(.secondary)

                if let lastUsed = passkey.lastUsed {
                    Text("Last used \(formatDate(lastUsed))")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                }
            }

            Spacer()

            Button("Remove") {
                onRemove(passkey)
            }
            .font(.caption2)
            .foregroundColor(.red)
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(Color.red.opacity(0.1))
            .cornerRadius(6)
        }
        .padding(.vertical, 8)
    }

    private func formatDate(_ dateString: String) -> String {
        let formatter = ISO8601DateFormatter()
        if let date = formatter.date(from: dateString) {
            let displayFormatter = DateFormatter()
            displayFormatter.dateStyle = .short
            return displayFormatter.string(from: date)
        }
        return dateString
    }
}

// MARK: - DTOs and helpers

struct UserPrefsDTO: Codable {
    var default_model: String
    var preferred_models: [String]
    var last_updated_at: String?

    init(
        default_model: String,
        preferred_models: [String],
        last_updated_at: String? = nil
    ) {
        self.default_model = default_model
        self.preferred_models = preferred_models
        self.last_updated_at = last_updated_at
    }

    func asJSON() -> [String: Any] {
        [
            "default_model": default_model,
            "preferred_models": preferred_models,
        ]
    }
}

struct AIModelDTO: Codable, Identifiable {
    let id: String
    let display_name: String
    let provider: String
    let status: String
    let replacement_id: String?

    var isAvailable: Bool {
        status == "available"
    }
}

enum SettingsModelCatalog {
    static let defaultModelId = "@cf/zai-org/glm-4.7-flash"

    static let defaultModels: [AIModelDTO] = [
        AIModelDTO(
            id: "@cf/zai-org/glm-4.7-flash",
            display_name: "GLM 4.7 Flash",
            provider: "Cloudflare Workers AI",
            status: "available",
            replacement_id: nil),
        AIModelDTO(
            id: "@cf/google/gemma-4-26b-a4b-it",
            display_name: "Gemma 4 26B",
            provider: "Cloudflare Workers AI",
            status: "available",
            replacement_id: nil),
        AIModelDTO(
            id: "@cf/moonshotai/kimi-k2.6",
            display_name: "Kimi K2.6",
            provider: "Cloudflare Workers AI",
            status: "available",
            replacement_id: nil),
    ]
}

// MARK: - API Prompt (copy-to-clipboard)

enum SettingsAPIPrompt {
    static let prompt: String = """
        Backend API design for Hamrah iOS client user preferences.

        Required Endpoints:
        - GET /v1/user/prefs
          Returns user preferences:
          {
            "default_model": string,
            "preferred_models": string[],
            "last_updated_at": RFC3339 string
          }

        - PUT /v1/user/prefs
          Updates user preferences:
          {
            "default_model": string,
            "preferred_models": string[]
          }
          Responds with same shape as GET.

        - GET /v1/models
          Returns the available AI model identifiers from Cloudflare AI platform:
          { "models": string[] }

        - POST /v1/links
          Create/sync new links from iOS app

        Auth:
        - Bearer access token required.
        - Include App Attestation headers from the iOS client.

        Semantics:
        - User preferences are scoped to authenticated user (not device-specific).
        - default_model is the user's primary AI model choice for content processing.
        - preferred_models is an additional list of models the user wants available.
        - Model selection influences summarization/ranking requests submitted by the client.

        Validation:
        - default_model and preferred_models must be subset of /v1/models.

        Notes:
        - /v1/models endpoint should query Cloudflare AI platform for available models
        - Models are used for content processing and summarization
        """
}

// MARK: - Preview

#if DEBUG
    struct SettingsView_Previews: PreviewProvider {
        static var previews: some View {
            NavigationView {
                SettingsView()
            }
            .environmentObject(NativeAuthManager())
            .environmentObject(BiometricAuthManager())
            .environmentObject(SyncEngine(modelContainer: AppModelSchema.makeInMemoryContainer()))
        }
    }
#endif

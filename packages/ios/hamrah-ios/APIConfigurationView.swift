import SwiftUI

struct APIConfigurationView: View {
    @Environment(\.dismiss) private var dismiss
    @State private var configuration = APIConfiguration.shared
    @State private var customApiURLText: String = ""
    @State private var showAlert = false
    @State private var alertMessage = ""

    var body: some View {
        NavigationView {
            Form {
                Section(header: Text("API Environment")) {
                    ForEach(APIConfiguration.Environment.allCases, id: \.self) { environment in
                        HStack {
                            Text(environment.rawValue)
                            Spacer()
                            if configuration.currentEnvironment == environment {
                                Image(systemName: "checkmark")
                                    .foregroundColor(.blue)
                            }
                        }
                        .contentShape(Rectangle())
                        .onTapGesture {
                            configuration.currentEnvironment = environment
                            if environment != .custom {
                                customApiURLText = ""
                            }
                        }
                    }
                }

                if configuration.currentEnvironment == .custom {
                    Section(header: Text("Custom URL")) {
                        VStack(alignment: .leading, spacing: 8) {
                            Text("API Base URL")
                                .font(.caption)
                                .foregroundColor(.secondary)
                            TextField(
                                "http://127.0.0.1:8787 or https://api.example.com",
                                text: $customApiURLText
                            )
                            #if os(iOS)
                                .textInputAutocapitalization(.never)
                                .autocorrectionDisabled()
                            #endif
                            .onSubmit {
                                updateCustomURL()
                            }

                            Button("Update Custom URL") {
                                updateCustomURL()
                            }
                            .disabled(customApiURLText.isEmpty)
                        }
                    }
                }

                #if targetEnvironment(simulator)
                    Section(header: Text("Simulator Settings")) {
                        Toggle(
                            "Use localhost:8080 on Simulator",
                            isOn: $configuration.simulatorLocalhostEnabled)
                    }
                #endif

                Section(header: Text("Current Configuration")) {
                    HStack {
                        Text("API Base URL")
                        Spacer()
                        Text(configuration.baseURL)
                            .foregroundColor(.secondary)
                            .font(.caption)
                            .lineLimit(2)
                    }

                    HStack {
                        Text("Environment")
                        Spacer()
                        Text(configuration.currentEnvironment.rawValue)
                            .foregroundColor(.secondary)
                    }
                }

                Section {
                    Button("Reset to Production") {
                        configuration.reset()
                        customApiURLText = ""
                        showSuccessAlert("Configuration reset to production")
                    }
                    .foregroundColor(.red)
                }
            }
            .navigationTitle("API Configuration")
            #if os(iOS)
                .navigationBarItems(
                    trailing: Button("Done") { dismiss() }
                )
            #else
                .formStyle(.grouped)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) {
                        Button("Close") { dismiss() }
                    }
                    ToolbarItem(placement: .automatic) {
                        Button("Apply") { updateCustomURL() }
                        .disabled(
                            configuration.currentEnvironment == .custom
                                && customApiURLText.trimmingCharacters(in: .whitespacesAndNewlines)
                                    .isEmpty
                        )
                    }
                }
                .frame(minWidth: 520, minHeight: 520)
            #endif
            .onAppear {
                customApiURLText = configuration.customApiBaseURL
            }
            .alert("Configuration", isPresented: $showAlert) {
                Button("OK", role: .cancel) {}
            } message: {
                Text(alertMessage)
            }
        }
    }

    private func updateCustomURL() {
        let api = customApiURLText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !api.isEmpty else {
            showErrorAlert("Please enter an API URL")
            return
        }
        configuration.setCustomURL(api)
        showSuccessAlert("Custom URL updated successfully")
    }

    private func showSuccessAlert(_ message: String) {
        alertMessage = message
        showAlert = true
    }

    private func showErrorAlert(_ message: String) {
        alertMessage = message
        showAlert = true
    }
}

#Preview {
    APIConfigurationView()
}

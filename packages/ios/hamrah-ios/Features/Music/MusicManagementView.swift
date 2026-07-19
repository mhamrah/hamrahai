import SwiftUI

struct MusicManagementView: View {
    @Environment(\.openURL) private var openURL
    private let api = MusicSyncAPI()
    @State private var connections: [MusicConnectionDTO] = []
    @State private var imports: [MusicImportDTO] = []
    @State private var unmatched: [MusicUnmatchedTrackDTO] = []
    @State private var showingUnmatched = false
    @State private var includeSavedPlaylists = false
    @State private var includeSavedTracks = false
    @State private var isLoading = false
    @State private var error: String?

    private var latest: MusicImportDTO? { imports.first }
    private var ready: Bool { ["spotify", "tidal"].allSatisfy { provider in connections.contains { $0.provider == provider && $0.status == "connected" } } }

    var body: some View {
        List {
            Section("Connected services") {
                ForEach(["spotify", "tidal"], id: \.self) { provider in
                    HStack {
                        Label(provider == "spotify" ? "Spotify" : "TIDAL", systemImage: provider == "spotify" ? "music.note" : "waveform")
                        Spacer()
                        if let connection = connections.first(where: { $0.provider == provider && $0.status == "connected" }) {
                            VStack(alignment: .trailing) {
                                Text("Connected").foregroundStyle(.green)
                                if let name = connection.provider_account_name { Text(name).font(.caption).foregroundStyle(.secondary) }
                            }
                            Menu("Manage") {
                                Button("Change Account") { Task { await connect(provider) } }
                                Button("Disconnect", role: .destructive) { Task { await disconnect(provider) } }
                            }
                        } else {
                            Button("Connect") { Task { await connect(provider) } }
                        }
                    }
                }
            }
            Section("Music sync") {
                Text("Syncs owned playlists in both directions using exact ISRC matches. Liked Songs added in TIDAL are saved to Spotify; removing music in TIDAL never removes it from Spotify.")
                    .font(.footnote).foregroundStyle(.secondary)
                Toggle("Include saved Spotify playlists", isOn: $includeSavedPlaylists).disabled(latest?.isActive == true)
                Toggle("Sync Liked Songs", isOn: $includeSavedTracks).disabled(latest?.isActive == true)
                Button(actionTitle) { Task { await startOrRestart() } }
                    .disabled(isLoading || latest?.isActive == true || !ready)
            }
            if let latest { resultSection(latest) }
        }
        .navigationTitle("Music")
        .task { await refresh() }
        .task(id: latest?.isActive == true) {
            while !Task.isCancelled && latest?.isActive == true { try? await Task.sleep(for: .seconds(2)); await refresh(silent: true) }
        }
        .alert("Music", isPresented: .constant(error != nil), actions: { Button("OK") { error = nil } }, message: { Text(error ?? "") })
    }

    @ViewBuilder private func resultSection(_ run: MusicImportDTO) -> some View {
        Section("Latest check") {
            Text(run.stageDescription).font(.footnote.weight(.medium))
            Text(run.resultSummary).font(.footnote).foregroundStyle(.secondary)
            if run.unmatched_items > 0 {
                Button(showingUnmatched ? "Hide unsupported songs" : "Review \(run.unmatched_items) unsupported songs") { Task { await loadUnmatched(run.id) } }
                if showingUnmatched {
                    ForEach(unmatched) { track in
                        VStack(alignment: .leading) {
                            Text(track.track_name).fontWeight(.medium)
                            if !track.detail.isEmpty { Text(track.detail).font(.caption).foregroundStyle(.secondary) }
                            Text("\(track.source_collection) · \(track.reasonDescription)").font(.caption2).foregroundStyle(.secondary)
                        }
                    }
                }
            }
            if let message = run.recoveryMessage { Text(message).font(.footnote).foregroundStyle(.secondary) }
        }
    }

    private var actionTitle: String { isLoading ? "Working…" : latest?.canRestart == true ? "Retry partial import" : "Sync now" }
    private func refresh(silent: Bool = false) async { do { async let c = api.connections(); async let i = api.imports(); connections = try await c; imports = try await i; if let latest { includeSavedPlaylists = latest.include_saved_playlists; includeSavedTracks = latest.include_saved_tracks } } catch let failure where !silent { error = failure.localizedDescription } catch {} }
    private func connect(_ provider: String) async { do { openURL(try await api.beginConnection(provider: provider)) } catch { self.error = error.localizedDescription } }
    private func disconnect(_ provider: String) async { do { try await api.disconnectConnection(provider: provider); await refresh() } catch { self.error = error.localizedDescription } }
    private func startOrRestart() async { isLoading = true; defer { isLoading = false }; do { let run = if let latest, latest.canRestart { try await api.restartImport(id: latest.id, includeSavedPlaylists: includeSavedPlaylists, includeSavedTracks: includeSavedTracks) } else { try await api.startImport(includeSavedPlaylists: includeSavedPlaylists, includeSavedTracks: includeSavedTracks) }; imports = [run] + imports.filter { $0.id != run.id } } catch { self.error = error.localizedDescription; await refresh(silent: true) } }
    private func loadUnmatched(_ id: String) async { if showingUnmatched { showingUnmatched = false; return }; do { unmatched = try await api.unmatchedTracks(importID: id); showingUnmatched = true } catch { self.error = error.localizedDescription } }
}

//
//  DebugLogsView.swift
//  hamrahIOS
//
//  Debug logs viewer for troubleshooting
//

import SwiftUI

struct DebugLogsView: View {
    @Environment(\.dismiss) private var dismiss
    @State private var logContent = ""
    @State private var isLoading = true
    @State private var showShareSheet = false
    
    private var toolbarMenuPlacement: ToolbarItemPlacement {
        #if os(iOS)
        return .navigationBarTrailing
        #else
        return .primaryAction
        #endif
    }
    
    private var toolbarDoneButtonPlacement: ToolbarItemPlacement {
        #if os(iOS)
        return .navigationBarLeading
        #else
        return .cancellationAction
        #endif
    }
    
    var body: some View {
        NavigationView {
            VStack {
                if isLoading {
                    ProgressView("Loading logs...")
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else if logContent.isEmpty {
                    VStack(spacing: 16) {
                        Image(systemName: "doc.text")
                            .font(.largeTitle)
                            .foregroundColor(.gray)
                        Text("No debug logs found")
                            .font(.headline)
                            .foregroundColor(.gray)
                        Text("Debug logs will appear here when authentication events occur")
                            .font(.caption)
                            .foregroundColor(.secondary)
                            .multilineTextAlignment(.center)
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else {
                    ScrollView {
                        Text(logContent)
                            .font(.system(.caption, design: .monospaced))
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding()
                    }
                }
            }
            .navigationTitle("Debug Logs")
            #if os(iOS)
            .navigationBarTitleDisplayMode(.inline)
            #endif
            .toolbar {
                ToolbarItem(placement: toolbarMenuPlacement) {
                    Menu {
                        Button(action: loadLogs) {
                            Label("Refresh", systemImage: "arrow.clockwise")
                        }
                        
                        Button(action: { showShareSheet = true }) {
                            Label("Share Logs", systemImage: "square.and.arrow.up")
                        }
                        .disabled(logContent.isEmpty)
                        
                        Button(action: clearLogs) {
                            Label("Clear Logs", systemImage: "trash")
                        }
                        .disabled(logContent.isEmpty)
                    } label: {
                        Image(systemName: "ellipsis.circle")
                    }
                }
                
                ToolbarItem(placement: toolbarDoneButtonPlacement) {
                    Button("Done") {
                        dismiss()
                    }
                }
            }
            .onAppear {
                loadLogs()
            }
            .sheet(isPresented: $showShareSheet) {
                if let logFileURL = DebugLogger.shared.getLogFileURL() {
                    NavigationView {
                        VStack(spacing: 20) {
                            Text("Share Debug Logs")
                                .font(.headline)

                            ShareLink(item: logFileURL) {
                                Label("Share Log File", systemImage: Theme.Icons.share)
                                    .frame(maxWidth: .infinity)
                                    .padding()
                                    .background(Theme.Colors.primary)
                                    .foregroundColor(.white)
                                    .cornerRadius(Theme.CornerRadius.button)
                            }

                            Spacer()
                        }
                        .padding()
                        .navigationTitle("Share Logs")
                        #if os(iOS)
                        .navigationBarTitleDisplayMode(.inline)
                        .toolbar {
                            ToolbarItem(placement: .navigationBarTrailing) {
                                Button("Done") {
                                    showShareSheet = false
                                }
                            }
                        }
                        #endif
                    }
                }
            }
        }
    }
    
    private func loadLogs() {
        isLoading = true
        
        DispatchQueue.global(qos: .userInitiated).async {
            let content: String
            
            if let logFileURL = DebugLogger.shared.getLogFileURL(),
               let data = try? Data(contentsOf: logFileURL),
               let logText = String(data: data, encoding: .utf8) {
                content = logText
            } else {
                content = ""
            }
            
            DispatchQueue.main.async {
                self.logContent = content
                self.isLoading = false
            }
        }
    }
    
    private func clearLogs() {
        DebugLogger.shared.clearLogs()
        logContent = ""
    }
}


#Preview {
    DebugLogsView()
}

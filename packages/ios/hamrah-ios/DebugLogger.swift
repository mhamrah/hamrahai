//
//  DebugLogger.swift
//  hamrahIOS
//
//  Debug logging utility for production debugging
//

import Foundation
import os.log

class DebugLogger {
    static let shared = DebugLogger()
    private let logger = Logger(subsystem: "com.hamrah.ios", category: "Authentication")
    
    private init() {}
    
    func log(_ message: String, level: OSLogType = .default) {
        // Print to console for development
        print(message)
        
        // Log to system log for production debugging
        logger.log(level: level, "\(message)")
        
        // Save to file for easy access
        saveToFile(message)
    }
    
    private func saveToFile(_ message: String) {
        guard let documentsPath = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first else { return }
        
        let logFile = documentsPath.appendingPathComponent("hamrah_debug.log")
        let timestamp = DateFormatter.logFormatter.string(from: Date())
        let logEntry = "[\(timestamp)] \(message)\n"
        
        if FileManager.default.fileExists(atPath: logFile.path) {
            if let handle = try? FileHandle(forWritingTo: logFile) {
                handle.seekToEndOfFile()
                handle.write(logEntry.data(using: .utf8) ?? Data())
                handle.closeFile()
            }
        } else {
            try? logEntry.write(to: logFile, atomically: true, encoding: .utf8)
        }
    }
    
    func getLogFileURL() -> URL? {
        guard let documentsPath = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first else { return nil }
        let logFile = documentsPath.appendingPathComponent("hamrah_debug.log")
        return FileManager.default.fileExists(atPath: logFile.path) ? logFile : nil
    }
    
    func clearLogs() {
        guard let logFileURL = getLogFileURL() else { return }
        try? FileManager.default.removeItem(at: logFileURL)
    }
}

extension DateFormatter {
    static let logFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd HH:mm:ss.SSS"
        return formatter
    }()
}
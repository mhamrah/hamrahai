import Foundation

//
//  Canonicalizer.swift
//  Hamrah
//
//  Created by AI Assistant on 2024-06-09.
//  Utility for canonicalizing URLs for deduplication and sync.
//
//  This file provides a pure Swift function for URL canonicalization,
//  following the rules specified in the project requirements.
//
//  Canonicalization steps:
//   - Lowercase host
//   - Normalize scheme to https
//   - Remove default ports (80, 443)
//   - Strip fragment
//   - Remove tracking params: utm_*, gclid, fbclid, msclkid, mc_eid, ref, ref_src, igshid
//   - Remove known session params (sid, session, PHPSESSID) unless on allowlist
//   - Collapse duplicate slashes in path
//   - Trim trailing slash (except root)
//   - Return canonical URL string
//

public struct Canonicalizer {
    /// Canonicalizes a URL string according to Hamrah rules.
    /// - Parameter url: The input URL.
    /// - Returns: The canonicalized URL string, or nil if input is invalid.
    public static func canonicalize(url: URL) -> String? {
        guard var comps = URLComponents(url: url, resolvingAgainstBaseURL: false),
            let host = comps.host?.lowercased()
        else { return nil }

        // Always use https
        comps.scheme = "https"
        comps.host = host

        // Remove default ports
        if comps.port == 80 || comps.port == 443 {
            comps.port = nil
        }

        // Remove fragment
        comps.fragment = nil

        // Remove tracking and session params
        let trackingPrefixes = ["utm_"]
        let trackingNames: Set<String> = [
            "gclid", "fbclid", "msclkid", "mc_eid", "ref", "ref_src", "igshid",
        ]
        let sessionNames: Set<String> = [
            "sid", "session", "PHPSESSID",
        ]
        // Allowlist for session params (add domains as needed)
        let sessionAllowlist: Set<String> = []

        if let items = comps.queryItems, !items.isEmpty {
            comps.queryItems = items.filter { item in
                // Remove tracking params by prefix
                if trackingPrefixes.contains(where: { item.name.hasPrefix($0) }) {
                    return false
                }
                // Remove tracking params by name
                if trackingNames.contains(item.name) {
                    return false
                }
                // Remove session params unless allowlisted
                if sessionNames.contains(item.name) {
                    if sessionAllowlist.contains(host) {
                        return true
                    }
                    return false
                }
                return true
            }
            // Remove empty query
            if comps.queryItems?.isEmpty == true {
                comps.queryItems = nil
            }
        }

        // Collapse duplicate slashes in path
        var path = comps.path
        while path.contains("//") {
            path = path.replacingOccurrences(of: "//", with: "/")
        }
        // Trim trailing slash (except root)
        if path.count > 1 && path.hasSuffix("/") {
            path = String(path.dropLast())
        }
        comps.path = path

        // Compose canonical URL string
        guard var canon = comps.string else { return nil }

        // Remove trailing slash (except root)
        if canon.hasSuffix("/") && canon != "https://\(host)/" {
            canon = String(canon.dropLast())
        }

        return canon
    }
}

import Foundation
import LinkPresentation
import SwiftData
import UIKit
import UniformTypeIdentifiers
import os

final class ShareViewController: UIViewController {

    private var hasProcessedShare = false
    private let logger = Logger(subsystem: "app.hamrah.ios.share", category: "ShareExtension")

    // MARK: - UI Elements

    private var activityIndicator: UIActivityIndicatorView!
    private var statusLabel: UILabel!
    private var containerView: UIView!

    // MARK: - Lifecycle

    override func viewDidLoad() {
        super.viewDidLoad()

        setupUI()
        setupNavigationBar()

        logger.log("viewDidLoad: ShareViewController initialized")
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)

        // Process share only once and after the view is fully presented
        guard !hasProcessedShare else { return }
        hasProcessedShare = true

        logger.log("viewDidAppear: beginning share processing")
        updateStatus("Processing share...", showActivity: true)
        processShare()
    }

    // MARK: - UI Setup

    private func setupUI() {
        view.backgroundColor = .systemBackground

        // Create container view
        containerView = UIView()
        containerView.translatesAutoresizingMaskIntoConstraints = false
        containerView.backgroundColor = .systemBackground
        view.addSubview(containerView)

        // Create activity indicator
        activityIndicator = UIActivityIndicatorView(style: .large)
        activityIndicator.translatesAutoresizingMaskIntoConstraints = false
        activityIndicator.hidesWhenStopped = true
        containerView.addSubview(activityIndicator)

        // Create status label
        statusLabel = UILabel()
        statusLabel.translatesAutoresizingMaskIntoConstraints = false
        statusLabel.textAlignment = .center
        statusLabel.textColor = .secondaryLabel
        statusLabel.font = UIFont.systemFont(ofSize: 17)
        statusLabel.text = "Initializing..."
        statusLabel.numberOfLines = 0
        containerView.addSubview(statusLabel)

        // Set up constraints
        NSLayoutConstraint.activate([
            // Container view
            containerView.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor),
            containerView.trailingAnchor.constraint(
                equalTo: view.safeAreaLayoutGuide.trailingAnchor),
            containerView.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            containerView.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor),

            // Activity indicator
            activityIndicator.centerXAnchor.constraint(equalTo: containerView.centerXAnchor),
            activityIndicator.centerYAnchor.constraint(equalTo: containerView.centerYAnchor),

            // Status label
            statusLabel.topAnchor.constraint(equalTo: activityIndicator.bottomAnchor, constant: 16),
            statusLabel.leadingAnchor.constraint(
                equalTo: containerView.leadingAnchor, constant: 20),
            statusLabel.trailingAnchor.constraint(
                equalTo: containerView.trailingAnchor, constant: -20),
        ])
    }

    private func setupNavigationBar() {
        title = "Save to Hamrah"

        // Add Cancel button
        navigationItem.leftBarButtonItem = UIBarButtonItem(
            barButtonSystemItem: .cancel,
            target: self,
            action: #selector(cancelTapped)
        )

        // Add Done button (initially disabled)
        let doneButton = UIBarButtonItem(
            barButtonSystemItem: .done,
            target: self,
            action: #selector(doneTapped)
        )
        doneButton.isEnabled = false
        navigationItem.rightBarButtonItem = doneButton
    }

    // MARK: - Actions

    @objc private func cancelTapped() {
        logger.log("cancelTapped: user cancelled share")
        extensionContext?.cancelRequest(
            withError: NSError(
                domain: "ShareExtensionError",
                code: -1,
                userInfo: [NSLocalizedDescriptionKey: "User cancelled share"]
            ))
    }

    @objc private func doneTapped() {
        logger.log("doneTapped: user confirmed completion")
        completeRequest()
    }

    // MARK: - Share Processing

    private func processShare() {
        logger.log("processShare: extracting URL from shared content")

        extractFirstURL { [weak self] url in
            guard let self = self else { return }

            DispatchQueue.main.async {
                if let url = url {
                    self.logger.log(
                        "processShare: successfully extracted URL: \(url.absoluteString, privacy: .public)"
                    )
                    self.updateStatus("Saving link...", showActivity: true)
                    self.saveLink(url: url) { [weak self] success in
                        if success {
                            self?.logger.log("processShare: link saved successfully")
                            self?.updateStatus("Link saved successfully!", showActivity: false)
                            self?.navigationItem.rightBarButtonItem?.isEnabled = true
                        } else {
                            self?.logger.error("processShare: failed to save link")
                            self?.updateStatus("Failed to save link", showActivity: false)
                        }
                        // Auto-complete after a short delay
                        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
                            self?.completeRequest()
                        }
                    }
                } else {
                    self.logger.error("processShare: no URL found in shared items")
                    self.updateStatus("No URL found in shared content", showActivity: false)
                    // Auto-complete after showing error
                    DispatchQueue.main.asyncAfter(deadline: .now() + 2.0) {
                        self.completeRequest()
                    }
                }
            }
        }
    }

    private func completeRequest() {
        logger.log("completeRequest: finishing share extension")
        extensionContext?.completeRequest(returningItems: [], completionHandler: nil)
    }

    // MARK: - URL Extraction

    private func extractFirstURL(completion: @escaping (URL?) -> Void) {
        guard let inputItems = extensionContext?.inputItems as? [NSExtensionItem],
            !inputItems.isEmpty
        else {
            logger.error("extractFirstURL: no input items found")
            completion(nil)
            return
        }

        logger.log("extractFirstURL: processing \(inputItems.count) input items")

        for item in inputItems {
            guard let attachments = item.attachments, !attachments.isEmpty else { continue }

            for provider in attachments {
                // Check for LPLinkMetadata first (most reliable for URLs)
                if provider.hasItemConformingToTypeIdentifier("com.apple.linkpresentation.metadata")
                {
                    logger.log("extractFirstURL: found LPLinkMetadata provider")
                    loadLinkMetadata(from: provider, completion: completion)
                    return
                }

                // Check for direct URL
                if provider.hasItemConformingToTypeIdentifier(UTType.url.identifier) {
                    logger.log("extractFirstURL: found URL provider")
                    loadURL(from: provider, completion: completion)
                    return
                }

                // Check for plain text (might contain URL)
                if provider.hasItemConformingToTypeIdentifier(UTType.plainText.identifier) {
                    logger.log("extractFirstURL: found plain text provider")
                    loadPlainTextURL(from: provider, completion: completion)
                    return
                }
            }
        }

        logger.error("extractFirstURL: no suitable URL providers found")
        completion(nil)
    }

    private func loadLinkMetadata(
        from provider: NSItemProvider, completion: @escaping (URL?) -> Void
    ) {
        provider.loadItem(forTypeIdentifier: "com.apple.linkpresentation.metadata", options: nil) {
            [weak self] item, error in
            if let error = error {
                self?.logger.error(
                    "loadLinkMetadata: error loading metadata: \(error.localizedDescription)")
                completion(nil)
                return
            }

            guard let metadata = item as? LPLinkMetadata else {
                self?.logger.error("loadLinkMetadata: item is not LPLinkMetadata")
                completion(nil)
                return
            }

            // Prefer originalURL if available
            if let originalURL = metadata.originalURL {
                self?.logger.log("loadLinkMetadata: found originalURL")
                completion(originalURL)
                return
            }

            // Fall back to URL (iOS 16+)
            if #available(iOS 16.0, *), let url = metadata.url {
                self?.logger.log("loadLinkMetadata: found URL")
                completion(url)
                return
            }

            self?.logger.error("loadLinkMetadata: no URL found in metadata")
            completion(nil)
        }
    }

    private func loadURL(from provider: NSItemProvider, completion: @escaping (URL?) -> Void) {
        provider.loadItem(forTypeIdentifier: UTType.url.identifier, options: nil) {
            [weak self] item, error in
            if let error = error {
                self?.logger.error("loadURL: error loading URL: \(error.localizedDescription)")
                completion(nil)
                return
            }

            let url = self?.coerceToURL(item)
            if let url = url {
                self?.logger.log("loadURL: successfully loaded URL")
            } else {
                self?.logger.error("loadURL: failed to coerce item to URL")
            }
            completion(url)
        }
    }

    private func loadPlainTextURL(
        from provider: NSItemProvider, completion: @escaping (URL?) -> Void
    ) {
        provider.loadItem(forTypeIdentifier: UTType.plainText.identifier, options: nil) {
            [weak self] item, error in
            if let error = error {
                self?.logger.error(
                    "loadPlainTextURL: error loading text: \(error.localizedDescription)")
                completion(nil)
                return
            }

            guard let text = item as? String else {
                self?.logger.error("loadPlainTextURL: item is not string")
                completion(nil)
                return
            }

            let trimmedText = text.trimmingCharacters(in: .whitespacesAndNewlines)
            if let url = URL(string: trimmedText), url.scheme != nil {
                self?.logger.log("loadPlainTextURL: successfully parsed URL from text")
                completion(url)
            } else {
                self?.logger.error("loadPlainTextURL: text is not a valid URL")
                completion(nil)
            }
        }
    }

    private func coerceToURL(_ item: NSSecureCoding?) -> URL? {
        if let url = item as? URL {
            return url
        }

        if let nsurl = item as? NSURL {
            return nsurl as URL
        }

        if let string = item as? String {
            let trimmed = string.trimmingCharacters(in: .whitespacesAndNewlines)
            return URL(string: trimmed)
        }

        return nil
    }

    // MARK: - Data Persistence

    private func saveLink(url: URL, completion: @escaping (Bool) -> Void) {
        let context = ShareExtensionDataStack.mainContext

        let link = LinkEntity(
            originalUrl: url,
            canonicalUrl: url,
            title: nil,
            snippet: nil,
            sourceApp: "share-extension",
            sharedText: nil,  // No shared text in this simplified version
            sharedAt: Date(),
            status: "queued",
            attempts: 0,
            lastError: nil,
            saveCount: 1,
            lastSavedAt: nil,
            updatedAt: Date(),
            createdAt: Date(),
            summaryShort: nil,
            summaryLong: nil,
            lang: nil,
            tags: [],
            serverId: nil
        )

        context.insert(link)

        do {
            try context.save()
            logger.log("saveLink: successfully saved link to SwiftData")
            completion(true)
        } catch {
            logger.error("saveLink: failed to save link - \(error.localizedDescription)")
            completion(false)
        }
    }

    // MARK: - UI Updates

    private func updateStatus(_ message: String, showActivity: Bool) {
        DispatchQueue.main.async { [weak self] in
            self?.statusLabel.text = message
            if showActivity {
                self?.activityIndicator.startAnimating()
            } else {
                self?.activityIndicator.stopAnimating()
            }
        }
    }
}

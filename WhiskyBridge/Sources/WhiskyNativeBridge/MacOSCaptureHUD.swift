//
//  MacOSCaptureHUD.swift
//  WhiskyNativeBridge
//
//  This file is part of Whisky.
//
//  Whisky is free software: you can redistribute it and/or modify it under the terms
//  of the GNU General Public License as published by the Free Software Foundation,
//  either version 3 of the License, or (at your option) any later version.
//
//  Whisky is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY;
//  without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
//  See the GNU General Public License for more details.
//
//  You should have received a copy of the GNU General Public License along with Whisky.
//  If not, see https://www.gnu.org/licenses/.
//

// SPDX-License-Identifier: GPL-3.0-or-later
import AppKit
import WhiskyApplication
import WhiskyBridgeProtocol
import WhiskyCore

private final class CaptureHUDPanel: NSPanel {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}

private final class FlippedView: NSView {
    override var isFlipped: Bool { true }
}

@MainActor
private final class HUDActionButton: NSButton {
    var onAction: (() -> Void)?

    convenience init(
        title: String,
        background: NSColor,
        foreground: NSColor,
        fontSize: CGFloat = 11,
        weight: NSFont.Weight = .medium,
        cornerRadius: CGFloat = 6,
        horizontalPadding: CGFloat = 10,
        verticalPadding: CGFloat = 4,
        onAction: @escaping () -> Void
    ) {
        self.init(frame: .zero)
        self.onAction = onAction
        isBordered = false
        bezelStyle = .regularSquare
        setButtonType(.momentaryPushIn)
        wantsLayer = true
        layer?.backgroundColor = background.cgColor
        layer?.cornerRadius = cornerRadius
        layer?.masksToBounds = true
        attributedTitle = NSAttributedString(
            string: title,
            attributes: [
                .font: NSFont.systemFont(ofSize: fontSize, weight: weight),
                .foregroundColor: foreground
            ]
        )
        contentTintColor = foreground
        target = self
        action = #selector(handleTap)
        translatesAutoresizingMaskIntoConstraints = false
        let textSize = attributedTitle.size()
        NSLayoutConstraint.activate([
            widthAnchor.constraint(greaterThanOrEqualToConstant: ceil(textSize.width) + horizontalPadding * 2),
            heightAnchor.constraint(equalToConstant: max(22, ceil(textSize.height) + verticalPadding * 2))
        ])
    }

    @objc private func handleTap() {
        onAction?()
    }
}

@MainActor
final class MacOSCaptureHUD: NSObject, NSWindowDelegate {
    private static let expandedWidth: CGFloat = 400
    private static let maxExpandedHeight: CGFloat = 460
    private static let collapsedSize = NSSize(width: 286, height: 34)
    private static let systemExecutables: Set<String> = [
        "cmd.exe", "conhost.exe", "control.exe", "explorer.exe",
        "maltorialunahost32.exe", "maltorialunahost64.exe", "msiexec.exe",
        "plugplay.exe", "regedit.exe", "rpcss.exe", "rundll32.exe",
        "services.exe", "start.exe", "svchost.exe", "tabtip.exe",
        "taskmgr.exe", "uninstaller.exe", "wineboot.exe", "winedevice.exe",
        "winemenubuilder.exe", "wineserver.exe"
    ]

    private struct ProviderOption: Equatable {
        let id: UUID
        let name: String
    }

    private let subtitleOverlay: MacOSSubtitleOverlay
    private weak var capture: ApplicationCaptureService?
    private weak var translation: ApplicationTranslationService?
    private var workspace: ApplicationWorkspace?

    private var panel: CaptureHUDPanel?
    private var contentContainer: NSView?
    private var currentScrollView: NSScrollView?
    private var activeContainer: URL?
    private var activeEntry: EntryRecord?
    private var activePreset = CapturePreset()
    private var providers: [ProviderOption] = []
    private var selectedProviderID: UUID?
    private var selectedArchitecture = "x64"
    private var latestSnapshot: JSONValue = .object([:])

    private var isVisible = false
    private var isCollapsed = false
    private var isWaitingForLaunch = false
    private var isWorking = false
    private var rawErrorMessage: String?
    private var errorMessageID: String?
    private var strings: CapturePopupStrings { subtitleOverlay.strings }
    private var errorMessage: String? {
        get {
            if let errorMessageID { return strings.text(errorMessageID) }
            return rawErrorMessage.map { strings.errorText($0) }
        }
        set {
            errorMessageID = nil
            rawErrorMessage = newValue
        }
    }

    func localize(_ values: [String: JSONValue]) {
        strings.update(values)
        subtitleOverlay.refreshLocalization()
        panel?.title = strings.text("hud_title")
        if isVisible { rebuildUI(preservingScroll: true) }
    }

    private var lastCaptureState: String = "stopped"
    private var topRightAnchor: NSPoint?
    private var applyingFrame = false
    private var launchTask: Task<Void, Never>?
    private var actionTask: Task<Void, Never>?
    private var reassertObserver: OverlayReassertObserver?

    var panelForTesting: NSPanel? { panel }
    var isVisibleForTesting: Bool { isVisible }
    var isCollapsedForTesting: Bool { isCollapsed }
    var errorMessageForTesting: String? { errorMessage }

    init(subtitleOverlay: MacOSSubtitleOverlay) {
        self.subtitleOverlay = subtitleOverlay
        super.init()
        reassertObserver = OverlayReassertObserver { [weak self] in
            self?.reassertPanelForFullscreen()
        }
        subtitleOverlay.onSettingsChanged = { [weak self] _ in
            guard let self, self.isVisible else { return }
            self.rebuildUI(preservingScroll: true)
        }
    }

    func invalidate() {
        launchTask?.cancel()
        launchTask = nil
        actionTask?.cancel()
        actionTask = nil
        reassertObserver?.invalidate()
        reassertObserver = nil
        subtitleOverlay.onSettingsChanged = nil
    }

    func configure(
        capture: ApplicationCaptureService,
        translation: ApplicationTranslationService,
        workspace: ApplicationWorkspace
    ) {
        self.capture = capture
        self.translation = translation
        self.workspace = workspace
    }

    func handleStateUpdate(_ snapshot: JSONValue) {
        latestSnapshot = snapshot
        let previousState = lastCaptureState
        let nextState = snapshot.object?["captureState"]?.string ?? "stopped"
        lastCaptureState = nextState
        if nextState == "targetExited", previousState != "targetExited", subtitleOverlay.isVisible {
            subtitleOverlay.setVisible(false)
        }
        if let presetObj = snapshot.object?["preset"], presetObj != .null,
           let decoded = try? presetObj.decoded(CapturePreset.self) {
            activePreset = decoded
        }
        if isVisible {
            rebuildUI(preservingScroll: true)
        }
    }

    @discardableResult
    func handleRuntimeEntriesEnded(_ entryIDs: Set<String>, stoppedCapture: Bool = false) -> Bool {
        if let activeID = activeEntry?.id, entryIDs.contains(activeID) {
            if subtitleOverlay.isVisible {
                subtitleOverlay.setVisible(false)
            }
            hide()
            return true
        }
        if stoppedCapture {
            if subtitleOverlay.isVisible {
                subtitleOverlay.setVisible(false)
            }
            return true
        }
        return false
    }

    func presentOnLaunch(container: URL, entry: EntryRecord, preset: CapturePreset) {
        launchTask?.cancel()
        actionTask?.cancel()
        activeContainer = container
        activeEntry = entry
        activePreset = preset
        selectedArchitecture = ["x86", "x64"].contains(preset.architecture) ? preset.architecture : "x64"
        selectedProviderID = preset.providerID
        isCollapsed = false
        isWaitingForLaunch = true
        isWorking = false
        errorMessage = nil

        launchTask = Task { @MainActor [weak self] in
            guard let self else { return }
            await self.loadProviders()
            if let capture = self.capture {
                self.latestSnapshot = await capture.currentSnapshot()
            }
            self.showPanel()

            do {
                try await Task.sleep(for: .milliseconds(1_200))
            } catch {
                return
            }
            guard !Task.isCancelled else { return }
            self.isWaitingForLaunch = false
            await self.startCaptureForLaunch(container: container, entryID: entry.id, preset: preset)
        }
    }

    func show(params: [String: JSONValue], workspace: ApplicationWorkspace) async throws -> JSONValue {
        self.workspace = workspace
        if params["container"] != nil, params["entryID"] != nil {
            let containerURL = try workspace.resolve(params)
            let libraryEntry = try await workspace.entry(params)
            let visibility = (try? LibraryVisibility.load(bottleURL: containerURL)) ?? LibraryVisibility()
            let record = try EntryRecord(libraryEntry, visibility: visibility)
            let preset = await capture?.preset(container: containerURL, entryID: libraryEntry.id) ?? CapturePreset()
            activeContainer = containerURL
            activeEntry = record
            activePreset = preset
            selectedArchitecture = ["x86", "x64"].contains(preset.architecture) ? preset.architecture : "x64"
            selectedProviderID = preset.providerID
        }
        errorMessage = nil
        isCollapsed = false
        await loadProviders()
        if let capture {
            latestSnapshot = await capture.currentSnapshot()
        }
        showPanel()
        return .object([
            "visible": .bool(true),
            "collapsed": .bool(isCollapsed)
        ])
    }

    func hide() {
        launchTask?.cancel()
        launchTask = nil
        actionTask?.cancel()
        actionTask = nil
        isVisible = false
        isWaitingForLaunch = false
        isWorking = false
        panel?.orderOut(nil)
    }

    // MARK: - Capture actions

    private func startCaptureForLaunch(container: URL, entryID: String, preset: CapturePreset) async {
        guard let capture, let workspace else {
            errorMessageID = "capture_unavailable"
            rebuildUI(preservingScroll: false)
            return
        }
        guard let resolvedProviderID = resolveLaunchProviderID(preset: preset) else {
            errorMessageID = providers.isEmpty ? "no_provider" : "pick_provider"
            rebuildUI(preservingScroll: false)
            return
        }
        selectedProviderID = resolvedProviderID
        await performStart(
            capture: capture,
            workspace: workspace,
            container: container,
            entryID: entryID,
            providerID: resolvedProviderID,
            architecture: selectedArchitecture,
            restoreHistory: preset.restoreHistory,
            autoAttach: preset.autoAttach
        )
    }

    private func resolveLaunchProviderID(preset: CapturePreset) -> UUID? {
        if let presetID = preset.providerID {
            if providers.isEmpty || providers.contains(where: { $0.id == presetID }) {
                return presetID
            }
        }
        if providers.count == 1 {
            return providers[0].id
        }
        return nil
    }

    private func performStart(
        capture: ApplicationCaptureService,
        workspace: ApplicationWorkspace,
        container: URL,
        entryID: String,
        providerID: UUID,
        architecture: String,
        restoreHistory: Bool,
        autoAttach: Bool
    ) async {
        isWorking = true
        errorMessage = nil
        rebuildUI(preservingScroll: false)
        defer { isWorking = false }

        do {
            let result = try await capture.call(
                "translation.capture.start",
                params: [
                    "container": .string(container.path),
                    "entryID": .string(entryID),
                    "providerID": .string(providerID.uuidString),
                    "architecture": .string(architecture),
                    "restoreHistory": .bool(restoreHistory),
                    "autoAttach": .bool(autoAttach)
                ],
                workspace: workspace
            )
            latestSnapshot = result
            if result.object?["captureState"]?.string == "attached", !subtitleOverlay.isVisible {
                subtitleOverlay.setVisible(true)
            }
        } catch {
            applyCaptureError(error)
        }
        rebuildUI(preservingScroll: false)
    }

    private func triggerManualStart(autoAttach: Bool) {
        guard let capture, let workspace, let container = activeContainer, let entry = activeEntry else {
            errorMessageID = "pick_game"
            rebuildUI(preservingScroll: false)
            return
        }
        guard let providerID = selectedProviderID ?? (providers.count == 1 ? providers[0].id : nil) else {
            errorMessageID = "no_provider"
            rebuildUI(preservingScroll: false)
            return
        }
        runAction { [weak self] in
            guard let self else { return }
            await self.performStart(
                capture: capture,
                workspace: workspace,
                container: container,
                entryID: entry.id,
                providerID: providerID,
                architecture: self.selectedArchitecture,
                restoreHistory: self.activePreset.restoreHistory,
                autoAttach: autoAttach
            )
        }
    }

    private func triggerRefresh() {
        runAction { [weak self] in
            guard let self else { return }
            await self.loadProviders()
            guard let capture = self.capture, let workspace = self.workspace else { return }
            let state = self.currentCaptureState()
            if state == "selecting" {
                do {
                    _ = try await capture.call("translation.capture.processes", params: [:], workspace: workspace)
                } catch {
                    self.applyCaptureError(error)
                }
            }
            self.latestSnapshot = await capture.currentSnapshot()
            self.rebuildUI(preservingScroll: true)
        }
    }

    private func triggerAutoAttach() {
        guard let capture, let workspace else { return }
        runAction { [weak self] in
            guard let self else { return }
            self.isWorking = true
            self.errorMessage = nil
            self.rebuildUI(preservingScroll: false)
            defer { self.isWorking = false }
            do {
                let response = try await capture.call(
                    "translation.capture.autoAttach",
                    params: [:],
                    workspace: workspace
                )
                if let snap = response.object?["snapshot"] {
                    self.latestSnapshot = snap
                } else {
                    self.latestSnapshot = await capture.currentSnapshot()
                }
                if response.object?["attached"]?.bool == true, !self.subtitleOverlay.isVisible {
                    self.subtitleOverlay.setVisible(true)
                } else if response.object?["attached"]?.bool == false {
                    self.errorMessageID = "preset_target_missing"
                }
            } catch {
                self.applyCaptureError(error)
            }
            self.rebuildUI(preservingScroll: false)
        }
    }

    private func triggerAttach(target: JSONValue) {
        guard let capture, let workspace else { return }
        runAction { [weak self] in
            guard let self else { return }
            self.isWorking = true
            self.errorMessage = nil
            self.rebuildUI(preservingScroll: false)
            defer { self.isWorking = false }
            do {
                let result = try await capture.call(
                    "translation.capture.attach",
                    params: ["target": target],
                    workspace: workspace
                )
                self.latestSnapshot = result
                if result.object?["captureState"]?.string == "attached", !self.subtitleOverlay.isVisible {
                    self.subtitleOverlay.setVisible(true)
                }
            } catch {
                self.applyCaptureError(error)
            }
            self.rebuildUI(preservingScroll: false)
        }
    }

    private func triggerDetach() {
        guard let capture, let workspace else { return }
        runAction { [weak self] in
            guard let self else { return }
            self.errorMessage = nil
            do {
                self.latestSnapshot = try await capture.call(
                    "translation.capture.detach",
                    params: [:],
                    workspace: workspace
                )
                if self.subtitleOverlay.isVisible {
                    self.subtitleOverlay.setVisible(false)
                }
            } catch {
                self.applyCaptureError(error)
            }
            self.rebuildUI(preservingScroll: false)
        }
    }

    private func triggerSelectThread(threadID: String, role: String?) {
        guard let capture, let workspace else { return }
        var nextRoles = currentThreadRoles()
        if let role {
            if role == "body" || role == "speaker" {
                nextRoles = nextRoles.filter { $0.value != role }
            }
            nextRoles[threadID] = role
        } else {
            nextRoles.removeValue(forKey: threadID)
        }
        let selections: [JSONValue] = nextRoles.map { key, value in
            .object(["threadID": .string(key), "role": .string(value)])
        }
        runAction { [weak self] in
            guard let self else { return }
            do {
                _ = try await capture.call(
                    "translation.threads.select",
                    params: ["selections": .array(selections)],
                    workspace: workspace
                )
                self.latestSnapshot = await capture.currentSnapshot()
            } catch {
                self.applyCaptureError(error)
            }
            self.rebuildUI(preservingScroll: true)
        }
    }

    private func saveQuickPresetSettings() {
        guard let capture, let workspace, let container = activeContainer, let entry = activeEntry else { return }
        var params: [String: JSONValue] = [
            "container": .string(container.path),
            "entryID": .string(entry.id),
            "architecture": .string(selectedArchitecture)
        ]
        if let selectedProviderID {
            params["providerID"] = .string(selectedProviderID.uuidString)
        }
        runAction { [weak self] in
            guard let self else { return }
            if let result = try? await capture.call(
                "translation.capture.preset/save",
                params: params,
                workspace: workspace
            ),
                let presetValue = result.object?["preset"],
                let decoded = try? presetValue.decoded(CapturePreset.self) {
                self.activePreset = decoded
            }
        }
    }

    private func runAction(_ operation: @escaping @MainActor () async -> Void) {
        actionTask?.cancel()
        actionTask = Task { @MainActor in
            await operation()
        }
    }

    private func loadProviders() async {
        guard let translation else { return }
        guard let result = try? await translation.call("translation.providers/list", params: [:]),
              let list = result.array
        else { return }
        providers = list.compactMap { item in
            guard let idString = item.object?["id"]?.string,
                  let uuid = UUID(uuidString: idString)
            else { return nil }
            let rawName = item.object?["name"]?.string?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            let model = item.object?["modelID"]?.string ?? ""
            let display = !rawName.isEmpty ? rawName : (!model.isEmpty ? model : uuid.uuidString)
            return ProviderOption(id: uuid, name: display)
        }
        if let selectedProviderID, providers.contains(where: { $0.id == selectedProviderID }) {
            return
        }
        if let presetID = activePreset.providerID, providers.contains(where: { $0.id == presetID }) {
            selectedProviderID = presetID
        } else if providers.count == 1 {
            selectedProviderID = providers[0].id
        } else {
            selectedProviderID = providers.first?.id
        }
    }

    private func applyCaptureError(_ error: any Error) {
        if let failure = error as? BridgeFailure {
            if failure.code == "provider_not_found" || failure.code == "invalid_provider" {
                errorMessageID = "no_provider"
            } else {
                errorMessage = strings.errorText(failure.code) == failure.code ? failure.message : failure.code
            }
        } else {
            errorMessage = error.localizedDescription
        }
    }

    // MARK: - Window & Workspace Management

    private func makePanel() -> CaptureHUDPanel {
        let panel = CaptureHUDPanel(
            contentRect: NSRect(origin: .zero, size: NSSize(width: Self.expandedWidth, height: 240)),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        FullscreenOverlaySupport.configurePanel(panel, title: strings.text("hud_title"), delegate: self)

        let root = NSView()
        root.wantsLayer = true
        root.layer?.backgroundColor = NSColor(calibratedWhite: 0.10, alpha: 0.92).cgColor
        root.layer?.borderColor = NSColor.white.withAlphaComponent(0.14).cgColor
        root.layer?.borderWidth = 1
        root.layer?.cornerRadius = 12
        root.layer?.masksToBounds = true
        panel.contentView = root
        contentContainer = root
        return panel
    }

    private func showPanel() {
        isVisible = true
        if panel == nil {
            panel = makePanel()
        }
        rebuildUI(preservingScroll: false)
        reassertPanelForFullscreen()
    }

    private func reassertPanelForFullscreen() {
        guard isVisible else { return }
        if panel == nil {
            panel = makePanel()
        }
        guard let panel else { return }
        applyingFrame = true
        defer { applyingFrame = false }
        if !NSScreen.screens.isEmpty,
           !NSScreen.screens.contains(where: { $0.visibleFrame.intersects(panel.frame) }) {
            topRightAnchor = nil
        }
        applyPanelSize(panel.frame.size)
    }

    private func applyPanelSize(_ size: NSSize) {
        guard let panel else { return }
        let screen = panel.screen ?? NSScreen.main ?? NSScreen.screens.first
        let bounds = screen?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1_440, height: 900)
        let clampedWidth = min(size.width, max(150, bounds.width - 24))
        let clampedHeight = min(size.height, max(34, bounds.height - 24))
        let anchor = topRightAnchor ?? NSPoint(x: bounds.maxX - 20, y: bounds.maxY - 20)
        var origin = NSPoint(x: anchor.x - clampedWidth, y: anchor.y - clampedHeight)
        origin.x = max(bounds.minX + 12, min(origin.x, bounds.maxX - clampedWidth - 12))
        origin.y = max(bounds.minY + 12, min(origin.y, bounds.maxY - clampedHeight - 12))

        applyingFrame = true
        defer { applyingFrame = false }
        panel.setFrame(NSRect(origin: origin, size: NSSize(width: clampedWidth, height: clampedHeight)), display: true)
        FullscreenOverlaySupport.applyOrderAndLevel(panel)
    }

    func windowDidMove(_ notification: Notification) {
        guard !applyingFrame, let frame = panel?.frame else { return }
        topRightAnchor = NSPoint(x: frame.maxX, y: frame.maxY)
    }

    // MARK: - UI Construction

    private func rebuildUI(preservingScroll: Bool) {
        guard let root = contentContainer else { return }
        let previousOffset = preservingScroll ? currentScrollView?.contentView.bounds.origin : nil
        currentScrollView = nil
        root.subviews.forEach { $0.removeFromSuperview() }

        if isCollapsed {
            buildCollapsedUI(in: root)
            root.layer?.cornerRadius = Self.collapsedSize.height / 2
            applyPanelSize(Self.collapsedSize)
            return
        }

        root.layer?.cornerRadius = 12
        let height = buildExpandedUI(in: root)
        applyPanelSize(NSSize(width: Self.expandedWidth, height: height))

        if let previousOffset, let scrollView = currentScrollView,
           let documentView = scrollView.documentView {
            let maxY = max(0, documentView.frame.height - scrollView.contentView.bounds.height)
            let clampedY = min(max(0, previousOffset.y), maxY)
            scrollView.contentView.scroll(to: NSPoint(x: 0, y: clampedY))
            scrollView.reflectScrolledClipView(scrollView.contentView)
        }
    }

    private func buildCollapsedUI(in root: NSView) {
        let attached = currentCaptureState() == "attached"
        let title = "🌐 " + strings.text(attached ? "attached" : "standby")
        let pillLabel = makeLabel(title, size: 11, weight: .semibold, color: .white, lines: 1)
        pillLabel.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        let subtitleButton = makeSubtitleToggleButton(
            cornerRadius: 9,
            horizontalPadding: 6,
            verticalPadding: 2
        )
        let dragButton = makeOverlayDragToggleButton(
            cornerRadius: 9,
            horizontalPadding: 6,
            verticalPadding: 2
        )
        let expandButton = HUDActionButton(
            title: strings.text("expand"),
            background: NSColor.white.withAlphaComponent(0.14),
            foreground: .white,
            fontSize: 11,
            cornerRadius: 9,
            horizontalPadding: 7,
            verticalPadding: 2
        ) { [weak self] in
            self?.isCollapsed = false
            self?.rebuildUI(preservingScroll: false)
        }
        expandButton.setContentCompressionResistancePriority(.required, for: .horizontal)
        let closeButton = makeCloseButton()
        closeButton.setContentCompressionResistancePriority(.required, for: .horizontal)

        let row = NSStackView(views: [pillLabel, subtitleButton, dragButton, expandButton, closeButton])
        row.orientation = .horizontal
        row.alignment = .centerY
        row.spacing = 4
        row.translatesAutoresizingMaskIntoConstraints = false
        root.addSubview(row)
        NSLayoutConstraint.activate([
            row.leadingAnchor.constraint(equalTo: root.leadingAnchor, constant: 10),
            row.trailingAnchor.constraint(equalTo: root.trailingAnchor, constant: -8),
            row.centerYAnchor.constraint(equalTo: root.centerYAnchor)
        ])
    }

    private func buildExpandedUI(in root: NSView) -> CGFloat {
        let stack = NSStackView()
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 10
        stack.translatesAutoresizingMaskIntoConstraints = false
        root.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: root.leadingAnchor, constant: 14),
            stack.trailingAnchor.constraint(equalTo: root.trailingAnchor, constant: -14),
            stack.topAnchor.constraint(equalTo: root.topAnchor, constant: 12),
            stack.bottomAnchor.constraint(lessThanOrEqualTo: root.bottomAnchor, constant: -12)
        ])

        let header = buildHeaderRow()
        stack.addArrangedSubview(header)
        header.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true

        let overlayBar = buildOverlayQuickBar()
        stack.addArrangedSubview(overlayBar)
        overlayBar.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true

        if !providers.isEmpty {
            let settingsRow = buildQuickSettingsRow()
            stack.addArrangedSubview(settingsRow)
            settingsRow.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
        }

        let divider = NSView()
        divider.wantsLayer = true
        divider.layer?.backgroundColor = NSColor.white.withAlphaComponent(0.10).cgColor
        divider.translatesAutoresizingMaskIntoConstraints = false
        divider.heightAnchor.constraint(equalToConstant: 1).isActive = true
        stack.addArrangedSubview(divider)
        divider.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true

        let bodyView = buildBodySection()
        stack.addArrangedSubview(bodyView)
        bodyView.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true

        root.layoutSubtreeIfNeeded()
        let fittingHeight = stack.fittingSize.height + 24
        return min(Self.maxExpandedHeight, max(168, fittingHeight))
    }

    private func buildOverlayQuickBar() -> NSView {
        let label = makeLabel(strings.text("subtitle_overlay"), size: 11, weight: .medium, color: .lightGray, lines: 1)
        label.setContentHuggingPriority(.defaultLow, for: .horizontal)
        let subtitleButton = makeSubtitleToggleButton()
        let dragButton = makeOverlayDragToggleButton()

        let controls = NSStackView(views: [subtitleButton, dragButton])
        controls.orientation = .horizontal
        controls.alignment = .centerY
        controls.spacing = 6

        let row = NSStackView(views: [label, controls])
        row.orientation = .horizontal
        row.alignment = .centerY
        row.distribution = .fill
        row.spacing = 8
        return row
    }

    private func makeSubtitleToggleButton(
        cornerRadius: CGFloat = 6,
        horizontalPadding: CGFloat = 8,
        verticalPadding: CGFloat = 3
    ) -> HUDActionButton {
        let subtitleOn = subtitleOverlay.isVisible
        let button = HUDActionButton(
            title: strings.text(subtitleOn ? "sub_on" : "sub_off"),
            background: subtitleOn
                ? NSColor.systemGreen.withAlphaComponent(0.30)
                : NSColor.white.withAlphaComponent(0.12),
            foreground: subtitleOn
                ? NSColor(calibratedRed: 0.65, green: 0.96, blue: 0.74, alpha: 1)
                : .lightGray,
            fontSize: 11,
            cornerRadius: cornerRadius,
            horizontalPadding: horizontalPadding,
            verticalPadding: verticalPadding
        ) { [weak self] in
            guard let self else { return }
            self.subtitleOverlay.setVisible(!self.subtitleOverlay.isVisible)
        }
        button.setContentCompressionResistancePriority(.required, for: .horizontal)
        return button
    }

    private func makeOverlayDragToggleButton(
        cornerRadius: CGFloat = 6,
        horizontalPadding: CGFloat = 8,
        verticalPadding: CGFloat = 3
    ) -> HUDActionButton {
        let unlocked = !subtitleOverlay.clickThrough
        let button = HUDActionButton(
            title: strings.text(unlocked ? "draggable" : "locked"),
            background: unlocked
                ? NSColor.systemBlue.withAlphaComponent(0.36)
                : NSColor.white.withAlphaComponent(0.12),
            foreground: unlocked
                ? NSColor(calibratedRed: 0.68, green: 0.86, blue: 1, alpha: 1)
                : .lightGray,
            fontSize: 11,
            cornerRadius: cornerRadius,
            horizontalPadding: horizontalPadding,
            verticalPadding: verticalPadding
        ) { [weak self] in
            guard let self else { return }
            self.subtitleOverlay.setClickThrough(!self.subtitleOverlay.clickThrough)
        }
        button.setContentCompressionResistancePriority(.required, for: .horizontal)
        return button
    }

    private func buildHeaderRow() -> NSView {
        let gameTitle = activeEntry?.displayName ?? strings.text("hud_title")
        let titleLabel = makeLabel(gameTitle, size: 13, weight: .semibold, color: .white, lines: 1)
        titleLabel.lineBreakMode = .byTruncatingTail
        titleLabel.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        let badge = makeStatusBadge()
        let titleStack = NSStackView(views: [titleLabel, badge])
        titleStack.orientation = .vertical
        titleStack.alignment = .leading
        titleStack.spacing = 3
        titleStack.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        let refreshButton = HUDActionButton(
            title: strings.text("refresh"),
            background: NSColor.white.withAlphaComponent(0.10),
            foreground: .white,
            fontSize: 11,
            cornerRadius: 6,
            horizontalPadding: 8,
            verticalPadding: 3
        ) { [weak self] in
            self?.triggerRefresh()
        }

        let collapseButton = HUDActionButton(
            title: strings.text("collapse"),
            background: NSColor.white.withAlphaComponent(0.10),
            foreground: .white,
            fontSize: 11,
            cornerRadius: 9,
            horizontalPadding: 8,
            verticalPadding: 3
        ) { [weak self] in
            self?.isCollapsed = true
            self?.rebuildUI(preservingScroll: false)
        }

        let closeButton = makeCloseButton()

        let controls = NSStackView(views: [refreshButton, collapseButton, closeButton])
        controls.orientation = .horizontal
        controls.alignment = .centerY
        controls.spacing = 5

        let row = NSStackView(views: [titleStack, controls])
        row.orientation = .horizontal
        row.alignment = .centerY
        row.distribution = .fill
        row.spacing = 8
        return row
    }

    private func makeStatusBadge() -> NSView {
        let (text, backgroundColor, foregroundColor) = statusAppearance()
        let container = NSView()
        container.wantsLayer = true
        container.layer?.backgroundColor = backgroundColor.cgColor
        container.layer?.cornerRadius = 5
        container.translatesAutoresizingMaskIntoConstraints = false
        let label = makeLabel(text, size: 10, weight: .medium, color: foregroundColor, lines: 1)
        label.lineBreakMode = .byTruncatingTail
        label.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(label)
        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 6),
            label.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -6),
            label.topAnchor.constraint(equalTo: container.topAnchor, constant: 2),
            label.bottomAnchor.constraint(equalTo: container.bottomAnchor, constant: -2),
            container.widthAnchor.constraint(lessThanOrEqualToConstant: 230)
        ])
        return container
    }

    // swiftlint:disable:next large_tuple
    private func statusAppearance() -> (String, NSColor, NSColor) {
        if isWaitingForLaunch {
            return (
                strings.text("waiting_game"),
                NSColor.systemBlue.withAlphaComponent(0.24),
                NSColor(calibratedRed: 0.65, green: 0.85, blue: 1, alpha: 1)
            )
        }
        if errorMessage != nil || currentCaptureState() == "error" {
            return (
                strings.text("error"),
                NSColor.systemRed.withAlphaComponent(0.25),
                NSColor(calibratedRed: 1, green: 0.62, blue: 0.62, alpha: 1)
            )
        }
        switch currentCaptureState() {
        case "selecting":
            return (
                strings.text("pick_process"),
                NSColor.systemOrange.withAlphaComponent(0.24),
                NSColor(calibratedRed: 1, green: 0.82, blue: 0.52, alpha: 1)
            )
        case "connecting", "attaching":
            return (
                strings.text("connecting"),
                NSColor.systemBlue.withAlphaComponent(0.24),
                NSColor(calibratedRed: 0.65, green: 0.85, blue: 1, alpha: 1)
            )
        case "attached":
            let exe = windowsBasename(latestSnapshot.object?["target"]?.object?["exePath"]?.string ?? "")
            let label = exe.isEmpty ? strings.text("attached") : strings.text("attached_target", ["exe": exe])
            return (
                label,
                NSColor.systemGreen.withAlphaComponent(0.24),
                NSColor(calibratedRed: 0.58, green: 0.94, blue: 0.68, alpha: 1)
            )
        default:
            return (strings.text("standby_scan"), NSColor.white.withAlphaComponent(0.10), NSColor.lightGray)
        }
    }

    private func buildQuickSettingsRow() -> NSView {
        let popup = NSPopUpButton(frame: .zero, pullsDown: false)
        popup.controlSize = .small
        popup.font = .systemFont(ofSize: 11)
        popup.target = self
        popup.action = #selector(providerChanged(_:))
        for option in providers {
            popup.addItem(withTitle: option.name)
            popup.lastItem?.representedObject = option.id
        }
        if let selectedProviderID,
           let index = providers.firstIndex(where: { $0.id == selectedProviderID }) {
            popup.selectItem(at: index)
        }
        popup.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        let archTitle = strings.text("architecture_label", ["arch": selectedArchitecture])
        let archButton = HUDActionButton(
            title: archTitle,
            background: NSColor.white.withAlphaComponent(0.12),
            foreground: .white,
            fontSize: 11,
            cornerRadius: 6,
            horizontalPadding: 8,
            verticalPadding: 3
        ) { [weak self] in
            guard let self else { return }
            self.selectedArchitecture = self.selectedArchitecture == "x64" ? "x86" : "x64"
            self.saveQuickPresetSettings()
            self.rebuildUI(preservingScroll: false)
        }

        let row = NSStackView(views: [popup, archButton])
        row.orientation = .horizontal
        row.alignment = .centerY
        row.spacing = 8
        return row
    }

    @objc private func providerChanged(_ sender: NSPopUpButton) {
        guard let uuid = sender.selectedItem?.representedObject as? UUID else { return }
        selectedProviderID = uuid
        saveQuickPresetSettings()
    }

    private func buildBodySection() -> NSView {
        let state = currentCaptureState()
        if isWaitingForLaunch {
            return makeInfoCard(
                message: strings.text("waiting_launch_detail"),
                color: NSColor(calibratedWhite: 0.85, alpha: 1)
            )
        }
        if state == "connecting" || state == "attaching" {
            let msg = strings.text(state == "attaching" ? "attaching_detail" : "starting_helper_detail")
            return makeInfoCard(message: msg, color: NSColor(calibratedWhite: 0.85, alpha: 1))
        }
        if state == "selecting" {
            return buildSelectingBody()
        }
        if state == "attached" {
            return buildAttachedBody()
        }
        return buildIdleBody()
    }

    private func buildIdleBody() -> NSView {
        let container = NSStackView()
        container.orientation = .vertical
        container.alignment = .leading
        container.spacing = 8

        let effectiveError = errorMessage ?? latestSnapshot.object?["lastError"]?.string
        if let effectiveError, !effectiveError.isEmpty {
            let errorCard = makeInfoCard(
                message: strings.errorText(effectiveError),
                color: NSColor(calibratedRed: 1, green: 0.65, blue: 0.65, alpha: 1),
                background: NSColor.systemRed.withAlphaComponent(0.18)
            )
            container.addArrangedSubview(errorCard)
            errorCard.widthAnchor.constraint(equalTo: container.widthAnchor).isActive = true
        } else if providers.isEmpty {
            let warnCard = makeInfoCard(
                message: strings.text("no_provider"),
                color: NSColor(calibratedRed: 1, green: 0.84, blue: 0.55, alpha: 1),
                background: NSColor.systemOrange.withAlphaComponent(0.18)
            )
            container.addArrangedSubview(warnCard)
            warnCard.widthAnchor.constraint(equalTo: container.widthAnchor).isActive = true
        }

        let startButton = HUDActionButton(
            title: strings.text(isWorking ? "starting_label" : "start_scan"),
            background: NSColor.systemBlue.withAlphaComponent(0.75),
            foreground: .white,
            fontSize: 12,
            weight: .semibold,
            cornerRadius: 7,
            horizontalPadding: 12,
            verticalPadding: 5
        ) { [weak self] in
            self?.triggerManualStart(autoAttach: false)
        }
        startButton.isEnabled = !isWorking

        var buttons: [NSView] = [startButton]
        if let targetPath = activePreset.targetExePath, !targetPath.isEmpty {
            let autoButton = HUDActionButton(
                title: strings.text("preset_attach_label"),
                background: NSColor.white.withAlphaComponent(0.14),
                foreground: .white,
                fontSize: 12,
                cornerRadius: 7,
                horizontalPadding: 10,
                verticalPadding: 5
            ) { [weak self] in
                self?.triggerManualStart(autoAttach: true)
            }
            autoButton.isEnabled = !isWorking
            buttons.append(autoButton)
        }

        let actionRow = NSStackView(views: buttons)
        actionRow.orientation = .horizontal
        actionRow.spacing = 8
        container.addArrangedSubview(actionRow)
        return container
    }

    private func buildSelectingBody() -> NSView {
        let container = NSStackView()
        container.orientation = .vertical
        container.alignment = .leading
        container.spacing = 8

        if let errorMessage, !errorMessage.isEmpty {
            let errorCard = makeInfoCard(
                message: errorMessage,
                color: NSColor(calibratedRed: 1, green: 0.72, blue: 0.55, alpha: 1),
                background: NSColor.systemOrange.withAlphaComponent(0.18)
            )
            container.addArrangedSubview(errorCard)
            errorCard.widthAnchor.constraint(equalTo: container.widthAnchor).isActive = true
        }

        if let savedTarget = activePreset.targetExePath, !savedTarget.isEmpty {
            let basename = windowsBasename(savedTarget)
            let autoBanner = HUDActionButton(
                title: strings.text("preset_attach_target", ["exe": basename.isEmpty ? savedTarget : basename]),
                background: NSColor.systemBlue.withAlphaComponent(0.55),
                foreground: .white,
                fontSize: 11,
                weight: .semibold,
                cornerRadius: 7,
                horizontalPadding: 10,
                verticalPadding: 5
            ) { [weak self] in
                self?.triggerAutoAttach()
            }
            autoBanner.isEnabled = !isWorking
            container.addArrangedSubview(autoBanner)
            autoBanner.widthAnchor.constraint(equalTo: container.widthAnchor).isActive = true
        }

        let processes = sortedCandidateProcesses()
        if processes.isEmpty {
            let emptyCard = makeInfoCard(
                message: strings.text("process_empty_detail"),
                color: NSColor(calibratedWhite: 0.78, alpha: 1)
            )
            container.addArrangedSubview(emptyCard)
            emptyCard.widthAnchor.constraint(equalTo: container.widthAnchor).isActive = true
            return container
        }

        let listStack = makeListStack()
        for process in processes {
            let row = buildProcessRow(process)
            listStack.addArrangedSubview(row)
            row.widthAnchor.constraint(equalTo: listStack.widthAnchor).isActive = true
        }
        let scroll = makeScrollView(for: listStack, maxHeight: 270)
        currentScrollView = scroll
        container.addArrangedSubview(scroll)
        scroll.widthAnchor.constraint(equalTo: container.widthAnchor).isActive = true
        return container
    }

    private func buildProcessRow(_ process: JSONValue) -> NSView {
        let obj = process.object ?? [:]
        let exePath = obj["exePath"]?.string ?? ""
        let basename = windowsBasename(exePath)
        let arch = obj["arch"]?.string ?? "?"
        let pid = obj["guestPID"]?.integer ?? 0
        let remembered = isRememberedTarget(exePath)

        let nameLabel = makeLabel(
            remembered ? "★ \(basename)" : basename,
            size: 12,
            weight: remembered ? .semibold : .medium,
            color: .white,
            lines: 1
        )
        nameLabel.lineBreakMode = .byTruncatingMiddle
        nameLabel.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        let metaLabel = makeLabel(
            strings.text("process_metadata", ["arch": arch, "pid": String(pid)]),
            size: 10,
            weight: .regular,
            color: .lightGray,
            lines: 1
        )
        let infoStack = NSStackView(views: [nameLabel, metaLabel])
        infoStack.orientation = .vertical
        infoStack.alignment = .leading
        infoStack.spacing = 2
        infoStack.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        let attachButton = HUDActionButton(
            title: strings.text("attach"),
            background: NSColor.systemBlue.withAlphaComponent(0.75),
            foreground: .white,
            fontSize: 11,
            weight: .semibold,
            cornerRadius: 6,
            horizontalPadding: 10,
            verticalPadding: 3
        ) { [weak self] in
            self?.triggerAttach(target: process)
        }
        attachButton.isEnabled = !isWorking

        return makeRowCard(leading: infoStack, trailing: attachButton)
    }

    private func buildAttachedBody() -> NSView {
        let container = NSStackView()
        container.orientation = .vertical
        container.alignment = .leading
        container.spacing = 8

        let exe = windowsBasename(latestSnapshot.object?["target"]?.object?["exePath"]?.string ?? "")
        let attachedLabel = makeLabel(
            strings.text("attached_target", ["exe": exe.isEmpty ? strings.text("game_process") : exe]),
            size: 11,
            weight: .medium,
            color: NSColor(calibratedRed: 0.62, green: 0.94, blue: 0.72, alpha: 1),
            lines: 1
        )
        attachedLabel.lineBreakMode = .byTruncatingMiddle
        attachedLabel.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        let detachButton = HUDActionButton(
            title: strings.text("detach"),
            background: NSColor.systemRed.withAlphaComponent(0.32),
            foreground: NSColor(calibratedRed: 1, green: 0.72, blue: 0.72, alpha: 1),
            fontSize: 11,
            cornerRadius: 6,
            horizontalPadding: 8,
            verticalPadding: 3
        ) { [weak self] in
            self?.triggerDetach()
        }

        let pillButton = HUDActionButton(
            title: strings.text("collapse_ball"),
            background: NSColor.white.withAlphaComponent(0.12),
            foreground: .white,
            fontSize: 11,
            cornerRadius: 6,
            horizontalPadding: 8,
            verticalPadding: 3
        ) { [weak self] in
            self?.isCollapsed = true
            self?.rebuildUI(preservingScroll: false)
        }

        let barControls = NSStackView(views: [detachButton, pillButton])
        barControls.orientation = .horizontal
        barControls.spacing = 5

        let attachedBar = NSStackView(views: [attachedLabel, barControls])
        attachedBar.orientation = .horizontal
        attachedBar.alignment = .centerY
        attachedBar.distribution = .fill
        attachedBar.spacing = 6
        container.addArrangedSubview(attachedBar)
        attachedBar.widthAnchor.constraint(equalTo: container.widthAnchor).isActive = true

        let threads = visibleThreads()
        if threads.isEmpty {
            let waitingCard = makeInfoCard(
                message: strings.text("waiting_text"),
                color: NSColor(calibratedWhite: 0.78, alpha: 1)
            )
            container.addArrangedSubview(waitingCard)
            waitingCard.widthAnchor.constraint(equalTo: container.widthAnchor).isActive = true
            return container
        }

        let listStack = makeListStack()
        for thread in threads {
            let row = buildThreadRow(thread)
            listStack.addArrangedSubview(row)
            row.widthAnchor.constraint(equalTo: listStack.widthAnchor).isActive = true
        }
        let scroll = makeScrollView(for: listStack, maxHeight: 260)
        currentScrollView = scroll
        container.addArrangedSubview(scroll)
        scroll.widthAnchor.constraint(equalTo: container.widthAnchor).isActive = true
        return container
    }

    private func buildThreadRow(_ thread: [String: JSONValue]) -> NSView {
        let threadID = thread["id"]?.string ?? ""
        let hookName = thread["hookName"]?.string ?? strings.text("thread")
        let currentRole = thread["role"]?.string
        let rawPreview = (thread["preview"]?.string ?? "")
            .replacingOccurrences(of: "\r", with: " ")
            .replacingOccurrences(of: "\n", with: " ")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        let preview = rawPreview.isEmpty ? strings.text("empty_preview") : String(rawPreview.prefix(80))

        let titleText = currentRole.map { "\(hookName) · \(roleDisplayName($0))" } ?? hookName
        let titleLabel = makeLabel(titleText, size: 11, weight: .semibold, color: .white, lines: 1)
        titleLabel.lineBreakMode = .byTruncatingTail
        let previewLabel = makeLabel(preview, size: 11, weight: .regular, color: .lightGray, lines: 2)
        previewLabel.lineBreakMode = .byTruncatingTail

        let buttons: [(String, String?)] = [
            (strings.text("role_body"), "body"),
            (strings.text("role_speaker"), "speaker"),
            (strings.text("role_choice"), "choice"),
            (strings.text("role_ignore"), nil)
        ]
        let roleButtons = buttons.map { title, roleValue -> NSView in
            let isSelected = (roleValue != nil && currentRole == roleValue)
            let backgroundColor = isSelected
                ? NSColor.systemBlue.withAlphaComponent(0.78)
                : NSColor.white.withAlphaComponent(0.11)
            let foregroundColor: NSColor = isSelected ? .white : NSColor(calibratedWhite: 0.88, alpha: 1)
            return HUDActionButton(
                title: title,
                background: backgroundColor,
                foreground: foregroundColor,
                fontSize: 10,
                weight: isSelected ? .semibold : .regular,
                cornerRadius: 5,
                horizontalPadding: 7,
                verticalPadding: 2
            ) { [weak self] in
                self?.triggerSelectThread(threadID: threadID, role: roleValue)
            }
        }
        let buttonRow = NSStackView(views: roleButtons)
        buttonRow.orientation = .horizontal
        buttonRow.spacing = 5

        let contentStack = NSStackView(views: [titleLabel, previewLabel, buttonRow])
        contentStack.orientation = .vertical
        contentStack.alignment = .leading
        contentStack.spacing = 4
        contentStack.translatesAutoresizingMaskIntoConstraints = false

        let card = NSView()
        card.wantsLayer = true
        card.layer?.backgroundColor = NSColor.white.withAlphaComponent(currentRole != nil ? 0.10 : 0.06).cgColor
        card.layer?.borderColor = (currentRole != nil
            ? NSColor.systemBlue.withAlphaComponent(0.45)
            : NSColor.white.withAlphaComponent(0.08)).cgColor
        card.layer?.borderWidth = 1
        card.layer?.cornerRadius = 8
        card.translatesAutoresizingMaskIntoConstraints = false
        card.addSubview(contentStack)
        NSLayoutConstraint.activate([
            contentStack.leadingAnchor.constraint(equalTo: card.leadingAnchor, constant: 10),
            contentStack.trailingAnchor.constraint(equalTo: card.trailingAnchor, constant: -10),
            contentStack.topAnchor.constraint(equalTo: card.topAnchor, constant: 8),
            contentStack.bottomAnchor.constraint(equalTo: card.bottomAnchor, constant: -8)
        ])
        return card
    }

    // MARK: - Data Helpers

    private func currentCaptureState() -> String {
        latestSnapshot.object?["captureState"]?.string ?? "stopped"
    }

    private func currentThreadRoles() -> [String: String] {
        var result: [String: String] = [:]
        for item in latestSnapshot.object?["threads"]?.array ?? [] {
            if let id = item.object?["id"]?.string, let role = item.object?["role"]?.string {
                result[id] = role
            }
        }
        return result
    }

    private func visibleThreads() -> [[String: JSONValue]] {
        (latestSnapshot.object?["threads"]?.array ?? []).compactMap { item -> [String: JSONValue]? in
            guard let obj = item.object else { return nil }
            let hasRole = obj["role"]?.string != nil
            let hasPreview = !(obj["preview"]?.string ?? "").trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            return (hasRole || hasPreview) ? obj : nil
        }
    }

    private func sortedCandidateProcesses() -> [JSONValue] {
        let list = latestSnapshot.object?["processes"]?.array ?? []
        return list.sorted { lhs, rhs in
            let lPath = lhs.object?["exePath"]?.string ?? ""
            let rPath = rhs.object?["exePath"]?.string ?? ""
            let lRemembered = isRememberedTarget(lPath)
            let rRemembered = isRememberedTarget(rPath)
            if lRemembered != rRemembered { return lRemembered }

            let lSystem = isSystemExecutable(lPath)
            let rSystem = isSystemExecutable(rPath)
            if lSystem != rSystem { return !lSystem }

            let lName = windowsBasename(lPath).lowercased()
            let rName = windowsBasename(rPath).lowercased()
            if lName != rName { return lName < rName }

            let lPid = lhs.object?["guestPID"]?.integer ?? 0
            let rPid = rhs.object?["guestPID"]?.integer ?? 0
            return lPid < rPid
        }
    }

    private func isRememberedTarget(_ exePath: String) -> Bool {
        guard let target = activePreset.targetExePath, !target.isEmpty else { return false }
        if exePath.caseInsensitiveCompare(target) == .orderedSame { return true }
        let targetName = windowsBasename(target)
        return !targetName.isEmpty && windowsBasename(exePath).caseInsensitiveCompare(targetName) == .orderedSame
    }

    private func isSystemExecutable(_ exePath: String) -> Bool {
        let lower = exePath.lowercased()
        if lower.contains("\\windows\\") { return true }
        return Self.systemExecutables.contains(windowsBasename(lower))
    }

    private func windowsBasename(_ path: String) -> String {
        path.split { $0 == "\\" || $0 == "/" }.last.map(String.init) ?? ""
    }

    private func roleDisplayName(_ role: String) -> String {
        switch role {
        case "body": strings.text("role_body")
        case "speaker": strings.text("role_speaker")
        case "choice": strings.text("role_choice")
        default: role
        }
    }

    // MARK: - View Helpers

    private func makeCloseButton() -> NSView {
        HUDActionButton(
            title: "✕",
            background: NSColor.white.withAlphaComponent(0.10),
            foreground: .lightGray,
            fontSize: 11,
            cornerRadius: 9,
            horizontalPadding: 6,
            verticalPadding: 2
        ) { [weak self] in
            self?.hide()
        }
    }

    private func makeLabel(
        _ text: String,
        size: CGFloat,
        weight: NSFont.Weight,
        color: NSColor,
        lines: Int
    ) -> NSTextField {
        let label = NSTextField(wrappingLabelWithString: text)
        label.font = .systemFont(ofSize: size, weight: weight)
        label.textColor = color
        label.maximumNumberOfLines = lines
        label.lineBreakMode = .byTruncatingTail
        label.isSelectable = false
        return label
    }

    private func makeInfoCard(
        message: String,
        color: NSColor,
        background: NSColor = NSColor.white.withAlphaComponent(0.06)
    ) -> NSView {
        let card = NSView()
        card.wantsLayer = true
        card.layer?.backgroundColor = background.cgColor
        card.layer?.cornerRadius = 8
        card.translatesAutoresizingMaskIntoConstraints = false
        let label = makeLabel(message, size: 11, weight: .regular, color: color, lines: 4)
        label.translatesAutoresizingMaskIntoConstraints = false
        card.addSubview(label)
        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: card.leadingAnchor, constant: 10),
            label.trailingAnchor.constraint(equalTo: card.trailingAnchor, constant: -10),
            label.topAnchor.constraint(equalTo: card.topAnchor, constant: 8),
            label.bottomAnchor.constraint(equalTo: card.bottomAnchor, constant: -8)
        ])
        return card
    }

    private func makeRowCard(leading: NSView, trailing: NSView) -> NSView {
        let card = NSView()
        card.wantsLayer = true
        card.layer?.backgroundColor = NSColor.white.withAlphaComponent(0.06).cgColor
        card.layer?.borderColor = NSColor.white.withAlphaComponent(0.08).cgColor
        card.layer?.borderWidth = 1
        card.layer?.cornerRadius = 8
        card.translatesAutoresizingMaskIntoConstraints = false

        let row = NSStackView(views: [leading, trailing])
        row.orientation = .horizontal
        row.alignment = .centerY
        row.distribution = .fill
        row.spacing = 8
        row.translatesAutoresizingMaskIntoConstraints = false
        card.addSubview(row)
        NSLayoutConstraint.activate([
            row.leadingAnchor.constraint(equalTo: card.leadingAnchor, constant: 10),
            row.trailingAnchor.constraint(equalTo: card.trailingAnchor, constant: -10),
            row.topAnchor.constraint(equalTo: card.topAnchor, constant: 7),
            row.bottomAnchor.constraint(equalTo: card.bottomAnchor, constant: -7)
        ])
        return card
    }

    private func makeListStack() -> NSStackView {
        let stack = NSStackView()
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 6
        stack.translatesAutoresizingMaskIntoConstraints = false
        return stack
    }

    private func makeScrollView(for stack: NSStackView, maxHeight: CGFloat) -> NSScrollView {
        let document = FlippedView()
        document.translatesAutoresizingMaskIntoConstraints = false
        document.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: document.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: document.trailingAnchor),
            stack.topAnchor.constraint(equalTo: document.topAnchor),
            stack.bottomAnchor.constraint(equalTo: document.bottomAnchor)
        ])

        let scroll = NSScrollView()
        scroll.drawsBackground = false
        scroll.hasVerticalScroller = true
        scroll.hasHorizontalScroller = false
        scroll.autohidesScrollers = true
        scroll.borderType = .noBorder
        scroll.documentView = document
        scroll.translatesAutoresizingMaskIntoConstraints = false

        document.widthAnchor.constraint(equalTo: scroll.contentView.widthAnchor).isActive = true
        document.layoutSubtreeIfNeeded()
        let contentHeight = max(44, min(maxHeight, stack.fittingSize.height + 2))
        scroll.heightAnchor.constraint(equalToConstant: contentHeight).isActive = true
        return scroll
    }
}

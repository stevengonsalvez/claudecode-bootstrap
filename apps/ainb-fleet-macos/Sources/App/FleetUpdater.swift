import Sparkle

@MainActor
final class FleetUpdater {
    private let controller = SPUStandardUpdaterController(
        startingUpdater: false,
        updaterDelegate: nil,
        userDriverDelegate: nil
    )

    func start() {
        guard isConfigured else { return }
        controller.startUpdater()
    }

    func checkForUpdates() {
        guard isConfigured else { return }
        controller.checkForUpdates(nil)
    }

    private var isConfigured: Bool {
        guard let key = Bundle.main.object(forInfoDictionaryKey: "SUPublicEDKey") as? String else { return false }
        return !key.isEmpty
    }
}

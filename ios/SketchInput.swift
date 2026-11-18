import UIKit

/// One pointer sample in view points; `force` is normalised pressure for a stylus.
struct SketchPointerSample {
    var location: CGPoint
    var timestamp: TimeInterval
    var force: CGFloat?
    var isStylus: Bool

    init(location: CGPoint, timestamp: TimeInterval, force: CGFloat?, isStylus: Bool) {
        self.location = location
        self.timestamp = timestamp
        self.force = force
        self.isStylus = isStylus
    }

    init(touch: UITouch, in view: UIView) {
        let isStylus = touch.type == .pencil
        self.init(
            location: touch.location(in: view),
            timestamp: touch.timestamp,
            force: isStylus && touch.maximumPossibleForce > 0 ? touch.force / touch.maximumPossibleForce : nil,
            isStylus: isStylus
        )
    }

    var inputSample: InputSample {
        InputSample(
            x: Double(location.x),
            y: Double(location.y),
            timeMs: timestamp * 1000,
            pressure: force.map(Double.init),
            kind: isStylus ? .stylus : .touch
        )
    }
}

/// Decides which pointer draws: one at a time, a stylus beats fingers (palm rejection), and a
/// second finger cancels a finger stroke and keeps fingers ignored until they all lift.
struct SketchPointerTracker {
    enum Decision {
        case begin
        case cancelThenBegin
        case cancel
        case ignore
    }

    private(set) var drawing: AnyHashable?
    private var drawingIsStylus = false
    private var down: [AnyHashable: Bool] = [:]
    private var fingersSuppressed = false

    var hasPointersDown: Bool {
        !down.isEmpty
    }

    mutating func began(_ pointer: AnyHashable, isStylus: Bool) -> Decision {
        let stylusDown = down.values.contains(true)
        down[pointer] = isStylus
        if isStylus {
            let fingerWasDrawing = drawing != nil && !drawingIsStylus
            drawing = pointer
            drawingIsStylus = true
            return fingerWasDrawing ? .cancelThenBegin : .begin
        }
        if stylusDown || fingersSuppressed {
            return .ignore
        }
        if drawing != nil {
            drawing = nil
            fingersSuppressed = true
            return .cancel
        }
        drawing = pointer
        drawingIsStylus = false
        return .begin
    }

    func isDrawing(_ pointer: AnyHashable) -> Bool {
        drawing == pointer
    }

    /// Stops the current stroke without lifting its pointer, e.g. when the view resizes.
    mutating func abandonDrawing() {
        drawing = nil
    }

    /// Returns whether the lifted pointer was the one drawing.
    mutating func ended(_ pointer: AnyHashable) -> Bool {
        down.removeValue(forKey: pointer)
        if !down.values.contains(false) {
            fingersSuppressed = false
        }
        guard drawing == pointer else { return false }
        drawing = nil
        return true
    }
}

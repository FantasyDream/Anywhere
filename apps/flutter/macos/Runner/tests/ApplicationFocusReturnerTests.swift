import AppKit

private final class Target: ApplicationReturnTarget {
  let processIdentifier: pid_t = 42
  var bundleURL: URL? = URL(fileURLWithPath: "/Applications/Target.app")
  var isTerminated = false
}

private final class Clock {
  var now: TimeInterval = 0
  var jobs: [(TimeInterval, () -> Void)] = []
  func schedule(_ delay: TimeInterval, _ work: @escaping () -> Void) {
    jobs.append((now + delay, work))
  }
  func advance(_ interval: TimeInterval) {
    let end = now + interval
    while let next = jobs.indices.min(by: { jobs[$0].0 < jobs[$1].0 }), jobs[next].0 <= end {
      let (time, work) = jobs.remove(at: next)
      now = time
      work()
    }
    now = end
  }
}

private final class Fixture {
  let clock = Clock()
  let target = Target()
  var ownsFocus = true
  var frontmost: pid_t? = 1
  var opens = 0
  var outcomes: [ApplicationFocusReturner.Outcome] = []
  var opened: ((pid_t?) -> Void)?
  lazy var returner = ApplicationFocusReturner(
    open: { [unowned self] url, completion in
      assert(url == target.bundleURL)
      opens += 1
      opened = completion
    },
    frontmost: { [unowned self] in frontmost },
    schedule: { [unowned self] delay, work in clock.schedule(delay, work) }
  )
  func run(_ target: ApplicationReturnTarget? = nil) {
    returner.returnFocus(
      to: target ?? self.target,
      mayReturn: { [unowned self] in ownsFocus },
      completion: { [unowned self] in outcomes.append($0) }
    )
  }
}

@main
struct ApplicationFocusReturnerTests {
  static func main() {
    let lost = Fixture()
    lost.ownsFocus = false
    lost.run()
    assert(lost.outcomes == [.skipped] && lost.opens == 0)

    for missingURL in [false, true] {
      let unavailable = Fixture()
      if missingURL {
        unavailable.target.bundleURL = nil
      } else {
        unavailable.target.isTerminated = true
      }
      unavailable.run()
      assert(unavailable.outcomes == [.unavailable] && unavailable.opens == 0)
    }
    let noTarget = Fixture()
    noTarget.returner.returnFocus(
      to: nil,
      mayReturn: { true },
      completion: { noTarget.outcomes.append($0) }
    )
    assert(noTarget.outcomes == [.unavailable] && noTarget.opens == 0)

    let active = Fixture()
    active.frontmost = 42
    active.run()
    assert(active.outcomes == [.returned] && active.opens == 0)

    let success = Fixture()
    success.run()
    success.opened?(42)
    success.clock.advance(0.1)
    assert(success.outcomes.isEmpty)  // accepted is not foreground confirmation
    success.frontmost = 42
    success.ownsFocus = false
    success.clock.advance(0.1)
    assert(success.outcomes == [.returned] && success.opens == 1)
    success.clock.advance(3)
    assert(success.outcomes == [.returned])

    for response: pid_t? in [nil, 99] {
      let rejected = Fixture()
      rejected.run()
      rejected.opened?(response)
      rejected.clock.advance(3)
      assert(rejected.outcomes == [.failed])
    }
    for accepted in [false, true] {
      let timeout = Fixture()
      timeout.run()
      if accepted { timeout.opened?(42) }
      timeout.clock.advance(2.1)
      assert(timeout.outcomes == [.failed])
      timeout.frontmost = 42
      timeout.opened?(42)  // no second completion after timeout
      assert(timeout.outcomes == [.failed] && timeout.opens == 1)
    }
    let switched = Fixture()
    switched.run()
    switched.opened?(42)
    switched.ownsFocus = false
    switched.frontmost = 7
    switched.clock.advance(0.1)
    assert(switched.outcomes == [.skipped] && switched.opens == 1)

    let exited = Fixture()
    exited.run()
    exited.target.isTerminated = true
    exited.opened?(42)
    assert(exited.outcomes == [.unavailable])
    print(
      "Application focus return: target identity, foreground verification, timeout, failure and stale focus passed"
    )
  }
}

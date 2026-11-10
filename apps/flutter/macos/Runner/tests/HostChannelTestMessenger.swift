import FlutterMacOS
import Foundation

/// Exercises the encoded Flutter boundary without creating an engine or window.
final class HostChannelTestMessenger: NSObject, FlutterBinaryMessenger {
  final class Reply {
    var responses: [Data?] = []
  }
  private(set) var handlers: [String: FlutterBinaryMessageHandler] = [:]
  private(set) var events: [(String, FlutterMethodCall)] = []
  private let codec = FlutterStandardMethodCodec.sharedInstance()
  private var channels: [FlutterBinaryMessengerConnection: String] = [:]
  private var nextConnection: FlutterBinaryMessengerConnection = 0

  func send(onChannel channel: String, message: Data?) {
    if let message { events.append((channel, codec.decodeMethodCall(message))) }
  }
  func send(onChannel channel: String, message: Data?, binaryReply callback: FlutterBinaryReply?) {
    send(onChannel: channel, message: message)
    callback?(nil)
  }
  func setMessageHandlerOnChannel(
    _ channel: String,
    binaryMessageHandler handler: FlutterBinaryMessageHandler?
  ) -> FlutterBinaryMessengerConnection {
    handlers[channel] = handler
    nextConnection += 1
    channels[nextConnection] = channel
    return nextConnection
  }
  func cleanUpConnection(_ connection: FlutterBinaryMessengerConnection) {
    if let channel = channels.removeValue(forKey: connection) { handlers[channel] = nil }
  }
  func request(_ channel: String, _ method: String, _ arguments: Any? = nil) -> Reply {
    let reply = Reply()
    handlers[channel]?(codec.encode(FlutterMethodCall(methodName: method, arguments: arguments))) {
      reply.responses.append($0)
    }
    return reply
  }
}

import Foundation
import AVFoundation
import ScreenCaptureKit
import CoreMedia
import CoreAudio
import AudioToolbox

// Native audio-only helper: stdout is signed 16-bit LE mono PCM at 16 kHz; diagnostics use stderr.
// No screen frames are registered or persisted. This source must be built and tested on macOS.
// Modes: --track system|microphone [--device CORE_AUDIO_UID] (microphone only) and
// --list-microphones (one JSON array of {id,name,isDefault} on stdout, no permission prompt).
// Microphone selection was added without access to Mac hardware and still needs a device test.
final class Sink: @unchecked Sendable {
    private var converter: AVAudioConverter?
    private var format: AVAudioFormat?
    private let destination = AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: 16000, channels: 1, interleaved: true)!
    private let lock = NSLock()
    func write(_ input: AVAudioPCMBuffer) {
        lock.lock(); defer { lock.unlock() }
        if format != input.format {
            format = input.format; converter = AVAudioConverter(from: input.format, to: destination)
        }
        guard let converter = converter else { fail("Cannot convert capture format") }
        let capacity = AVAudioFrameCount(ceil(Double(input.frameLength) * 16000 / input.format.sampleRate) + 64)
        guard let output = AVAudioPCMBuffer(pcmFormat: destination, frameCapacity: capacity) else { fail("Cannot allocate audio buffer") }
        var consumed = false
        var error: NSError?
        let result = converter.convert(to: output, error: &error) { _, status in
            if consumed { status.pointee = .noDataNow; return nil }
            consumed = true; status.pointee = .haveData; return input
        }
        if result == .error || error != nil { fail("Audio conversion failed") }
        if output.frameLength > 0, let data = output.int16ChannelData {
            do { try FileHandle.standardOutput.write(contentsOf: Data(bytes: data[0], count: Int(output.frameLength) * 2)) }
            catch { exit(0) } // Parent closed its pipe; never keep recording without its writer.
        }
    }
}
func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data((message + "\n").utf8)); exit(1)
}
func announceReady() {
    // Startup acknowledgement is separate from PCM: silence is valid after permissions succeed.
    FileHandle.standardError.write(Data("HANDYMEETING_AUDIO_READY\n".utf8))
}
func address(_ selector: AudioObjectPropertySelector, _ scope: AudioObjectPropertyScope = kAudioObjectPropertyScopeGlobal) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress(mSelector: selector, mScope: scope, mElement: kAudioObjectPropertyElementMain)
}
func audioDevices() -> [AudioDeviceID] {
    let system = AudioObjectID(kAudioObjectSystemObject)
    var request = address(kAudioHardwarePropertyDevices)
    var size: UInt32 = 0
    guard AudioObjectGetPropertyDataSize(system, &request, 0, nil, &size) == noErr, size > 0 else { return [] }
    var devices = [AudioDeviceID](repeating: 0, count: Int(size) / MemoryLayout<AudioDeviceID>.size)
    guard AudioObjectGetPropertyData(system, &request, 0, nil, &size, &devices) == noErr else { return [] }
    return Array(devices.prefix(Int(size) / MemoryLayout<AudioDeviceID>.size))
}
func hasInput(_ device: AudioDeviceID) -> Bool {
    var request = address(kAudioDevicePropertyStreams, kAudioObjectPropertyScopeInput)
    var size: UInt32 = 0
    return AudioObjectGetPropertyDataSize(device, &request, 0, nil, &size) == noErr && size > 0
}
func text(_ device: AudioDeviceID, _ selector: AudioObjectPropertySelector) -> String? {
    var request = address(selector)
    var value: Unmanaged<CFString>?
    var size = UInt32(MemoryLayout<Unmanaged<CFString>?>.size)
    let status = withUnsafeMutablePointer(to: &value) { AudioObjectGetPropertyData(device, &request, 0, nil, &size, $0) }
    guard status == noErr, let result = value else { return nil }
    return result.takeRetainedValue() as String // Core Audio returns a +1 CFString.
}
func defaultInput() -> AudioDeviceID? {
    var request = address(kAudioHardwarePropertyDefaultInputDevice)
    var device = AudioDeviceID(0)
    var size = UInt32(MemoryLayout<AudioDeviceID>.size)
    guard AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &request, 0, nil, &size, &device) == noErr, device != 0 else { return nil }
    return device
}
func listMicrophones() -> Never {
    let current = defaultInput()
    var rows: [[String: Any]] = []
    for device in audioDevices() where hasInput(device) {
        guard let uid = text(device, kAudioDevicePropertyDeviceUID), !uid.isEmpty else { continue }
        rows.append(["id": uid, "name": text(device, kAudioObjectPropertyName) ?? uid, "isDefault": device == current])
    }
    guard let data = try? JSONSerialization.data(withJSONObject: rows, options: [.sortedKeys]) else { fail("Cannot list microphones") }
    FileHandle.standardOutput.write(data + Data("\n".utf8)); exit(0)
}
// AVAudioEngine records the system default input unless its input unit is pointed at a device.
func select(microphone uid: String, for engine: AVAudioEngine) {
    guard var device = audioDevices().first(where: { hasInput($0) && text($0, kAudioDevicePropertyDeviceUID) == uid }) else {
        fail("The selected microphone is not available. Choose another microphone or the default.")
    }
    guard let unit = engine.inputNode.audioUnit,
          AudioUnitSetProperty(unit, kAudioOutputUnitProperty_CurrentDevice, kAudioUnitScope_Global, 0, &device, UInt32(MemoryLayout<AudioDeviceID>.size)) == noErr else {
        fail("Cannot select the requested microphone")
    }
}
@available(macOS 13.0, *)
final class SystemCapture: NSObject, SCStreamOutput, SCStreamDelegate {
    let sink = Sink()
    var stream: SCStream?
    func start() async throws {
        let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
        guard let display = content.displays.first else { fail("No display available for system audio capture") }
        let filter = SCContentFilter(display: display, excludingApplications: [], exceptingWindows: [])
        let configuration = SCStreamConfiguration()
        configuration.width = 2; configuration.height = 2
        configuration.capturesAudio = true; configuration.excludesCurrentProcessAudio = true
        configuration.sampleRate = 16000; configuration.channelCount = 1
        let stream = SCStream(filter: filter, configuration: configuration, delegate: self)
        try stream.addStreamOutput(self, type: .audio, sampleHandlerQueue: DispatchQueue(label: "HandyMeeting.audio"))
        self.stream = stream; try await stream.startCapture(); announceReady()
    }
    func stream(_ stream: SCStream, didOutputSampleBuffer sample: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .audio, sample.isValid, sample.numSamples > 0, let description = sample.formatDescription,
              let basic = CMAudioFormatDescriptionGetStreamBasicDescription(description),
              let inputFormat = AVAudioFormat(streamDescription: basic) else { return }
        var bytes = 0
        _ = CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(sample, bufferListSizeNeededOut: &bytes,
            bufferListOut: nil, bufferListSize: 0, blockBufferAllocator: nil, blockBufferMemoryAllocator: nil, flags: 0, blockBufferOut: nil)
        guard bytes > 0, bytes <= 65536 else { fail("Cannot read captured audio") }
        let pointer = UnsafeMutableRawPointer.allocate(byteCount: bytes, alignment: MemoryLayout<AudioBufferList>.alignment)
        defer { pointer.deallocate() }
        let list = pointer.bindMemory(to: AudioBufferList.self, capacity: 1)
        var block: CMBlockBuffer?
        guard CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(sample, bufferListSizeNeededOut: nil,
            bufferListOut: list, bufferListSize: bytes, blockBufferAllocator: nil, blockBufferMemoryAllocator: nil, flags: 0, blockBufferOut: &block) == noErr,
            let pcm = AVAudioPCMBuffer(pcmFormat: inputFormat, frameCapacity: AVAudioFrameCount(sample.numSamples)) else { fail("Cannot access captured audio") }
        pcm.frameLength = AVAudioFrameCount(sample.numSamples)
        let sourceBuffers = UnsafeMutableAudioBufferListPointer(list)
        let destinationBuffers = UnsafeMutableAudioBufferListPointer(pcm.mutableAudioBufferList)
        guard sourceBuffers.count == destinationBuffers.count else { fail("Unexpected captured channel layout") }
        for index in 0..<sourceBuffers.count {
            guard let source = sourceBuffers[index].mData, let target = destinationBuffers[index].mData,
                  sourceBuffers[index].mDataByteSize <= destinationBuffers[index].mDataByteSize else { fail("Unexpected captured buffer size") }
            memcpy(target, source, Int(sourceBuffers[index].mDataByteSize))
        }
        sink.write(pcm)
    }
    func stream(_ stream: SCStream, didStopWithError error: Error) { fail("System audio capture stopped: " + error.localizedDescription) }
}
@main struct AudioMain {
    static func main() async {
        let arguments = CommandLine.arguments
        // Listing reads device names only; it opens no stream and needs no stdin.
        if arguments.count == 2, arguments[1] == "--list-microphones" { listMicrophones() }
        guard arguments.count == 3 || arguments.count == 5, arguments[1] == "--track" else { fail("Use --track system|microphone [--device UID] or --list-microphones") }
        var selected: String? = nil
        if arguments.count == 5 {
            guard arguments[2] == "microphone", arguments[3] == "--device", !arguments[4].isEmpty else { fail("Only --track microphone accepts --device UID") }
            selected = arguments[4]
        }
        // Parent EOF ends either track, including a parent process crash.
        DispatchQueue.global().async { _ = FileHandle.standardInput.readDataToEndOfFile(); exit(0) }
        if arguments[2] == "microphone" {
            guard await AVCaptureDevice.requestAccess(for: .audio) else { fail("Microphone access denied") }
            let engine = AVAudioEngine(); let sink = Sink()
            if let uid = selected { select(microphone: uid, for: engine) } // Before reading the input format.
            let input = engine.inputNode; let format = input.outputFormat(forBus: 0)
            input.installTap(onBus: 0, bufferSize: 1600, format: format) { buffer, _ in sink.write(buffer) }
            do { try engine.start(); announceReady(); while true { try await Task.sleep(nanoseconds: 1_000_000_000) } }
            catch { fail("Microphone capture failed: " + error.localizedDescription) }
        } else if arguments[2] == "system" {
            guard #available(macOS 13.0, *) else { fail("System audio requires macOS 13 or later") }
            let capture = SystemCapture()
            do { try await capture.start(); while true { try await Task.sleep(nanoseconds: 1_000_000_000) } }
            catch { fail("System audio access failed: " + error.localizedDescription) }
        } else { fail("Unknown track") }
    }
}

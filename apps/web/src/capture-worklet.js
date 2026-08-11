// Runs on the audio rendering thread: forwards each 128-sample input block
// to the main thread and outputs silence.
class CaptureProcessor extends AudioWorkletProcessor {
  process(inputs) {
    const channel = inputs[0][0];
    if (channel) {
      this.port.postMessage(channel.slice(0));
    }
    return true;
  }
}

registerProcessor("capture", CaptureProcessor);

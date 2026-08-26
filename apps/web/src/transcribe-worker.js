// One member of the transcription worker pool: owns a Transcriber (its own
// copy of the model) and runs whatever window the main thread sends it.
import init, { Transcriber } from "@sconote/web";

const ready = init().then(() => new Transcriber());

self.onmessage = async ({ data: { index, samples } }) => {
  const transcriber = await ready;
  const output = transcriber.predictWindow(samples);
  self.postMessage({ index, output }, [output.buffer]);
};

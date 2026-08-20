// Encode captured Float32Array chunks as a WAV blob (format 3: IEEE float,
// 32-bit, mono) - lossless for the samples the tracker actually saw.
export function encodeWavFloat32(chunks, sampleRate) {
  const sampleCount = chunks.reduce((total, chunk) => total + chunk.length, 0);
  const buffer = new ArrayBuffer(44 + sampleCount * 4);
  const view = new DataView(buffer);
  const writeAscii = (offset, text) => {
    for (let i = 0; i < text.length; i++) {
      view.setUint8(offset + i, text.charCodeAt(i));
    }
  };
  writeAscii(0, "RIFF");
  view.setUint32(4, 36 + sampleCount * 4, true);
  writeAscii(8, "WAVE");
  writeAscii(12, "fmt ");
  view.setUint32(16, 16, true); // fmt chunk size
  view.setUint16(20, 3, true); // format 3 = IEEE float
  view.setUint16(22, 1, true); // mono
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, sampleRate * 4, true); // bytes per second
  view.setUint16(32, 4, true); // bytes per frame
  view.setUint16(34, 32, true); // bits per sample
  writeAscii(36, "data");
  view.setUint32(40, sampleCount * 4, true);
  let offset = 44;
  for (const chunk of chunks) {
    for (const sample of chunk) {
      view.setFloat32(offset, sample, true);
      offset += 4;
    }
  }
  return new Blob([buffer], { type: "audio/wav" });
}

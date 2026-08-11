import init, { NoteTracker, Transcriber } from "@sconote/web";
import { OpenSheetMusicDisplay } from "opensheetmusicdisplay";
import { encodeWavFloat32 } from "./wav-encoder.js";

const WINDOW_SIZE = 2048;
const IN_TUNE_CENTS = 5;

const noteEl = document.getElementById("note");
const needleEl = document.getElementById("needle");
const detailsEl = document.getElementById("details");
const startButton = document.getElementById("start");
const uploadButton = document.getElementById("upload");
const uploadInput = document.getElementById("upload-input");
const recordButton = document.getElementById("record");
const transcribeButton = document.getElementById("transcribe");
const transcriptionEl = document.getElementById("transcription");
const rollCanvas = document.getElementById("roll");
const scoreEl = document.getElementById("score");
const summaryEl = document.getElementById("transcription-summary");
const downloadWavButton = document.getElementById("download-wav");
const downloadMidiButton = document.getElementById("download-midi");
const downloadNotesButton = document.getElementById("download-notes");

// While set: { chunks, sampleRate } accumulating the session.
let recording = null;
// Last finished recording, kept for transcription.
let lastRecording = null;
// Created lazily on first transcription (loading the model isn't free).
let transcriber = null;
// Last transcription result: { midis, onsets, offsets, midiBytes }.
let lastTranscription = null;
// The wasm module must be initialized exactly once, from whichever entry
// point (mic or upload) runs first.
let wasmReady = null;
const ensureWasm = () => (wasmReady ??= init());

function download(name, blob) {
  const link = document.createElement("a");
  link.href = URL.createObjectURL(blob);
  link.download = name;
  link.click();
  URL.revokeObjectURL(link.href);
}

function stopRecording() {
  lastRecording = recording;
  recording = null;
  recordButton.textContent = "● Record session";
  transcribeButton.hidden = false;
}

async function transcribeRecording() {
  transcribeButton.disabled = true;
  const { chunks, sampleRate } = lastRecording;
  const merged = new Float32Array(
    chunks.reduce((total, chunk) => total + chunk.length, 0),
  );
  let position = 0;
  for (const chunk of chunks) {
    merged.set(chunk, position);
    position += chunk.length;
  }
  transcriber ??= new Transcriber();
  const job = transcriber.begin(merged, sampleRate);
  const total = job.totalWindows;
  while (job.processNextWindow(transcriber)) {
    const percent = Math.round((100 * job.windowsDone) / total);
    transcribeButton.textContent = `Transcribing… ${percent}%`;
    // Yield to the event loop so the page (and the live tuner) stays alive.
    await new Promise((resolve) => setTimeout(resolve));
  }
  const notes = job.finish(0.5, 0.3);
  job.free();
  lastTranscription = {
    midis: notes.midis(),
    onsets: notes.onsets(),
    offsets: notes.offsets(),
    midiBytes: notes.toMidi(),
    bpm: notes.estimatedBpm(),
    musicXml: notes.toMusicXml(),
  };
  notes.free();
  await renderTranscription(lastTranscription);
  transcribeButton.textContent = "Transcribe recording";
  transcribeButton.disabled = false;
}

const NOTE_NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

function noteName(midi) {
  return `${NOTE_NAMES[midi % 12]}${Math.floor(midi / 12) - 1}`;
}

// Engraving is delegated to OpenSheetMusicDisplay: the WASM core produces
// MusicXML (quantization, key detection, grand staff) and OSMD draws it.
let osmd = null;

async function renderScore(musicXml) {
  scoreEl.replaceChildren();
  // Must be visible (laid out) before render(): OSMD reads offsetWidth.
  scoreEl.hidden = false;
  osmd = new OpenSheetMusicDisplay(scoreEl, {
    autoResize: false,
    drawTitle: false,
    drawPartNames: false,
  });
  await osmd.load(musicXml);
  osmd.render();
}

async function renderTranscription({ midis, onsets, offsets, musicXml, bpm }) {
  transcriptionEl.hidden = false;
  if (midis.length === 0) {
    scoreEl.replaceChildren();
    scoreEl.hidden = true;
    summaryEl.textContent = "no notes found";
    return;
  }
  await renderScore(musicXml);
  const duration = Math.max(...offsets);
  const low = Math.min(...midis) - 1;
  const high = Math.max(...midis) + 1;
  const ctx = rollCanvas.getContext("2d");
  const { width, height } = rollCanvas;
  ctx.clearRect(0, 0, width, height);
  const rowHeight = height / (high - low + 1);
  ctx.fillStyle = "#4ade80";
  for (let i = 0; i < midis.length; i++) {
    const x = (onsets[i] / duration) * width;
    const w = Math.max(2, ((offsets[i] - onsets[i]) / duration) * width);
    const y = height - (midis[i] - low + 1) * rowHeight;
    ctx.fillRect(x, y, w, Math.max(1, rowHeight - 1));
  }
  summaryEl.textContent =
    `${midis.length} notes over ${duration.toFixed(1)} s · ` +
    `range ${noteName(low + 1)}–${noteName(high - 1)} · ` +
    `~${Math.round(bpm)} BPM`;
}

downloadWavButton.addEventListener("click", () => {
  const { chunks, sampleRate } = lastRecording;
  download("sconote-session.wav", encodeWavFloat32(chunks, sampleRate));
});

downloadMidiButton.addEventListener("click", () => {
  download(
    "sconote-transcription.mid",
    new Blob([lastTranscription.midiBytes], { type: "audio/midi" }),
  );
});

downloadNotesButton.addEventListener("click", () => {
  const { midis, onsets, offsets } = lastTranscription;
  const notes = Array.from(midis, (midi, i) => ({
    midi,
    note: noteName(midi),
    onsetS: onsets[i],
    offsetS: offsets[i],
  }));
  download(
    "sconote-transcription.json",
    new Blob([JSON.stringify({ notes }, null, 2)], { type: "application/json" }),
  );
});

function render(event) {
  noteEl.textContent = event.noteName;
  noteEl.classList.toggle("in-tune", Math.abs(event.centsOffset) < IN_TUNE_CENTS);
  // Map -50..+50 cents onto the meter width.
  needleEl.style.left = `${50 + event.centsOffset}%`;
  detailsEl.textContent =
    `${event.frequencyHz.toFixed(1)} Hz · ` +
    `${event.centsOffset >= 0 ? "+" : ""}${event.centsOffset.toFixed(0)} cents · ` +
    `clarity ${event.clarity.toFixed(2)}`;
}

async function start() {
  startButton.disabled = true;
  await ensureWasm();

  const stream = await navigator.mediaDevices.getUserMedia({
    audio: {
      echoCancellation: false,
      noiseSuppression: false,
      autoGainControl: false,
    },
  });

  const audioContext = new AudioContext();
  await audioContext.audioWorklet.addModule(
    new URL("./capture-worklet.js", import.meta.url),
  );

  const tracker = new NoteTracker(audioContext.sampleRate, WINDOW_SIZE);
  const capture = new AudioWorkletNode(audioContext, "capture");
  capture.port.onmessage = ({ data }) => {
    if (recording) {
      recording.chunks.push(data);
    }
    const update = tracker.process(data);
    const live = update.live;
    if (live) {
      render(live);
      live.free();
    }
    const started = update.noteStarted;
    if (started) {
      started.free();
    }
    update.free();
  };

  audioContext.createMediaStreamSource(stream).connect(capture);
  // The graph only pulls nodes reachable from the destination; the worklet
  // outputs silence, so this is inaudible.
  capture.connect(audioContext.destination);

  startButton.remove();
  detailsEl.textContent = `listening at ${audioContext.sampleRate} Hz…`;

  recordButton.hidden = false;
  recordButton.addEventListener("click", () => {
    if (recording) {
      stopRecording();
    } else {
      recording = { chunks: [], sampleRate: audioContext.sampleRate };
      recordButton.textContent = "■ Stop & save";
    }
  });
}

startButton.addEventListener("click", () => {
  start().catch((error) => {
    startButton.disabled = false;
    detailsEl.textContent = String(error);
  });
});

// Upload path: skip the mic entirely and transcribe an existing audio file.
async function loadAudioFile(file) {
  uploadButton.disabled = true;
  uploadButton.textContent = "Decoding…";
  await ensureWasm();
  // decodeAudioData handles wav/mp3/ogg/… and hands back raw f32 samples.
  const decoder = new AudioContext();
  const buffer = await decoder.decodeAudioData(await file.arrayBuffer());
  await decoder.close();
  lastRecording = {
    chunks: [buffer.getChannelData(0)],
    sampleRate: buffer.sampleRate,
  };
  uploadButton.textContent = `Upload audio… (${file.name})`;
  uploadButton.disabled = false;
  transcribeButton.hidden = false;
  await transcribeRecording();
}

uploadButton.addEventListener("click", () => uploadInput.click());
uploadInput.addEventListener("change", () => {
  const [file] = uploadInput.files;
  // Reset so picking the same file again still fires `change`.
  uploadInput.value = "";
  if (!file) return;
  loadAudioFile(file).catch((error) => {
    uploadButton.textContent = "Upload audio…";
    uploadButton.disabled = false;
    summaryEl.textContent = String(error);
    transcriptionEl.hidden = false;
  });
});

transcribeButton.addEventListener("click", () => {
  transcribeRecording().catch((error) => {
    transcribeButton.textContent = "Transcribe recording";
    transcribeButton.disabled = false;
    summaryEl.textContent = String(error);
    transcriptionEl.hidden = false;
  });
});

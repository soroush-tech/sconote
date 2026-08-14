// Hands control back to the event loop so the page stays responsive mid-job.
//
// Deliberately not setTimeout: browsers clamp timers in hidden tabs to one
// wake-up per second, and Chrome drops that to one per minute once a tab has
// been hidden for five minutes. A long transcription yields once per analysis
// window, so a backgrounded tab would appear to freeze. MessagePort delivery
// is not a timer and is not throttled.
export function yieldToEventLoop() {
  return new Promise((resolve) => {
    const { port1, port2 } = new MessageChannel();
    port1.onmessage = () => {
      port1.close();
      port2.close();
      resolve();
    };
    port2.postMessage(null);
  });
}

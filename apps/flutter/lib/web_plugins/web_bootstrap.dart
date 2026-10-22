// This is a transport, not a capability. Rust checks every call using native identity.
const webBootstrap = r'''
(() => {
  if (window !== window.top) return;
  // WebRTC ICE may bypass connect-src in some engines. Remove alternate network
  // constructors before package code executes; this still is not an OS sandbox.
  for (const name of ['RTCPeerConnection','webkitRTCPeerConnection','WebTransport']) {
    try { Object.defineProperty(window, name, {value:undefined,writable:false,configurable:false}); } catch {}
  }
  const receivers = new Set();
  const pending = new Set();
  let focused;
  window.addEventListener('anywhere:panel', event => {
    if (event.detail) focused = document.activeElement;
    else focused?.focus?.();
  });
  let visible = true;
  const encoder = new TextEncoder();
  const decoder = new TextDecoder('utf-8', {fatal:true,ignoreBOM:true});
  window.anywhereNative.onmessage = event => {
    const bytes = event.data;
    if (!(bytes instanceof ArrayBuffer) || bytes.byteLength > 65536) return;
    let frame, message;
    try { frame = decoder.decode(bytes); message = JSON.parse(frame); } catch { return; }
    pending.delete(message.requestId);
    for (const receive of receivers) receive(frame);
  };
  Object.defineProperty(window, '__anywhereTransport', {value: Object.freeze({
    send(frame) {
      if (!visible) throw new Error('INACTIVE');
      const message = JSON.parse(frame);
      const cancellation = message.method === 'request.cancel';
      // An abandoned native reply must not permanently occupy the local budget.
      // Cancellation itself is unqueued and still checked independently by Rust.
      if (encoder.encode(frame).length > 65536 || (!cancellation && pending.size >= 32)) throw new Error('BUSY');
      if (cancellation) pending.delete(message.params?.request_id);
      else pending.add(message.requestId);
      try { window.anywhereNative.postMessage(frame); }
      catch (error) { pending.delete(message.requestId); throw error; }
    },
    subscribe(receive) { receivers.add(receive); return () => receivers.delete(receive); }
  })});
  window.addEventListener('anywhere:visibility', event => {
    visible = event.detail === true;
    if (!visible) {
      for (const requestId of pending) {
        const frame = JSON.stringify({requestId,error:{code:'INACTIVE'}});
        for (const receive of receivers) receive(frame);
      }
      pending.clear();
    }
  });
})();
''';

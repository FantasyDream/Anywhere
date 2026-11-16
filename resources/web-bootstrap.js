(() => {
  if (window !== window.top) return;
  // WebRTC ICE may bypass connect-src in some engines. Remove alternate network
  // constructors before package code executes; this still is not an OS sandbox.
  for (const name of [
    'RTCPeerConnection',
    'webkitRTCPeerConnection',
    'WebTransport',
  ]) {
    try {
      Object.defineProperty(window, name, {
        value: undefined,
        writable: false,
        configurable: false,
      });
    } catch {
      // Some constructors may be non-configurable in a given WebKit build.
    }
  }
  const native = window.anywhereNative ?? {};
  Object.defineProperty(native, 'onmessage', {
    value: null,
    writable: true,
    configurable: false,
  });
  if (typeof native.postMessage !== 'function') {
    Object.defineProperty(native, 'postMessage', {
      value(frame) {
        if (typeof frame !== 'string') throw new TypeError('INVALID_FRAME');
        window.ipc.postMessage(frame);
      },
      configurable: false,
      writable: true,
    });
  }
  Object.defineProperty(window, 'anywhereNative', {
    value: native,
    configurable: false,
    writable: false,
  });
  const hostEscapeFrame = '{"method":"host.escape"}';
  window.addEventListener(
    'keydown',
    event => {
      if (
        event.key !== 'Escape' ||
        event.repeat ||
        event.isComposing ||
        event.keyCode === 229 ||
        event.metaKey ||
        event.ctrlKey ||
        event.altKey
      ) {
        return;
      }
      native.postMessage(hostEscapeFrame);
    },
    true,
  );
  const receivers = new Set();
  const pending = new Set();
  let focused;
  window.addEventListener('anywhere:panel', event => {
    if (event.detail) focused = document.activeElement;
    else focused?.focus?.();
  });
  let visible = true;
  const encoder = new TextEncoder();
  const decoder = new TextDecoder('utf-8', {fatal: true, ignoreBOM: true});
  window.anywhereNative.onmessage = event => {
    const bytes = event.data;
    if (!(bytes instanceof ArrayBuffer) || bytes.byteLength > 65536) return;
    let frame, message;
    try {
      frame = decoder.decode(bytes);
      message = JSON.parse(frame);
    } catch {
      return;
    }
    pending.delete(message.requestId);
    for (const receive of receivers) receive(frame);
  };
  window.addEventListener('anywhere:state', event => {
    if (!visible || typeof event.detail !== 'string') return;
    for (const receive of receivers) receive(event.detail);
  });
  Object.defineProperty(window, '__anywhereTransport', {
    value: Object.freeze({
      send(frame) {
        if (!visible) throw new Error('INACTIVE');
        const message = JSON.parse(frame);
        const cancellation = message.method === 'request.cancel';
        // Cancellation is unqueued and still checked independently by Rust.
        if (
          encoder.encode(frame).length > 65536 ||
          (!cancellation && pending.size >= 32)
        ) {
          throw new Error('BUSY');
        }
        if (cancellation) pending.delete(message.params?.request_id);
        else pending.add(message.requestId);
        try {
          window.anywhereNative.postMessage(frame);
        } catch (error) {
          pending.delete(message.requestId);
          throw error;
        }
      },
      subscribe(receive) {
        receivers.add(receive);
        return () => receivers.delete(receive);
      },
    }),
    configurable: false,
    writable: false,
  });
  window.addEventListener('anywhere:visibility', event => {
    visible = event.detail === true;
    if (!visible) {
      for (const requestId of pending) {
        const frame = JSON.stringify({
          requestId,
          error: {code: 'INACTIVE'},
        });
        for (const receive of receivers) receive(frame);
      }
      pending.clear();
    }
  });
})();

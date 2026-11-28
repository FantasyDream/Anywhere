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
  window.addEventListener('keydown', event => {
    if (
      !visible ||
      event.defaultPrevented ||
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
    event.preventDefault();
    native.postMessage(hostEscapeFrame);
  });
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
    const data = event.data;
    let frame, message;
    try {
      if (typeof data === 'string') {
        if (data.length > 65536 || encoder.encode(data).length > 65536) return;
        frame = data;
      } else {
        if (!(data instanceof ArrayBuffer) || data.byteLength > 65536) return;
        frame = decoder.decode(data);
      }
      message = JSON.parse(frame);
    } catch {
      return;
    }
    if (
      !visible ||
      message === null ||
      typeof message !== 'object' ||
      Array.isArray(message) ||
      typeof message.requestId !== 'string' ||
      !pending.has(message.requestId)
    )
      return;
    const object = value =>
      value !== null && typeof value === 'object' && !Array.isArray(value);
    const result = Object.hasOwn(message, 'result');
    const error = Object.hasOwn(message, 'error');
    if (
      result === error ||
      (result && !object(message.result)) ||
      (error &&
        (!object(message.error) || typeof message.error.code !== 'string'))
    )
      return;
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
        if (encoder.encode(frame).length > 65536) {
          throw new Error('TOO_LARGE');
        }
        const message = JSON.parse(frame);
        const cancellation = message.method === 'request.cancel';
        // Cancellation is unqueued and only forwarded for an active pending request.
        if (cancellation) {
          if (!pending.delete(message.params?.requestId)) return;
        } else {
          if (pending.size >= 32) throw new Error('BUSY');
          pending.add(message.requestId);
        }
        try {
          window.anywhereNative.postMessage(frame);
        } catch (error) {
          if (!cancellation) pending.delete(message.requestId);
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

/** Host presentation events. A listener cannot grant or extend a capability. */
export const lifecycle = {
  onVisibility(listener: (visible: boolean) => void): () => void {
    const receive = (event: Event) =>
      listener((event as CustomEvent).detail === true);
    globalThis.addEventListener('anywhere:visibility', receive);
    return () => globalThis.removeEventListener('anywhere:visibility', receive);
  },
};

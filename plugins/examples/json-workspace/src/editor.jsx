import {Fragment, h} from 'preact';
import {useLayoutEffect, useMemo, useRef} from 'preact/hooks';
import {highlightJson} from './highlight.mjs';

export function JsonEditor({editorRef, value, composing, ...props}) {
  const ownRef = useRef();
  const editor = editorRef ?? ownRef;
  const mirror = useRef();
  // The textarea normalizes newlines. Match its display without changing the
  // input that is passed to the formatter or copied to the clipboard.
  const text = useMemo(() => value.replace(/\r\n?/g, '\n'), [value]);
  const spans = useMemo(
    () => (composing ? null : highlightJson(text)),
    [text, composing],
  );

  function syncScroll() {
    if (!mirror.current || !editor.current) return;
    const {scrollLeft, scrollTop} = editor.current;
    mirror.current.style.transform = `translate(${-scrollLeft}px, ${-scrollTop}px)`;
  }

  useLayoutEffect(syncScroll);
  let end = 0;
  return (
    <div class="json-editor" data-highlighted={!!spans}>
      {spans && (
        <div class="json-editor-overlay" aria-hidden="true">
          <pre class="aw-editor" ref={mirror}>
            {spans.map(span => {
              const gap = text.slice(end, span.start);
              end = span.end;
              return (
                <Fragment key={span.start}>
                  {gap}
                  <span class={`json-token-${span.color}`}>
                    {text.slice(span.start, span.end)}
                  </span>
                </Fragment>
              );
            })}
            {text.slice(end)}
          </pre>
        </div>
      )}
      <textarea
        {...props}
        class="aw-editor"
        ref={editor}
        value={value}
        wrap="off"
        spellcheck={false}
        onScroll={syncScroll}
      />
    </div>
  );
}

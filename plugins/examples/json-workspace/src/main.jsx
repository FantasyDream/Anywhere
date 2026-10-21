import {h, render} from 'preact';
import {useEffect, useLayoutEffect, useMemo, useRef, useState} from 'preact/hooks';
import {definePlugin} from '@anywhere/plugin-sdk';
import {MAX_BYTES} from './json.mjs';
import {JSON_ACTIONS, decodeDocument, prepareJsonAction, prepareJsonLaunch} from './actions.mjs';
import {QueryEngine, QueryError, MAX_QUERY_LENGTH} from './query.mjs';
import {stripRoot} from './expression.mjs';
import './style.css';

const encoder = new TextEncoder();
const stamp = (input, path, spaces, mode) => ({input, path: path.trim(), spaces, mode});
const matches = (result, input, path, spaces, mode) => result?.input === input && result.path === path.trim() && result.spaces === spaces && result.mode === mode;
function App() {
  const [input, setInput] = useState('');
  const [mode, setMode] = useState('javascript');
  const [paths, setPaths] = useState({jsonpath:'', javascript:''});
  const path = paths[mode];
  const setPath = value => setPaths(previous => ({...previous, [mode]:value}));
  const [spaces, setSpaces] = useState(2);
  const [processed, setProcessed] = useState(null);
  const [notice, setNotice] = useState(null);
  const [busy, setBusy] = useState(false);
  const [composing, setComposing] = useState(false);
  const inputRef = useRef(), current = useRef({}), timer = useRef();
  const selection = useRef(), edited = useRef(false), indentEdited = useRef(false), operationBusy = useRef(false);
  const engine = useRef(), plugin = useRef();
  engine.current ??= new QueryEngine();
  const fresh = matches(processed, input, path, spaces, mode) && !composing;
  const output = fresh ? processed.output ?? '' : '';
  const count = fresh && !processed.error ? processed.count : null;
  const inputBytes = useMemo(() => encoder.encode(input).length, [input]);
  const outputBytes = useMemo(() => encoder.encode(output).length, [output]);

  function report(error) {
    if (['USER_CANCELLED', 'CANCELLED', 'INACTIVE'].includes(error.code)) {
      setNotice({text: 'Operation cancelled', failed: false});
      return;
    }
    setNotice({text: error.code ? `Host request: ${error.code}` : error.message, failed: true});
  }
  function run() {
    const state = current.current;
    clearTimeout(timer.current);
    if (state.composing) return;
    setNotice(null);
    if (!state.input.trim()) {
      engine.current.clear();
      setProcessed({...stamp(state.input, state.path, state.spaces, state.mode), output: '', count: null});
      return;
    }
    let formatted = state.input;
    try {
      const editor = inputRef.current;
      const anchor = document.activeElement === editor ? {
        start: editor.selectionStart, end: editor.selectionEnd,
        direction: editor.selectionDirection, top: editor.scrollTop, left: editor.scrollLeft,
      } : undefined;
      const result = engine.current.format(state.input, state.spaces, anchor);
      formatted = result.text;
      if (formatted !== state.input) {
        selection.current = result.selection;
        setInput(formatted);
      }
      const resultStamp = stamp(formatted, state.path, state.spaces, state.mode);
      try {
        setProcessed({...resultStamp, ...engine.current.evaluate(formatted, state.path, state.spaces, state.mode)});
      } catch (error) {
        setProcessed({...resultStamp, error: error.message, queryError: error instanceof QueryError});
      }
    } catch (error) {
      setProcessed({...stamp(state.input, state.path, state.spaces, state.mode), error: error.message});
    }
  }
  async function operation(work) {
    if (operationBusy.current) return;
    operationBusy.current = true;
    setBusy(true);
    try { await work(); } catch (error) { report(error); }
    finally { operationBusy.current = false; setBusy(false); }
  }
  function copyResult() {
    const state = current.current;
    if (state.composing || !matches(state.processed, state.input, state.path, state.spaces, state.mode) || !state.processed.output) return;
    const text = state.processed.output;
    void operation(async () => {
      await plugin.current.host.clipboard.writeText(text);
      setNotice({text: 'Copied query result', failed: false});
    });
  }
  function jsonAction(id) {
    const state = current.current;
    if (state.composing) return;
    void operation(async () => {
      const result = prepareJsonAction(state.input, id, state.spaces);
      if (result.copy) await plugin.current.host.clipboard.writeText(result.text);
      else { edited.current = true; load(result.text); }
      setNotice({text: result.message, failed: false});
    });
  }
  function load(text) {
    if (encoder.encode(text).length > MAX_BYTES) throw new Error('Input exceeds 1 MiB');
    selection.current = {start: 0, end: 0, direction: 'none', top: 0, left: 0};
    setNotice(null); setInput(text);
    requestAnimationFrame(() => inputRef.current?.focus());
  }
  function edit(event) {
    const text = event.currentTarget.value;
    if (encoder.encode(text).length > MAX_BYTES) {
      event.currentTarget.value = input;
      report(new Error('Input exceeds 1 MiB')); return;
    }
    edited.current = true;
    setNotice(null); setInput(text);
  }
  current.current = {input, path, spaces, mode, processed, composing, run, copyResult, setMode};

  useEffect(() => {
    if (composing || matches(processed, input, path, spaces, mode)) return;
    timer.current = setTimeout(() => current.current.run(), 300);
    return () => clearTimeout(timer.current);
  }, [input, path, spaces, mode, composing]);
  useLayoutEffect(() => {
    const pending = selection.current, editor = inputRef.current;
    if (!pending || !editor) return;
    selection.current = undefined;
    editor.setSelectionRange(pending.start, pending.end, pending.direction);
    editor.scrollTop = pending.top; editor.scrollLeft = pending.left;
  }, [input]);
  useEffect(() => {
    const instance = definePlugin({
      onError: report,
      async onLaunch(invocation, {signal}) {
        // Show the source immediately. Optional preferences cannot block input.
        if (!edited.current && invocation.input.kind === 'text') load(invocation.input.text);
        else if (!edited.current) inputRef.current?.focus();
        let indent = 2;
        try {
          const stored = await instance.host.storage.get('indent');
          if ([2,4].includes(stored)) indent = stored;
        } catch { /* A copy command still works with the default indentation. */ }
        if (signal.aborted || edited.current || operationBusy.current) return;
        if (!indentEdited.current) setSpaces(indent);
        try {
          const launch = prepareJsonLaunch(invocation, indent);
          if (!launch) return;
          load(launch.input);
          if (launch.clipboard !== null) {
            await operation(async () => {
              await instance.host.clipboard.writeText(launch.clipboard);
              setNotice({text: launch.message, failed: false});
            });
          } else if (launch.message) setNotice({text: launch.message, failed: false});
        } catch (error) { report(error); }
      },
    });
    plugin.current = instance;
    const shortcut = event => {
      if (event.isComposing || current.current.composing) return;
      if (event.ctrlKey && !event.metaKey && !event.altKey && event.key === 'Tab') {
        event.preventDefault();
        current.current.setMode(value => value === 'jsonpath' ? 'javascript' : 'jsonpath');
        document.getElementById('json-path')?.focus(); return;
      }
      if (!event.metaKey || event.altKey || event.ctrlKey) return;
      if (event.key === 'Enter') { event.preventDefault(); current.current.run(); }
      else if (event.shiftKey && event.key.toLowerCase() === 'c') { event.preventDefault(); current.current.copyResult(); }
    };
    window.addEventListener('keydown', shortcut);
    return () => { instance.dispose(); clearTimeout(timer.current); engine.current.clear(); window.removeEventListener('keydown', shortcut); };
  }, []);

  const error = fresh ? processed.error : null;
  const isJavaScript = mode === 'javascript';
  const summary = error || (!input.trim() ? 'Enter JSON to begin · processing stays on this device'
    : !fresh ? (composing ? 'Editing…' : 'Updating…')
    : count === 0 ? 'No matches · try a different path'
    : isJavaScript ? 'JavaScript calculation · only JSON data is available'
    : `${count} ${count === 1 ? 'match' : 'matches'} · single values are shown directly; multiple values as an array`);
  return <main class="aw-ui aw-workspace">
    <nav class="aw-workspace-toolbar" aria-label="JSON tools">
      <div class="aw-tabs" role="group" aria-label="Formula mode">
        {[['javascript','JavaScript'],['jsonpath','JSONPath']].map(([value,label]) =>
          <button class="aw-tab" aria-pressed={mode === value} title="Switch mode · Ctrl+Tab / Ctrl+Shift+Tab" onClick={() => {
            setMode(value); setNotice(null); document.getElementById('json-path')?.focus();
          }}>{label}</button>)}
      </div>
      <div class="aw-actions"><label class="indent">Indent <select value={spaces} onChange={event => {
        const value = Number(event.currentTarget.value); indentEdited.current = true; setNotice(null); setSpaces(value);
        void operation(() => plugin.current.host.storage.set('indent', value));
      }}><option value={2}>2 spaces</option><option value={4}>4 spaces</option></select></label>
      <button disabled={busy} onClick={() => operation(async () => {const text = await plugin.current.host.clipboard.readText(); if (text != null) {edited.current = true; const document = decodeDocument(text); load(document.text); if (document.escaped) setNotice({text: 'Decoded escaped JSON', failed: false});}})}>Paste…</button></div>
    </nav>
    <div class="query-bar">
      <div class="formula-field" data-invalid={fresh && processed.queryError ? 'true' : 'false'}>
      <label for="json-path" class="formula-root" title="Automatically included in the formula">{isJavaScript ? 'this' : '$'}</label>
      <input id="json-path" aria-label={isJavaScript ? 'JavaScript formula after this' : 'JSONPath after $'} type="text" value={path} maxLength={MAX_QUERY_LENGTH} spellcheck={false} autocomplete="off" autocapitalize="off"
        aria-describedby={fresh && processed.queryError ? "json-error" : undefined}
        placeholder={isJavaScript ? ".users.map(x => x.name)" : ".users[*].name"} aria-invalid={fresh && processed.queryError ? 'true' : 'false'}
        onInput={event => {setNotice(null); setPath(event.currentTarget.value);}}
        onPaste={event => {
          const editor = event.currentTarget, text = event.clipboardData?.getData('text/plain');
          if (text && editor.selectionStart === 0 && editor.selectionEnd === path.length && stripRoot(text) !== text) {
            event.preventDefault(); setNotice(null); setPath(stripRoot(text));
          }
        }}
        onBlur={() => setPath(stripRoot(path))}
        onCompositionStart={() => setComposing(true)} onCompositionEnd={() => setComposing(false)}
        onKeyDown={event => {if (event.key === 'Enter' && !event.isComposing && !event.metaKey && !event.ctrlKey) {event.preventDefault(); current.current.run();}}}/>
      </div>
      <button class="aw-button--action" disabled={busy || composing || !input.trim()} onClick={run} title="Calculate result · ⌘↵">{isJavaScript ? 'Calculate' : 'Query'} <kbd>⌘↵</kbd></button>
    </div>
    {(error || notice?.failed) && <aside id="json-error" role="alert" class="aw-workspace-notice" data-kind="error">{notice?.failed ? notice.text : error}</aside>}
    <section class="aw-workspace-panes">
      <article class="aw-pane"><header class="aw-pane-header"><label for="json-input">JSON input</label><span>{inputBytes.toLocaleString()} bytes</span></header>
        <div class="aw-pane-body"><textarea id="json-input" class="aw-editor" ref={inputRef} spellcheck={false} autocomplete="off" autocapitalize="off"
          placeholder="Paste JSON here" value={input} onInput={edit}
          onCompositionStart={() => setComposing(true)} onCompositionEnd={() => setComposing(false)}/></div>
        <footer class="aw-pane-footer json-input-actions">
          <div class="aw-actions" aria-label="JSON actions">{JSON_ACTIONS.map(action =>
            <button disabled={!input.trim() || busy || composing} onClick={() => jsonAction(action.id)}>{action.label}</button>)}</div>
          <div class="input-status"><span>{notice && !notice.failed ? notice.text : 'Auto-format on'}</span>
          <button disabled={!input || busy} onClick={() => {
            edited.current = true; engine.current.clear(); setInput(''); setNotice(null); inputRef.current?.focus();
          }}>Clear</button></div></footer></article>
      <article class="aw-pane"><header class="aw-pane-header"><label for="json-output">Result</label><span>{count == null ? '—' : isJavaScript ? '1 result' : `${count} ${count === 1 ? 'match' : 'matches'}`} · {outputBytes.toLocaleString()} bytes</span></header>
        <div class="aw-pane-body"><textarea id="json-output" class="aw-editor" readonly spellcheck={false} placeholder="Results appear here" value={output}/></div>
        <footer class="aw-pane-footer"><span/>
          <button class="aw-button--action" disabled={!output || busy} onClick={copyResult} title="Copy result · ⌘⇧C">{notice?.text === 'Copied query result' ? 'Copied' : 'Copy result…'} <kbd>⌘⇧C</kbd></button></footer></article>
    </section>
    <output role="status" aria-live="polite" class="sr-only">{busy ? 'Waiting for Host…' : notice?.text ?? summary}</output>
  </main>;
}
render(<App/>, document.getElementById('app'));

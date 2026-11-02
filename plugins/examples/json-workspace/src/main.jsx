import {h, render} from 'preact';
import {
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from 'preact/hooks';
import {definePlugin, environment} from '@anywhere/sdk';
import {i18n, t} from './i18n.mjs';
import {MAX_BYTES} from './json.mjs';
import {JSON_ACTIONS, decodeDocument, prepareJsonAction} from './actions.mjs';
import {QueryEngine, QueryError, MAX_QUERY_LENGTH} from './query.mjs';
import {stripRoot} from './expression.mjs';
import {workspaceInput} from './workspace-input.mjs';
import {jsonInputText} from './input.mjs';
import './style.css';

const encoder = new TextEncoder();
const stamp = (input, path, spaces, mode) => ({
  input,
  path: path.trim(),
  spaces,
  mode,
});
const matches = (result, input, path, spaces, mode) =>
  result?.input === input &&
  result.path === path.trim() &&
  result.spaces === spaces &&
  result.mode === mode;
function App() {
  const [language, setLanguage] = useState(environment.language);
  useEffect(
    () =>
      environment.onLanguageChange(value => {
        i18n.changeLanguage(value);
        setLanguage(value);
      }),
    [],
  );
  const [input, setInput] = useState('');
  const [mode, setMode] = useState('javascript');
  const [paths, setPaths] = useState({jsonpath: '', javascript: ''});
  const path = paths[mode];
  const setPath = value => setPaths(previous => ({...previous, [mode]: value}));
  const [spaces, setSpaces] = useState(2);
  const [processed, setProcessed] = useState(null);
  const [notice, setNotice] = useState(null);
  const [busy, setBusy] = useState(false);
  const [actionsOpen, setActionsOpen] = useState(false);
  const [composing, setComposing] = useState(false);
  const inputRef = useRef(),
    current = useRef({}),
    timer = useRef();
  const selection = useRef(),
    edited = useRef(false),
    indentEdited = useRef(false),
    operationBusy = useRef(false);
  const engine = useRef(),
    plugin = useRef(),
    actionsRef = useRef();
  engine.current ??= new QueryEngine();
  const fresh = matches(processed, input, path, spaces, mode) && !composing;
  const output = fresh ? (processed.output ?? '') : '';
  const count = fresh && !processed.error ? processed.count : null;
  const inputBytes = useMemo(() => encoder.encode(input).length, [input]);
  const outputBytes = useMemo(() => encoder.encode(output).length, [output]);

  function report(error) {
    if (['USER_CANCELLED', 'CANCELLED', 'INACTIVE'].includes(error.code)) {
      setNotice({key: 'cancelled', failed: false});
      return;
    }
    setNotice({
      text: error.code ? `Host request: ${error.code}` : error.message,
      failed: true,
    });
  }
  function run() {
    const state = current.current;
    clearTimeout(timer.current);
    if (state.composing) return;
    setNotice(null);
    if (!state.input.trim()) {
      engine.current.clear();
      setProcessed({
        ...stamp(state.input, state.path, state.spaces, state.mode),
        output: '',
        count: null,
      });
      return;
    }
    let formatted = state.input;
    try {
      const editor = inputRef.current;
      const anchor =
        document.activeElement === editor
          ? {
              start: editor.selectionStart,
              end: editor.selectionEnd,
              direction: editor.selectionDirection,
              top: editor.scrollTop,
              left: editor.scrollLeft,
            }
          : undefined;
      const result = engine.current.format(state.input, state.spaces, anchor);
      formatted = result.text;
      if (formatted !== state.input) {
        selection.current = result.selection;
        setInput(formatted);
      }
      const resultStamp = stamp(
        formatted,
        state.path,
        state.spaces,
        state.mode,
      );
      try {
        setProcessed({
          ...resultStamp,
          ...engine.current.evaluate(
            formatted,
            state.path,
            state.spaces,
            state.mode,
          ),
        });
      } catch (error) {
        setProcessed({
          ...resultStamp,
          error: error.message,
          queryError: error instanceof QueryError,
        });
      }
    } catch (error) {
      setProcessed({
        ...stamp(state.input, state.path, state.spaces, state.mode),
        error: error.message,
      });
    }
  }
  async function operation(work) {
    if (operationBusy.current) return;
    operationBusy.current = true;
    setBusy(true);
    try {
      await work();
    } catch (error) {
      report(error);
    } finally {
      operationBusy.current = false;
      setBusy(false);
    }
  }
  function copyResult() {
    const state = current.current;
    if (
      state.composing ||
      !matches(
        state.processed,
        state.input,
        state.path,
        state.spaces,
        state.mode,
      ) ||
      !state.processed.output
    )
      return;
    const text = state.processed.output;
    void operation(async () => {
      await plugin.current.anywhere.clipboard.writeText(text);
      setNotice({key: 'copiedResult', failed: false});
    });
  }
  function jsonAction(id) {
    const state = current.current;
    if (state.composing) return;
    void operation(async () => {
      const result = prepareJsonAction(state.input, id, state.spaces);
      await plugin.current.anywhere.clipboard.writeText(result.text);
      setNotice({key: result.message, failed: false});
    });
  }
  function load(text) {
    if (encoder.encode(text).length > MAX_BYTES)
      throw new Error('Input exceeds 1 MiB');
    selection.current = {start: 0, end: 0, direction: 'none', top: 0, left: 0};
    setNotice(null);
    setInput(text);
  }
  function edit(event) {
    const text = event.currentTarget.value;
    if (encoder.encode(text).length > MAX_BYTES) {
      event.currentTarget.value = input;
      report(new Error('Input exceeds 1 MiB'));
      return;
    }
    edited.current = true;
    setNotice(null);
    setInput(text);
  }
  current.current = {
    input,
    path,
    spaces,
    mode,
    processed,
    composing,
    run,
    copyResult,
    setMode,
  };

  useEffect(() => {
    if (composing || matches(processed, input, path, spaces, mode)) return;
    timer.current = setTimeout(() => current.current.run(), 300);
    return () => clearTimeout(timer.current);
  }, [input, path, spaces, mode, composing]);
  useLayoutEffect(() => {
    const pending = selection.current,
      editor = inputRef.current;
    if (!pending || !editor) return;
    selection.current = undefined;
    editor.setSelectionRange(pending.start, pending.end, pending.direction);
    editor.scrollTop = pending.top;
    editor.scrollLeft = pending.left;
  }, [input]);
  useEffect(() => {
    const incoming = workspaceInput({
      canReplace: () => !edited.current && !operationBusy.current,
      load,
      notice: key => setNotice({key, failed: false}),
      report,
    });
    const instance = definePlugin({
      onError: report,
      async onLaunch(invocation, {signal}) {
        incoming.launch(invocation);
        requestAnimationFrame(() => {
          const target =
            jsonInputText(invocation.input) === null
              ? inputRef.current
              : document.getElementById('json-path');
          target?.focus();
        });
        try {
          const stored = await instance.anywhere.storage.get('indent');
          if (
            !signal.aborted &&
            !edited.current &&
            !indentEdited.current &&
            [2, 4].includes(stored)
          )
            setSpaces(stored);
        } catch {
          /* Optional preferences never block the launch input. */
        }
      },
    });
    const stopContext = instance.anywhere.context.onChange(input =>
      incoming.context(input),
    );
    plugin.current = instance;
    const shortcut = event => {
      if (event.isComposing || current.current.composing) return;
      if (
        event.ctrlKey &&
        !event.metaKey &&
        !event.altKey &&
        event.key === 'Tab'
      ) {
        event.preventDefault();
        current.current.setMode(value =>
          value === 'jsonpath' ? 'javascript' : 'jsonpath',
        );
        document.getElementById('json-path')?.focus();
        return;
      }
      if (!event.metaKey || event.altKey || event.ctrlKey) return;
      if (!event.shiftKey && event.key.toLowerCase() === 'l') {
        event.preventDefault();
        document.getElementById('json-path')?.focus();
      } else if (event.key === 'Enter') {
        event.preventDefault();
        current.current.run();
      } else if (event.shiftKey && event.key.toLowerCase() === 'c') {
        event.preventDefault();
        current.current.copyResult();
      }
    };
    const dismissActions = event => {
      const menu = actionsRef.current;
      if (menu && !menu.contains(event.target)) setActionsOpen(false);
    };
    window.addEventListener('pointerdown', dismissActions);
    window.addEventListener('keydown', shortcut);
    return () => {
      stopContext();
      instance.dispose();
      clearTimeout(timer.current);
      engine.current.clear();
      window.removeEventListener('keydown', shortcut);
      window.removeEventListener('pointerdown', dismissActions);
    };
  }, []);

  const error = fresh ? processed.error : null;
  const isJavaScript = mode === 'javascript';
  const summary =
    error ||
    (!input.trim()
      ? t('begin')
      : !fresh
        ? composing
          ? t('editing')
          : t('updating')
        : count === 0
          ? t('noMatches')
          : isJavaScript
            ? t('jsSummary')
            : t('pathSummary', {count}));
  return (
    <main class="aw-ui aw-workspace json-workspace" lang={language}>
      <nav class="json-query-toolbar" aria-label={t('tools')}>
        <select
          aria-label={t('mode')}
          title={t('switchMode')}
          value={mode}
          onChange={event => {
            setMode(event.currentTarget.value);
            setNotice(null);
            document.getElementById('json-path')?.focus();
          }}
        >
          <option value="javascript">JavaScript</option>
          <option value="jsonpath">JSONPath</option>
        </select>
        <div
          class="aw-input-group"
          data-invalid={fresh && processed.queryError ? 'true' : 'false'}
        >
          <label
            for="json-path"
            class="aw-input-prefix aw-code"
            title={t('rootHint')}
          >
            {isJavaScript ? 'this' : '$'}
          </label>
          <input
            id="json-path"
            class="aw-code"
            aria-label={isJavaScript ? t('jsFormula') : t('pathFormula')}
            title={t('focusFormula')}
            type="text"
            value={path}
            maxLength={MAX_QUERY_LENGTH}
            spellcheck={false}
            autocomplete="off"
            autocapitalize="off"
            aria-describedby={
              fresh && processed.queryError ? 'json-error' : undefined
            }
            placeholder={
              isJavaScript ? '.users.map(x => x.name)' : '.users[*].name'
            }
            aria-invalid={fresh && processed.queryError ? 'true' : 'false'}
            onInput={event => {
              setNotice(null);
              setPath(event.currentTarget.value);
            }}
            onPaste={event => {
              const editor = event.currentTarget,
                text = event.clipboardData?.getData('text/plain');
              if (
                text &&
                editor.selectionStart === 0 &&
                editor.selectionEnd === path.length &&
                stripRoot(text) !== text
              ) {
                event.preventDefault();
                setNotice(null);
                setPath(stripRoot(text));
              }
            }}
            onBlur={() => setPath(stripRoot(path))}
            onCompositionStart={() => setComposing(true)}
            onCompositionEnd={() => setComposing(false)}
            onKeyDown={event => {
              if (
                event.key === 'Enter' &&
                !event.isComposing &&
                !event.metaKey &&
                !event.ctrlKey
              ) {
                event.preventDefault();
                current.current.run();
              }
            }}
          />
        </div>
        <button
          class="aw-button--action"
          disabled={busy || composing || !input.trim()}
          onClick={run}
          title={t('calculateHint')}
        >
          <span class="aw-icon aw-icon--run" aria-hidden="true" />
          {isJavaScript ? t('calculate') : t('query')} <kbd>⌘↵</kbd>
        </button>
      </nav>
      {(error || notice?.failed) && (
        <aside
          id="json-error"
          role="alert"
          class="aw-workspace-notice"
          data-kind="error"
        >
          {notice?.failed ? notice.text : error}
        </aside>
      )}
      <section class="aw-workspace-panes json-panes">
        <article class="aw-pane">
          <header class="aw-pane-header">
            <div class="json-pane-heading">
              <label for="json-input" title={t('autoFormat')}>
                {t('input')}
              </label>
              <span role="status">
                {notice && !notice.failed && notice.key !== 'copiedResult'
                  ? t(notice.key)
                  : t('bytes', {count: inputBytes})}
              </span>
            </div>
            <div
              class="json-menu"
              data-open={actionsOpen}
              ref={actionsRef}
              onKeyDown={event => {
                if (event.isComposing || composing) return;
                const menu = event.currentTarget;
                if (event.key === 'Escape' && actionsOpen) {
                  event.preventDefault();
                  event.stopPropagation();
                  setActionsOpen(false);
                  menu.querySelector('.json-more').focus();
                } else if (
                  ['ArrowDown', 'ArrowUp'].includes(event.key) &&
                  event.target.tagName !== 'SELECT'
                ) {
                  event.preventDefault();
                  setActionsOpen(true);
                  const focusItem = () => {
                    const controls = [
                      ...menu.querySelectorAll(
                        '.json-menu-panel select:not(:disabled), .json-menu-panel button:not(:disabled)',
                      ),
                    ];
                    if (!controls.length) return;
                    const index = controls.indexOf(event.target);
                    const next =
                      index < 0
                        ? event.key === 'ArrowDown'
                          ? 0
                          : controls.length - 1
                        : (index +
                            (event.key === 'ArrowDown' ? 1 : -1) +
                            controls.length) %
                          controls.length;
                    controls[next].focus();
                  };
                  if (actionsOpen) focusItem();
                  else requestAnimationFrame(focusItem);
                }
              }}
              onFocusOut={event => {
                if (
                  event.relatedTarget &&
                  !event.currentTarget.contains(event.relatedTarget)
                )
                  setActionsOpen(false);
              }}
            >
              <button
                class="json-more"
                aria-label={t('actions')}
                title={t('actions')}
                aria-expanded={actionsOpen}
                aria-controls={actionsOpen ? 'json-actions-panel' : undefined}
                onClick={() => setActionsOpen(value => !value)}
              >
                <span aria-hidden="true">⋯</span>
              </button>
              {actionsOpen && (
                <div
                  id="json-actions-panel"
                  class="json-menu-panel"
                  role="group"
                  aria-label={t('actions')}
                >
                  <label class="indent">
                    {t('indent')}
                    <select
                      aria-label={t('indent')}
                      value={spaces}
                      disabled={busy || composing}
                      onChange={event => {
                        const value = Number(event.currentTarget.value);
                        setActionsOpen(false);
                        actionsRef.current.querySelector('.json-more').focus();
                        indentEdited.current = true;
                        setNotice(null);
                        setSpaces(value);
                        void operation(() =>
                          plugin.current.anywhere.storage.set('indent', value),
                        );
                      }}
                    >
                      <option value={2}>{t('spaces', {count: 2})}</option>
                      <option value={4}>{t('spaces', {count: 4})}</option>
                    </select>
                  </label>
                  <hr />
                  {JSON_ACTIONS.map(action => (
                    <button
                      disabled={!input.trim() || busy || composing}
                      onClick={() => {
                        setActionsOpen(false);
                        actionsRef.current.querySelector('.json-more').focus();
                        jsonAction(action.id);
                      }}
                    >
                      <span
                        class={`aw-icon aw-icon--${action.icon}`}
                        aria-hidden="true"
                      />
                      {t(action.id)}
                    </button>
                  ))}
                  <hr />
                  <button
                    disabled={!input || busy || composing}
                    onClick={() => {
                      setActionsOpen(false);
                      edited.current = true;
                      engine.current.clear();
                      setInput('');
                      setNotice(null);
                      inputRef.current?.focus();
                    }}
                  >
                    <span class="aw-icon aw-icon--clear" aria-hidden="true" />
                    {t('clear')}
                  </button>
                </div>
              )}
            </div>
          </header>
          <div class="aw-pane-body">
            <textarea
              id="json-input"
              class="aw-editor"
              ref={inputRef}
              spellcheck={false}
              autocomplete="off"
              autocapitalize="off"
              placeholder={t('pasteHint')}
              value={input}
              onInput={edit}
              onPaste={event => {
                const editor = event.currentTarget;
                if (
                  composing ||
                  (input &&
                    (editor.selectionStart !== 0 ||
                      editor.selectionEnd !== input.length))
                )
                  return;
                const text = event.clipboardData?.getData('text/plain');
                if (!text) return;
                try {
                  const document = decodeDocument(text);
                  if (!document.escaped) return;
                  event.preventDefault();
                  edited.current = true;
                  load(document.text);
                  setNotice({key: 'decoded', failed: false});
                } catch {
                  // Let native paste and the existing validator handle incomplete text.
                }
              }}
              onCompositionStart={() => setComposing(true)}
              onCompositionEnd={() => setComposing(false)}
            />
          </div>
        </article>
        <article class="aw-pane">
          <header class="aw-pane-header">
            <div class="json-pane-heading">
              <label for="json-output">{t('result')}</label>
              <span>
                {count == null
                  ? '—'
                  : isJavaScript
                    ? t('resultCount', {count: 1})
                    : t('match', {count})}
                {' · '}
                {t('bytes', {count: outputBytes})}
              </span>
            </div>
            <button
              class="aw-button--action"
              disabled={!output || busy || composing}
              onClick={copyResult}
              title={t('copyHint')}
            >
              <span
                class={`aw-icon aw-icon--${notice?.key === 'copiedResult' ? 'check' : 'copy'}`}
                aria-hidden="true"
              />
              {notice?.key === 'copiedResult' ? t('copied') : t('copyResult')}
              <kbd>⌘⇧C</kbd>
            </button>
          </header>
          <div class="aw-pane-body">
            <textarea
              id="json-output"
              class="aw-editor"
              readonly
              spellcheck={false}
              placeholder={t('outputHint')}
              value={output}
            />
          </div>
        </article>
      </section>
      <output role="status" aria-live="polite" class="sr-only">
        {busy
          ? t('waiting')
          : ((notice?.key ? t(notice.key) : notice?.text) ?? summary)}
      </output>
    </main>
  );
}
render(<App />, document.getElementById('app'));

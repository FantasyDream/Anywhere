import {jsonInputText} from './input.mjs';
import {prepareJsonLaunch} from './actions.mjs';

/** Launch input and live Context share one ordering rule; edited drafts always win. */
export function workspaceInput({canReplace, load, notice, report}) {
  let ready = false,
    pending,
    lastSource;
  function apply(input, entrypoint = 'workspace') {
    if (!canReplace()) return;
    const source = jsonInputText(input);
    if (source === null || source === lastSource) return;
    lastSource = source;
    try {
      const launch = prepareJsonLaunch({input, entrypoint});
      load(launch.input);
      if (launch.message) notice(launch.message);
    } catch (error) {
      load(source);
      report(error);
    }
  }
  return {
    launch(invocation) {
      apply(invocation.input, invocation.entrypoint);
      ready = true;
      if (pending) apply(pending);
      pending = undefined;
    },
    context(input) {
      if (!ready) pending = input;
      else apply(input);
    },
  };
}

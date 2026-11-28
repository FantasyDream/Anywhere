import {anywhere, defineResolver} from '@anywhere/sdk';

const typeId = 'plugin.dev.example.issue-link.issue';
const issue = /^[A-Z][A-Z0-9]{1,9}-[1-9]\d{0,8}$/;

export const resolve = defineResolver(async ({context}) => {
  const input = await context.get();
  if (input.kind !== 'text') return [];
  const key = input.text.trim();
  return issue.test(key) ? [{typeId, value: key}] : [];
});

export async function open() {
  const input = await anywhere.context.get();
  if (
    input.kind !== 'data' ||
    input.typeId !== typeId ||
    typeof input.value !== 'string' ||
    !issue.test(input.value)
  ) {
    return {message: 'Select an issue number such as HELP-123.'};
  }
  try {
    await anywhere.url.open(`https://issues.example.com/browse/${input.value}`);
  } catch (error) {
    if (error.code === 'USER_CANCELLED')
      return {message: 'Opening the issue was cancelled.'};
    throw error;
  }
}

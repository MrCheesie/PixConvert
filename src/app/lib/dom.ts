/** Minimal DOM helpers — keeps the components free of HTML strings. */

type Attrs = Record<string, string | number | boolean | undefined | EventListener>;
type Child = Node | string | null | undefined;

export function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attrs: Attrs = {},
  children: Child[] = [],
): HTMLElementTagNameMap[K] {
  const element = document.createElement(tag);
  for (const [key, value] of Object.entries(attrs)) {
    if (value === undefined || value === false) continue;
    if (key.startsWith('on') && typeof value === 'function') {
      element.addEventListener(key.slice(2).toLowerCase(), value as EventListener);
    } else if (key === 'class') {
      element.className = String(value);
    } else if (key === 'value' || key === 'checked') {
      (element as unknown as Record<string, unknown>)[key] = value;
    } else if (value === true) {
      element.setAttribute(key, '');
    } else {
      element.setAttribute(key, String(value));
    }
  }
  for (const child of children) {
    if (child === null || child === undefined) continue;
    element.append(typeof child === 'string' ? document.createTextNode(child) : child);
  }
  return element;
}

/** `<label>` + control wrapper used by every option panel. */
export function field(labelText: string, control: HTMLElement, hint?: string): HTMLElement {
  return h('label', { class: 'field' }, [
    h('span', { class: 'field__label' }, [labelText]),
    control,
    hint ? h('span', { class: 'field__hint' }, [hint]) : null,
  ]);
}

export function numberInput(value: number, min = 0, max = 100000, step = 1): HTMLInputElement {
  const input = h('input', { type: 'number', class: 'input', value, min, max, step });
  return input;
}

export function textInput(placeholder = '', value = ''): HTMLInputElement {
  return h('input', { type: 'text', class: 'input', placeholder, value });
}

export function selectBox(options: { value: string; label: string }[], value?: string): HTMLSelectElement {
  const select = h('select', { class: 'input' });
  for (const option of options) {
    const node = h('option', { value: option.value }, [option.label]);
    if (option.value === value) node.selected = true;
    select.append(node);
  }
  return select;
}

export function toggle(label: string, checked = false): { element: HTMLElement; input: HTMLInputElement } {
  const input = h('input', { type: 'checkbox', checked });
  const element = h('label', { class: 'toggle' }, [input, h('span', {}, [label])]);
  return { element, input };
}

export function button(label: string, onClick: () => void, variant = 'primary'): HTMLButtonElement {
  return h('button', { class: `btn btn--${variant}`, type: 'button', onClick }, [label]);
}

export function panel(children: Child[]): HTMLElement {
  return h('div', { class: 'panel' }, children);
}

export function statusLine(): HTMLElement {
  return h('p', { class: 'status', role: 'status' });
}

export function setStatus(element: HTMLElement, message: string, kind: 'info' | 'error' | 'success' = 'info'): void {
  element.textContent = message;
  element.className = `status status--${kind}`;
}

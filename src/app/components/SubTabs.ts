/** Convert / Compress / Edit tab bar used inside every studio. */

import { h } from '../lib/dom';
import type { SubTabId } from '../lib/types';

export interface SubTabDef {
  id: SubTabId;
  label: string;
}

export interface SubTabsHandle {
  element: HTMLElement;
  setActive: (id: SubTabId) => void;
  getActive: () => SubTabId;
}

export function createSubTabs(
  tabs: SubTabDef[],
  panels: Record<SubTabId, HTMLElement>,
  initial: SubTabId = 'convert',
  onChange?: (id: SubTabId) => void,
): SubTabsHandle {
  let active = initial;
  const buttons = new Map<SubTabId, HTMLButtonElement>();

  const bar = h('div', { class: 'subtabs', role: 'tablist' });
  for (const tab of tabs) {
    const button = h('button', {
      class: 'subtabs__tab',
      type: 'button',
      role: 'tab',
      'data-tab': tab.id,
    }, [tab.label]);
    button.addEventListener('click', () => {
      setActive(tab.id);
      onChange?.(tab.id);
    });
    buttons.set(tab.id, button);
    bar.append(button);
  }

  const setActive = (id: SubTabId): void => {
    active = id;
    for (const [key, button] of buttons) {
      const isActive = key === id;
      button.classList.toggle('is-active', isActive);
      button.setAttribute('aria-selected', String(isActive));
      const panel = panels[key];
      if (panel) panel.hidden = !isActive;
    }
  };

  setActive(initial);
  return { element: bar, setActive, getActive: () => active };
}

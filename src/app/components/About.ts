/** Header "About" button and the dialog it opens. */

import { button, h } from '../lib/dom';
import { APP_VERSION, DEVELOPER, LICENSE, REPOSITORY } from '../lib/version';

const TITLE_ID = 'aboutTitle';

export function mountAbout(host: HTMLElement): void {
  let lastFocus: HTMLElement | null = null;

  const row = (label: string, value: string, strong = false) => [
    h('dt', {}, [label]),
    h('dd', {}, [strong ? h('strong', {}, [value]) : value]),
  ];

  const closeButton = button('Close', () => hide(), 'secondary');

  const card = h('div', { class: 'about-card' }, [
    h('div', { class: 'about-card__head' }, [
      h('span', { class: 'brand__mark' }, ['PC']),
      h('div', {}, [
        h('h2', { class: 'about-card__title', id: TITLE_ID }, ['PixConvert']),
        h('p', { class: 'about-card__meta' }, [`Version ${APP_VERSION} · local-first`]),
      ]),
    ]),
    h('p', { class: 'about-card__blurb' }, [
      'Image, video, audio and document conversion that runs entirely on this machine — nothing is ever uploaded.',
    ]),
    h('dl', { class: 'about-card__rows' }, [
      ...row('Developer', DEVELOPER, true),
      ...row('Version', APP_VERSION),
      ...row('License', LICENSE),
      ...row('Source', REPOSITORY),
      ...row('Tools', 'ffmpeg and pandoc run locally when a studio needs them'),
    ]),
    h('div', { class: 'about-card__foot' }, [closeButton]),
  ]);

  const overlay = h(
    'div',
    {
      class: 'about-overlay',
      role: 'dialog',
      'aria-modal': 'true',
      'aria-labelledby': TITLE_ID,
      hidden: true,
      onClick: (event: Event) => {
        if (event.target === overlay) hide();
      },
    },
    [card],
  );

  const trigger = button('About', () => show(), 'ghost');
  trigger.classList.add('about-trigger');

  function show(): void {
    lastFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    overlay.hidden = false;
    closeButton.focus();
  }

  function hide(): void {
    if (overlay.hidden) return;
    overlay.hidden = true;
    (lastFocus ?? trigger).focus();
  }

  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') hide();
  });

  host.append(trigger, overlay);
}

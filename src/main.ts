/** PixConvert bootstrap: mounts the router, the About panel and reports browser-mode limits. */

import { mountAbout } from './app/components/About';
import { initRouter } from './app/router';
import { runningInTauri } from './app/lib/tauri';
import { APP_VERSION } from './app/lib/version';

function boot(): void {
  initRouter();

  const aboutHost = document.getElementById('aboutHost');
  if (aboutHost) mountAbout(aboutHost);

  // One place owns the version string so the header can never drift from the
  // About panel (and from the Rust crate / bundle version).
  const brandMeta = document.querySelector<HTMLElement>('.brand__meta');
  if (brandMeta) brandMeta.textContent = `v${APP_VERSION} · local-first · no uploads`;

  if (!runningInTauri()) {
    const notice = document.getElementById('browserNotice');
    if (notice) notice.hidden = false;
  }
}

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', boot, { once: true });
} else {
  boot();
}


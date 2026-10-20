/** PixConvert bootstrap: mounts the router and reports browser-mode limits. */

import { initRouter } from './app/router';
import { runningInTauri } from './app/lib/tauri';

function boot(): void {
  initRouter();

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

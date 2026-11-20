(() => {
  'use strict';

  const nativeWindow = window.__TAURI__?.window.getCurrentWindow();
  if (!nativeWindow) return;
  const label = nativeWindow.label.startsWith('memo-tab-') ? 'memo' : nativeWindow.label;
  const button = document.querySelector(`[data-pin-window="${label}"]`);
  if (!button) return;

  function render(pinned) {
    button.setAttribute('aria-pressed', String(pinned));
    button.title = pinned ? '항상 위 고정 해제' : '항상 위에 고정';
  }

  async function refresh() {
    if (button.disabled) return;
    try {
      render(await nativeWindow.isAlwaysOnTop());
      button.hidden = false;
    } catch (error) {
      console.error('창 고정 상태를 확인하지 못했습니다.', error);
    }
  }

  button.addEventListener('click', async () => {
    button.disabled = true;
    try {
      const pinned = !await nativeWindow.isAlwaysOnTop();
      await nativeWindow.setAlwaysOnTop(pinned);
      render(pinned);
    } catch (error) {
      button.title = '창 고정 상태를 바꾸지 못했습니다.';
      console.error(error);
    } finally {
      button.disabled = false;
    }
  });

  window.addEventListener('focus', refresh);
  refresh();
})();
